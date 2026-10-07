#[cfg(any(test, feature = "test-support"))]
use super::FilesystemFaultPoint;
use super::paths::{metadata_is_redirect, validate_no_redirect_tree, validate_secure_path};
use super::{TRANSACTION_DIRECTORY, TransactionContext, mark_recovery};
use crate::{FilesystemError, FilesystemLeaseSet};
use std::path::{Path, PathBuf};

pub(super) struct TransactionWorkspace {
    pub(super) context: TransactionContext,
    pub(super) lease: FilesystemLeaseSet,
    pub(super) staging_root: PathBuf,
    cleanup_on_drop: bool,
}

impl TransactionWorkspace {
    pub(super) fn new(context: TransactionContext, lease: FilesystemLeaseSet) -> Self {
        let staging_root = context
            .root
            .as_path()
            .join(TRANSACTION_DIRECTORY)
            .join(context.transaction_id.to_string());
        Self {
            context,
            lease,
            staging_root,
            cleanup_on_drop: true,
        }
    }

    pub(super) fn cleanup(&mut self) -> std::io::Result<()> {
        // Explicit commit/rollback paths report failures themselves. Only an
        // abandoned workspace needs Drop to perform and report cleanup.
        self.cleanup_on_drop = false;
        let root = self.context.root.as_path();
        let relative = self
            .staging_root
            .strip_prefix(root)
            .map_err(|_| std::io::Error::other("staging directory escapes filesystem root"))?;
        validate_secure_path(root, relative, false)?;
        cleanup_staging(&self.staging_root, &self.lease)
    }
}

impl Drop for TransactionWorkspace {
    fn drop(&mut self) {
        if self.cleanup_on_drop
            && let Err(cause) = self.cleanup()
        {
            let error = FilesystemError::TransactionRollbackFailed {
                message: format!("abandoned transaction staging cleanup failed: {cause}"),
                recovery_required: true,
            };
            mark_recovery(&self.context.recovery_marker, &error);
        }
        // The lease is released only after cleanup and recovery publication.
    }
}

fn cleanup_staging(staging_root: &Path, _lease: &FilesystemLeaseSet) -> std::io::Result<()> {
    #[cfg(any(test, feature = "test-support"))]
    if _lease.take_fault(FilesystemFaultPoint::StagingCleanup) {
        return Err(std::io::Error::other("injected staging cleanup failure"));
    }
    if staging_root.exists() {
        validate_no_redirect_tree(staging_root)?;
        std::fs::remove_dir_all(staging_root)?;
    }
    if let Some(parent) = staging_root.parent() {
        match std::fs::symlink_metadata(parent) {
            Ok(metadata) if metadata_is_redirect(&metadata) => {
                return Err(std::io::Error::other(format!(
                    "staging parent '{}' is a redirect",
                    parent.display()
                )));
            }
            Ok(metadata) if metadata.is_dir() && std::fs::read_dir(parent)?.next().is_none() => {
                std::fs::remove_dir(parent)?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
