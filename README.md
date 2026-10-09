# hugging-Loader

Browse and download GGUF models from Hugging Face Hub for use with llama.cpp.

A [Tauri](https://tauri.app/) (Rust + native WebView) desktop application, with a Svelte 5 +
TypeScript frontend.

## Recommended IDE Setup

- [VSCode](https://code.visualstudio.com/) + [ESLint](https://marketplace.visualstudio.com/items?itemName=dbaeumer.vscode-eslint) + [Prettier](https://marketplace.visualstudio.com/items?itemName=esbenp.prettier-vscode) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Project Setup

### Prerequisites

- Node.js
- Rust (see [Tauri's prerequisites guide](https://v2.tauri.app/start/prerequisites/) for
  platform-specific system dependencies)

### Install

```bash
$ npm install
```

### Development

```bash
$ npm run dev
```

### Build

```bash
$ npm run build
```

Produces platform-native installers (`.app`/`.dmg` on macOS, `.exe`/NSIS installer on Windows,
`.deb`/AppImage on Linux) under `src-tauri/target/release/bundle/`.
