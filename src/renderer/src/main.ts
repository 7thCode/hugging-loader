import { mount } from 'svelte'

import './app.css'

import App from './App.svelte'
import { installTauriApi } from './lib/tauriShim'

// Under Electron, the preload script (src/preload/index.ts) exposes `window.api` via
// contextBridge before this script runs. Under Tauri there's no preload step, so install
// the invoke()/listen()-backed shim ourselves — but only when Electron hasn't already
// supplied the real thing, so this file works unmodified under either shell during the
// migration.
if (!window.api) {
  installTauriApi()
}

const app = mount(App, {
  target: document.getElementById('app')!
})

export default app
