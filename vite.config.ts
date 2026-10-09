import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// Frontend build for the Tauri shell.
const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  root: 'src/renderer',
  plugins: [
    // configFile is explicit because `root` is src/renderer, not the project root where
    // svelte.config.mjs actually lives — without this, vite-plugin-svelte can't find it.
    svelte({ configFile: fileURLToPath(new URL('./svelte.config.mjs', import.meta.url)) })
  ],

  // Tauri expects a fixed, predictable dev server port (see src-tauri/tauri.conf.json's
  // build.devUrl) and to fail fast rather than silently moving to another port.
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: {
      // Don't rebuild the frontend when Rust sources change under src-tauri.
      ignored: ['**/src-tauri/**']
    }
  },
  envPrefix: ['VITE_', 'TAURI_'],

  build: {
    // src/renderer -> src -> <project root> -> dist, matching tauri.conf.json's frontendDist.
    outDir: '../../dist',
    emptyOutDir: true,
    // Tauri's minimum supported webview targets.
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG
  }
})
