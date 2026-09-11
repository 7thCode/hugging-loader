<script lang="ts">
  import { onMount } from 'svelte'
  import FilterBar from './lib/components/FilterBar.svelte'
  import ResultsList from './lib/components/ResultsList.svelte'
  import SettingsPanel from './lib/components/SettingsPanel.svelte'
  import { loadSettings } from './lib/state/settings.svelte'
  import { subscribeDownloadProgress } from './lib/state/downloads.svelte'
  import { runSearch } from './lib/state/results.svelte'

  onMount(() => {
    subscribeDownloadProgress()
    void loadSettings()
    void runSearch()
  })
</script>

<header>
  <h1>hugging-Loader</h1>
  <p class="subtitle">llama.cpp 用の GGUF モデルを Hugging Face から検索・ダウンロード</p>
</header>

<SettingsPanel />
<FilterBar />
<ResultsList />

<style>
  header {
    padding: 12px 16px 4px;
  }

  h1 {
    font-size: 16px;
    margin: 0;
  }

  .subtitle {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
