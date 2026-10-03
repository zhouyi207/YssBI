use super::{StagedFilesystemMutation, TRANSACTION_DIRECTORY, prepare_error};
use crate::FilesystemError;
use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug)]
enum PortablePathOwner {
    Write { spelling: PathBuf },
    RemoveFile,
    Exclusive,
    RewritePairComplete,
}

#[derive(Clone, Copy)]
enum PortablePathClaim {
    Write,
    RemoveFile,
    Exclusive,
}

pub(super) fn validate_mutation_paths(
    mutations: &[StagedFilesystemMutation],
) -> Result<(), FilesystemError> {
    let mut owners = HashMap::new();
    for mutation in mutations {
        let paths = mutation.relative_paths();
        for relative in &paths {
            let valid = !relative.as_os_str().is_empty()
                && !relative.is_absolute()
                && relative.components().all(|component| {
                    matches!(component, Component::Normal(_) | Component::CurDir)
                })
                && relative.components().next().is_some_and(|component| {
                    !matches!(component, Component::Normal(name) if name == TRANSACTION_DIRECTORY)
                });
            if !valid {
                return Err(prepare_error(format!(
                    "invalid transaction target '{}'",
                    relative.display()
                )));
            }
        }

        match mutation {
            StagedFilesystemMutation::Write { relative_path, .. }
            | StagedFilesystemMutation::CopyFile { relative_path, .. } => {
                register_portable_path(&mut owners, relative_path, PortablePathClaim::Write)?
            }
            StagedFilesystemMutation::RemoveFile { relative_path } => {
                register_portable_path(&mut owners, relative_path, PortablePathClaim::RemoveFile)?
            }
            StagedFilesystemMutation::MoveFile { from, to }
                if portable_path_key(from) == portable_path_key(to) =>
            {
                register_portable_path(&mut owners, from, PortablePathClaim::Exclusive)?;
            }
            StagedFilesystemMutation::MoveFile { from, to } => {
                register_portable_path(&mut owners, from, PortablePathClaim::Exclusive)?;
                register_portable_path(&mut owners, to, PortablePathClaim::Exclusive)?;
            }
            StagedFilesystemMutation::CreateDirectory { relative_path }
            | StagedFilesystemMutation::RemoveDirectoryIfEmpty { relative_path } => {
                register_portable_path(&mut owners, relative_path, PortablePathClaim::Exclusive)?;
            }
        }
    }
    Ok(())
}

fn register_portable_path(
    owners: &mut HashMap<String, PortablePathOwner>,
    relative: &Path,
    claim: PortablePathClaim,
) -> Result<(), FilesystemError> {
    let key = portable_path_key(relative);
    let spelling = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect::<PathBuf>();
    match owners.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(match claim {
                PortablePathClaim::Write => PortablePathOwner::Write { spelling },
                PortablePathClaim::RemoveFile => PortablePathOwner::RemoveFile,
                PortablePathClaim::Exclusive => PortablePathOwner::Exclusive,
            });
            Ok(())
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if matches!(claim, PortablePathClaim::RemoveFile)
                && matches!(
                    entry.get(),
                    PortablePathOwner::Write {
                        spelling: write_spelling
                    } if write_spelling != &spelling
                )
            {
                entry.insert(PortablePathOwner::RewritePairComplete);
                return Ok(());
            }
            Err(prepare_error(format!(
                "duplicate portable path '{}' in filesystem transaction",
                relative.display()
            )))
        }
    }
}

