import { mount } from 'svelte'

import './app.css'

import App from './App.svelte'
import { installTauriApi } from './lib/tauriShim'

installTauriApi()

const app = mount(App, {
  target: document.getElementById('app')!
})

export default app
