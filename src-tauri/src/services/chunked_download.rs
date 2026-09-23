// Ports src/main/services/chunkedDownload.ts.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::constants::{DOWNLOAD_CHUNK_COUNT, MIN_CHUNK_SIZE_BYTES};
use crate::error::{MapErrString, Result};

/// Called with the number of newly-received bytes, from any of the concurrent chunk workers —
/// shared via `Arc` rather than a plain closure since multiple tasks call it concurrently.
pub type ByteCallback = Arc<dyn Fn(u64) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Chunk {
    start: u64,
    end: u64, // inclusive
    downloaded: u64,
}

fn is_chunk_complete(chunk: &Chunk) -> bool {
    chunk.downloaded > chunk.end - chunk.start
}

// Sidecar JSON shape (`<partPath>.json`) is unchanged from the Electron build, so a `.part` +
// sidecar left behind by an interrupted download under the old build resumes correctly here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sidecar {
    url: String,
    total_bytes: u64,
    chunks: Vec<Chunk>,
}

struct RangeProbe {
    supported: bool,
    total_bytes: Option<u64>,
}

async fn probe_range_support(client: &reqwest::Client, url: &str) -> RangeProbe {
    let outcome: std::result::Result<RangeProbe, reqwest::Error> = async {
        let res = client
            .get(url)
            .header(reqwest::header::RANGE, "bytes=0-0")
            .send()
            .await?;
        if res.status().as_u16() != 206 {
            return Ok(RangeProbe {
                supported: false,
                total_bytes: None,
            });
        }
        let content_range = res
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok());
        let total_bytes = content_range.and_then(parse_content_range_total);
        Ok(RangeProbe {
            supported: total_bytes.is_some(),
            total_bytes,
        })
    }
    .await;
    outcome.unwrap_or(RangeProbe {
        supported: false,
        total_bytes: None,
    })
}

fn parse_content_range_total(header: &str) -> Option<u64> {
    // "bytes 0-0/338607520"
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/(\d+)$").unwrap());
    RE.captures(header)?.get(1)?.as_str().parse().ok()
}

fn plan_chunks(total_bytes: u64) -> Vec<Chunk> {
    let count = if total_bytes < MIN_CHUNK_SIZE_BYTES {
        1
    } else {
        (DOWNLOAD_CHUNK_COUNT as u64).min(total_bytes.div_ceil(MIN_CHUNK_SIZE_BYTES)) as usize
    };
    let size = total_bytes.div_ceil(count as u64);
    let mut chunks = Vec::new();
    for i in 0..count as u64 {
        let start = i * size;
        if start >= total_bytes {
            break;
        }
        let end = (start + size - 1).min(total_bytes - 1);
        chunks.push(Chunk {
            start,
            end,
            downloaded: 0,
        });
    }
    chunks
}

async fn load_sidecar(sidecar_path: &Path) -> Option<Sidecar> {
    let raw = tokio::fs::read_to_string(sidecar_path).await.ok()?;
    serde_json::from_str(&raw).ok()
}

async fn save_sidecar(sidecar_path: &Path, sidecar: &Sidecar) {
    // Best-effort: matches the original's `.catch(() => {})` — a failed sidecar write
    // shouldn't abort the download.
    if let Ok(json) = serde_json::to_vec(sidecar) {
        let _ = tokio::fs::write(sidecar_path, json).await;
    }
}

async fn remove_sidecar(sidecar_path: &Path) {
    let _ = tokio::fs::remove_file(sidecar_path).await;
}

#[cfg(unix)]
fn write_at(file: &std::fs::File, buf: &[u8], offset: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::write_at(file, buf, offset)
}

#[cfg(windows)]
fn write_at(file: &std::fs::File, buf: &[u8], offset: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_write(file, buf, offset)
}

