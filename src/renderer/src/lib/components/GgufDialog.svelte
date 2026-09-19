<script lang="ts">
  import { ggufDialog, closeGgufDialog } from '../state/ggufDialog.svelte'
  import { formatBytes } from '../format'
  import type { GgufMetadataEntry, GgufScalar } from '../../../../shared/ipc-types'

  // Rendering thousands of rows at once makes the dialog sluggish; the filter box finds the rest.
  const MAX_ROWS = 500
  const LONG_STRING_LENGTH = 120

  let dialogEl: HTMLDialogElement | undefined = $state()
  let tab = $state<'metadata' | 'tensors'>('metadata')
  let query = $state('')

  const header = $derived(ggufDialog.header)

  $effect(() => {
    if (!dialogEl) return
    if (ggufDialog.filename !== null && !dialogEl.open) {
      tab = 'metadata'
      query = ''
      dialogEl.showModal()
    } else if (ggufDialog.filename === null && dialogEl.open) {
      dialogEl.close()
    }
  })

  function setTab(next: 'metadata' | 'tensors'): void {
    tab = next
    query = ''
  }

  function onBackdropClick(event: MouseEvent): void {
    if (event.target === dialogEl) closeGgufDialog()
  }

  function findEntry(key: string): GgufMetadataEntry | undefined {
    return header?.metadata.find((entry) => entry.key === key)
  }

  function scalarText(key: string): string | null {
    const value = findEntry(key)?.value
    return value === undefined || value === null ? null : formatScalar(value, findEntry(key)?.type)
  }

  function formatScalar(value: GgufScalar, type?: string): string {
    if (typeof value === 'number') {
      // float32 values arrive widened to double (0.00001 -> 0.000009999999747...); show the float32 form.
      if (type === 'float32') return String(parseFloat(value.toPrecision(7)))
      return value.toLocaleString('ja-JP')
    }
    return String(value)
  }

  function formatPreviewItem(item: GgufScalar): string {
    return typeof item === 'string' ? JSON.stringify(item) : String(item)
  }

  const summary = $derived.by(() => {
    if (!header) return []
    const arch = findEntry('general.architecture')?.value
    const prefix = typeof arch === 'string' ? arch : null
    const vocab = findEntry('tokenizer.ggml.tokens')?.array?.length
    const heads = prefix ? scalarText(`${prefix}.attention.head_count`) : null
    const kvHeads = prefix ? scalarText(`${prefix}.attention.head_count_kv`) : null
    const items: { label: string; value: string | null }[] = [
      { label: 'アーキテクチャ', value: scalarText('general.architecture') },
      { label: 'モデル名', value: scalarText('general.name') },
      {
        label: '量子化 (file_type)',
        value: header.fileTypeName ?? scalarText('general.file_type')
      },
      { label: 'コンテキスト長', value: prefix ? scalarText(`${prefix}.context_length`) : null },
      { label: '埋め込み次元', value: prefix ? scalarText(`${prefix}.embedding_length`) : null },
      { label: 'レイヤー数', value: prefix ? scalarText(`${prefix}.block_count`) : null },
      {
        label: 'アテンションヘッド数',
        value: heads && kvHeads ? `${heads}（KV: ${kvHeads}）` : heads
      },
      { label: '語彙数', value: vocab !== undefined ? vocab.toLocaleString('ja-JP') : null },
      { label: 'トークナイザー', value: scalarText('tokenizer.ggml.model') }
    ]
    return items.filter((item) => item.value)
  })

  const stats = $derived(
    header
      ? [
          { label: 'GGUFバージョン', value: String(header.version) },
          { label: 'テンソル数', value: header.tensorCount.toLocaleString('ja-JP') },
          { label: 'メタデータ数', value: header.metadataCount.toLocaleString('ja-JP') },
          { label: 'ファイルサイズ', value: formatBytes(header.fileSize) },
          { label: 'ヘッダーサイズ', value: formatBytes(header.headerSize) },
          {
            label: `データ開始位置（アライメント ${header.alignment}）`,
            value: `${header.dataOffset.toLocaleString('ja-JP')} B`
          }
        ]
      : []
  )

  const needle = $derived(query.trim().toLowerCase())

  const filteredMetadata = $derived(
    (header?.metadata ?? []).filter((entry) => {
      if (!needle) return true
      if (entry.key.toLowerCase().includes(needle)) return true
      if (entry.type.toLowerCase().includes(needle)) return true
      if (entry.value !== null && String(entry.value).toLowerCase().includes(needle)) return true
      return (
        entry.array?.preview.some((item) => String(item).toLowerCase().includes(needle)) ?? false
      )
    })
  )

  const filteredTensors = $derived(
    (header?.tensors ?? []).filter(
      (tensor) =>
        !needle ||
        tensor.name.toLowerCase().includes(needle) ||
        tensor.type.toLowerCase().includes(needle)
    )
  )

  const activeCount = $derived(
    tab === 'metadata' ? filteredMetadata.length : filteredTensors.length
  )
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialogEl}
  aria-labelledby="gguf-dialog-title"
  onclose={closeGgufDialog}
  onclick={onBackdropClick}
