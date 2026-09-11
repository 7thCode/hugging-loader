import { promises as fs, createWriteStream } from 'fs'
import { Readable } from 'stream'
import type { ReadableStream as NodeWebReadableStream } from 'stream/web'
import { DOWNLOAD_CHUNK_COUNT, MIN_CHUNK_SIZE_BYTES } from '../constants'

interface Chunk {
  start: number
  end: number // inclusive
  downloaded: number
}

interface Sidecar {
  url: string
  totalBytes: number
  chunks: Chunk[]
}

function toNodeStream(body: ReadableStream): Readable {
  return Readable.fromWeb(body as unknown as NodeWebReadableStream<Uint8Array>)
}

async function probeRangeSupport(
  url: string,
  signal: AbortSignal
): Promise<{ supported: boolean; totalBytes: number | null }> {
  try {
    const res = await fetch(url, { signal, headers: { Range: 'bytes=0-0' } })
    await res.body?.cancel().catch(() => {})
    if (res.status !== 206) return { supported: false, totalBytes: null }
    const contentRange = res.headers.get('content-range') // "bytes 0-0/338607520"
    const match = contentRange?.match(/\/(\d+)$/)
    const totalBytes = match ? parseInt(match[1], 10) : null
    return { supported: totalBytes !== null, totalBytes }
  } catch {
    return { supported: false, totalBytes: null }
  }
}

function planChunks(totalBytes: number): Chunk[] {
  const count =
    totalBytes < MIN_CHUNK_SIZE_BYTES
      ? 1
      : Math.min(DOWNLOAD_CHUNK_COUNT, Math.ceil(totalBytes / MIN_CHUNK_SIZE_BYTES))
  const size = Math.ceil(totalBytes / count)
  const chunks: Chunk[] = []
  for (let i = 0; i < count; i++) {
    const start = i * size
    if (start >= totalBytes) break
    const end = Math.min(start + size - 1, totalBytes - 1)
    chunks.push({ start, end, downloaded: 0 })
  }
  return chunks
}

function isChunkComplete(chunk: Chunk): boolean {
  return chunk.downloaded > chunk.end - chunk.start
}

async function loadSidecar(sidecarPath: string): Promise<Sidecar | null> {
  try {
    const raw = await fs.readFile(sidecarPath, 'utf-8')
    const parsed = JSON.parse(raw)
    if (parsed && typeof parsed.totalBytes === 'number' && Array.isArray(parsed.chunks)) {
      return parsed as Sidecar
    }
    return null
  } catch {
    return null
  }
}

async function saveSidecar(sidecarPath: string, sidecar: Sidecar): Promise<void> {
  await fs.writeFile(sidecarPath, JSON.stringify(sidecar), 'utf-8').catch(() => {})
}

async function removeSidecar(sidecarPath: string): Promise<void> {
  await fs.rm(sidecarPath, { force: true })
}

/**
 * Downloads a single HTTP Range chunk into `partPath` at the chunk's byte offset,
 * resuming from `chunk.downloaded` if partially fetched already.
 */
async function downloadChunk(
  url: string,
  partPath: string,
  chunk: Chunk,
  signal: AbortSignal,
  onBytes: (n: number) => void
): Promise<void> {
  if (isChunkComplete(chunk)) return
  const rangeStart = chunk.start + chunk.downloaded
  const res = await fetch(url, {
    signal,
    headers: { Range: `bytes=${rangeStart}-${chunk.end}` }
  })
  if (res.status !== 206 || !res.body) {
    throw new Error(`Chunk download failed: ${res.status} ${res.statusText}`)
  }

  const handle = await fs.open(partPath, 'r+')
  try {
    let position = rangeStart
    for await (const piece of toNodeStream(res.body)) {
      const buf = piece as Buffer
      await handle.write(buf, 0, buf.length, position)
      position += buf.length
      chunk.downloaded += buf.length
      onBytes(buf.length)
    }
  } finally {
    await handle.close()
  }
}

/**
 * Parallel, resumable download via HTTP Range requests. Splits the file into
 * chunks (src/main/constants.ts) fetched concurrently, tracking per-chunk
 * progress in a `<partPath>.json`-style sidecar so an interrupted download can
 * resume the remaining bytes of each chunk instead of restarting from zero.
 *
 * Each chunk request hits the stable `resolve/main` URL directly (not a captured
 * signed CDN redirect target) — verified live that Range requests against that
 * URL are correctly forwarded through the redirect, which avoids any signed-URL
 * expiry concerns for long-running multi-chunk downloads.
 */