/// Multiple chunk workers hold independent `Arc<File>` handles to the same `.part` file and
/// write at their own byte offsets concurrently — safe because positional writes to disjoint
/// ranges of the same file don't race, unlike a shared cursor-based write would.
fn write_all_at(file: &std::fs::File, mut buf: &[u8], mut offset: u64) -> std::io::Result<()> {
    while !buf.is_empty() {
        let n = write_at(file, buf, offset)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "failed to write whole buffer",
            ));
        }
        buf = &buf[n..];
        offset += n as u64;
    }
    Ok(())
}

/// Throttles sidecar persistence to at most once per 500ms (plus any `force`d write), sharing
/// one `Sidecar` snapshot across every concurrent chunk worker.
struct SidecarPersister {
    sidecar_path: std::path::PathBuf,
    shared: Arc<Mutex<Sidecar>>,
    last_persist_at: Mutex<Instant>,
}

impl SidecarPersister {
    fn new(sidecar_path: std::path::PathBuf, shared: Arc<Mutex<Sidecar>>) -> Self {
        Self {
            sidecar_path,
            shared,
            last_persist_at: Mutex::new(Instant::now() - Duration::from_secs(1)),
        }
    }

    async fn persist(&self, force: bool) {
        let should_write = {
            let mut last = self.last_persist_at.lock().unwrap();
            if !force && last.elapsed() < Duration::from_millis(500) {
                false
            } else {
                *last = Instant::now();
                true
            }
        };
        if should_write {
            let snapshot = self.shared.lock().unwrap().clone();
            save_sidecar(&self.sidecar_path, &snapshot).await;
        }
    }
}

// Each `spawn_blocking` call has real dispatch overhead (crossing onto tokio's blocking
// thread pool), and a raw network stream piece can be as small as a few KB — writing one
// syscall per piece (as the original TS `for await` loop does; Node's per-call overhead for
// this is far lower) made the Rust port dramatically slower. Buffering pieces up to this size
// before issuing one write cuts the number of `spawn_blocking` calls (and Sidecar-mutex
// locks) by roughly this factor, and is a deliberate deviation from the 1:1 port rather than
// an oversight — chunk.downloaded only advances after a successful write either way, so
// resumability is unaffected.
const WRITE_BUFFER_SIZE: usize = 256 * 1024;

