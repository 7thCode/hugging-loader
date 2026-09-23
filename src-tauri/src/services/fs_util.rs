// Ports src/main/services/fsUtil.ts.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use futures_util::future::join_all;
use path_clean::PathClean;

use crate::error::Result;
use crate::ipc_types::CheckExistsResponse;

/// `filename` throughout the app is a Hugging Face rfilename: a path relative to the
/// destination folder that may include subfolders (e.g. "BF16/model-00001-of-00002.gguf").
/// The subfolder layout is preserved on disk so the shards of a split GGUF stay together
/// (llama.cpp needs them in one directory) and identically-named files from different
/// folders of a repo can't collide. It also comes from the renderer, so it must never be
/// able to point outside the destination folder (e.g. "../../.ssh/id_rsa").
///
/// Both `root` and the joined result are lexically `.clean()`ed (collapsing `.`/`..`
/// segments, no filesystem access — unlike `std::fs::canonicalize`, which requires the path
/// to already exist, this must work for files not downloaded yet). `Path::starts_with`
/// compares whole path components rather than raw bytes, so "/dest".starts_with-style
/// substring escapes (e.g. a sibling folder "/destbogus") are already excluded without the
/// manual separator-appending trick the original TS needed for its string-based check.
pub fn resolve_in_destination(destination_dir: &str, filename: &str) -> Result<PathBuf> {
    let root = Path::new(destination_dir).clean();
    let resolved = root.join(filename).clean();
    if !resolved.starts_with(&root) {
        return Err("保存先フォルダ外のパスは指定できません".to_string());
    }
    Ok(resolved)
}

/// Stats each candidate instead of listing the destination: files live in subfolders, and a
/// recursive walk of a large models folder would be far more work than one stat per file.
pub async fn find_existing_filenames(
    destination_dir: &str,
    filenames: &[String],
) -> HashSet<String> {
    let checks = filenames.iter().map(|filename| async move {
        let path = resolve_in_destination(destination_dir, filename).ok()?;
        let metadata = tokio::fs::metadata(&path).await.ok()?;
        metadata.is_file().then(|| filename.clone())
    });
    join_all(checks).await.into_iter().flatten().collect()
}

/// Unlike `find_existing_filenames`, path resolution failures here propagate as an `Err`
/// (matching checkExists.ts, where `resolveInDestination` is called outside the try/catch) —
/// so a traversal attempt surfaces as an error to the frontend rather than a quiet "not found".
pub async fn check_exists(destination_dir: &str, filename: &str) -> Result<CheckExistsResponse> {
    let file_path = resolve_in_destination(destination_dir, filename)?;
    let path_string = file_path.to_string_lossy().into_owned();
    let metadata = tokio::fs::metadata(&file_path).await.ok();
    let is_file = metadata.as_ref().is_some_and(|m| m.is_file());
    Ok(CheckExistsResponse {
        exists: is_file,
        size_bytes: if is_file {
            metadata.map(|m| m.len() as i64)
        } else {
            None
        },
        path: path_string,
    })
}

/// Unlike `check_exists`, a path resolution failure here is swallowed into `false` (matching
/// deleteFile.ts, where `resolveInDestination` is called inside the try/catch).
pub async fn delete_file(destination_dir: &str, filename: &str) -> bool {
    let Ok(file_path) = resolve_in_destination(destination_dir, filename) else {
        return false;
    };
    if tokio::fs::remove_file(&file_path).await.is_err() {
        return false;
    }
    if let Some(parent) = file_path.parent() {
        prune_empty_parents(destination_dir, parent).await;
    }
    true
}

