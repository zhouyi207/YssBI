use super::mutation::ReplacementFile;
use super::paths::{
    create_missing_directories, create_missing_parents, metadata_is_redirect,
    validate_real_directory, validate_secure_path,
};
use super::{StagedFilesystemMutation, prepare_error};
use crate::{FilesystemError, FilesystemLeaseSet};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct MutationJournal {
    kind: MutationJournalKind,
    entries: Vec<JournalEntry>,
}

#[derive(Debug)]
enum MutationJournalKind {
    Generic,
    Move { from: PathBuf, to: PathBuf },
}

#[derive(Debug)]
struct JournalEntry {
    relative_path: PathBuf,
    before: BeforeImage,
}

#[derive(Debug)]
enum BeforeImage {
    Absent,
    File(Vec<u8>),
    Directory { children: BTreeSet<PathBuf> },
}

pub(super) fn capture_mutation_before_images(
    root: &Path,
    mutation: &StagedFilesystemMutation,
) -> Result<MutationJournal, FilesystemError> {
    let kind = match mutation {
        StagedFilesystemMutation::MoveFile { from, to } => MutationJournalKind::Move {
            from: from.clone(),
            to: to.clone(),
        },
        _ => MutationJournalKind::Generic,
    };
    let entries = mutation
        .relative_paths()
        .map(|path| capture_before_image(root, path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MutationJournal { kind, entries })
}

fn capture_before_image(
    root: &Path,
    relative_path: &Path,
) -> Result<JournalEntry, FilesystemError> {
    let path = root.join(relative_path);
    let before = match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            BeforeImage::File(std::fs::read(&path).map_err(prepare_error)?)
        }
        Ok(metadata) if metadata.is_dir() => {
            let children = std::fs::read_dir(&path)
                .map_err(prepare_error)?
                .map(|entry| {
                    entry
                        .map(|entry| PathBuf::from(entry.file_name()))
                        .map_err(prepare_error)
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            BeforeImage::Directory { children }
        }
        Ok(_) => {
            return Err(prepare_error(format!(
                "unsupported transaction target '{}'",
                relative_path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BeforeImage::Absent,
        Err(error) => return Err(prepare_error(error)),
    };
    Ok(JournalEntry {
        relative_path: relative_path.to_path_buf(),
        before,
    })
}

pub(super) fn restore_before_images(
    root: &Path,
    journal: &[MutationJournal],
    created_parent_directories: &BTreeSet<PathBuf>,
    _lease: &FilesystemLeaseSet,
) -> std::io::Result<()> {
    let move_recovery_copies = journal
        .iter()
        .enumerate()
        .filter_map(|(index, mutation)| match &mutation.kind {
            MutationJournalKind::Move { from, .. } => Some(
                move_source_contents(mutation, from)
                    .and_then(|contents| create_move_recovery_copy(root, contents, index))
                    .map(|path| (index, path)),
            ),
            MutationJournalKind::Generic => None,
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;

    #[cfg(any(test, feature = "test-support"))]
    if _lease.take_rollback_fault() {
        return Err(std::io::Error::other(
            "injected rollback restore failure after move recovery copies were retained",
        ));
    }

    let mut replacement_index = 0;
    let mut move_messages = Vec::new();
    for (mutation_index, mutation) in journal.iter().enumerate().rev() {
        match &mutation.kind {
            MutationJournalKind::Generic => {
                restore_journal_entries(root, &mutation.entries, &mut replacement_index)?;
            }
            MutationJournalKind::Move { from, to } => {
                let recovery_copy = move_recovery_copies
                    .get(&mutation_index)
                    .expect("every move journal has a retained recovery copy");
                let contents = move_source_contents(mutation, from)?;
                let source = root.join(from);
                validate_secure_path(root, from, true)?;
                let source_result = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&source)
                    .and_then(|mut file| {
                        file.write_all(contents)?;
                        file.sync_all()
                    });
                let source_status = match source_result {
                    Ok(()) => format!("source '{}' restored without replacement", from.display()),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => format!(
                        "source '{}' was already present and was not overwritten",
                        from.display()
                    ),
                    Err(error) => {
                        format!("source '{}' restoration failed: {error}", from.display())
                    }
                };
                move_messages.push(format!(
                    "{source_status}; move target '{}' retained because ownership cannot be proven atomically; original bytes retained at '{}'",
                    to.display(),
                    recovery_copy.display()
                ));
            }
        }
    }

    let protected_directories = move_target_directories(journal);
    for relative in created_parent_directories.iter().rev() {
        if protected_directories.contains(relative) {
            continue;
        }
        validate_secure_path(root, relative, false)?;
        let path = root.join(relative);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir(path)?,
            Ok(_) => return Err(std::io::Error::other("created parent is not a directory")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    if move_messages.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::other(move_messages.join("; ")))
    }
}

fn move_source_contents<'a>(
    mutation: &'a MutationJournal,
    from: &Path,
) -> std::io::Result<&'a [u8]> {
    mutation
        .entries
        .iter()
        .find_map(|entry| {
            if entry.relative_path != from {
                return None;
            }
            match &entry.before {
                BeforeImage::File(contents) => Some(contents.as_slice()),
                _ => None,
            }
        })
        .ok_or_else(|| std::io::Error::other("move journal has no source file before-image"))
}

fn create_move_recovery_copy(
    root: &Path,
    contents: &[u8],
    index: usize,
) -> std::io::Result<PathBuf> {
    validate_real_directory(root)?;
    for _ in 0..32 {
        let path = root.join(format!(
            ".yssbi-move-recovery-{index}-{}",
            uuid::Uuid::new_v4()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(contents)?;
                file.sync_all()?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a collision-safe move recovery file",
    ))
}

fn move_target_directories(journal: &[MutationJournal]) -> BTreeSet<PathBuf> {
    let mut protected = BTreeSet::new();
    for mutation in journal {
        let MutationJournalKind::Move { to, .. } = &mutation.kind else {
            continue;
        };
        let mut current = PathBuf::new();
        if let Some(parent) = to.parent() {
            for component in parent.components() {
                current.push(component.as_os_str());
                protected.insert(current.clone());
            }
        }
    }
    protected
}

fn restore_journal_entries(
    root: &Path,
    entries: &[JournalEntry],
    replacement_index: &mut usize,
) -> std::io::Result<()> {
    for entry in entries.iter().rev() {
        validate_secure_path(root, &entry.relative_path, true)?;
        let path = root.join(&entry.relative_path);
        match &entry.before {
            BeforeImage::Absent => remove_path_if_present(&path)?,
            BeforeImage::File(contents) => {
                remove_path_if_present(&path)?;
                let mut restored_parents = BTreeSet::new();
                create_missing_parents(root, &path, &mut restored_parents)?;
                let mut temporary = ReplacementFile::create(&path, *replacement_index)?;
                *replacement_index += 1;
                temporary.file_mut().write_all(contents)?;
                temporary.file_mut().sync_all()?;
                temporary.close();
                std::fs::rename(temporary.path(), &path)?;
                temporary.disarm();
            }
            BeforeImage::Directory { children } => {
                match std::fs::symlink_metadata(&path) {
                    Ok(metadata) if metadata.is_file() => std::fs::remove_file(&path)?,
                    Ok(metadata) if metadata.is_dir() => {}
                    Ok(_) => return Err(std::io::Error::other("rollback target is unsupported")),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        let mut restored_parents = BTreeSet::new();
                        create_missing_directories(root, &path, &mut restored_parents)?;
                    }
                    Err(error) => return Err(error),
                }
                validate_real_directory(&path)?;
                let current = std::fs::read_dir(&path)?
                    .map(|entry| entry.map(|entry| PathBuf::from(entry.file_name())))
                    .collect::<Result<BTreeSet<_>, _>>()?;
                if &current != children {
                    return Err(std::io::Error::other(format!(
                        "directory topology changed at '{}'",
                        entry.relative_path.display()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn remove_path_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata_is_redirect(&metadata) => Err(std::io::Error::other(format!(
            "rollback target '{}' is a redirect",
            path.display()
        ))),
        Ok(metadata) if metadata.is_file() => std::fs::remove_file(path),
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir(path),
        Ok(_) => Err(std::io::Error::other("rollback target is unsupported")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
