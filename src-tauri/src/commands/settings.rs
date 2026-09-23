// Ports src/main/ipc/settingsHandlers.ts (settings_get, settings_set_destination_dir) and
// src/main/ipc/dialogHandlers.ts (dialog_choose_folder). Command names are the snake_case
// counterparts of the old IPC channel names (settings:get -> settings_get, etc.) that
// src/renderer/src/lib/tauriShim.ts already invokes.

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::{MapErrString, Result};
use crate::ipc_types::{ChooseFolderResponse, SetDestinationDirRequest, Settings};
use crate::services::settings_store;
use crate::state::AppState;

#[tauri::command]
pub fn settings_get(app: AppHandle, state: State<AppState>) -> Result<Settings> {
    settings_store::load_settings(&app, &state.settings_cache)
}

#[tauri::command]
pub fn settings_set_destination_dir(
    app: AppHandle,
    state: State<AppState>,
    req: SetDestinationDirRequest,
) -> Result<Settings> {
    settings_store::set_destination_dir(&app, &state.settings_cache, req.dir)
}

#[tauri::command]
pub async fn dialog_choose_folder(app: AppHandle) -> Result<ChooseFolderResponse> {
    // The plugin's pick_folder is callback-based (it has to hop to the platform's native
    // dialog on the main thread), so bridge it into this async command with a oneshot channel
    // — same shape as awaiting Electron's `dialog.showOpenDialog(...)` promise.
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });

    let picked = rx.await.map_err_string()?;
    Ok(match picked {
        Some(file_path) => {
            let path = file_path.into_path().map_err_string()?;
            ChooseFolderResponse {
                canceled: false,
                path: Some(path.to_string_lossy().into_owned()),
            }
        }
        None => ChooseFolderResponse {
            canceled: true,
            path: None,
        },
    })
}
