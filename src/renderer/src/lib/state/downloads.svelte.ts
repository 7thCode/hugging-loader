import { SvelteMap } from 'svelte/reactivity'
import type { DownloadProgressEvent } from '../../../../shared/ipc-types'
import { markExistsOnDisk } from './results.svelte'

export const downloadsByFilename = new SvelteMap<string, DownloadProgressEvent>()

let subscribed = false

export function subscribeDownloadProgress(): void {
  if (subscribed) return
  subscribed = true
  window.api.onDownloadProgress((event) => {
    downloadsByFilename.set(event.filename, event)
    if (event.state === 'completed') {
      // One-shot timestamp string, not stored as a live Date — SvelteReactivity doesn't apply.
      // eslint-disable-next-line svelte/prefer-svelte-reactivity
      markExistsOnDisk(event.repoId, event.filename, true, new Date().toISOString())
    }
  })
}
