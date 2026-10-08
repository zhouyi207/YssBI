#[cfg(any(test, feature = "test-support"))]
use super::FilesystemFaultPoint;
use super::journal::restore_before_images;
use super::mutation::apply_mutation;
use super::paths::validate_secure_path;
use super::{
    CommittedFilesystemMutation, PreparedFilesystemTransaction, StagedFilesystemMutation,
    mark_recovery,
};
use crate::FilesystemError;
use std::path::Path;

impl PreparedFilesystemTransaction {
    pub fn staging_root(&self) -> &Path {
        &self.transaction.workspace.staging_root
    }

    pub fn commit(mut self) -> Result<CommittedFilesystemMutation, FilesystemError> {
        let root = self
            .transaction
            .workspace
            .context
            .root
            .as_path()
            .to_path_buf();
        let prepared_root = self.transaction.workspace.staging_root.join("prepared");
        for (index, mutation) in self.transaction.mutations.iter().enumerate() {
            #[cfg(any(test, feature = "test-support"))]
            {
                let point = if index == 0 {
                    FilesystemFaultPoint::FirstLiveReplacement
                } else {
                    FilesystemFaultPoint::SecondLiveReplacement
                };
                if self.transaction.workspace.lease.take_fault(point) {
                    return self.commit_failed(
                        &root,
                        index,
                        format!(
                            "injected live replacement failure at mutation {}",
                            index + 1
                        ),
                        false,
                    );
                }
            }
            if matches!(
                mutation,
                StagedFilesystemMutation::RemoveFile { .. }
                    | StagedFilesystemMutation::MoveFile { .. }
                    | StagedFilesystemMutation::RemoveDirectoryIfEmpty { .. }
            ) {
                let validation_error = mutation
                    .relative_paths()
                    .find_map(|relative| validate_secure_path(&root, relative, true).err());
                if let Some(error) = validation_error {
                    return self.commit_failed(&root, index, error.to_string(), false);
                }
            }
            if let Err(error) = apply_mutation(
                &root,
                &prepared_root,
                mutation,
                index,
                &mut self.created_parent_directories,
                &self.transaction.workspace.lease,
            ) {
                let applied_count = index + usize::from(error.include_current_in_rollback);
                return self.commit_failed(
                    &root,
                    applied_count,
                    error.source.to_string(),
                    error.recovery_required,
                );
            }
        }

        if let Err(error) = self.transaction.workspace.cleanup() {
            let mutation_count = self.transaction.mutations.len();
            return self.commit_failed(&root, mutation_count, error.to_string(), false);
        }

        Ok(CommittedFilesystemMutation {
            workspace: self.transaction.workspace,
            journal: self.journal,
            created_parent_directories: self.created_parent_directories,
            armed: true,
        })
    }

    fn commit_failed(
        mut self,
        root: &Path,
        applied_count: usize,
        commit_message: String,
        apply_recovery_required: bool,
    ) -> Result<CommittedFilesystemMutation, FilesystemError> {
        let rollback_result = restore_before_images(
            root,
            &self.journal[..applied_count.min(self.journal.len())],
            &self.created_parent_directories,
            &self.transaction.workspace.lease,
        );
        let cleanup_result = self.transaction.workspace.cleanup();
        if let Err(rollback_error) = rollback_result {
            let error = FilesystemError::TransactionRollbackFailed {
                message: format!(
                    "{commit_message}; rollback failed: {rollback_error}{}",
                    cleanup_result
                        .err()
                        .map(|error| format!("; staging cleanup failed: {error}"))
                        .unwrap_or_default()
                ),
                recovery_required: true,
            };
            mark_recovery(&self.transaction.workspace.context.recovery_marker, &error);
            return Err(error);
        }
        if let Err(cleanup_error) = cleanup_result {
            let error = FilesystemError::TransactionRollbackFailed {
                message: format!(
                    "{commit_message}; rollback staging cleanup failed: {cleanup_error}"
                ),
                recovery_required: true,
            };
            mark_recovery(&self.transaction.workspace.context.recovery_marker, &error);
            return Err(error);
        }
        if apply_recovery_required {
            let error = FilesystemError::TransactionRollbackFailed {
                message: commit_message,
                recovery_required: true,
            };
            mark_recovery(&self.transaction.workspace.context.recovery_marker, &error);
            return Err(error);
        }
        Err(FilesystemError::TransactionCommitFailed {
            message: commit_message,
        })
    }
}

impl CommittedFilesystemMutation {
    pub fn finalize(mut self) {
        self.armed = false;
    }

    pub fn rollback(mut self) -> Result<(), FilesystemError> {
        self.armed = false;
        #[cfg(any(test, feature = "test-support"))]
        self.workspace.lease.run_rollback_hook();
        let rollback_result = restore_before_images(
            self.workspace.context.root.as_path(),
            &self.journal,
            &self.created_parent_directories,
            &self.workspace.lease,
        );
        let cleanup_result = self.workspace.cleanup();
        match (rollback_result, cleanup_result) {
            (Ok(()), Ok(())) => Ok(()),
            (rollback, cleanup) => {
                let error = FilesystemError::TransactionRollbackFailed {
                    message: format!(
                        "{}{}",
                        rollback
                            .err()
                            .map(|error| format!("restore failed: {error}"))
                            .unwrap_or_default(),
                        cleanup
                            .err()
                            .map(|error| format!("; staging cleanup failed: {error}"))
                            .unwrap_or_default()
                    ),
                    recovery_required: true,
                };
                mark_recovery(&self.workspace.context.recovery_marker, &error);
                Err(error)
            }
        }
    }
}

impl Drop for CommittedFilesystemMutation {
    fn drop(&mut self) {
        if self.armed {
            let rollback = restore_before_images(
                self.workspace.context.root.as_path(),
                &self.journal,
                &self.created_parent_directories,
                &self.workspace.lease,
            );
            let cleanup = self.workspace.cleanup();
            if rollback.is_err() || cleanup.is_err() {
                let rollback_error = FilesystemError::TransactionRollbackFailed {
                    message: format!(
                        "unwind rollback failed: {}{}",
                        rollback
                            .err()
                            .map(|error| error.to_string())
                            .unwrap_or_default(),
                        cleanup
                            .err()
                            .map(|error| format!("; staging cleanup failed: {error}"))
                            .unwrap_or_default()
                    ),
                    recovery_required: true,
                };
                mark_recovery(&self.workspace.context.recovery_marker, &rollback_error);
            }
            self.armed = false;
        }
    }
}
