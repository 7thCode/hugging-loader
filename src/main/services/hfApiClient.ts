import { HF_API_BASE, SEARCH_PAGE_LIMIT } from '../constants'
import { parseQuant } from './quantParser'
import { resolveParamCount } from './paramCount'
import type {
  FileEntry,
  ListFilesResponse,
  RepoSummary,
  SearchModelsRequest,
  SearchModelsResponse
} from '../../shared/ipc-types'

interface HfSearchItem {
  id: string
  author?: string
  downloads?: number
  likes?: number
  pipeline_tag?: string | null
  tags?: string[]
}

interface HfSibling {
  rfilename: string
  size?: number
}

interface HfModelInfo {
  id: string
  author?: string
  downloads?: number
  likes?: number
  pipeline_tag?: string | null
  tags?: string[]
  siblings?: HfSibling[]
  gguf?: { total?: number } | null
}

function parseNextCursor(linkHeader: string | null): string | null {
  if (!linkHeader) return null
  const match = linkHeader.match(/<([^>]+)>;\s*rel="next"/)
  if (!match) return null
  try {
    const url = new URL(match[1])
    return url.searchParams.get('cursor')
  } catch {
    return null
  }
}

export async function searchRepos(req: SearchModelsRequest): Promise<SearchModelsResponse> {
  const params = new URLSearchParams()
  if (req.search) params.set('search', req.search)
  params.set('filter', 'gguf')
  if (req.pipelineTag) params.set('pipeline_tag', req.pipelineTag)
  params.set('sort', 'downloads')
  params.set('direction', '-1')
  params.set('limit', String(req.limit ?? SEARCH_PAGE_LIMIT))
  if (req.cursor) params.set('cursor', req.cursor)

  const url = `${HF_API_BASE}?${params.toString()}`
  const res = await fetch(url)
  if (!res.ok) {
    throw new Error(`Hugging Face search failed: ${res.status} ${res.statusText}`)
  }
  const items = (await res.json()) as HfSearchItem[]
  const nextCursor = parseNextCursor(res.headers.get('Link'))

  const repos: RepoSummary[] = items.map((item) => ({
    id: item.id,
    author: item.author ?? item.id.split('/')[0] ?? '',
    downloads: item.downloads ?? 0,
    likes: item.likes ?? 0,
    pipelineTag: item.pipeline_tag ?? null,
    tags: item.tags ?? [],
    paramCount: null,
    paramCountSource: 'unknown'
  }))

  return { items: repos, nextCursor }
}

export async function fetchRepoFiles(
  repoId: string
): Promise<ListFilesResponse & { tags: string[]; pipelineTag: string | null }> {
  const url = `${HF_API_BASE}/${repoId}?blobs=true`
  const res = await fetch(url)
  if (!res.ok) {
    throw new Error(`Hugging Face model info failed for ${repoId}: ${res.status} ${res.statusText}`)
  }
  const info = (await res.json()) as HfModelInfo

  const { paramCount, paramCountSource } = resolveParamCount(
    repoId,
    info.tags ?? [],
    info.gguf?.total ?? null
  )

  const files: FileEntry[] = (info.siblings ?? [])
    .filter((s) => s.rfilename.toLowerCase().endsWith('.gguf'))
    .map((s) => ({
      repoId,
      filename: s.rfilename,
      sizeBytes: s.size ?? 0,
      quant: parseQuant(s.rfilename),
      existsOnDisk: false, // annotated by the fs layer in the IPC handler
      downloadedAt: null, // annotated by the fs layer in the IPC handler
      paramCount,
      paramCountSource
    }))

  return {
    repoId,
    files,
    paramCount,
    paramCountSource,
    tags: info.tags ?? [],
    pipelineTag: info.pipeline_tag ?? null
  }
}