async function runChunkedDownload(
  url: string,
  partPath: string,
  sidecarPath: string,
  totalBytes: number,
  signal: AbortSignal,
  onBytes: (n: number) => void
): Promise<void> {
  let sidecar = await loadSidecar(sidecarPath)
  if (!sidecar || sidecar.url !== url || sidecar.totalBytes !== totalBytes) {
    sidecar = { url, totalBytes, chunks: planChunks(totalBytes) }
    // Fresh start: don't let stale bytes from a different download linger.
    await fs.rm(partPath, { force: true })
  }

  const preallocateHandle = await fs.open(partPath, 'a+')
  await preallocateHandle.truncate(totalBytes)
  await preallocateHandle.close()

  await saveSidecar(sidecarPath, sidecar)

  const alreadyDownloaded = sidecar.chunks.reduce((sum, c) => sum + c.downloaded, 0)
  if (alreadyDownloaded > 0) onBytes(alreadyDownloaded)

  let lastPersistAt = 0
  const persist = async (force = false): Promise<void> => {
    const now = Date.now()
    if (!force && now - lastPersistAt < 500) return
    lastPersistAt = now
    await saveSidecar(sidecarPath, sidecar!)
  }

  const pending = sidecar.chunks.filter((c) => !isChunkComplete(c))
  let cursor = 0

  async function worker(): Promise<void> {
    while (cursor < pending.length) {
      const chunk = pending[cursor++]
      await downloadChunk(url, partPath, chunk, signal, (n) => {
        onBytes(n)
        void persist()
      })
    }
  }

  try {
    const workerCount = Math.min(DOWNLOAD_CHUNK_COUNT, pending.length)
    await Promise.all(Array.from({ length: workerCount }, () => worker()))
  } catch (err) {
    await persist(true)
    throw err
  }

  await removeSidecar(sidecarPath)
}

async function runSingleStreamDownload(
  url: string,
  partPath: string,
  totalBytesHint: number,
  signal: AbortSignal,
  onBytes: (n: number) => void,
  onTotalBytesKnown: (total: number) => void
): Promise<number> {
  try {
    const res = await fetch(url, { signal })
    if (!res.ok || !res.body) {
      throw new Error(`Download failed: ${res.status} ${res.statusText}`)
    }
    const contentLength = res.headers.get('content-length')
    const totalBytes = contentLength ? parseInt(contentLength, 10) : totalBytesHint
    onTotalBytesKnown(totalBytes)

    const writeStream = createWriteStream(partPath)
    await new Promise<void>((resolve, reject) => {
      const nodeStream = toNodeStream(res.body!)
      nodeStream.on('data', (chunk: Buffer) => onBytes(chunk.length))
      nodeStream.on('error', reject)
      writeStream.on('error', reject)
      writeStream.on('finish', resolve)
      nodeStream.pipe(writeStream)
    })
    return totalBytes
  } catch (err) {
    // No sidecar/resumability for this path — mirror the old always-restart behavior.
    await fs.rm(partPath, { force: true })
    throw err
  }
}

/**
 * Download `url` into `partPath`, resuming from a prior interrupted attempt when
 * possible. Falls back to a plain single-stream download (no resume) if the
 * server doesn't support Range requests for this URL.
 */
export async function downloadFileResumable(
  url: string,
  partPath: string,
  sidecarPath: string,
  totalBytesHint: number,
  signal: AbortSignal,
  onBytes: (n: number) => void,
  onTotalBytesKnown: (total: number) => void
): Promise<number> {
  const probe = await probeRangeSupport(url, signal)
  if (!probe.supported || !probe.totalBytes) {
    return runSingleStreamDownload(
      url,
      partPath,
      totalBytesHint,
      signal,
      onBytes,
      onTotalBytesKnown
    )
  }

  onTotalBytesKnown(probe.totalBytes)
  await runChunkedDownload(url, partPath, sidecarPath, probe.totalBytes, signal, onBytes)
  return probe.totalBytes
}
