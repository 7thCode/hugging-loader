import type { FileEntry } from '../../../../shared/ipc-types'

export type SortKey = 'name' | 'size' | 'quant' | 'paramCount'
export type SortDir = 'asc' | 'desc'

export class SortState {
  key = $state<SortKey | null>(null)
  dir = $state<SortDir>('asc')
}

export const sortState = new SortState()

export function toggleSort(key: SortKey): void {
  if (sortState.key === key) {
    sortState.dir = sortState.dir === 'asc' ? 'desc' : 'asc'
  } else {
    sortState.key = key
    sortState.dir = 'asc'
  }
}

export function sortFiles(files: FileEntry[], key: SortKey | null, dir: SortDir): FileEntry[] {
  if (!key) return files
  const sign = dir === 'asc' ? 1 : -1
  return [...files].sort((a, b) => {
    switch (key) {
      case 'name':
        return sign * a.filename.localeCompare(b.filename)
      case 'size':
        return sign * (a.sizeBytes - b.sizeBytes)
      case 'quant':
        return sign * (a.quant ?? '').localeCompare(b.quant ?? '')
      case 'paramCount':
        // Unknown (null) param counts sort to the end regardless of direction.
        if (a.paramCount === null && b.paramCount === null) return 0
        if (a.paramCount === null) return 1
        if (b.paramCount === null) return -1
        return sign * (a.paramCount - b.paramCount)
    }
  })
}
