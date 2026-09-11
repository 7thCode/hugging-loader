<script lang="ts">
  import { settingsState, changeDestinationFolder } from '../state/settings.svelte'
</script>

<div class="settings">
  <span class="label">保存先:</span>
  <span class="path" title={settingsState.destinationDir}
    >{settingsState.destinationDir || '読み込み中...'}</span
  >
  <button onclick={changeDestinationFolder}>フォルダ変更...</button>
</div>

<style>
  .settings {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    color: var(--text-muted);
  }

  .label {
    font-weight: 600;
  }

  .path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, Menlo, Consolas, monospace;
  }
</style>
