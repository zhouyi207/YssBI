#[cfg(any(test, feature = "test-support"))]
use super::FilesystemFaultPoint;
use super::StagedFilesystemMutation;
use super::paths::{
    create_missing_directories, create_missing_parents, metadata_is_redirect, portable_path_key,
    validate_real_directory, validate_regular_file, validate_secure_path,
};
use crate::FilesystemLeaseSet;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(super) struct ReplacementFile {
    path: PathBuf,
    file: Option<std::fs::File>,
    armed: bool,
}

impl ReplacementFile {
    pub(super) fn create(live: &Path, index: usize) -> std::io::Result<Self> {
        let parent = live
            .parent()
            .ok_or_else(|| std::io::Error::other("replacement target has no parent"))?;
        validate_real_directory(parent)?;
        for _ in 0..32 {
            let path = parent.join(format!(
                ".{}.yssbi-replacement-{index}-{}",
                live.file_name().unwrap_or_default().to_string_lossy(),
                uuid::Uuid::new_v4()
            ));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        armed: true,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a collision-safe replacement file",
        ))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn file_mut(&mut self) -> &mut std::fs::File {
        self.file.as_mut().expect("replacement file is open")
    }

    pub(super) fn close(&mut self) {
        self.file.take();
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ReplacementFile {
    fn drop(&mut self) {
        self.file.take();
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(super) struct ApplyMutationError {
    pub(super) source: std::io::Error,
    pub(super) include_current_in_rollback: bool,
    pub(super) recovery_required: bool,
}

impl From<std::io::Error> for ApplyMutationError {
    fn from(source: std::io::Error) -> Self {
        Self {
            source,
            include_current_in_rollback: true,
            recovery_required: false,
        }
    }
}

struct NoReplaceMoveError {
    source: std::io::Error,
    recovery_required: bool,
}

pub(super) fn apply_mutation(
    root: &Path,
    prepared_root: &Path,
    mutation: &StagedFilesystemMutation,
    index: usize,
    created_parent_directories: &mut BTreeSet<PathBuf>,
    _lease: &FilesystemLeaseSet,
) -> Result<(), ApplyMutationError> {
    let relative = mutation
        .relative_paths()
        .next()
        .expect("filesystem mutation has at least one path");
    let live = root.join(relative);
    match mutation {
        StagedFilesystemMutation::Write { .. } | StagedFilesystemMutation::CopyFile { .. } => {
            create_missing_parents(root, &live, created_parent_directories)?;
            validate_secure_path(root, relative, true)?;
            validate_secure_path(prepared_root, relative, true)?;
            validate_regular_file(&prepared_root.join(relative))?;
            let mut temporary = ReplacementFile::create(&live, index)?;
            let mut staged = std::fs::File::open(prepared_root.join(relative))?;
            std::io::copy(&mut staged, temporary.file_mut())?;
            temporary.file_mut().sync_all()?;
            temporary.close();
            if live.is_dir() {
                std::fs::remove_dir(&live)?;
            } else if live.exists() {
                std::fs::remove_file(&live)?;
            }
            std::fs::rename(temporary.path(), &live)?;
            temporary.disarm();
            Ok(())
        }
        StagedFilesystemMutation::RemoveFile { .. } => {
            #[cfg(any(test, feature = "test-support"))]
            _lease.run_before_remove_hook();
            validate_secure_path(root, relative, true)?;
            let result = match std::fs::symlink_metadata(&live) {
                Ok(metadata) if metadata_is_redirect(&metadata) => {
                    Err(std::io::Error::other("remove-file target is a redirect"))
                }
                Ok(metadata) if metadata.is_file() => std::fs::remove_file(live),
                Ok(_) => Err(std::io::Error::other("remove-file target is not a file")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            };
            result.map_err(Into::into)
        }
        StagedFilesystemMutation::MoveFile { from, to } => {
            let source = root.join(from);
            let target = root.join(to);
            create_missing_parents(root, &target, created_parent_directories)?;
            validate_regular_file(&source)?;
            if from == to {
                return Ok(());
            }
            if portable_path_key(from) != portable_path_key(to) {
                #[cfg(any(test, feature = "test-support"))]
                _lease.run_before_remove_hook();
                return move_file_no_replace(&source, &target, _lease).map_err(|error| {
                    ApplyMutationError {
                        source: error.source,
                        include_current_in_rollback: false,
                        recovery_required: error.recovery_required,
                    }
                });
            }

            let parent = source
                .parent()
                .ok_or_else(|| std::io::Error::other("move source has no parent"))?;
            for _ in 0..32 {
                let temporary = parent.join(format!(
                    ".{}.yssbi-move-{}",
                    source.file_name().unwrap_or_default().to_string_lossy(),
                    uuid::Uuid::new_v4()
                ));
                match move_file_no_replace(&source, &temporary, _lease) {
                    Ok(()) => {}
                    Err(error)
                        if error.source.kind() == std::io::ErrorKind::AlreadyExists
                            && !error.recovery_required =>
                    {
                        continue;
                    }
                    Err(error) => {
                        return Err(ApplyMutationError {
                            source: error.source,
                            include_current_in_rollback: false,
                            recovery_required: error.recovery_required,
                        });
                    }
                }
                #[cfg(any(test, feature = "test-support"))]
                _lease.run_before_remove_hook();
                match move_file_no_replace(&temporary, &target, _lease) {
                    Ok(()) => return Ok(()),
                    Err(target_error) => {
                        #[cfg(any(test, feature = "test-support"))]
                        if _lease.take_fault(FilesystemFaultPoint::MoveRestoration) {
                            return Err(ApplyMutationError {
                                source: std::io::Error::other(format!(
                                    "case-only move target failed: {}; injected source restoration failure; source retained at '{}'",
                                    target_error.source,
                                    temporary.display()
                                )),
                                include_current_in_rollback: false,
                                recovery_required: true,
                            });
                        }
                        return match move_file_no_replace(&temporary, &source, _lease) {
                            Ok(()) => Err(ApplyMutationError {
                                source: target_error.source,
                                include_current_in_rollback: false,
                                recovery_required: target_error.recovery_required,
                            }),
                            Err(recovery_error) => Err(ApplyMutationError {
                                source: std::io::Error::other(format!(
                                    "case-only move target failed: {}; source restoration failed: {}; source retained at '{}'",
                                    target_error.source,
                                    recovery_error.source,
                                    temporary.display()
                                )),
                                include_current_in_rollback: false,
                                recovery_required: true,
                            }),
                        };
                    }
                }
            }
            Err(ApplyMutationError {
                source: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "could not allocate an internal case-only move path",
                ),
                include_current_in_rollback: false,
                recovery_required: false,
            })
        }
        StagedFilesystemMutation::CreateDirectory { .. } => {
            let result = if live.exists() {
                validate_real_directory(&live)
            } else {
                create_missing_directories(root, &live, created_parent_directories)
            };
            result.map_err(Into::into)
        }
        StagedFilesystemMutation::RemoveDirectoryIfEmpty { .. } => {
            #[cfg(any(test, feature = "test-support"))]
            _lease.run_before_remove_hook();
            validate_secure_path(root, relative, false)?;
            let result = match std::fs::symlink_metadata(&live) {
                Ok(metadata) if metadata_is_redirect(&metadata) => Err(std::io::Error::other(
                    "remove-directory target is a redirect",
                )),
                Ok(metadata) if metadata.is_dir() => std::fs::remove_dir(live),
                Ok(_) => Err(std::io::Error::other(
                    "remove-directory target is not a directory",
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            };
            result.map_err(Into::into)
        }
    }
}

fn move_file_no_replace(
    source: &Path,
    target: &Path,
    _lease: &FilesystemLeaseSet,
) -> Result<(), NoReplaceMoveError> {
    std::fs::hard_link(source, target).map_err(|source| NoReplaceMoveError {
        source,
        recovery_required: false,
    })?;

    #[cfg(any(test, feature = "test-support"))]
    let injected_cleanup_failure = _lease.take_fault(FilesystemFaultPoint::MoveTargetCleanup);
    #[cfg(not(any(test, feature = "test-support")))]
    let injected_cleanup_failure = false;
    #[cfg(any(test, feature = "test-support"))]
    let source_removal =
        if injected_cleanup_failure || _lease.take_fault(FilesystemFaultPoint::MoveSourceRemoval) {
            Err(std::io::Error::other(
                "injected move source removal failure",
            ))
        } else {
            std::fs::remove_file(source)
        };
    #[cfg(not(any(test, feature = "test-support")))]
    let source_removal = std::fs::remove_file(source);

    if let Err(source_error) = source_removal {
        #[cfg(any(test, feature = "test-support"))]
        _lease.run_before_move_target_delete_hook();
        let preservation_reason = if injected_cleanup_failure {
            "injected move target cleanup failure"
        } else {
            "move target ownership cannot be proven atomically; target retained for recovery"
        };
        return Err(NoReplaceMoveError {
            source: std::io::Error::other(format!(
                "move source removal failed: {source_error}; {preservation_reason}; target '{}' was not deleted",
                target.display()
            )),
            recovery_required: true,
        });
    }
    Ok(())
}
