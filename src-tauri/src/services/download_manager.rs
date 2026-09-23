// Ports src/main/services/downloadManager.ts.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::constants::HF_RESOLVE_BASE;
use crate::error::Result;
use crate::ipc_types::{DownloadProgressEvent, DownloadState};
use crate::services::{chunked_download, fs_util, manifest_store};
use crate::state::AppState;

pub struct ActiveDownload {
    pub cancel_token: CancellationToken,
    pub final_path: PathBuf,
}

pub struct DownloadMeta {
    pub quant: Option<String>,
    pub param_count: Option<i64>,
}

/// Bundles `run_download`'s parameters (one per field of the eventual `DownloadProgressEvent`
/// plus the file paths and cancellation handle it needs) — mainly to keep that function's own
/// signature down to one argument.
struct DownloadJob {
    app: AppHandle,
    http_client: reqwest::Client,
    download_id: String,
    repo_id: String,
    filename: String,
    size_bytes_hint: i64,
    destination_dir: String,
    final_path: PathBuf,
    part_path: PathBuf,
    meta: DownloadMeta,
    cancel_token: CancellationToken,
}

// Matches JS `encodeURIComponent`'s escape set exactly: everything except
// A-Z a-z 0-9 - _ . ! ~ * ' ( )
const URI_COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

fn resolve_url(repo_id: &str, filename: &str) -> String {
    let encoded_filename = filename
        .split('/')
        .map(|segment| percent_encoding::utf8_percent_encode(segment, URI_COMPONENT).to_string())
        .collect::<Vec<_>>()
        .join("/");
    format!("{HF_RESOLVE_BASE}/{repo_id}/resolve/main/{encoded_filename}")
}

pub async fn start_download(
    app: AppHandle,
    repo_id: String,
    filename: String,
    size_bytes_hint: i64,
    destination_dir: String,
    meta: DownloadMeta,
) -> Result<String> {
    // `filename` is the rfilename and may include subfolders ("BF16/model-00001-of-00002.gguf");
    // the layout is mirrored under destination_dir so split-GGUF shards land side by side.
    let final_path = fs_util::resolve_in_destination(&destination_dir, &filename)?;
    let part_path = PathBuf::from(format!("{}.part", final_path.display()));

    // The destination is keyed by relative path, so two repos publishing a file at the same
    // path (rare but real — e.g. bartowski/unsloth both shipping a "Qwen3.8-27B-Q4_0.gguf")
    // would otherwise race on the same `.part`/sidecar path and corrupt each other's data.
    // Only one active download per target path is allowed.
    //
    // Unlike the Electron build (which relied on Node's single-threaded-until-first-`await`
    // semantics to make its check-then-reserve atomic), Tauri's async commands are dispatched
    // on a real multi-threaded tokio runtime, so that assumption doesn't hold here. The
    // `std::sync::Mutex` critical section below — check, then insert, all synchronous, no
    // `.await` crossed while holding the guard — is what actually provides the atomicity here.
    let download_id = Uuid::new_v4().to_string();
    let cancel_token = CancellationToken::new();
    {
        let state = app.state::<AppState>();
        let mut guard = state.active_downloads.lock().unwrap();
        for active in guard.values() {
            if active.final_path == final_path {
                return Err(format!(
                    "Already downloading {filename} (from another repo, or a duplicate request)"
                ));
            }
        }
        guard.insert(
            download_id.clone(),
            ActiveDownload {
                cancel_token: cancel_token.clone(),
                final_path: final_path.clone(),
            },
        );
    }

    let already_exists = tokio::fs::metadata(&final_path)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false);
    if already_exists {
        app.state::<AppState>()
            .active_downloads
            .lock()
            .unwrap()
            .remove(&download_id);
        return Err(format!("File already exists: {filename}"));
    }
    // A stale `.part` from a prior attempt is NOT deleted here: chunked_download.rs inspects
    // it (and its resume sidecar) and decides whether to resume or restart.
    if let Some(parent) = final_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            app.state::<AppState>()
                .active_downloads
                .lock()
                .unwrap()
                .remove(&download_id);
            return Err(e.to_string());
        }
    }

    let job = DownloadJob {
        app: app.clone(),
        http_client: app.state::<AppState>().http_client.clone(),
        download_id: download_id.clone(),
        repo_id,
        filename,
        size_bytes_hint,
        destination_dir,
        final_path,
        part_path,
        meta,
        cancel_token,
    };
    tokio::spawn(run_download(job));

    Ok(download_id)
}

/// Throttles progress-event emission to at most once per 250ms while downloading (terminal
/// states and the resume-seed event always emit immediately via `force`).
struct ProgressEmitter {
    app: AppHandle,
    download_id: String,
    repo_id: String,
    filename: String,
    received_bytes: AtomicU64,
    total_bytes: AtomicU64,
    last_emit_at: Mutex<Instant>,
}

