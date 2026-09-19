import { promises as fs } from 'fs'
import path from 'path'
import { randomUUID } from 'crypto'
import { HF_RESOLVE_BASE } from '../constants'
import { recordDownload } from './manifestStore'
import { resolveInDestination } from './fsUtil'
import { downloadFileResumable } from './chunkedDownload'
import type { DownloadProgressEvent, DownloadState } from '../../shared/ipc-types'

type ProgressCallback = (event: DownloadProgressEvent) => void

export interface DownloadMeta {
  quant: string | null
  paramCount: number | null
}

interface ActiveDownload {
  controller: AbortController
  finalPath: string
}

const activeDownloads = new Map<string, ActiveDownload>()

export async function startDownload(
  repoId: string,
  filename: string,
  sizeBytesHint: number,
  destinationDir: string,
  meta: DownloadMeta,
  onProgress: ProgressCallback
): Promise<string> {
  // `filename` is the rfilename and may include subfolders ("BF16/model-00001-of-00002.gguf");
  // the layout is mirrored under destinationDir so split-GGUF shards land side by side.
  const finalPath = resolveInDestination(destinationDir, filename)
  const partPath = `${finalPath}.part`

  // The destination is keyed by relative path, so two repos publishing a file at the same
  // path (rare but real — e.g. bartowski/unsloth both shipping a "Qwen3.8-27B-Q4_0.gguf")
  // would otherwise race on the same `.part`/sidecar path and corrupt each other's data.
  // Only one active download per target path is allowed (compared after resolution, so
  // "BF16/x.gguf" and "./BF16//x.gguf" count as the same file).
  //
  // The check-then-reserve below is intentionally synchronous (no `await` in between):
  // ipcMain.handle invocations run one at a time up to their first await, so two
  // near-simultaneous fs:startDownload calls for the same file can't both pass
  // the check before either reserves it — the second always sees the first's entry.
  for (const active of activeDownloads.values()) {
    if (active.finalPath === finalPath) {
      throw new Error(`Already downloading ${filename} (from another repo, or a duplicate request)`)
    }
  }
  const downloadId = randomUUID()
  const controller = new AbortController()
  activeDownloads.set(downloadId, { controller, finalPath })

  try {
    const alreadyExists = await fs
      .stat(finalPath)
      .then((s) => s.isFile())
      .catch(() => false)
    if (alreadyExists) {
      throw new Error(`File already exists: ${filename}`)
    }
    // A stale `.part` from a prior attempt is NOT deleted here: chunkedDownload.ts
    // inspects it (and its resume sidecar) and decides whether to resume or restart.
    // Creates the destination folder and any subfolder the file lives in.
    await fs.mkdir(path.dirname(finalPath), { recursive: true })
  } catch (err) {
    activeDownloads.delete(downloadId)
    throw err
  }

  void runDownload(
    downloadId,
    repoId,
    filename,
    sizeBytesHint,
    destinationDir,
    finalPath,
    partPath,
    meta,
    controller,
    onProgress
  )

  return downloadId
}

async function runDownload(
  downloadId: string,
  repoId: string,
  filename: string,
  sizeBytesHint: number,
  destinationDir: string,
  finalPath: string,
  partPath: string,
  meta: DownloadMeta,
  controller: AbortController,
  onProgress: ProgressCallback
): Promise<void> {
  let receivedBytes = 0
  let totalBytes = sizeBytesHint
  let lastEmitAt = 0

  const emit = (state: DownloadState, errorMessage?: string, force = false): void => {
    const now = Date.now()
    if (!force && state === 'downloading' && now - lastEmitAt < 250) return
    lastEmitAt = now
    const percent =
      totalBytes > 0 ? Math.min(100, Math.round((receivedBytes / totalBytes) * 100)) : 0
    onProgress({
      downloadId,
      repoId,
      filename,
      receivedBytes,
      totalBytes,
      percent,
      state,
      errorMessage
    })
  }

  const sidecarPath = `${partPath}.json`

  try {
    const url = `${HF_RESOLVE_BASE}/${repoId}/resolve/main/${filename.split('/').map(encodeURIComponent).join('/')}`

    totalBytes = await downloadFileResumable(
      url,
      partPath,
      sidecarPath,
      totalBytes,
      controller.signal,
      (n) => {
        receivedBytes += n
        emit('downloading')
      },
      (total) => {
        totalBytes = total
      }
    )

    await fs.rename(partPath, finalPath)
    try {
      await recordDownload(destinationDir, filename, {
        repoId,
        quant: meta.quant,
        sizeBytes: totalBytes,
        paramCount: meta.paramCount,
        downloadedAt: new Date().toISOString()
      })
    } catch {
      // The download itself succeeded; a manifest write failure (e.g. read-only
      // destination) shouldn't be reported as a failed download.
    }
    emit('completed', undefined, true)
  } catch (err) {
    // Cleanup-on-failure is handled inside chunkedDownload.ts: the single-stream
    // fallback deletes partPath (no resume possible there), while the chunked
    // path deliberately keeps partPath + its sidecar so a retry can resume.
    if (controller.signal.aborted) {
      emit('canceled', undefined, true)
    } else {
      const message = err instanceof Error ? err.message : String(err)
      emit('error', message, true)
    }
  } finally {
    activeDownloads.delete(downloadId)
  }
}

export function cancelDownload(downloadId: string): boolean {
  const active = activeDownloads.get(downloadId)
  if (!active) return false
  active.controller.abort()
  return true
}