pub fn metadata_is_redirect(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

pub(super) fn validate_real_directory(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata_is_redirect(&metadata) || !metadata.is_dir() {
        return Err(std::io::Error::other(format!(
            "path '{}' is not a real directory",
            path.display()
        )));
    }
    Ok(())
}

pub(super) fn validate_regular_file(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata_is_redirect(&metadata) || !metadata.is_file() {
        return Err(std::io::Error::other(format!(
            "path '{}' is not a regular file",
            path.display()
        )));
    }
    Ok(())
}

pub fn read_secure_file(root: &Path, relative: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(validate_copy_source(root, relative)?)
}

pub(super) fn validate_copy_source(root: &Path, relative: &Path) -> std::io::Result<PathBuf> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(std::io::Error::other(format!(
            "source '{}' is not a safe relative path",
            relative.display()
        )));
    }
    validate_secure_path(root, relative, true)?;
    let source = root.join(relative);
    validate_regular_file(&source)?;
    Ok(source)
}

pub(super) fn validate_secure_path(
    root: &Path,
    relative: &Path,
    allow_final_non_directory: bool,
) -> std::io::Result<()> {
    validate_real_directory(root)?;
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata_is_redirect(&metadata) {
                    return Err(std::io::Error::other(format!(
                        "path '{}' traverses a redirect",
                        current.display()
                    )));
                }
                let final_component = index + 1 == components.len();
                if (!final_component || !allow_final_non_directory) && !metadata.is_dir() {
                    return Err(std::io::Error::other(format!(
                        "path ancestor '{}' is not a directory",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn create_secure_directories(root: &Path, directory: &Path) -> std::io::Result<()> {
    let relative = directory
        .strip_prefix(root)
        .map_err(|_| std::io::Error::other("directory escapes filesystem root"))?;
    let mut ignored = BTreeSet::new();
    create_missing_directories(root, &root.join(relative), &mut ignored)
}

pub(super) fn validate_no_redirect_tree(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata_is_redirect(&metadata) {
        return Err(std::io::Error::other(format!(
            "cleanup path '{}' is a redirect",
            path.display()
        )));
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            validate_no_redirect_tree(&entry?.path())?;
        }
    }
    Ok(())
}

pub(super) fn portable_path_key(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
        .case_fold()
        .nfc()
        .collect()
}

pub(super) fn validate_move_preconditions(
    root: &Path,
    mutation: &StagedFilesystemMutation,
) -> Result<(), FilesystemError> {
    let StagedFilesystemMutation::MoveFile { from, to } = mutation else {
        return Ok(());
    };
    validate_regular_file(&root.join(from)).map_err(prepare_error)?;
    let target_parent = to.parent().unwrap_or_else(|| Path::new(""));
    let target_name = to
        .file_name()
        .ok_or_else(|| prepare_error(format!("move target '{}' has no file name", to.display())))?;
    let source_key = portable_path_key(from);
    let target_key = portable_path_key(to);
    let directory = root.join(target_parent);
    if directory.exists() {
        for entry in std::fs::read_dir(&directory).map_err(prepare_error)? {
            let entry = entry.map_err(prepare_error)?;
            let candidate = target_parent.join(entry.file_name());
            if entry
                .file_name()
                .to_string_lossy()
                .case_fold()
                .nfc()
                .collect::<String>()
                != target_name
                    .to_string_lossy()
                    .case_fold()
                    .nfc()
                    .collect::<String>()
            {
                continue;
            }
            if candidate == *from && source_key == target_key {
                continue;
            }
            return Err(prepare_error(format!(
                "move target '{}' has an existing portable conflict at '{}'",
                to.display(),
                candidate.display()
            )));
        }
    }
    Ok(())
}

pub(super) fn create_missing_parents(
    root: &Path,
    path: &Path,
    created: &mut BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    create_missing_directories(root, parent, created)
}

pub(super) fn create_missing_directories(
    root: &Path,
    directory: &Path,
    created: &mut BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    let relative = directory.strip_prefix(root).unwrap_or(directory);
    let mut current = root.to_path_buf();
    validate_real_directory(root)?;
    for component in relative.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(_) => validate_real_directory(&current)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current)?;
                validate_real_directory(&current)?;
                created.insert(current.strip_prefix(root).unwrap().to_path_buf());
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
