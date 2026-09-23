/// <reference types="svelte" />
/// <reference types="vite/client" />

import type { HuggingLoaderApi } from '../../shared/ipc-types'

// Installed by src/renderer/src/lib/tauriShim.ts (via src/renderer/src/main.ts). Used to
// live in src/preload/index.d.ts back when this app ran on Electron, alongside a
// `window.electron: ElectronAPI` declaration that was never actually used by this app's own
// code and so wasn't carried over here.
declare global {
  interface Window {
    api: HuggingLoaderApi
  }
}