/// Downloading into a subfolder creates it, so deleting the last file in it removes it again.
/// `remove_dir` refuses non-empty directories, so a folder still holding other shards, a
/// `.part`, or anything the user put there is left alone; the destination folder itself is
/// never removed (`current != root` below).
async fn prune_empty_parents(destination_dir: &str, dir: &Path) {
    let root = Path::new(destination_dir).clean();
    let mut current = dir.to_path_buf();
    while current != root && current.starts_with(&root) {
        if tokio::fs::remove_dir(&current).await.is_err() {
            return;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_a_plain_filename() {
        let resolved = resolve_in_destination("/dest", "model.gguf").unwrap();
        assert_eq!(resolved, PathBuf::from("/dest/model.gguf"));
    }

    #[test]
    fn allows_a_subfolder_path() {
        let resolved = resolve_in_destination("/dest", "BF16/model-00001-of-00002.gguf").unwrap();
        assert_eq!(
            resolved,
            PathBuf::from("/dest/BF16/model-00001-of-00002.gguf")
        );
    }

    #[test]
    fn rejects_parent_directory_traversal() {
        assert!(resolve_in_destination("/dest", "../outside.gguf").is_err());
        assert!(resolve_in_destination("/dest", "../../.ssh/id_rsa").is_err());
        assert!(resolve_in_destination("/dest", "BF16/../../outside.gguf").is_err());
    }

    #[test]
    fn rejects_an_absolute_path_escaping_the_destination() {
        assert!(resolve_in_destination("/dest", "/etc/passwd").is_err());
    }

    #[test]
    fn rejects_a_sibling_folder_that_merely_shares_a_string_prefix() {
        // "/destbogus/x" is NOT under "/dest" as a path component, even though it shares the
        // "/dest" string prefix — this is the substring-escape the original TS code's manual
        // `startsWith(root + path.sep)` check exists to guard against.
        assert!(resolve_in_destination("/dest", "../destbogus/x.gguf").is_err());
    }

    #[tokio::test]
    async fn check_exists_reports_existing_file_size() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("model.gguf"), b"hello")
            .await
            .unwrap();

        let result = check_exists(dir.path().to_str().unwrap(), "model.gguf")
            .await
            .unwrap();
        assert!(result.exists);
        assert_eq!(result.size_bytes, Some(5));
    }

    #[tokio::test]
    async fn check_exists_reports_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = check_exists(dir.path().to_str().unwrap(), "missing.gguf")
            .await
            .unwrap();
        assert!(!result.exists);
        assert_eq!(result.size_bytes, None);
    }

    #[tokio::test]
    async fn check_exists_propagates_a_traversal_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            check_exists(dir.path().to_str().unwrap(), "../outside.gguf")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn delete_file_removes_a_plain_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("model.gguf");
        tokio::fs::write(&file_path, b"x").await.unwrap();

        assert!(delete_file(dir.path().to_str().unwrap(), "model.gguf").await);
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn delete_file_returns_false_for_a_traversal_attempt() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!delete_file(dir.path().to_str().unwrap(), "../outside.gguf").await);
    }

    #[tokio::test]
    async fn delete_file_prunes_the_now_empty_subfolder_but_never_the_destination_root() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("BF16");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        let file_path = sub.join("model-00001-of-00002.gguf");
        tokio::fs::write(&file_path, b"x").await.unwrap();

        assert!(
            delete_file(
                dir.path().to_str().unwrap(),
                "BF16/model-00001-of-00002.gguf"
            )
            .await
        );
        assert!(!sub.exists());
        assert!(dir.path().exists());
    }

    #[tokio::test]
    async fn delete_file_leaves_a_subfolder_with_remaining_shards_intact() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("BF16");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        let shard1 = sub.join("model-00001-of-00002.gguf");
        let shard2 = sub.join("model-00002-of-00002.gguf");
        tokio::fs::write(&shard1, b"x").await.unwrap();
        tokio::fs::write(&shard2, b"y").await.unwrap();

        assert!(
            delete_file(
                dir.path().to_str().unwrap(),
                "BF16/model-00001-of-00002.gguf"
            )
            .await
        );
        assert!(sub.exists());
        assert!(shard2.exists());
    }
}
