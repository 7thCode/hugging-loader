// Ports the read-side of src/main/services/fsUtil.ts (`resolveInDestination`,
// `findExistingFilenames`). `checkExists`/`deleteFile` land in a later phase, once the
// fs_check_exists/fs_delete_file commands that need them are wired up.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use futures_util::future::join_all;
use path_clean::PathClean;

use crate::error::Result;

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
}
