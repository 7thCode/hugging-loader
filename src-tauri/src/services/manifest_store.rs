// Ports src/main/services/manifestStore.ts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MapErrString, Result};

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

async fn save_manifest(destination_dir: &str, manifest: &Manifest) -> Result<()> {
    let path = manifest_path(destination_dir);
    let json = serde_json::to_string_pretty(manifest).map_err_string()?;
    tokio::fs::write(&path, json).await.map_err_string()?;
    Ok(())
}

// record_download() (recordDownload.ts) is added in Phase 6, alongside download_manager.rs,
// the only caller it has.

pub async fn remove_manifest_entry(destination_dir: &str, filename: &str) -> Result<()> {
    let mut manifest = load_manifest(destination_dir).await;
    if manifest.remove(filename).is_some() {
        save_manifest(destination_dir, &manifest).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(repo_id: &str) -> ManifestEntry {
        ManifestEntry {
            repo_id: repo_id.to_string(),
            quant: Some("Q4_K_M".to_string()),
            size_bytes: 123,
            param_count: Some(8_000_000_000),
            downloaded_at: "2026-09-23T00:00:00.000Z".to_string(),
        }
    }

    #[tokio::test]
    async fn load_manifest_returns_empty_when_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_manifest(dir.path().to_str().unwrap()).await.is_empty());
    }

    #[tokio::test]
    async fn load_manifest_returns_empty_for_corrupt_json() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join(MANIFEST_FILENAME), b"not json")
            .await
            .unwrap();
        assert!(load_manifest(dir.path().to_str().unwrap()).await.is_empty());
    }

    #[tokio::test]
    async fn remove_manifest_entry_deletes_only_the_matching_key_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().to_str().unwrap();
        let mut manifest = Manifest::new();
        manifest.insert("a.gguf".to_string(), sample_entry("repo/a"));
        manifest.insert("b.gguf".to_string(), sample_entry("repo/b"));
        save_manifest(dest, &manifest).await.unwrap();

        remove_manifest_entry(dest, "a.gguf").await.unwrap();

        let reloaded = load_manifest(dest).await;
        assert!(!reloaded.contains_key("a.gguf"));
        assert!(reloaded.contains_key("b.gguf"));
    }

    #[tokio::test]
    async fn remove_manifest_entry_is_a_no_op_when_the_key_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().to_str().unwrap();
        let mut manifest = Manifest::new();
        manifest.insert("a.gguf".to_string(), sample_entry("repo/a"));
        save_manifest(dest, &manifest).await.unwrap();

        remove_manifest_entry(dest, "missing.gguf").await.unwrap();

        assert_eq!(load_manifest(dest).await.len(), 1);
    }
}
