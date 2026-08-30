//! Project-root identity, filesystem admission, and atomic mutation support.
//!
//! Project publication and resource revision validation remain owned by the
//! caller. This crate owns native root identity, root-scoped lease ordering,
//! lifecycle admission, transaction rollback, and recovery marking.

mod coordinator;
mod error;
mod lifecycle;
mod recovery;
mod root;
mod transaction;
#[cfg(windows)]
mod windows_path_identity;

pub use coordinator::{
    ProjectFilesystemCoordinator, ProjectFilesystemLeaseSet, ProjectRootLifecycleGuard,
};
pub use error::ProjectFilesystemError;
pub use lifecycle::{
    ProjectSourceTree, ensure_directory, read_project_source_tree, remove_directory_if_created,
    validate_deletion_root, validate_destination_policy,
};
pub use recovery::ProjectRecoveryMarker;
pub use root::{NormalizedProjectRoot, ProjectRootBinding, project_root_from_path};
pub use transaction::{
    CommittedFilesystemMutation, PreparedProjectFilesystemTransaction,
    ProjectFilesystemTransaction, ProjectFilesystemTransactionContext, StagedFilesystemMutation,
    metadata_is_redirect, read_secure_project_file,
};

#[cfg(any(test, feature = "test-support"))]
pub use root::{
    normalized_root_reconstruction_count_for_test,
    reset_normalized_root_reconstruction_count_for_test,
};
#[cfg(any(test, feature = "test-support"))]
pub use transaction::ProjectFilesystemFaultPoint;

#[cfg(test)]
mod tests;