impl ProgressEmitter {
    fn new(
        app: AppHandle,
        download_id: String,
        repo_id: String,
        filename: String,
        total_bytes_hint: i64,
    ) -> Self {
        Self {
            app,
            download_id,
            repo_id,
            filename,
            received_bytes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(total_bytes_hint.max(0) as u64),
            last_emit_at: Mutex::new(Instant::now() - Duration::from_secs(1)),
        }
    }

    fn set_total_bytes(&self, total: u64) {
        self.total_bytes.store(total, Ordering::SeqCst);
    }

    fn add_received_bytes(&self, n: u64) {
        self.received_bytes.fetch_add(n, Ordering::SeqCst);
        self.emit(DownloadState::Downloading, None, false);
    }

    fn emit(&self, state: DownloadState, error_message: Option<String>, force: bool) {
        if !force && state == DownloadState::Downloading {
            let mut last = self.last_emit_at.lock().unwrap();
            if last.elapsed() < Duration::from_millis(250) {
                return;
            }
            *last = Instant::now();
        }
        let received = self.received_bytes.load(Ordering::SeqCst);
        let total = self.total_bytes.load(Ordering::SeqCst);
        let percent = if total > 0 {
            ((received as f64 / total as f64) * 100.0)
                .min(100.0)
                .round() as i64
        } else {
            0
        };
        let _ = self.app.emit(
            "download:progress",
            DownloadProgressEvent {
                download_id: self.download_id.clone(),
                repo_id: self.repo_id.clone(),
                filename: self.filename.clone(),
                received_bytes: received as i64,
                total_bytes: total as i64,
                percent,
                state,
                error_message,
            },
        );
    }
}

async fn run_download(job: DownloadJob) {
    let DownloadJob {
        app,
        http_client,
        download_id,
        repo_id,
        filename,
        size_bytes_hint,
        destination_dir,
        final_path,
        part_path,
        meta,
        cancel_token,
    } = job;

    let emitter = Arc::new(ProgressEmitter::new(
        app.clone(),
        download_id.clone(),
        repo_id.clone(),
        filename.clone(),
        size_bytes_hint,
    ));
    let sidecar_path = PathBuf::from(format!("{}.json", part_path.display()));
    let url = resolve_url(&repo_id, &filename);

    let on_bytes: chunked_download::ByteCallback = {
        let emitter = Arc::clone(&emitter);
        Arc::new(move |n: u64| emitter.add_received_bytes(n))
    };
    let on_total_bytes_known = {
        let emitter = Arc::clone(&emitter);
        move |total: u64| emitter.set_total_bytes(total)
    };

    let download = chunked_download::download_file_resumable(
        &http_client,
        &url,
        &part_path,
        &sidecar_path,
        size_bytes_hint.max(0) as u64,
        on_bytes,
        on_total_bytes_known,
    );

    // Cleanup-on-failure is handled inside chunked_download.rs: the single-stream fallback
    // deletes part_path (no resume possible there), while the chunked path deliberately keeps
    // part_path + its sidecar so a retry can resume.
    let outcome = tokio::select! {
        result = download => Some(result),
        _ = cancel_token.cancelled() => None,
    };

    match outcome {
        None => emitter.emit(DownloadState::Canceled, None, true),
        Some(Ok(total_bytes)) => {
            emitter.set_total_bytes(total_bytes);
            match tokio::fs::rename(&part_path, &final_path).await {
                Ok(()) => {
                    let entry = manifest_store::ManifestEntry {
                        repo_id: repo_id.clone(),
                        quant: meta.quant,
                        size_bytes: total_bytes as i64,
                        param_count: meta.param_count,
                        downloaded_at: chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    };
                    // The download itself succeeded; a manifest write failure (e.g. read-only
                    // destination) shouldn't be reported as a failed download.
                    let _ =
                        manifest_store::record_download(&destination_dir, &filename, entry).await;
                    emitter.emit(DownloadState::Completed, None, true);
                }
                Err(e) => emitter.emit(DownloadState::Error, Some(e.to_string()), true),
            }
        }
        Some(Err(message)) => emitter.emit(DownloadState::Error, Some(message), true),
    }

    app.state::<AppState>()
        .active_downloads
        .lock()
        .unwrap()
        .remove(&download_id);
}

pub fn cancel_download(state: &AppState, download_id: &str) -> bool {
    let guard = state.active_downloads.lock().unwrap();
    match guard.get(download_id) {
        Some(active) => {
            active.cancel_token.cancel();
            true
        }
        None => false,
    }
}
