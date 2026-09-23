// Ports the fs:checkExists / fs:deleteFile handlers of src/main/ipc/fsHandlers.ts.
// fs:startDownload / fs:cancelDownload land in Phase 6 alongside the download manager.

use tauri::{AppHandle, State};

use crate::error::Result;
use crate::ipc_types::{
    CheckExistsRequest, CheckExistsResponse, DeleteFileRequest, DeleteFileResponse,
};
use crate::services::{fs_util, manifest_store, settings_store};
use crate::state::AppState;

#[tauri::command]
pub async fn fs_check_exists(
    app: AppHandle,
    state: State<'_, AppState>,
    req: CheckExistsRequest,
) -> Result<CheckExistsResponse> {
    let settings = settings_store::load_settings(&app, &state.settings_cache)?;
    fs_util::check_exists(&settings.destination_dir, &req.filename).await
}

#[tauri::command]
pub async fn fs_delete_file(
    app: AppHandle,
    state: State<'_, AppState>,
    req: DeleteFileRequest,
) -> Result<DeleteFileResponse> {
    let settings = settings_store::load_settings(&app, &state.settings_cache)?;
    let success = fs_util::delete_file(&settings.destination_dir, &req.filename).await;
    if success {
        // fsHandlers.ts awaits removeManifestEntry unguarded too: a manifest write failure
        // here rejects the whole command even though the file itself was already deleted.
        // Surprising, but that's today's behavior — preserved rather than silently improved.
        manifest_store::remove_manifest_entry(&settings.destination_dir, &req.filename).await?;
    }
    Ok(DeleteFileResponse { success })
}
