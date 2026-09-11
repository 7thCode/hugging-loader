import type { FileEntry, RepoSummary } from '../../../../shared/ipc-types'
import { filters } from './filters.svelte'

const FILE_LISTING_CONCURRENCY = 4

export class ResultsState {
  repos = $state<RepoSummary[]>([])
  files = $state<FileEntry[]>([])
  nextCursor = $state<string | null>(null)
  loading = $state(false)
  error = $state<string | null>(null)
  hasSearched = $state(false)
}

export const results = new ResultsState()

// Incremented on every new search so in-flight responses from a stale search
// (superseded by a newer keystroke/filter change) are discarded when they arrive.
let requestSeq = 0

export async function runSearch(): Promise<void> {
  const seq = ++requestSeq
  results.loading = true
  results.error = null
  results.repos = []
  results.files = []
  results.nextCursor = null
  results.hasSearched = true

  try {
    const res = await window.api.searchModels({
      search: filters.search.trim() || undefined,
      pipelineTag: filters.pipelineTag
    })
    if (seq !== requestSeq) return
    results.repos = res.items
    results.nextCursor = res.nextCursor
    await loadFilesForRepos(
      res.items.map((r) => r.id),
      seq
    )
  } catch (err) {
    if (seq !== requestSeq) return
    results.error = err instanceof Error ? err.message : String(err)
  } finally {
    if (seq === requestSeq) results.loading = false
  }
}

export async function loadMore(): Promise<void> {
  if (!results.nextCursor || results.loading) return
  const seq = requestSeq
  const cursor = results.nextCursor
  results.loading = true

  try {
    const res = await window.api.searchModels({
      search: filters.search.trim() || undefined,
      pipelineTag: filters.pipelineTag,
      cursor
    })
    if (seq !== requestSeq) return
    results.repos = [...results.repos, ...res.items]
    results.nextCursor = res.nextCursor
    await loadFilesForRepos(
      res.items.map((r) => r.id),
      seq
    )
  } catch (err) {
    if (seq !== requestSeq) return
    results.error = err instanceof Error ? err.message : String(err)
  } finally {
    if (seq === requestSeq) results.loading = false
  }
}

async function loadFilesForRepos(repoIds: string[], seq: number): Promise<void> {
  let cursor = 0

  async function worker(): Promise<void> {
    while (cursor < repoIds.length) {
      const repoId = repoIds[cursor++]
      try {
        const res = await window.api.listFiles({ repoId })
        if (seq !== requestSeq) return
        results.files = [...results.files, ...res.files]
        const repo = results.repos.find((r) => r.id === repoId)
        if (repo) {
          repo.paramCount = res.paramCount
          repo.paramCountSource = res.paramCountSource
        }
      } catch {
        // One repo failing to list files shouldn't block the rest of the page.
      }
    }
  }

  await Promise.all(
    Array.from({ length: Math.min(FILE_LISTING_CONCURRENCY, repoIds.length) }, () => worker())
  )
}

export function markExistsOnDisk(
  repoId: string,
  filename: string,
  exists: boolean,
  downloadedAt: string | null = null
): void {
  for (const file of results.files) {
    if (file.filename === filename && file.repoId === repoId) {
      file.existsOnDisk = exists
      file.downloadedAt = downloadedAt
    }
  }
}
