<script lang="ts">
  import { results, loadMore } from '../state/results.svelte'
  import { filters } from '../state/filters.svelte'
  import { sortState, sortFiles, toggleSort, type SortKey } from '../state/sort.svelte'
  import { bucketForParamCount, type RepoSummary } from '../../../../shared/ipc-types'
  import ResultRow from './ResultRow.svelte'

  const filteredFiles = $derived(
    results.files.filter((f) => {
      if (filters.quants.size > 0 && (!f.quant || !filters.quants.has(f.quant))) return false
      if (filters.paramBucket && filters.paramBucket !== bucketForParamCount(f.paramCount))
        return false
      return true
    })
  )

  const visibleFiles = $derived(sortFiles(filteredFiles, sortState.key, sortState.dir))

  function repoFor(repoId: string): RepoSummary | undefined {
    return results.repos.find((r) => r.id === repoId)
  }

  function sortIndicator(key: SortKey): string {
    if (sortState.key !== key) return ''
    return sortState.dir === 'asc' ? ' ▲' : ' ▼'
  }
</script>

<div class="results">
  {#if results.error}
    <p class="error">エラー: {results.error}</p>
  {/if}

  {#if !results.hasSearched}
    <p class="empty">モデル名で検索するか、フィルタを選択してください。</p>
  {:else if !results.loading && visibleFiles.length === 0}
    <p class="empty">該当するモデルが見つかりませんでした。</p>
  {/if}

  {#if visibleFiles.length > 0}
    <table>
      <thead>
        <tr>
          <th
            ><button class="sort-head" onclick={() => toggleSort('name')}
              >名称{sortIndicator('name')}</button
            ></th
          >
          <th
            ><button class="sort-head" onclick={() => toggleSort('quant')}
              >量子化{sortIndicator('quant')}</button
            ></th
          >
          <th
            ><button class="sort-head" onclick={() => toggleSort('size')}
              >サイズ{sortIndicator('size')}</button
            ></th
          >
          <th
            ><button class="sort-head" onclick={() => toggleSort('paramCount')}
              >パラメータ数{sortIndicator('paramCount')}</button
            ></th
          >
          <th
            ><button
              class="sort-head"
              title="ダウンロード済みを上に並べます"
              onclick={() => toggleSort('downloaded')}>操作{sortIndicator('downloaded')}</button
            ></th
          >
        </tr>
      </thead>
      <tbody>
        {#each visibleFiles as file (file.repoId + '/' + file.filename)}
          <ResultRow {file} repo={repoFor(file.repoId)} />
        {/each}
      </tbody>
    </table>
  {/if}

  {#if results.loading}
    <p class="loading">読み込み中...</p>
  {/if}

  {#if results.nextCursor && !results.loading}
    <div class="load-more">
      <button onclick={loadMore}>もっと見る</button>
    </div>
  {/if}
</div>

<style>
  .results {
    flex: 1;
    overflow-y: auto;
    padding: 0 12px 16px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th {
    text-align: left;
    font-size: 12px;
    color: var(--text-muted);
    padding: 0;
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    background: var(--bg);
  }

  .sort-head {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-radius: 0;
    padding: 8px 10px;
    font-size: 12px;
    font-weight: inherit;
    color: inherit;
    cursor: pointer;
  }

  .sort-head:hover {
    color: var(--text);
    border-color: transparent;
  }

  .empty,
  .loading {
    color: var(--text-muted);
    padding: 24px 4px;
    text-align: center;
  }

  .error {
    color: var(--danger);
    padding: 8px 4px;
  }

  .load-more {
    display: flex;
    justify-content: center;
    padding: 16px 0;
  }
</style>
