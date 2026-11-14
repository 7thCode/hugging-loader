# hugging-Loader

Hugging Face Hub から llama.cpp 用の GGUF モデルを検索・絞り込み・並列ダウンロードする
デスクトップアプリ。**Tauri 2 (Rust) + Svelte 5** 構成（Electron ではない）。

## コマンド

```bash
npm run dev          # tauri dev でアプリを起動（開発モード）
npm run typecheck    # tsc (node) + svelte-check
npm run lint          # eslint --cache .
npm run format        # prettier --write .（svelte含む）
npm run build         # typecheck → tauri build（パッケージング）
```

Rust 側のチェックは `src-tauri` 内で `cargo check` / `cargo clippy` を使う。

## 構成

- `src/renderer/src/` — Svelte 5 フロントエンド（runes: `$state`/`$derived`、
  状態は `lib/state/*.svelte.ts`）
- `src/shared/ipc-types.ts` — フロントエンド↔Rust の IPC 型定義。
  **`src-tauri/src/ipc_types.rs` と手動で同期させる**（型を変えたら両方を直す）
- `src-tauri/src/commands/` — Tauri コマンド（hf, fs_cmd, gguf, settings）
- `src-tauri/src/services/` — ビジネスロジック（chunked_download, download_manager,
  gguf_parser, hf_api, manifest_store, quant_parser, settings_store, fs_util）
- `docs/index.html` — GitHub Pages のダウンロードページ（バージョンアップ時に手動更新）

## リリース手順

1. `package.json` の `version` と `src-tauri/tauri.conf.json` の `version` を上げる
2. `docs/index.html` 内のダウンロードリンク・バージョン表記を更新
3. 変更内容を確認（typecheck/lint、動作確認）— タグを push すると即公開されるため、
   **タグを push する前に**確認を終えておく
4. `git tag vX.Y.Z && git push origin vX.Y.Z`
   → GitHub Actions が mac/Windows/Linux をビルドし、`.github/workflows/build.yml` の
   `releaseDraft: false` 設定により **ドラフトを経由せず GitHub Release として即公開**される

ビルドは**未署名**（有効な Apple 証明書なし）。macOS は右クリック→「開く」、
Windows は SmartScreen の「詳細情報」→「実行」が必要。

## 開発ルール

- `main` には直接 push しない。フィーチャーブランチ → PR
- IPC の型（`ipc-types.ts` / `ipc_types.rs`）を変更したら、必ず両言語側を同時に直す
- コミット前に `npm run typecheck` と `npm run lint` を通す