>
  <div class="frame">
    <div class="title-bar">
      <div class="title-text">
        <h2 id="gguf-dialog-title">GGUF ヘッダー情報</h2>
        <div class="filename">{ggufDialog.filename}</div>
      </div>
      <button type="button" class="close" aria-label="閉じる" onclick={closeGgufDialog}>✕</button>
    </div>

    <div class="body">
      {#if ggufDialog.loading}
        <p class="state">読み込み中...</p>
      {:else if ggufDialog.error}
        <p class="state error">{ggufDialog.error}</p>
      {:else if header}
        <div class="stats">
          {#each stats as stat (stat.label)}
            <div class="stat">
              <div class="stat-label">{stat.label}</div>
              <div class="stat-value">{stat.value}</div>
            </div>
          {/each}
        </div>

        {#if summary.length > 0}
          <dl class="summary">
            {#each summary as item (item.label)}
              <dt>{item.label}</dt>
              <dd>{item.value}</dd>
            {/each}
          </dl>
        {/if}

        <div class="toolbar">
          <div class="tabs" role="tablist">
            <button
              type="button"
              role="tab"
              aria-selected={tab === 'metadata'}
              class:active={tab === 'metadata'}
              onclick={() => setTab('metadata')}
            >
              メタデータ ({header.metadata.length.toLocaleString('ja-JP')})
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={tab === 'tensors'}
              class:active={tab === 'tensors'}
              onclick={() => setTab('tensors')}
            >
              テンソル ({header.tensors.length.toLocaleString('ja-JP')})
            </button>
          </div>
          <input
            type="text"
            placeholder={tab === 'metadata' ? 'キー・値で絞り込み' : 'テンソル名・型で絞り込み'}
            bind:value={query}
          />
        </div>

        {#if tab === 'metadata'}
          <table>
            <thead>
              <tr><th class="col-key">キー</th><th class="col-type">型</th><th>値</th></tr>
            </thead>
            <tbody>
              {#each filteredMetadata.slice(0, MAX_ROWS) as entry (entry.key)}
                <tr>
                  <td class="mono">{entry.key}</td>
                  <td class="muted">{entry.type}</td>
                  <td>
                    {#if entry.array}
                      <div class="muted">{entry.array.length.toLocaleString('ja-JP')} 要素</div>
                      <div class="mono preview">
                        [{entry.array.preview.map(formatPreviewItem).join(', ')}{entry.array
                          .length > entry.array.preview.length
                          ? ', …'
                          : ''}]
                      </div>
                    {:else if typeof entry.value === 'string' && (entry.value.length > LONG_STRING_LENGTH || entry.value.includes('\n'))}
                      <pre class="long">{entry.value}</pre>
                    {:else if entry.value !== null}
                      <span class="mono">{formatScalar(entry.value, entry.type)}</span>
                      {#if entry.key === 'general.file_type' && header.fileTypeName}
                        <span class="muted"> ({header.fileTypeName})</span>
                      {/if}
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <table>
            <thead>
              <tr>
                <th>名前</th><th class="col-shape">形状</th><th class="col-type">型</th>
                <th class="col-offset">オフセット</th>
              </tr>
            </thead>
            <tbody>
              {#each filteredTensors.slice(0, MAX_ROWS) as tensor (tensor.name)}
                <tr>
                  <td class="mono">{tensor.name}</td>
                  <td class="mono">{tensor.dims.join(' × ') || '-'}</td>
                  <td class="mono">{tensor.type}</td>
                  <td class="mono num">{tensor.offset.toLocaleString('ja-JP')}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}

        {#if activeCount === 0}
          <p class="state">該当する項目がありません。</p>
        {:else if activeCount > MAX_ROWS}
          <p class="state">
            {activeCount.toLocaleString('ja-JP')} 件中、先頭 {MAX_ROWS} 件のみ表示しています。上の欄で絞り込んでください。
          </p>
        {/if}
      {/if}
    </div>
  </div>
</dialog>

<style>
  dialog {
    width: min(980px, 94vw);
    max-height: 88vh;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
  }

  dialog[open] {
    display: flex;
    flex-direction: column;
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.45);
  }

  .frame {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }

  .title-bar {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    font-size: 15px;
    margin: 0 0 2px;
  }

  .filename {
    font-size: 12px;
    color: var(--text-muted);
    word-break: break-all;
    font-family: ui-monospace, Menlo, Consolas, monospace;
  }

  .close {
    flex-shrink: 0;
    padding: 2px 9px;
  }

  .body {
    overflow-y: auto;
    padding: 16px;
    min-height: 0;
  }

  .state {
    text-align: center;
    color: var(--text-muted);
    padding: 24px 4px;
  }

  .state.error {
    color: var(--danger);
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    margin-bottom: 16px;
  }

  @media (max-width: 560px) {
    .stats {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .stat {
    background: var(--bg-alt);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
  }

  .stat-label {
    font-size: 11px;
    color: var(--text-muted);
  }

  .stat-value {
    font-size: 14px;
    font-weight: 600;
  }

  .summary {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 16px;
    margin: 0 0 18px;
    font-size: 13px;
  }

  .summary dt {
    color: var(--text-muted);
  }

  .summary dd {
    margin: 0;
    font-weight: 600;
    word-break: break-all;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
    flex-wrap: wrap;
  }

  .tabs {
    display: flex;
    gap: 6px;
  }

  .tabs button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }

  .toolbar input {
    min-width: 220px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  th {
    text-align: left;
    color: var(--text-muted);
    font-weight: 600;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
  }

  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
    word-break: break-all;
  }

  .col-key {
    width: 32%;
  }

  .col-type,
  .col-shape {
    width: 14%;
  }

  .col-offset {
    width: 16%;
  }

  .mono {
    font-family: ui-monospace, Menlo, Consolas, monospace;
  }

  .num {
    text-align: right;
  }

  .muted {
    color: var(--text-muted);
  }

  .preview {
    word-break: break-word;
  }

  pre.long {
    margin: 0;
    max-height: 160px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: ui-monospace, Menlo, Consolas, monospace;
    font-size: 12px;
    background: var(--bg-alt);
    border-radius: 6px;
    padding: 6px 8px;
  }
</style>
