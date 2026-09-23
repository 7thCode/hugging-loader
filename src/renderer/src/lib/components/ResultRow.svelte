<script lang="ts">
  import type { FileEntry, RepoSummary } from '../../../../shared/ipc-types'
  import { downloadsByFilename } from '../state/downloads.svelte'
  import { markExistsOnDisk } from '../state/results.svelte'
  import { openGgufDialog } from '../state/ggufDialog.svelte'
  import { formatBytes, formatParamCount } from '../format'
  import ProgressBar from './ProgressBar.svelte'

  let { file, repo }: { file: FileEntry; repo: RepoSummary | undefined } = $props()

  let starting = $state(false)
  let localError = $state<string | null>(null)

  const progress = $derived(downloadsByFilename.get(file.filename))

  function formatDownloadedAt(iso: string): string {
    return `ダウンロード日時: ${new Date(iso).toLocaleString('ja-JP')}`
  }

  async function handleDownload(): Promise<void> {
    localError = null
    starting = true
    try {
      await window.api.startDownload({
        repoId: file.repoId,
        filename: file.filename,
        sizeBytes: file.sizeBytes,
        quant: file.quant,
        paramCount: file.paramCount
      })
    } catch (err) {
      localError = err instanceof Error ? err.message : String(err)
    } finally {
      starting = false
    }
  }

  async function handleCancel(): Promise<void> {
    if (!progress) return
    await window.api.cancelDownload({ downloadId: progress.downloadId })
  }

  async function handleDelete(): Promise<void> {
    if (!(await window.api.confirm(`${file.filename} を削除しますか？`))) return
    const res = await window.api.deleteFile({ filename: file.filename })
    if (res.success) {
      markExistsOnDisk(file.repoId, file.filename, false)
    }
  }
</script>

<tr>
  <td class="name">
    <div class="filename">{file.filename}</div>
    <div class="repo">
      {file.repoId}{#if repo?.pipelineTag}
        · {repo.pipelineTag}{/if}
    </div>
  </td>
  <td>{file.quant ?? '-'}</td>
  <td>{formatBytes(file.sizeBytes)}</td>
  <td>
    {formatParamCount(file.paramCount)}
    {#if file.paramCountSource === 'regex-estimate'}
      <span class="estimate" title="ファイル名からの推定値">推定</span>
    {/if}
  </td>
  <td class="actions">
    {#if progress?.state === 'downloading'}
      <ProgressBar percent={progress.percent} />
      <button onclick={handleCancel}>キャンセル</button>
    {:else if file.existsOnDisk}
      <span
        class="badge success"
        title={file.downloadedAt ? formatDownloadedAt(file.downloadedAt) : ''}
        >ダウンロード済み</span
      >
      <button onclick={() => openGgufDialog(file.filename)}>GGUF情報</button>
      <button class="danger" onclick={handleDelete}>削除</button>
    {:else}
      <button class="primary" disabled={starting} onclick={handleDownload}>ダウンロード</button>
      {#if progress?.state === 'error' || localError}
        <span class="error-badge" title={progress?.errorMessage ?? localError ?? ''}>エラー</span>
      {/if}
    {/if}
  </td>
</tr>

<style>
  td {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
    font-size: 13px;
  }

  .name {
    max-width: 360px;
  }

  .filename {
    font-weight: 600;
    word-break: break-all;
  }

  .repo {
    color: var(--text-muted);
    font-size: 12px;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
  }

  .badge {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 10px;
  }

  .badge.success {
    color: var(--success);
    background: color-mix(in srgb, var(--success) 15%, transparent);
  }

  .estimate {
    font-size: 11px;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0 6px;
  }

  .error-badge {
    font-size: 12px;
    color: var(--danger);
  }
</style>
