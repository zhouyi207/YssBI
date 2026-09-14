//! Project-specific path interpretation and manifest admission on top of filesystem primitives.
use crate::ProjectOperationError;
use std::path::{Path, PathBuf};
use yss_filesystem::{NormalizedRoot, metadata_is_redirect};
use yss_project_layout::PROJECT_METADATA_FILE;

fn trim_path(path: &Path) -> PathBuf {
    path.to_str()
        .map(|text| PathBuf::from(text.trim()))
        .unwrap_or_else(|| path.to_path_buf())
}

pub fn project_root_from_path(path: impl AsRef<Path>) -> PathBuf {
    let path = trim_path(path.as_ref());
    let is_metadata_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(PROJECT_METADATA_FILE));
    if path.is_file() || is_metadata_file {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or(path)
    } else {
        path
    }
}

pub(crate) fn validate_deletion_root(root: &NormalizedRoot) -> Result<(), ProjectOperationError> {
    let metadata = std::fs::symlink_metadata(root.as_path()).map_err(prepare_error)?;
    if metadata_is_redirect(&metadata) || !metadata.is_dir() {
        return Err(invalid_root(
            root.as_path(),
            "project root is not a real directory",
        ));
    }
    let manifest = root.as_path().join(PROJECT_METADATA_FILE);
    let metadata = std::fs::symlink_metadata(&manifest).map_err(prepare_error)?;
    if metadata_is_redirect(&metadata) || !metadata.is_file() {
        return Err(invalid_root(
            root.as_path(),
            "project manifest is not a regular file",
        ));
    }
    Ok(())
}

fn invalid_root(path: impl AsRef<Path>, message: impl Into<String>) -> ProjectOperationError {
    ProjectOperationError::InvalidRoot {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}
fn prepare_error(error: impl ToString) -> ProjectOperationError {
    ProjectOperationError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_entry_paths_resolve_to_directories_and_deletion_requires_a_manifest() {
        let root =
            std::env::temp_dir().join(format!("project-root-policy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let metadata = root.join(PROJECT_METADATA_FILE);
        std::fs::write(&metadata, "{}").unwrap();
        let normalized = NormalizedRoot::from_path(&root).unwrap();
        assert_eq!(
            NormalizedRoot::from_path(project_root_from_path(&metadata)).unwrap(),
            normalized
        );
        assert_eq!(
            NormalizedRoot::from_path(project_root_from_path(root.join("METADATA.YSSBI"))).unwrap(),
            normalized
        );
        validate_deletion_root(&normalized).unwrap();
        std::fs::remove_file(metadata).unwrap();
        assert!(validate_deletion_root(&normalized).is_err());
        assert!(yss_filesystem::RootBinding::for_existing(&root).is_ok());
        std::fs::remove_dir(&root).unwrap();
    }
}
