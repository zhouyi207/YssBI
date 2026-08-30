mod coordinator;
mod lifecycle_io;
mod root;
mod transaction;
#[cfg(windows)]
mod windows_path_identity;

pub use crate::project::project_error::ProjectFilesystemError;
pub use coordinator::{
    ProjectFilesystemCoordinator, ProjectFilesystemLeaseSet, ProjectRootLifecycleGuard,
};
pub(crate) use lifecycle_io::{
    ensure_directory, read_project_source_tree, remove_directory_if_created,
    validate_deletion_root, validate_destination_policy,
};
pub use root::{NormalizedProjectRoot, ProjectRootBinding};
pub use transaction::{
    CommittedFilesystemMutation, PreparedProjectFilesystemTransaction,
    ProjectFilesystemTransaction, StagedFilesystemMutation,
};
pub(crate) use transaction::{metadata_is_redirect, read_secure_project_file};

#[cfg(test)]
pub(crate) use root::{
    normalized_root_reconstruction_count_for_test,
    reset_normalized_root_reconstruction_count_for_test,
};

#[cfg(test)]
pub use transaction::ProjectFilesystemFaultPoint;

#[cfg(test)]
mod tests;
