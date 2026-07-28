mod coordinator;
mod root;
mod transaction;
#[cfg(windows)]
mod windows_path_identity;

pub use crate::project::project_error::ProjectFilesystemError;
pub use coordinator::{ProjectFilesystemCoordinator, ProjectFilesystemLeaseSet};
pub use root::NormalizedProjectRoot;
pub use transaction::{
    CommittedFilesystemMutation, PreparedProjectFilesystemTransaction,
    ProjectFilesystemTransaction, StagedFilesystemMutation,
};

#[cfg(test)]
pub use transaction::{
    ProjectFilesystemFaultPoint, set_before_remove_mutation_hook, set_project_filesystem_fault,
    set_project_filesystem_rollback_fault,
};

#[cfg(test)]
mod tests;
