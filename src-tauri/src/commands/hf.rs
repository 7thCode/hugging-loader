// Ports src/main/ipc/hfHandlers.ts.

use tauri::{AppHandle, State};

use crate::error::Result;
use crate::ipc_types::{
    ListFilesRequest, ListFilesResponse, SearchModelsRequest, SearchModelsResponse,
};
use crate::services::{fs_util, hf_api, manifest_store, settings_store};
use crate::state::AppState;

#[tauri::command]
pub async fn hf_search_models(
    state: State<'_, AppState>,
    req: SearchModelsRequest,
) -> Result<SearchModelsResponse> {
    hf_api::search_repos(&state.http_client, &req).await
}

#[tauri::command]
pub async fn hf_list_files(
    app: AppHandle,
    state: State<'_, AppState>,
    req: ListFilesRequest,
) -> Result<ListFilesResponse> {
    let result = hf_api::fetch_repo_files(&state.http_client, &req.repo_id).await?;
    let settings = settings_store::load_settings(&app, &state.settings_cache)?;

    let filenames: Vec<String> = result.files.iter().map(|f| f.filename.clone()).collect();
    let (existing, manifest) = tokio::join!(
        fs_util::find_existing_filenames(&settings.destination_dir, &filenames),
        manifest_store::load_manifest(&settings.destination_dir)
    );

    let files = result
        .files
        .into_iter()
        .map(|mut f| {
            let on_disk = existing.contains(&f.filename);
            let manifest_entry = manifest.get(&f.filename);
            // If the manifest attributes this filename to a different repo, don't claim it
            // for this repo — two repos can legitimately publish an identically-named file.
            let exists_on_disk = on_disk
                && manifest_entry
                    .map(|e| e.repo_id == f.repo_id)
                    .unwrap_or(true);
            f.exists_on_disk = exists_on_disk;
            f.downloaded_at = if exists_on_disk {
                manifest_entry.map(|e| e.downloaded_at.clone())
            } else {
                None
            };
            f
        })
        .collect();

    Ok(ListFilesResponse {
        repo_id: result.repo_id,
        files,
        param_count: result.param_count,
        param_count_source: result.param_count_source,
    })
}