/// Downloads a single HTTP Range chunk into `file` at the chunk's byte offset, resuming from
/// `chunk.downloaded` if partially fetched already.
async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    file: &Arc<std::fs::File>,
    shared: &Arc<Mutex<Sidecar>>,
    chunk_index: usize,
    on_bytes: &ByteCallback,
    persister: &SidecarPersister,
) -> Result<()> {
    let (start, end, downloaded) = {
        let sidecar = shared.lock().unwrap();
        let chunk = &sidecar.chunks[chunk_index];
        (chunk.start, chunk.end, chunk.downloaded)
    };
    if downloaded > end - start {
        return Ok(());
    }

    let range_start = start + downloaded;
    let res = client
        .get(url)
        .header(reqwest::header::RANGE, format!("bytes={range_start}-{end}"))
        .send()
        .await
        .map_err_string()?;
    if res.status().as_u16() != 206 {
        return Err(format!(
            "Chunk download failed: {} {}",
            res.status().as_u16(),
            res.status().canonical_reason().unwrap_or("")
        ));
    }

    let mut position = range_start;
    let mut buffer: Vec<u8> = Vec::with_capacity(WRITE_BUFFER_SIZE);
    let mut stream = res.bytes_stream();
    while let Some(piece) = stream.next().await {
        let bytes = piece.map_err_string()?;
        buffer.extend_from_slice(&bytes);
        if buffer.len() >= WRITE_BUFFER_SIZE {
            flush_chunk_buffer(
                file,
                &mut buffer,
                &mut position,
                shared,
                chunk_index,
                on_bytes,
                persister,
            )
            .await?;
        }
    }
    if !buffer.is_empty() {
        flush_chunk_buffer(
            file,
            &mut buffer,
            &mut position,
            shared,
            chunk_index,
            on_bytes,
            persister,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn flush_chunk_buffer(
    file: &Arc<std::fs::File>,
    buffer: &mut Vec<u8>,
    position: &mut u64,
    shared: &Arc<Mutex<Sidecar>>,
    chunk_index: usize,
    on_bytes: &ByteCallback,
    persister: &SidecarPersister,
) -> Result<()> {
    let file_clone = Arc::clone(file);
    let buf = std::mem::take(buffer);
    let n = buf.len() as u64;
    let write_pos = *position;
    tokio::task::spawn_blocking(move || write_all_at(&file_clone, &buf, write_pos))
        .await
        .map_err_string()?
        .map_err_string()?;
    *position += n;

    {
        let mut sidecar = shared.lock().unwrap();
        sidecar.chunks[chunk_index].downloaded += n;
    }
    on_bytes(n);
    persister.persist(false).await;
    Ok(())
}

/// Parallel, resumable download via HTTP Range requests. Splits the file into chunks
/// (src-tauri/src/constants.rs) fetched concurrently, tracking per-chunk progress in a
/// `<partPath>.json`-style sidecar so an interrupted download can resume the remaining bytes
/// of each chunk instead of restarting from zero.
///
/// Each chunk request hits the stable `resolve/main` URL directly (not a captured signed CDN
/// redirect target) — the original Electron build verified live that Range requests against
/// that URL are correctly forwarded through the redirect, avoiding signed-URL expiry concerns
/// for long-running multi-chunk downloads; reqwest's default redirect policy also preserves
/// headers across redirects, so this carries over unchanged.
async fn run_chunked_download(
    client: &reqwest::Client,
    url: &str,
    part_path: &Path,
    sidecar_path: &Path,
    total_bytes: u64,
    on_bytes: ByteCallback,
) -> Result<()> {
    let mut sidecar = load_sidecar(sidecar_path).await;
    let needs_fresh = !matches!(&sidecar, Some(s) if s.url == url && s.total_bytes == total_bytes);
    if needs_fresh {
        sidecar = Some(Sidecar {
            url: url.to_string(),
            total_bytes,
            chunks: plan_chunks(total_bytes),
        });
        // Fresh start: don't let stale bytes from a different download linger.
        let _ = tokio::fs::remove_file(part_path).await;
    }
    let sidecar = sidecar.unwrap();

    {
        // Matches `fs.open(partPath, 'a+')`: create if missing, but never truncate existing
        // content — resumed bytes from an earlier attempt must survive. `set_len` below then
        // grows (zero-filling) or shrinks the file to exactly `total_bytes` regardless.
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(part_path)
            .await
            .map_err_string()?;
        file.set_len(total_bytes).await.map_err_string()?;
    }

    save_sidecar(sidecar_path, &sidecar).await;

    let already_downloaded: u64 = sidecar.chunks.iter().map(|c| c.downloaded).sum();
    if already_downloaded > 0 {
        on_bytes(already_downloaded);
    }

    let pending_indices: Arc<Vec<usize>> = Arc::new(
        sidecar
            .chunks
            .iter()
            .enumerate()
            .filter(|(_, c)| !is_chunk_complete(c))
            .map(|(i, _)| i)
            .collect(),
    );
    let shared = Arc::new(Mutex::new(sidecar));
    let persister = Arc::new(SidecarPersister::new(
        sidecar_path.to_path_buf(),
        Arc::clone(&shared),
    ));

    // Positional writes at disjoint offsets don't race, so every worker shares one open handle.
    let file = Arc::new(
        std::fs::OpenOptions::new()
            .write(true)
            .open(part_path)
            .map_err_string()?,
    );
    let cursor = Arc::new(AtomicUsize::new(0));
    let worker_count = DOWNLOAD_CHUNK_COUNT.min(pending_indices.len());

    // Deliberately NOT `tokio::spawn`: a spawned task is detached from this function's own
    // future and keeps running in the background even if this future is dropped — which is
    // exactly what `run_download`'s `tokio::select!` does on cancellation. Polling the worker
    // futures directly (via `join_all`, all owned by this call) means dropping this future
    // (and therefore its in-flight HTTP requests) actually stops the download; each worker
    // still runs fully concurrently for I/O, since async tasks don't need separate OS threads
    // to overlap network waits.
    let workers = (0..worker_count).map(|_| {
        let client = client.clone();
        let url = url.to_string();
        let file = Arc::clone(&file);
        let shared = Arc::clone(&shared);
        let persister = Arc::clone(&persister);
        let cursor = Arc::clone(&cursor);
        let pending_indices = Arc::clone(&pending_indices);
        let on_bytes = Arc::clone(&on_bytes);
        async move {
            loop {
                let i = cursor.fetch_add(1, Ordering::SeqCst);
                if i >= pending_indices.len() {
                    return Ok(());
                }
                download_chunk(
                    &client,
                    &url,
                    &file,
                    &shared,
                    pending_indices[i],
                    &on_bytes,
                    &persister,
                )
                .await?;
            }
        }
    });

    // Unlike `Promise.all` (which lets stragglers keep running in the background after the
    // first rejection), every worker runs to completion here even once an error is found —
    // simpler lifecycle, no dangling writers left running past this function's return.
    let mut first_err: Option<String> = None;
    for result in futures_util::future::join_all(workers).await {
        if let Err(err) = result {
            first_err.get_or_insert(err);
        }
    }

    if let Some(err) = first_err {
        persister.persist(true).await;
        return Err(err);
    }

    remove_sidecar(sidecar_path).await;
    Ok(())
}

async fn run_single_stream_download(
    client: &reqwest::Client,
    url: &str,
    part_path: &Path,
    total_bytes_hint: u64,
    on_bytes: &ByteCallback,
    on_total_bytes_known: &mut (dyn FnMut(u64) + Send),
) -> Result<u64> {
    let outcome: Result<u64> = async {
        let res = client.get(url).send().await.map_err_string()?;
        if !res.status().is_success() {
            return Err(format!(
                "Download failed: {} {}",
                res.status().as_u16(),
                res.status().canonical_reason().unwrap_or("")
            ));
        }
        let total_bytes = res.content_length().unwrap_or(total_bytes_hint);
        on_total_bytes_known(total_bytes);

        let mut file = tokio::fs::File::create(part_path).await.map_err_string()?;
        let mut stream = res.bytes_stream();
        while let Some(piece) = stream.next().await {
            let bytes = piece.map_err_string()?;
            file.write_all(&bytes).await.map_err_string()?;
            on_bytes(bytes.len() as u64);
        }
        Ok(total_bytes)
    }
    .await;

    // No sidecar/resumability for this path — mirror the old always-restart behavior.
    if outcome.is_err() {
        let _ = tokio::fs::remove_file(part_path).await;
    }
    outcome
}

/// Download `url` into `part_path`, resuming from a prior interrupted attempt when possible.
/// Falls back to a plain single-stream download (no resume) if the server doesn't support
/// Range requests for this URL.
pub async fn download_file_resumable(
    client: &reqwest::Client,
    url: &str,
    part_path: &Path,
    sidecar_path: &Path,
    total_bytes_hint: u64,
    on_bytes: ByteCallback,
    mut on_total_bytes_known: impl FnMut(u64) + Send,
) -> Result<u64> {
    let probe = probe_range_support(client, url).await;
    let Some(total_bytes) = probe.total_bytes.filter(|_| probe.supported) else {
        return run_single_stream_download(
            client,
            url,
            part_path,
            total_bytes_hint,
            &on_bytes,
            &mut on_total_bytes_known,
        )
        .await;
    };

    on_total_bytes_known(total_bytes);
    run_chunked_download(client, url, part_path, sidecar_path, total_bytes, on_bytes).await?;
    Ok(total_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_range_total() {
        assert_eq!(
            parse_content_range_total("bytes 0-0/338607520"),
            Some(338607520)
        );
        assert_eq!(parse_content_range_total("garbage"), None);
    }

    #[test]
    fn plans_a_single_chunk_for_small_files() {
        let chunks = plan_chunks(1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0],
            Chunk {
                start: 0,
                end: 1023,
                downloaded: 0
            }
        );
    }

    #[test]
    fn plans_the_max_chunk_count_for_large_files() {
        let total = MIN_CHUNK_SIZE_BYTES * 100;
        let chunks = plan_chunks(total);
        assert_eq!(chunks.len(), DOWNLOAD_CHUNK_COUNT);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks.last().unwrap().end, total - 1);
        // Contiguous, non-overlapping.
        for pair in chunks.windows(2) {
            assert_eq!(pair[0].end + 1, pair[1].start);
        }
    }

    #[test]
    fn plans_fewer_than_the_max_chunk_count_when_the_file_is_only_moderately_large() {
        // Between 1x and DOWNLOAD_CHUNK_COUNT x MIN_CHUNK_SIZE_BYTES: one chunk per
        // MIN_CHUNK_SIZE_BYTES, not the full worker count.
        let total = MIN_CHUNK_SIZE_BYTES * 3 + 1;
        let chunks = plan_chunks(total);
        assert_eq!(chunks.len(), 4);
    }

    #[test]
    fn is_chunk_complete_matches_the_inclusive_end_boundary() {
        let chunk = Chunk {
            start: 0,
            end: 9,
            downloaded: 10,
        }; // 10 bytes total (0..=9)
        assert!(is_chunk_complete(&chunk));
        let chunk = Chunk {
            start: 0,
            end: 9,
            downloaded: 9,
        };
        assert!(!is_chunk_complete(&chunk));
    }

    #[tokio::test]
    async fn write_all_at_writes_to_the_correct_offset_without_disturbing_other_regions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.bin");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        file.set_len(10).unwrap();

        write_all_at(&file, b"AB", 0).unwrap();
        write_all_at(&file, b"CD", 5).unwrap();

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(&contents[0..2], b"AB");
        assert_eq!(&contents[5..7], b"CD");
        // Untouched regions stay zero-filled from set_len's allocation.
        assert_eq!(contents[2], 0);
        assert_eq!(contents[9], 0);
    }

    #[tokio::test]
    async fn sidecar_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.gguf.part.json");
        let sidecar = Sidecar {
            url: "https://example.com/x".to_string(),
            total_bytes: 100,
            chunks: vec![
                Chunk {
                    start: 0,
                    end: 49,
                    downloaded: 10,
                },
                Chunk {
                    start: 50,
                    end: 99,
                    downloaded: 0,
                },
            ],
        };

        save_sidecar(&path, &sidecar).await;
        let loaded = load_sidecar(&path).await.unwrap();
        assert_eq!(loaded.url, sidecar.url);
        assert_eq!(loaded.total_bytes, sidecar.total_bytes);
        assert_eq!(loaded.chunks, sidecar.chunks);

        // Field names on the wire stay camelCase, matching what the Electron build wrote.
        let raw = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(raw.contains("\"totalBytes\""));
    }

    #[tokio::test]
    async fn load_sidecar_returns_none_for_a_missing_or_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_sidecar(&dir.path().join("missing.json"))
            .await
            .is_none());

        let corrupt = dir.path().join("corrupt.json");
        tokio::fs::write(&corrupt, b"not json").await.unwrap();
        assert!(load_sidecar(&corrupt).await.is_none());
    }

    // --- HTTP-level integration tests, against a local mock server ---
    //
    // Cover the actual network protocol (Range probing, 206 handling, resume-from-offset,
    // the Range-vs-single-stream fallback, and the asymmetric .part/sidecar cleanup on
    // failure) rather than just the pure planning/byte-math functions above.

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn no_bytes_callback() -> ByteCallback {
        Arc::new(|_n| {})
    }

    async fn mount_range_supported(server: &MockServer, content: &[u8]) {
        let total = content.len();
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .and(header("Range", "bytes=0-0"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", format!("bytes 0-0/{total}"))
                    .set_body_bytes(content[0..1].to_vec()),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .and(header("Range", format!("bytes=0-{}", total - 1)))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", format!("bytes 0-{}/{total}", total - 1))
                    .set_body_bytes(content.to_vec()),
            )
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn chunked_download_writes_the_exact_bytes_and_cleans_up_the_sidecar() {
        let content: Vec<u8> = (0..2000u32).map(|i| (i % 251) as u8).collect();
        let server = MockServer::start().await;
        mount_range_supported(&server, &content).await;

        let dir = tempfile::tempdir().unwrap();
        let part_path = dir.path().join("model.bin.part");
        let sidecar_path = dir.path().join("model.bin.part.json");
        let url = format!("{}/model.bin", server.uri());
        let client = reqwest::Client::new();

        let total = download_file_resumable(
            &client,
            &url,
            &part_path,
            &sidecar_path,
            0,
            no_bytes_callback(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(total, content.len() as u64);
        assert_eq!(std::fs::read(&part_path).unwrap(), content);
        // Successful completion removes the resume sidecar.
        assert!(!sidecar_path.exists());
    }

    #[tokio::test]
    async fn chunked_download_resumes_from_the_sidecars_recorded_offset() {
        let content: Vec<u8> = (0..200u32).map(|i| i as u8).collect();
        let resume_from = 120usize;
        let server = MockServer::start().await;
        // Only the remaining-bytes range is ever requested; mounting solely this range means
        // the test fails loudly (connection refused / no matching mock) if the implementation
        // ever re-requests from byte 0 instead of resuming.
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .and(header(
                "Range",
                format!("bytes={resume_from}-{}", content.len() - 1),
            ))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header(
                        "Content-Range",
                        format!(
                            "bytes {resume_from}-{}/{}",
                            content.len() - 1,
                            content.len()
                        ),
                    )
                    .set_body_bytes(content[resume_from..].to_vec()),
            )
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let part_path = dir.path().join("model.bin.part");
        let sidecar_path = dir.path().join("model.bin.part.json");
        let url = format!("{}/model.bin", server.uri());

        // Simulate a prior interrupted attempt: part file pre-allocated with the first
        // `resume_from` bytes already correctly written, and a sidecar recording that.
        std::fs::write(&part_path, {
            let mut buf = content.clone();
            buf[resume_from..].fill(0);
            buf
        })
        .unwrap();
        let sidecar = Sidecar {
            url: url.clone(),
            total_bytes: content.len() as u64,
            chunks: vec![Chunk {
                start: 0,
                end: content.len() as u64 - 1,
                downloaded: resume_from as u64,
            }],
        };
        save_sidecar(&sidecar_path, &sidecar).await;

        let client = reqwest::Client::new();
        run_chunked_download(
            &client,
            &url,
            &part_path,
            &sidecar_path,
            content.len() as u64,
            no_bytes_callback(),
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(&part_path).unwrap(), content);
        assert!(!sidecar_path.exists());
    }

    #[tokio::test]
    async fn falls_back_to_single_stream_when_the_server_ignores_range() {
        let content = b"hello from a non-range server".to_vec();
        let server = MockServer::start().await;
        // No Content-Range / 206 handling at all — a plain 200 regardless of the Range header,
        // which is exactly how a server that doesn't support Range requests behaves.
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(content.clone()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let part_path = dir.path().join("model.bin.part");
        let sidecar_path = dir.path().join("model.bin.part.json");
        let url = format!("{}/model.bin", server.uri());
        let client = reqwest::Client::new();

        let total = download_file_resumable(
            &client,
            &url,
            &part_path,
            &sidecar_path,
            0,
            no_bytes_callback(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(total, content.len() as u64);
        assert_eq!(std::fs::read(&part_path).unwrap(), content);
        // The single-stream path never writes a sidecar in the first place.
        assert!(!sidecar_path.exists());
    }

    #[tokio::test]
    async fn single_stream_failure_deletes_the_part_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let part_path = dir.path().join("model.bin.part");
        // Simulate a stray partial file from some earlier state, to confirm the failure path
        // actively removes it rather than merely never having created one.
        std::fs::write(&part_path, b"stale").unwrap();
        let sidecar_path = dir.path().join("model.bin.part.json");
        let url = format!("{}/model.bin", server.uri());
        let client = reqwest::Client::new();

        let result = download_file_resumable(
            &client,
            &url,
            &part_path,
            &sidecar_path,
            0,
            no_bytes_callback(),
            |_| {},
        )
        .await;

        assert!(result.is_err());
        assert!(!part_path.exists());
    }

    #[tokio::test]
    async fn chunked_failure_keeps_the_part_file_and_sidecar_for_a_retry() {
        let total: usize = 50;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .and(header("Range", "bytes=0-0"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", format!("bytes 0-0/{total}"))
                    .set_body_bytes(vec![0u8]),
            )
            .mount(&server)
            .await;
        // The probe succeeds, but the real chunk request then fails server-side — this is the
        // asymmetric-cleanup case: unlike the single-stream path, the .part + sidecar must
        // survive so a subsequent call can resume instead of restarting from zero.
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .and(header("Range", format!("bytes=0-{}", total - 1)))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let part_path = dir.path().join("model.bin.part");
        let sidecar_path = dir.path().join("model.bin.part.json");
        let url = format!("{}/model.bin", server.uri());
        let client = reqwest::Client::new();

        let result = download_file_resumable(
            &client,
            &url,
            &part_path,
            &sidecar_path,
            total as u64,
            no_bytes_callback(),
            |_| {},
        )
        .await;

        assert!(result.is_err());
        assert!(part_path.exists());
        assert!(sidecar_path.exists());
    }

    #[tokio::test]
    async fn cancellation_actually_stops_an_in_flight_chunk_worker() {
        // Regression test for a real bug: chunk workers were originally launched with
        // `tokio::spawn`, which detaches them from run_chunked_download's own future. Racing
        // that future against a CancellationToken via `tokio::select!` (as download_manager.rs
        // does) then dropped the *outer* future when cancelled, but the already-spawned worker
        // tasks kept running in the background regardless — cancel appeared to do nothing.
        let total: usize = 100;
        let content = vec![9u8; total];
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .and(header("Range", "bytes=0-0"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", format!("bytes 0-0/{total}"))
                    .set_body_bytes(vec![0u8]),
            )
            .mount(&server)
            .await;
        // Delayed well past the cancellation point below: if the worker is still running in
        // the background after cancellation, it eventually receives and writes this body;
        // if cancellation genuinely stopped it, this response is never consumed at all.
        Mock::given(method("GET"))
            .and(path("/model.bin"))
            .and(header("Range", format!("bytes=0-{}", total - 1)))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", format!("bytes 0-{}/{}", total - 1, total))
                    .set_body_bytes(content.clone())
                    .set_delay(Duration::from_millis(300)),
            )
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let part_path = dir.path().join("model.bin.part");
        let sidecar_path = dir.path().join("model.bin.part.json");
        let url = format!("{}/model.bin", server.uri());
        let client = reqwest::Client::new();

        let cancel_token = tokio_util::sync::CancellationToken::new();
        let canceller = cancel_token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            canceller.cancel();
        });

        let download = download_file_resumable(
            &client,
            &url,
            &part_path,
            &sidecar_path,
            total as u64,
            no_bytes_callback(),
            |_| {},
        );
        let outcome = tokio::select! {
            result = download => Some(result),
            _ = cancel_token.cancelled() => None,
        };
        assert!(
            outcome.is_none(),
            "cancellation should have won the race, before the 300ms-delayed response"
        );

        // Give a (buggy, still-running-in-the-background) worker ample time to have finished
        // writing the delayed response, if cancellation had failed to actually stop it.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let on_disk = std::fs::read(&part_path).unwrap_or_default();
        assert_ne!(
            on_disk, content,
            "the delayed response must never have been written after cancellation"
        );
    }
}
