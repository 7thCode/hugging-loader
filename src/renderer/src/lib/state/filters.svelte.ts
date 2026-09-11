import { SvelteSet } from 'svelte/reactivity'
import type { ParamCountBucketId } from '../../../../shared/ipc-types'

export class FiltersState {
  search = $state('')
  pipelineTag = $state<string | null>(null)
  quants = new SvelteSet<string>()
  paramBucket = $state<ParamCountBucketId | null>(null)
}

export const filters = new FiltersState()

export function toggleQuant(quant: string): void {
  if (filters.quants.has(quant)) {
    filters.quants.delete(quant)
  } else {
    filters.quants.add(quant)
  }
}
