//! File access, root-scoped transactions, filesystem changes and watcher lifecycles.

pub mod change;
mod coordinator;
mod error;
mod identity;
mod lifecycle;
mod recovery;
mod root;
mod transaction;
pub mod watcher;
#[cfg(windows)]
mod windows_path_identity;

pub use coordinator::{FilesystemCoordinator, FilesystemLeaseSet, RootLifecycleGuard};
pub use error::FilesystemError;
pub use lifecycle::{
    FileInventory, SourceTree, ensure_directory, read_file_inventory, read_source_tree,
    remove_directory_if_created, validate_destination_policy,
};
pub use recovery::RecoveryMarker;
pub use root::{NormalizedRoot, RootBinding};
pub use transaction::{
    CommittedFilesystemMutation, FilesystemTransaction, PreparedFilesystemTransaction,
    StagedFilesystemMutation, TransactionContext, metadata_is_redirect, read_secure_file,
};

#[cfg(any(test, feature = "test-support"))]
pub use root::{
    normalized_root_reconstruction_count_for_test,
    reset_normalized_root_reconstruction_count_for_test,
};
#[cfg(any(test, feature = "test-support"))]
pub use transaction::FilesystemFaultPoint;

#[cfg(test)]
mod tests;

pub use change::{FileChange, FileChangeKind, FilesystemChange, RelativePath, RelativePathError};
pub use identity::{RootIdentity, TransactionId};
