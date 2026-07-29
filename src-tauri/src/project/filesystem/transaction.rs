use super::ProjectFilesystemLeaseSet;
use crate::project::{ProjectFilesystemError, ProjectTransactionContext};
use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const TRANSACTION_DIRECTORY: &str = ".yssbi-transaction";

#[derive(Clone, Debug)]
pub enum StagedFilesystemMutation {
    Write {
        relative_path: PathBuf,
        contents: Vec<u8>,
    },
    RemoveFile {
        relative_path: PathBuf,
    },
    CreateDirectory {
        relative_path: PathBuf,
    },
    RemoveDirectoryIfEmpty {
        relative_path: PathBuf,
    },
}

impl StagedFilesystemMutation {
    fn relative_path(&self) -> &Path {
        match self {
            Self::Write { relative_path, .. }
            | Self::RemoveFile { relative_path }
            | Self::CreateDirectory { relative_path }
            | Self::RemoveDirectoryIfEmpty { relative_path } => relative_path,
        }
    }
}

pub struct ProjectFilesystemTransaction {
    context: ProjectTransactionContext,
    lease: ProjectFilesystemLeaseSet,
    staging_root: PathBuf,
    mutations: Vec<StagedFilesystemMutation>,
}

pub struct PreparedProjectFilesystemTransaction {
    transaction: ProjectFilesystemTransaction,
    journal: Vec<JournalEntry>,
    created_parent_directories: BTreeSet<PathBuf>,
}

impl std::fmt::Debug for PreparedProjectFilesystemTransaction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedProjectFilesystemTransaction")
            .field("context", &self.transaction.context)
            .field("staging_root", &self.transaction.staging_root)
            .field("mutations", &self.transaction.mutations)
            .finish_non_exhaustive()
    }
}

pub struct CommittedFilesystemMutation {
    root: PathBuf,
    staging_root: PathBuf,
    journal: Vec<JournalEntry>,
    created_parent_directories: BTreeSet<PathBuf>,
    _lease: ProjectFilesystemLeaseSet,
    recovery_marker: Option<crate::project::ProjectRecoveryMarker>,
    armed: bool,
}

impl std::fmt::Debug for CommittedFilesystemMutation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommittedFilesystemMutation")
            .field("root", &self.root)
            .field("staging_root", &self.staging_root)
            .field("journal", &self.journal)
            .finish_non_exhaustive()
    }
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

