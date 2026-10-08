use crate::{FilesystemError, RootBinding, metadata_is_redirect};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct FileInventory {
    pub directories: BTreeSet<PathBuf>,
    pub files: BTreeSet<PathBuf>,
}

pub fn ensure_directory(root: &Path) -> Result<bool, FilesystemError> {
    let created = !root.exists();
    if created {
        std::fs::create_dir_all(root).map_err(prepare_error)?;
    }
    Ok(created)
}

pub fn remove_directory_if_created(binding: &RootBinding, created: bool) {
    if created && binding.revalidate().is_ok() {
        let _ = std::fs::remove_dir_all(binding.normalized().as_path());
    }
}

pub fn validate_destination_policy(root: &Path) -> Result<(), FilesystemError> {
    if !root.exists() {
        let parent = root
            .parent()
            .ok_or_else(|| invalid_root(root, "destination has no parent"))?;
        let metadata = std::fs::symlink_metadata(parent).map_err(prepare_error)?;
        if metadata_is_redirect(&metadata) || !metadata.is_dir() {
            return Err(invalid_root(
                root,
                "destination parent is not a real directory",
            ));
        }
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(root).map_err(prepare_error)?;
    if metadata_is_redirect(&metadata) || !metadata.is_dir() {
        return Err(invalid_root(root, "destination is not a real directory"));
    }
    let mut entries = std::fs::read_dir(root).map_err(prepare_error)?;
    if entries.next().transpose().map_err(prepare_error)?.is_some() {
        return Err(invalid_root(root, "destination directory must be empty"));
    }
    Ok(())
}

pub fn read_file_inventory(source_root: &Path) -> Result<FileInventory, FilesystemError> {
    let binding = RootBinding::for_existing(source_root)?;
    let mut tree = FileInventory {
        directories: BTreeSet::new(),
        files: BTreeSet::new(),
    };
    collect_source_files(source_root, source_root, &mut tree)?;
    binding.revalidate()?;
    Ok(tree)
}

fn collect_source_files(
    source_root: &Path,
    directory: &Path,
    tree: &mut FileInventory,
) -> Result<(), FilesystemError> {
    for entry in std::fs::read_dir(directory).map_err(prepare_error)? {
        let entry = entry.map_err(prepare_error)?;
        let relative = entry
            .path()
            .strip_prefix(source_root)
            .map_err(prepare_error)?
            .to_path_buf();
        if relative.starts_with(".yssbi-transaction") {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(prepare_error)?;
        if metadata_is_redirect(&metadata) {
            return Err(prepare_error(format!(
                "copy source '{}' is a redirect",
                relative.display()
            )));
        }
        if metadata.is_dir() {
            tree.directories.insert(relative);
            collect_source_files(source_root, &entry.path(), tree)?;
        } else if metadata.is_file() {
            tree.files.insert(relative);
        } else {
            return Err(prepare_error(format!(
                "copy source '{}' is not a regular file or directory",
                relative.display()
            )));
        }
    }
    Ok(())
}

fn invalid_root(path: impl AsRef<Path>, message: impl Into<String>) -> FilesystemError {
    FilesystemError::InvalidRoot {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}

fn prepare_error(error: impl ToString) -> FilesystemError {
    FilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}
