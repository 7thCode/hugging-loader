// Ports src/main/ipc/fsHandlers.ts.

use tauri::{AppHandle, State};

use crate::error::Result;
use crate::ipc_types::{
    CancelDownloadRequest, CancelDownloadResponse, CheckExistsRequest, CheckExistsResponse,
    DeleteFileRequest, DeleteFileResponse, StartDownloadRequest, StartDownloadResponse,
};
use crate::services::{download_manager, fs_util, manifest_store, settings_store};
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

#[tauri::command]
pub async fn fs_start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    req: StartDownloadRequest,
) -> Result<StartDownloadResponse> {
    let settings = settings_store::load_settings(&app, &state.settings_cache)?;
    let download_id = download_manager::start_download(
        app.clone(),
        req.repo_id,
        req.filename,
        req.size_bytes,
        settings.destination_dir,
        download_manager::DownloadMeta {
            quant: req.quant,
            param_count: req.param_count,
        },
    )
    .await?;
    Ok(StartDownloadResponse { download_id })
}

#[tauri::command]
pub fn fs_cancel_download(
    state: State<AppState>,
    req: CancelDownloadRequest,
) -> CancelDownloadResponse {
    CancelDownloadResponse {
        success: download_manager::cancel_download(&state, &req.download_id),
    }
}
