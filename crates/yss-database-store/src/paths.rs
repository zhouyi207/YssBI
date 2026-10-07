use std::path::{Component, Path};

use crate::DatasetStoreError;

/// Reject redirects before using catalog-owned paths, including destinations not yet created.
pub(crate) fn validate(root: &Path, path: &Path) -> Result<(), DatasetStoreError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| DatasetStoreError::InvalidIdentity)?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DatasetStoreError::InvalidIdentity);
    }
    let mut current = root.to_owned();
    check(&current)?;
    for component in relative.components() {
        current.push(component);
        check(&current)?;
    }
    Ok(())
}

fn check(path: &Path) -> Result<(), DatasetStoreError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(DatasetStoreError::InvalidIdentity);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(DatasetStoreError::InvalidIdentity);
        }
    }
    Ok(())
}

pub(crate) fn validate_catalog(root: &Path) -> Result<(), DatasetStoreError> {
    let name = yss_project_layout::PROJECT_DATASET_CATALOG_FILE;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        validate(root, &root.join(format!("{name}{suffix}")))?;
    }
    Ok(())
}
