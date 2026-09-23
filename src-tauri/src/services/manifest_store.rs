// Ports the read-side of src/main/services/manifestStore.ts (`loadManifest`).
// `recordDownload`/`removeManifestEntry` land in a later phase, alongside the download and
// delete commands that need them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// A small metadata file written into the destination directory itself, so the download
// history travels with the models folder (survives app reinstall, is inspectable by hand)
// and disambiguates same-named files from different repos.
pub const MANIFEST_FILENAME: &str = ".hugging-loader-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub repo_id: String,
    pub quant: Option<String>,
    pub size_bytes: i64,
    pub param_count: Option<i64>,
    /// ISO timestamp.
    pub downloaded_at: String,
}

/// Keyed by rfilename, i.e. the path relative to the destination folder with '/' separators
/// ("BF16/model-00001-of-00002.gguf" for a file in a subfolder), on every platform.
pub type Manifest = HashMap<String, ManifestEntry>;

fn manifest_path(destination_dir: &str) -> PathBuf {
    Path::new(destination_dir).join(MANIFEST_FILENAME)
}

pub async fn load_manifest(destination_dir: &str) -> Manifest {
    let path = manifest_path(destination_dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => Manifest::default(),
    }
}
