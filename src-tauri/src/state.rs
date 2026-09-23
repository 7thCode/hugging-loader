use std::sync::Mutex;

use crate::ipc_types::Settings;

/// Shared app state, injected via `tauri::Builder::manage` and accessed from commands as
/// `tauri::State<AppState>`.
#[derive(Default)]
pub struct AppState {
    /// Mirrors the module-level `cached` variable in src/main/services/settingsStore.ts.
    pub settings_cache: Mutex<Option<Settings>>,
}
