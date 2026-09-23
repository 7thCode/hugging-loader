use std::collections::HashMap;
use std::sync::Mutex;

use crate::ipc_types::Settings;
use crate::services::download_manager::ActiveDownload;

/// Shared app state, injected via `tauri::Builder::manage` and accessed from commands as
/// `tauri::State<AppState>`.
#[derive(Default)]
pub struct AppState {
    /// Mirrors the module-level `cached` variable in src/main/services/settingsStore.ts.
    pub settings_cache: Mutex<Option<Settings>>,
    /// Reused across every hf_api request instead of building a new client per call.
    pub http_client: reqwest::Client,
    /// Mirrors the module-level `activeDownloads` Map in src/main/services/downloadManager.ts.
    /// Keyed by downloadId.
    pub active_downloads: Mutex<HashMap<String, ActiveDownload>>,
}
