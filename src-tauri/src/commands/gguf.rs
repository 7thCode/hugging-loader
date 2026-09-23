// Ports src/main/ipc/ggufHandlers.ts.

use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::error::Result;
use crate::ipc_types::{GgufHeaderResponse, ReadGgufHeaderRequest};
use crate::services::{fs_util, gguf_parser, settings_store};
use crate::state::AppState;

/// On top of the destination-folder containment check, only .gguf files may be read. Matches
/// resolveModelPath.ts's plain string-suffix check exactly (rather than Rust's `Path::extension`,
/// which — unlike this — treats a file literally named ".gguf" as having no extension at all).
fn resolve_model_path(destination_dir: &str, filename: &str) -> Result<PathBuf> {
    let resolved = fs_util::resolve_in_destination(destination_dir, filename)?;
    if !resolved.to_string_lossy().to_lowercase().ends_with(".gguf") {
        return Err(".gguf ファイルのみ読み込めます".to_string());
    }
    Ok(resolved)
}

#[tauri::command]
pub async fn gguf_read_header(
    app: AppHandle,
    state: State<'_, AppState>,
    req: ReadGgufHeaderRequest,
) -> Result<GgufHeaderResponse> {
    let settings = settings_store::load_settings(&app, &state.settings_cache)?;
    let path = resolve_model_path(&settings.destination_dir, &req.filename)?;
    gguf_parser::read_gguf_header(path).await
}
