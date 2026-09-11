import type { ParamCountSource } from '../../shared/ipc-types'

// Matches patterns like "8B", "8.5B" (billions) or "500M" (millions, normalized to raw units).
const BILLIONS_RE = /(\d+(?:\.\d+)?)\s*[Bb](?![a-zA-Z])/
const MILLIONS_RE = /(\d+(?:\.\d+)?)\s*[Mm](?![a-zA-Z])/

export interface ResolvedParamCount {
  paramCount: number | null
  paramCountSource: ParamCountSource
}

/**
 * Resolve a repo's parameter count.
 * Primary signal: `gguf.total` from the HF model-info response (?blobs=true), which is
 * parsed server-side from the actual GGUF metadata and is authoritative.
 * Fallback: regex over the repo id / tags when `gguf` metadata is absent.
 */
export function resolveParamCount(
  repoId: string,
  tags: string[],
  ggufTotal: number | null | undefined
): ResolvedParamCount {
  if (typeof ggufTotal === 'number' && ggufTotal > 0) {
    return { paramCount: ggufTotal, paramCountSource: 'gguf-metadata' }
  }

  const candidates = [repoId, ...tags]
  for (const candidate of candidates) {
    const bMatch = candidate.match(BILLIONS_RE)
    if (bMatch) {
      return {
        paramCount: Math.round(parseFloat(bMatch[1]) * 1_000_000_000),
        paramCountSource: 'regex-estimate'
      }
    }
  }
  for (const candidate of candidates) {
    const mMatch = candidate.match(MILLIONS_RE)
    if (mMatch) {
      return {
        paramCount: Math.round(parseFloat(mMatch[1]) * 1_000_000),
        paramCountSource: 'regex-estimate'
      }
    }
  }

  return { paramCount: null, paramCountSource: 'unknown' }
}
