<script lang="ts">
  import {
    COMMON_PIPELINE_TAGS,
    PARAM_COUNT_BUCKETS,
    QUANT_TYPES
  } from '../../../../shared/ipc-types'
  import { filters, toggleQuant } from '../state/filters.svelte'
  import { runSearch } from '../state/results.svelte'

  let quantMenuOpen = $state(false)

  let searchTimeout: ReturnType<typeof setTimeout> | undefined

  function onSearchInput(): void {
    if (searchTimeout) clearTimeout(searchTimeout)
    searchTimeout = setTimeout(() => runSearch(), 350)
  }

  function onPipelineTagChange(): void {
    runSearch()
  }

  function onParamBucketChange(): void {
    // Client-side filter only — no need to re-fetch.
  }

  function onQuantToggle(quant: string): void {
    toggleQuant(quant)
  }
</script>

<div class="filter-bar">
  <input
    type="text"
    placeholder="モデル名で検索 (例: llama)"
    bind:value={filters.search}
    oninput={onSearchInput}
  />

  <select bind:value={filters.pipelineTag} onchange={onPipelineTagChange}>
    <option value={null}>タスク: すべて</option>
    {#each COMMON_PIPELINE_TAGS as tag (tag)}
      <option value={tag}>{tag}</option>
    {/each}
  </select>

  <select bind:value={filters.paramBucket} onchange={onParamBucketChange}>
    <option value={null}>パラメータ数: すべて</option>
    {#each PARAM_COUNT_BUCKETS as bucket (bucket.id)}
      <option value={bucket.id}>{bucket.label}</option>
    {/each}
  </select>

  <div class="quant-select">
    <button type="button" onclick={() => (quantMenuOpen = !quantMenuOpen)}>
      量子化タイプ{filters.quants.size > 0 ? ` (${filters.quants.size})` : ''} ▾
    </button>
    {#if quantMenuOpen}
      <div class="quant-menu">
        {#each QUANT_TYPES as quant (quant)}
          <label>
            <input
              type="checkbox"
              checked={filters.quants.has(quant)}
              onchange={() => onQuantToggle(quant)}
            />
            {quant}
          </label>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .filter-bar {
    display: flex;
    gap: 8px;
    padding: 12px;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
    align-items: flex-start;
  }

  input[type='text'] {
    flex: 1;
    min-width: 200px;
  }

  .quant-select {
    position: relative;
  }

  .quant-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 10;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px;
    display: grid;
    grid-template-columns: repeat(3, minmax(80px, 1fr));
    gap: 4px 12px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    max-height: 260px;
    overflow-y: auto;
  }

  .quant-menu label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    white-space: nowrap;
  }
</style>
