// Ports src/main/services/settingsStore.ts. Behavior kept identical:
// - settings live in `<app config dir>/config.json`
// - a valid on-disk file is cached and never rewritten just because it was read
// - a missing/corrupt/empty-destinationDir file falls back to DEFAULT_DESTINATION_DIR (if it
//   exists on disk) or else the OS Downloads folder, and that computed default IS persisted

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::constants::DEFAULT_DESTINATION_DIR;
use crate::error::{MapErrString, Result};
use crate::ipc_types::Settings;

fn config_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err_string()?
        .join("config.json"))
}

fn directory_exists(dir: &str) -> bool {
    std::fs::metadata(dir).map(|m| m.is_dir()).unwrap_or(false)
}

fn resolve_default_destination(app: &AppHandle) -> Result<String> {
    if directory_exists(DEFAULT_DESTINATION_DIR) {
        return Ok(DEFAULT_DESTINATION_DIR.to_string());
    }
    let downloads = app.path().download_dir().map_err_string()?;
    Ok(downloads.to_string_lossy().into_owned())
}

fn read_from_disk(app: &AppHandle) -> Option<Settings> {
    let path = config_path(app).ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let settings: Settings = serde_json::from_str(&raw).ok()?;
    if settings.destination_dir.is_empty() {
        return None;
    }
    Some(settings)
}

pub fn load_settings(app: &AppHandle, cache: &Mutex<Option<Settings>>) -> Result<Settings> {
    if let Some(cached) = cache.lock().unwrap().as_ref() {
        return Ok(cached.clone());
    }

    if let Some(settings) = read_from_disk(app) {
        *cache.lock().unwrap() = Some(settings.clone());
        return Ok(settings);
    }

    // Missing or corrupt config file: fall through to defaults, and persist them (also
    // populates the cache via save_settings).
    let settings = Settings {
        destination_dir: resolve_default_destination(app)?,
    };
    save_settings(app, cache, settings.clone())?;
    Ok(settings)
}

pub fn save_settings(
    app: &AppHandle,
    cache: &Mutex<Option<Settings>>,
    settings: Settings,
) -> Result<()> {
    *cache.lock().unwrap() = Some(settings.clone());
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err_string()?;
    }
    let json = serde_json::to_string_pretty(&settings).map_err_string()?;
    std::fs::write(path, json).map_err_string()?;
    Ok(())
}

pub fn set_destination_dir(
    app: &AppHandle,
    cache: &Mutex<Option<Settings>>,
    dir: String,
) -> Result<Settings> {
    let settings = Settings {
        destination_dir: dir,
    };
    save_settings(app, cache, settings.clone())?;
    Ok(settings)
}
