#[cfg(any(test, feature = "test-support"))]
use super::FilesystemFaultPoint;
use super::journal::capture_mutation_before_images;
use super::paths::{
    create_secure_directories, validate_copy_source, validate_move_preconditions,
    validate_mutation_paths, validate_real_directory, validate_regular_file, validate_secure_path,
};
use super::workspace::TransactionWorkspace;
use super::{
    FilesystemTransaction, PreparedFilesystemTransaction, StagedFilesystemMutation,
    TransactionContext, prepare_error,
};
use crate::{FilesystemError, FilesystemLeaseSet};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

impl FilesystemTransaction {
    pub fn prepare(
        context: TransactionContext,
        lease: FilesystemLeaseSet,
        mutations: Vec<StagedFilesystemMutation>,
    ) -> Result<PreparedFilesystemTransaction, FilesystemError> {
        Self::prepare_with_file_validator(context, lease, mutations, |_, _| Ok(()))
    }

    pub fn prepare_with_validator(
        context: TransactionContext,
        lease: FilesystemLeaseSet,
        mutations: Vec<StagedFilesystemMutation>,
        mut validator: impl FnMut(&Path, &[u8]) -> Result<(), String>,
    ) -> Result<PreparedFilesystemTransaction, FilesystemError> {
        Self::prepare_with_file_validator(context, lease, mutations, |relative, staged| {
            let contents = std::fs::read(staged).map_err(|error| error.to_string())?;
            validator(relative, &contents)
        })
    }

    pub fn prepare_with_file_validator(
        context: TransactionContext,
        lease: FilesystemLeaseSet,
        mutations: Vec<StagedFilesystemMutation>,
        validator: impl FnMut(&Path, &Path) -> Result<(), String>,
    ) -> Result<PreparedFilesystemTransaction, FilesystemError> {
        if !lease.contains(&context.root) {
            return Err(FilesystemError::TransactionPrepareFailed {
                message: "transaction lease does not own the filesystem root".into(),
            });
        }
        validate_mutation_paths(&mutations)?;
        let root = context.root.as_path().to_path_buf();
        validate_real_directory(&root).map_err(prepare_error)?;
        for mutation in &mutations {
            if let StagedFilesystemMutation::CopyFile {
                relative_path,
                source_root,
                source_relative_path,
            } = mutation
            {
                if !lease.contains(source_root) || root.join(relative_path).exists() {
                    return Err(prepare_error(
                        "copy requires a leased source and an absent destination",
                    ));
                }
                validate_copy_source(source_root.as_path(), source_relative_path)
                    .map_err(prepare_error)?;
            }
            for relative_path in mutation.relative_paths() {
                validate_secure_path(&root, relative_path, true).map_err(prepare_error)?;
            }
            validate_move_preconditions(&root, mutation)?;
        }
        let transaction = Self {
            workspace: TransactionWorkspace::new(context, lease),
            mutations,
        };
        transaction.stage_files(validator)?;
        let journal = transaction
            .mutations
            .iter()
            .map(|mutation| capture_mutation_before_images(&root, mutation))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedFilesystemTransaction {
            transaction,
            journal,
            created_parent_directories: BTreeSet::new(),
        })
    }

    fn stage_files(
        &self,
        mut validator: impl FnMut(&Path, &Path) -> Result<(), String>,
    ) -> Result<(), FilesystemError> {
        let root = self.workspace.context.root.as_path();
        let prepared_root = self.workspace.staging_root.join("prepared");
        create_secure_directories(root, &prepared_root).map_err(prepare_error)?;
        for mutation in &self.mutations {
            let relative_path = match mutation {
                StagedFilesystemMutation::Write { relative_path, .. }
                | StagedFilesystemMutation::CopyFile { relative_path, .. } => relative_path,
                _ => continue,
            };
            #[cfg(any(test, feature = "test-support"))]
            if self
                .workspace
                .lease
                .take_fault(FilesystemFaultPoint::StagedSerialization)
            {
                return Err(prepare_error("injected staged serialization failure"));
            }
            let staged_path = prepared_root.join(relative_path);
            if let Some(parent) = staged_path.parent() {
                create_secure_directories(root, parent).map_err(prepare_error)?;
            }
            let mut staged = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged_path)
                .map_err(prepare_error)?;
            match mutation {
                StagedFilesystemMutation::Write { contents, .. } => {
                    staged.write_all(contents).map_err(prepare_error)?
                }
                StagedFilesystemMutation::CopyFile {
                    source_root,
                    source_relative_path,
                    ..
                } => {
                    let source = validate_copy_source(source_root.as_path(), source_relative_path)
                        .map_err(prepare_error)?;
                    let mut source_file = std::fs::File::open(source).map_err(prepare_error)?;
                    let before = source_file.metadata().map_err(prepare_error)?;
                    let copied =
                        std::io::copy(&mut source_file, &mut staged).map_err(prepare_error)?;
                    let after = source_file.metadata().map_err(prepare_error)?;
                    validate_copy_source(source_root.as_path(), source_relative_path)
                        .map_err(prepare_error)?;
                    if copied != before.len()
                        || before.len() != after.len()
                        || before.modified().ok() != after.modified().ok()
                    {
                        return Err(prepare_error("copy source changed during preparation"));
                    }
                }
                _ => unreachable!("only file writes enter staging"),
            }
            staged.sync_all().map_err(prepare_error)?;
            drop(staged);
            validate_regular_file(&staged_path).map_err(prepare_error)?;
            validator(relative_path, &staged_path).map_err(prepare_error)?;
        }

        Ok(())
    }
}