impl ProjectFilesystemTransaction {
    pub fn prepare(
        context: ProjectTransactionContext,
        lease: ProjectFilesystemLeaseSet,
        mutations: Vec<StagedFilesystemMutation>,
    ) -> Result<PreparedProjectFilesystemTransaction, ProjectFilesystemError> {
        Self::prepare_with_validator(context, lease, mutations, |_, contents| {
            serde_json::from_slice::<serde_json::Value>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
    }

    pub fn prepare_with_validator(
        context: ProjectTransactionContext,
        lease: ProjectFilesystemLeaseSet,
        mutations: Vec<StagedFilesystemMutation>,
        mut validator: impl FnMut(&Path, &[u8]) -> Result<(), String>,
    ) -> Result<PreparedProjectFilesystemTransaction, ProjectFilesystemError> {
        if !lease.contains(&context.session.root) {
            return Err(ProjectFilesystemError::TransactionPrepareFailed {
                message: "transaction lease does not own the project root".into(),
            });
        }
        validate_mutation_paths(&mutations)?;
        let root = context.session.root.as_path().to_path_buf();
        validate_real_directory(&root).map_err(prepare_error)?;
        for mutation in &mutations {
            validate_secure_path(&root, mutation.relative_path(), true).map_err(prepare_error)?;
        }
        let staging_root = root
            .join(TRANSACTION_DIRECTORY)
            .join(context.operation_id.to_string());
        let prepared_root = staging_root.join("prepared");
        let transaction = Self {
            context,
            lease,
            staging_root: staging_root.clone(),
            mutations,
        };

        let prepare_result = (|| {
            create_secure_directories(&root, &prepared_root).map_err(prepare_error)?;
            for mutation in &transaction.mutations {
                let StagedFilesystemMutation::Write {
                    relative_path,
                    contents,
                } = mutation
                else {
                    continue;
                };
                #[cfg(test)]
                if take_fault(ProjectFilesystemFaultPoint::StagedSerialization) {
                    return Err(prepare_error("injected staged serialization failure"));
                }
                let staged_path = prepared_root.join(relative_path);
                if let Some(parent) = staged_path.parent() {
                    create_secure_directories(&root, parent).map_err(prepare_error)?;
                }
                let mut staged = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&staged_path)
                    .map_err(prepare_error)?;
                staged.write_all(contents).map_err(prepare_error)?;
                staged.sync_all().map_err(prepare_error)?;
                drop(staged);
                validate_regular_file(&staged_path).map_err(prepare_error)?;
                let mut staged_contents = Vec::new();
                std::fs::File::open(&staged_path)
                    .and_then(|mut file| file.read_to_end(&mut staged_contents))
                    .map_err(prepare_error)?;
                validator(relative_path, &staged_contents).map_err(prepare_error)?;
            }

            let journal = transaction
                .mutations
                .iter()
                .map(|mutation| capture_before_image(&root, mutation.relative_path()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(journal)
        })();

        match prepare_result {
            Ok(journal) => Ok(PreparedProjectFilesystemTransaction {
                transaction,
                journal,
                created_parent_directories: BTreeSet::new(),
            }),
            Err(error) => {
                let _ = cleanup_staging(&staging_root);
                Err(error)
            }
        }
    }
}

impl PreparedProjectFilesystemTransaction {
    pub fn staging_root(&self) -> &Path {
        &self.transaction.staging_root
    }

    pub fn commit(mut self) -> Result<CommittedFilesystemMutation, ProjectFilesystemError> {
        let root = self
            .transaction
            .context
            .session
            .root
            .as_path()
            .to_path_buf();
        let prepared_root = self.transaction.staging_root.join("prepared");
        for (index, mutation) in self.transaction.mutations.iter().enumerate() {
            #[cfg(test)]
            {
                let point = if index == 0 {
                    ProjectFilesystemFaultPoint::FirstLiveReplacement
                } else {
                    ProjectFilesystemFaultPoint::SecondLiveReplacement
                };
                if take_fault(point) {
                    return self.commit_failed(
                        &root,
                        index,
                        format!(
                            "injected live replacement failure at mutation {}",
                            index + 1
                        ),
                    );
                }
            }
            if matches!(
                mutation,
                StagedFilesystemMutation::RemoveFile { .. }
                    | StagedFilesystemMutation::RemoveDirectoryIfEmpty { .. }
            ) {
                if let Err(error) = validate_secure_path(&root, mutation.relative_path(), true) {
                    return self.commit_failed(&root, index, error.to_string());
                }
            }
            if let Err(error) = apply_mutation(
                &root,
                &prepared_root,
                mutation,
                index,
                &mut self.created_parent_directories,
            ) {
                return self.commit_failed(&root, index + 1, error.to_string());
            }
        }

        if let Err(error) = cleanup_staging(&self.transaction.staging_root) {
            let mutation_count = self.transaction.mutations.len();
            return self.commit_failed(&root, mutation_count, error.to_string());
        }

        Ok(CommittedFilesystemMutation {
            root,
            staging_root: self.transaction.staging_root,
            journal: self.journal,
            created_parent_directories: self.created_parent_directories,
            _lease: self.transaction.lease,
            recovery_marker: self.transaction.context.recovery_marker.clone(),
            armed: true,
        })
    }

    fn commit_failed(
        self,
        root: &Path,
        applied_count: usize,
        commit_message: String,
    ) -> Result<CommittedFilesystemMutation, ProjectFilesystemError> {
        let rollback_result = restore_before_images(
            root,
            &self.journal[..applied_count.min(self.journal.len())],
            &self.created_parent_directories,
        );
        let cleanup_result = cleanup_staging(&self.transaction.staging_root);
        if let Err(rollback_error) = rollback_result {
            let error = ProjectFilesystemError::TransactionRollbackFailed {
                message: format!(
                    "{commit_message}; rollback failed: {rollback_error}{}",
                    cleanup_result
                        .err()
                        .map(|error| format!("; staging cleanup failed: {error}"))
                        .unwrap_or_default()
                ),
                recovery_required: true,
            };
            mark_recovery(&self.transaction.context.recovery_marker, &error);
            return Err(error);
        }
        if let Err(cleanup_error) = cleanup_result {
            let error = ProjectFilesystemError::TransactionRollbackFailed {
                message: format!(
                    "{commit_message}; rollback staging cleanup failed: {cleanup_error}"
                ),
                recovery_required: true,
            };
            mark_recovery(&self.transaction.context.recovery_marker, &error);
            return Err(error);
        }
        Err(ProjectFilesystemError::TransactionCommitFailed {
            message: commit_message,
        })
    }
}

impl CommittedFilesystemMutation {
    pub fn finalize(mut self) {
        self.armed = false;
    }

    pub fn rollback(mut self) -> Result<(), ProjectFilesystemError> {
        self.armed = false;
        #[cfg(test)]
        run_project_filesystem_rollback_test_hook();
        let rollback_result =
            restore_before_images(&self.root, &self.journal, &self.created_parent_directories);
        let cleanup_result = cleanup_staging(&self.staging_root);
        match (rollback_result, cleanup_result) {
            (Ok(()), Ok(())) => Ok(()),
            (rollback, cleanup) => {
                let error = ProjectFilesystemError::TransactionRollbackFailed {
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
                mark_recovery(&self.recovery_marker, &error);
                Err(error)
            }
        }
    }
}

impl Drop for CommittedFilesystemMutation {
    fn drop(&mut self) {
        if self.armed {
            let rollback =
                restore_before_images(&self.root, &self.journal, &self.created_parent_directories);
            let cleanup = cleanup_staging(&self.staging_root);
            if rollback.is_err() || cleanup.is_err() {
                let rollback_error = ProjectFilesystemError::TransactionRollbackFailed {
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
                mark_recovery(&self.recovery_marker, &rollback_error);
            }
            self.armed = false;
        }
    }
}

fn validate_mutation_paths(
    mutations: &[StagedFilesystemMutation],
) -> Result<(), ProjectFilesystemError> {
    let mut targets = HashSet::new();
    for mutation in mutations {
        let relative = mutation.relative_path();
        let valid = !relative.as_os_str().is_empty()
            && !relative.is_absolute()
            && relative.components().all(|component| {
                matches!(component, Component::Normal(_) | Component::CurDir)
            })
            && relative.components().next().is_some_and(|component| {
                !matches!(component, Component::Normal(name) if name == TRANSACTION_DIRECTORY)
            });
        if !valid {
            return Err(prepare_error(format!(
                "invalid transaction target '{}'",
                relative.display()
            )));
        }
        let normalized = relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name),
                _ => None,
            })
            .collect::<PathBuf>();
        if !targets.insert(normalized) {
            return Err(prepare_error(format!(
                "duplicate transaction target '{}'",
                relative.display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn metadata_is_redirect(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn validate_real_directory(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata_is_redirect(&metadata) || !metadata.is_dir() {
        return Err(std::io::Error::other(format!(
            "path '{}' is not a real directory",
            path.display()
        )));
    }
    Ok(())
}

fn validate_regular_file(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata_is_redirect(&metadata) || !metadata.is_file() {
        return Err(std::io::Error::other(format!(
            "path '{}' is not a regular file",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn read_secure_project_file(root: &Path, relative: &Path) -> std::io::Result<Vec<u8>> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(std::io::Error::other(format!(
            "project source '{}' is not a safe relative path",
            relative.display()
        )));
    }
    validate_secure_path(root, relative, true)?;
    let source = root.join(relative);
    validate_regular_file(&source)?;
    std::fs::read(source)
}

fn validate_secure_path(
    root: &Path,
    relative: &Path,
    allow_final_non_directory: bool,
) -> std::io::Result<()> {
    validate_real_directory(root)?;
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata_is_redirect(&metadata) {
                    return Err(std::io::Error::other(format!(
                        "path '{}' traverses a redirect",
                        current.display()
                    )));
                }
                let final_component = index + 1 == components.len();
                if (!final_component || !allow_final_non_directory) && !metadata.is_dir() {
                    return Err(std::io::Error::other(format!(
                        "path ancestor '{}' is not a directory",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn create_secure_directories(root: &Path, directory: &Path) -> std::io::Result<()> {
    let relative = directory
        .strip_prefix(root)
        .map_err(|_| std::io::Error::other("directory escapes project root"))?;
    let mut ignored = BTreeSet::new();
    create_missing_directories(root, &root.join(relative), &mut ignored)
}

fn validate_no_redirect_tree(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata_is_redirect(&metadata) {
        return Err(std::io::Error::other(format!(
            "cleanup path '{}' is a redirect",
            path.display()
        )));
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            validate_no_redirect_tree(&entry?.path())?;
        }
    }
    Ok(())
}

fn capture_before_image(
    root: &Path,
    relative_path: &Path,
) -> Result<JournalEntry, ProjectFilesystemError> {
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

struct ReplacementFile {
    path: PathBuf,
    file: Option<std::fs::File>,
    armed: bool,
}

impl ReplacementFile {
    fn create(live: &Path, index: usize) -> std::io::Result<Self> {
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

    fn path(&self) -> &Path {
        &self.path
    }

    fn file_mut(&mut self) -> &mut std::fs::File {
        self.file.as_mut().expect("replacement file is open")
    }

    fn close(&mut self) {
        self.file.take();
    }

    fn disarm(&mut self) {
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

fn apply_mutation(
    root: &Path,
    prepared_root: &Path,
    mutation: &StagedFilesystemMutation,
    index: usize,
    created_parent_directories: &mut BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    let relative = mutation.relative_path();
    let live = root.join(relative);
    match mutation {
        StagedFilesystemMutation::Write { .. } => {
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
            run_before_remove_mutation_hook();
            validate_secure_path(root, relative, true)?;
            match std::fs::symlink_metadata(&live) {
                Ok(metadata) if metadata_is_redirect(&metadata) => {
                    Err(std::io::Error::other("remove-file target is a redirect"))
                }
                Ok(metadata) if metadata.is_file() => std::fs::remove_file(live),
                Ok(_) => Err(std::io::Error::other("remove-file target is not a file")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
        StagedFilesystemMutation::CreateDirectory { .. } => {
            if live.exists() {
                validate_real_directory(&live)
            } else {
                create_missing_directories(root, &live, created_parent_directories)
            }
        }
        StagedFilesystemMutation::RemoveDirectoryIfEmpty { .. } => {
            run_before_remove_mutation_hook();
            validate_secure_path(root, relative, false)?;
            match std::fs::symlink_metadata(&live) {
                Ok(metadata) if metadata_is_redirect(&metadata) => Err(std::io::Error::other(
                    "remove-directory target is a redirect",
                )),
                Ok(metadata) if metadata.is_dir() => std::fs::remove_dir(live),
                Ok(_) => Err(std::io::Error::other(
                    "remove-directory target is not a directory",
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
    }
}

fn create_missing_parents(
    root: &Path,
    path: &Path,
    created: &mut BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    create_missing_directories(root, parent, created)
}

fn create_missing_directories(
    root: &Path,
    directory: &Path,
    created: &mut BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    let relative = directory.strip_prefix(root).unwrap_or(directory);
    let mut current = root.to_path_buf();
    validate_real_directory(root)?;
    for component in relative.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(_) => validate_real_directory(&current)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current)?;
                validate_real_directory(&current)?;
                created.insert(current.strip_prefix(root).unwrap().to_path_buf());
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn restore_before_images(
    root: &Path,
    journal: &[JournalEntry],
    created_parent_directories: &BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    #[cfg(test)]
    if ROLLBACK_FAULT.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(std::io::Error::other("injected rollback restore failure"));
    }
    for (index, entry) in journal.iter().rev().enumerate() {
        validate_secure_path(root, &entry.relative_path, true)?;
        let path = root.join(&entry.relative_path);
        match &entry.before {
            BeforeImage::Absent => remove_path_if_present(&path)?,
            BeforeImage::File(contents) => {
                remove_path_if_present(&path)?;
                let mut restored_parents = BTreeSet::new();
                create_missing_parents(root, &path, &mut restored_parents)?;
                let mut temporary = ReplacementFile::create(&path, index)?;
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
    for relative in created_parent_directories.iter().rev() {
        validate_secure_path(root, relative, false)?;
        let path = root.join(relative);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir(path)?,
            Ok(_) => return Err(std::io::Error::other("created parent is not a directory")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
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

fn cleanup_staging(staging_root: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if take_fault(ProjectFilesystemFaultPoint::StagingCleanup) {
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

fn mark_recovery(
    marker: &Option<crate::project::ProjectRecoveryMarker>,
    error: &ProjectFilesystemError,
) {
    if let Some(marker) = marker {
        marker.mark(error.to_string());
    }
}

fn prepare_error(error: impl std::fmt::Display) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectFilesystemFaultPoint {
    StagedSerialization,
    FirstLiveReplacement,
    SecondLiveReplacement,
    StagingCleanup,
}

#[cfg(test)]
static FAULT_POINT: std::sync::Mutex<Option<ProjectFilesystemFaultPoint>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
static ROLLBACK_FAULT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(test)]
static BEFORE_REMOVE_MUTATION_HOOK: std::sync::Mutex<
    Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
> = std::sync::Mutex::new(None);

#[cfg(test)]
static ROLLBACK_TEST_HOOK: std::sync::Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub fn set_project_filesystem_rollback_test_hook(
    hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
) {
    *ROLLBACK_TEST_HOOK.lock().unwrap() = hook;
}

#[cfg(test)]
fn run_project_filesystem_rollback_test_hook() {
    let hook = ROLLBACK_TEST_HOOK.lock().unwrap().clone();
    if let Some(hook) = hook {
        hook();
    }
}

#[cfg(test)]
pub fn set_before_remove_mutation_hook(hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
    *BEFORE_REMOVE_MUTATION_HOOK.lock().unwrap() = hook;
}

#[cfg(test)]
fn run_before_remove_mutation_hook() {
    if let Some(hook) = BEFORE_REMOVE_MUTATION_HOOK.lock().unwrap().take() {
        hook();
    }
}

#[cfg(not(test))]
fn run_before_remove_mutation_hook() {}

#[cfg(test)]
pub fn set_project_filesystem_fault(fault: Option<ProjectFilesystemFaultPoint>) {
    *FAULT_POINT.lock().unwrap() = fault;
}

#[cfg(test)]
pub fn set_project_filesystem_rollback_fault(enabled: bool) {
    ROLLBACK_FAULT.store(enabled, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
fn take_fault(point: ProjectFilesystemFaultPoint) -> bool {
    let mut fault = FAULT_POINT.lock().unwrap();
    if *fault == Some(point) {
        *fault = None;
        true
    } else {
        false
    }
}
