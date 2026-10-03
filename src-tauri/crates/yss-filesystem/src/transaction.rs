use crate::{FilesystemError, NormalizedRoot, RecoveryMarker, TransactionId};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

mod commit;
mod journal;
mod mutation;
mod paths;
mod prepare;
mod workspace;

use journal::MutationJournal;
pub use paths::{metadata_is_redirect, read_secure_file};
use workspace::TransactionWorkspace;

const TRANSACTION_DIRECTORY: &str = ".yssbi-transaction";

#[derive(Clone, Debug)]
pub struct TransactionContext {
    pub root: NormalizedRoot,
    pub transaction_id: TransactionId,
    pub recovery_marker: Option<RecoveryMarker>,
}

#[derive(Clone, Debug)]
pub enum StagedFilesystemMutation {
    /// Copy a new file under a leased source root without retaining its contents in memory.
    CopyFile {
        relative_path: PathBuf,
        source_root: NormalizedRoot,
        source_relative_path: PathBuf,
    },
    Write {
        relative_path: PathBuf,
        contents: Vec<u8>,
    },
    RemoveFile {
        relative_path: PathBuf,
    },
    MoveFile {
        from: PathBuf,
        to: PathBuf,
    },
    CreateDirectory {
        relative_path: PathBuf,
    },
    RemoveDirectoryIfEmpty {
        relative_path: PathBuf,
    },
}

impl StagedFilesystemMutation {
    fn relative_paths(&self) -> Vec<&Path> {
        match self {
            Self::Write { relative_path, .. }
            | Self::CopyFile { relative_path, .. }
            | Self::RemoveFile { relative_path }
            | Self::CreateDirectory { relative_path }
            | Self::RemoveDirectoryIfEmpty { relative_path } => vec![relative_path],
            Self::MoveFile { from, to } => vec![from, to],
        }
    }
}

pub struct FilesystemTransaction {
    workspace: TransactionWorkspace,
    mutations: Vec<StagedFilesystemMutation>,
}

pub struct PreparedFilesystemTransaction {
    transaction: FilesystemTransaction,
    journal: Vec<MutationJournal>,
    created_parent_directories: BTreeSet<PathBuf>,
}

impl std::fmt::Debug for PreparedFilesystemTransaction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedFilesystemTransaction")
            .field("context", &self.transaction.workspace.context)
            .field("staging_root", &self.transaction.workspace.staging_root)
            .field("mutations", &self.transaction.mutations)
            .finish_non_exhaustive()
    }
}

pub struct CommittedFilesystemMutation {
    workspace: TransactionWorkspace,
    journal: Vec<MutationJournal>,
    created_parent_directories: BTreeSet<PathBuf>,
    armed: bool,
}

impl std::fmt::Debug for CommittedFilesystemMutation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommittedFilesystemMutation")
            .field("root", &self.workspace.context.root)
            .field("staging_root", &self.workspace.staging_root)
            .field("journal", &self.journal)
            .finish_non_exhaustive()
    }
}

fn mark_recovery(marker: &Option<RecoveryMarker>, error: &FilesystemError) {
    if let Some(marker) = marker {
        marker.mark(error.to_string());
    }
}

fn prepare_error(error: impl std::fmt::Display) -> FilesystemError {
    FilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesystemFaultPoint {
    StagedSerialization,
    FirstLiveReplacement,
    SecondLiveReplacement,
    MoveSourceRemoval,
    MoveTargetCleanup,
    MoveRestoration,
    StagingCleanup,
}
