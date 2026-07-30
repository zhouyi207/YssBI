use crate::project::{
    NormalizedProjectRoot, PROJECT_METADATA_FILE, ProjectFilesystemError, metadata_is_redirect,
    read_secure_project_file,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) struct ProjectSourceTree {
    pub directories: BTreeSet<PathBuf>,
    pub files: BTreeMap<PathBuf, Vec<u8>>,
}

pub(crate) fn ensure_directory(root: &Path) -> Result<bool, ProjectFilesystemError> {
    let created = !root.exists();
    if created {
        std::fs::create_dir_all(root).map_err(prepare_error)?;
    }
    Ok(created)
}

pub(crate) fn remove_directory_if_created(root: &Path, created: bool) {
    if created {
        let _ = std::fs::remove_dir_all(root);
    }
}

pub(crate) fn rename_project_root(
    source: &Path,
    destination: &Path,
) -> Result<(), ProjectFilesystemError> {
    std::fs::rename(source, destination).map_err(|error| {
        ProjectFilesystemError::TransactionCommitFailed {
            message: format!("failed to atomically tombstone project root: {error}"),
        }
    })
}

pub(crate) fn validate_destination_policy(root: &Path) -> Result<(), ProjectFilesystemError> {
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

pub(crate) fn validate_deletion_root(
    root: &NormalizedProjectRoot,
) -> Result<(), ProjectFilesystemError> {
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

pub(crate) fn read_project_source_tree(
    source_root: &Path,
) -> Result<ProjectSourceTree, ProjectFilesystemError> {
    let mut tree = ProjectSourceTree {
        directories: BTreeSet::new(),
        files: BTreeMap::new(),
    };
    collect_source_files(source_root, source_root, &mut tree)?;
    Ok(tree)
}

fn collect_source_files(
    source_root: &Path,
    directory: &Path,
    tree: &mut ProjectSourceTree,
) -> Result<(), ProjectFilesystemError> {
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
                "project copy source '{}' is a redirect",
                relative.display()
            )));
        }
        if metadata.is_dir() {
            tree.directories.insert(relative);
            collect_source_files(source_root, &entry.path(), tree)?;
        } else if metadata.is_file() {
            let contents =
                read_secure_project_file(source_root, &relative).map_err(prepare_error)?;
            tree.files.insert(relative, contents);
        } else {
            return Err(prepare_error(format!(
                "project copy source '{}' is not a regular file or directory",
                relative.display()
            )));
        }
    }
    Ok(())
}

fn invalid_root(path: impl AsRef<Path>, message: impl Into<String>) -> ProjectFilesystemError {
    ProjectFilesystemError::InvalidRoot {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}

fn prepare_error(error: impl ToString) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}
