use super::ProjectFilesystemLeaseSet;
use crate::project::{ProjectFilesystemError, ProjectTransactionContext};
use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

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
            | Self::RemoveFile { relative_path }
            | Self::CreateDirectory { relative_path }
            | Self::RemoveDirectoryIfEmpty { relative_path } => vec![relative_path],
            Self::MoveFile { from, to } => vec![from, to],
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
    journal: Vec<MutationJournal>,
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
    journal: Vec<MutationJournal>,
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
struct MutationJournal {
    kind: MutationJournalKind,
    entries: Vec<JournalEntry>,
}

#[derive(Debug)]
enum MutationJournalKind {
    Generic,
    Move { from: PathBuf, to: PathBuf },
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
            for relative_path in mutation.relative_paths() {
                validate_secure_path(&root, relative_path, true).map_err(prepare_error)?;
            }
            validate_move_preconditions(&root, mutation)?;
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
                if transaction
                    .lease
                    .take_fault(ProjectFilesystemFaultPoint::StagedSerialization)
                {
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
                .map(|mutation| capture_mutation_before_images(&root, mutation))
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
                let _ = cleanup_staging(&staging_root, &transaction.lease);
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
                if self.transaction.lease.take_fault(point) {
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
                for relative_path in mutation.relative_paths() {
                    if let Err(error) = validate_secure_path(&root, relative_path, true) {
                        return self.commit_failed(&root, index, error.to_string(), false);
                    }
                }
            }
            if let Err(error) = apply_mutation(
                &root,
                &prepared_root,
                mutation,
                index,
                &mut self.created_parent_directories,
                &self.transaction.lease,
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

        if let Err(error) = cleanup_staging(&self.transaction.staging_root, &self.transaction.lease)
        {
            let mutation_count = self.transaction.mutations.len();
            return self.commit_failed(&root, mutation_count, error.to_string(), false);
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
        apply_recovery_required: bool,
    ) -> Result<CommittedFilesystemMutation, ProjectFilesystemError> {
        let rollback_result = restore_before_images(
            root,
            &self.journal[..applied_count.min(self.journal.len())],
            &self.created_parent_directories,
            &self.transaction.lease,
        );
        let cleanup_result =
            cleanup_staging(&self.transaction.staging_root, &self.transaction.lease);
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
        if apply_recovery_required {
            let error = ProjectFilesystemError::TransactionRollbackFailed {
                message: commit_message,
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
        self._lease.run_rollback_hook();
        let rollback_result = restore_before_images(
            &self.root,
            &self.journal,
            &self.created_parent_directories,
            &self._lease,
        );
        let cleanup_result = cleanup_staging(&self.staging_root, &self._lease);
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
            let rollback = restore_before_images(
                &self.root,
                &self.journal,
                &self.created_parent_directories,
                &self._lease,
            );
            let cleanup = cleanup_staging(&self.staging_root, &self._lease);
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

#[derive(Debug)]
enum PortablePathOwner {
    Write { spelling: PathBuf },
    RemoveFile,
    Exclusive,
    RewritePairComplete,
}

#[derive(Clone, Copy)]
enum PortablePathClaim {
    Write,
    RemoveFile,
    Exclusive,
}

fn validate_mutation_paths(
    mutations: &[StagedFilesystemMutation],
) -> Result<(), ProjectFilesystemError> {
    let mut owners = HashMap::new();
    for mutation in mutations {
        let paths = mutation.relative_paths();
        for relative in &paths {
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
        }

        match mutation {
            StagedFilesystemMutation::Write { relative_path, .. } => {
                register_portable_path(&mut owners, relative_path, PortablePathClaim::Write)?
            }
            StagedFilesystemMutation::RemoveFile { relative_path } => {
                register_portable_path(&mut owners, relative_path, PortablePathClaim::RemoveFile)?
            }
            StagedFilesystemMutation::MoveFile { from, to }
                if portable_path_key(from) == portable_path_key(to) =>
            {
                register_portable_path(&mut owners, from, PortablePathClaim::Exclusive)?;
            }
            StagedFilesystemMutation::MoveFile { from, to } => {
                register_portable_path(&mut owners, from, PortablePathClaim::Exclusive)?;
                register_portable_path(&mut owners, to, PortablePathClaim::Exclusive)?;
            }
            StagedFilesystemMutation::CreateDirectory { relative_path }
            | StagedFilesystemMutation::RemoveDirectoryIfEmpty { relative_path } => {
                register_portable_path(&mut owners, relative_path, PortablePathClaim::Exclusive)?;
            }
        }
    }
    Ok(())
}

fn register_portable_path(
    owners: &mut HashMap<String, PortablePathOwner>,
    relative: &Path,
    claim: PortablePathClaim,
) -> Result<(), ProjectFilesystemError> {
    let key = portable_path_key(relative);
    let spelling = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect::<PathBuf>();
    match owners.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(match claim {
                PortablePathClaim::Write => PortablePathOwner::Write { spelling },
                PortablePathClaim::RemoveFile => PortablePathOwner::RemoveFile,
                PortablePathClaim::Exclusive => PortablePathOwner::Exclusive,
            });
            Ok(())
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if matches!(claim, PortablePathClaim::RemoveFile)
                && matches!(
                    entry.get(),
                    PortablePathOwner::Write {
                        spelling: write_spelling
                    } if write_spelling != &spelling
                )
            {
                entry.insert(PortablePathOwner::RewritePairComplete);
                return Ok(());
            }
            Err(prepare_error(format!(
                "duplicate portable path '{}' in filesystem transaction",
                relative.display()
            )))
        }
    }
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

fn portable_path_key(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
        .case_fold()
        .nfc()
        .collect()
}

fn validate_move_preconditions(
    root: &Path,
    mutation: &StagedFilesystemMutation,
) -> Result<(), ProjectFilesystemError> {
    let StagedFilesystemMutation::MoveFile { from, to } = mutation else {
        return Ok(());
    };
    validate_regular_file(&root.join(from)).map_err(prepare_error)?;
    let target_parent = to.parent().unwrap_or_else(|| Path::new(""));
    let target_name = to
        .file_name()
        .ok_or_else(|| prepare_error(format!("move target '{}' has no file name", to.display())))?;
    let source_key = portable_path_key(from);
    let target_key = portable_path_key(to);
    let directory = root.join(target_parent);
    if directory.exists() {
        for entry in std::fs::read_dir(&directory).map_err(prepare_error)? {
            let entry = entry.map_err(prepare_error)?;
            let candidate = target_parent.join(entry.file_name());
            if entry
                .file_name()
                .to_string_lossy()
                .case_fold()
                .nfc()
                .collect::<String>()
                != target_name
                    .to_string_lossy()
                    .case_fold()
                    .nfc()
                    .collect::<String>()
            {
                continue;
            }
            if candidate == *from && source_key == target_key {
                continue;
            }
            return Err(prepare_error(format!(
                "move target '{}' has an existing portable conflict at '{}'",
                to.display(),
                candidate.display()
            )));
        }
    }
    Ok(())
}

fn capture_mutation_before_images(
    root: &Path,
    mutation: &StagedFilesystemMutation,
) -> Result<MutationJournal, ProjectFilesystemError> {
    let kind = match mutation {
        StagedFilesystemMutation::MoveFile { from, to } => MutationJournalKind::Move {
            from: from.clone(),
            to: to.clone(),
        },
        _ => MutationJournalKind::Generic,
    };
    let entries = mutation
        .relative_paths()
        .into_iter()
        .map(|path| capture_before_image(root, path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MutationJournal { kind, entries })
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

struct ApplyMutationError {
    source: std::io::Error,
    include_current_in_rollback: bool,
    recovery_required: bool,
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

fn apply_mutation(
    root: &Path,
    prepared_root: &Path,
    mutation: &StagedFilesystemMutation,
    index: usize,
    created_parent_directories: &mut BTreeSet<PathBuf>,
    _lease: &ProjectFilesystemLeaseSet,
) -> Result<(), ApplyMutationError> {
    let relative = mutation
        .relative_paths()
        .into_iter()
        .next()
        .expect("filesystem mutation has at least one path");
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
            #[cfg(test)]
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
                #[cfg(test)]
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
                #[cfg(test)]
                _lease.run_before_remove_hook();
                match move_file_no_replace(&temporary, &target, _lease) {
                    Ok(()) => return Ok(()),
                    Err(target_error) => {
                        #[cfg(test)]
                        if _lease.take_fault(ProjectFilesystemFaultPoint::MoveRestoration) {
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
            #[cfg(test)]
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
    _lease: &ProjectFilesystemLeaseSet,
) -> Result<(), NoReplaceMoveError> {
    std::fs::hard_link(source, target).map_err(|source| NoReplaceMoveError {
        source,
        recovery_required: false,
    })?;

    #[cfg(test)]
    let injected_cleanup_failure =
        _lease.take_fault(ProjectFilesystemFaultPoint::MoveTargetCleanup);
    #[cfg(not(test))]
    let injected_cleanup_failure = false;
    #[cfg(test)]
    let source_removal = if injected_cleanup_failure
        || _lease.take_fault(ProjectFilesystemFaultPoint::MoveSourceRemoval)
    {
        Err(std::io::Error::other(
            "injected move source removal failure",
        ))
    } else {
        std::fs::remove_file(source)
    };
    #[cfg(not(test))]
    let source_removal = std::fs::remove_file(source);

    if let Err(source_error) = source_removal {
        #[cfg(test)]
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
    journal: &[MutationJournal],
    created_parent_directories: &BTreeSet<PathBuf>,
    _lease: &ProjectFilesystemLeaseSet,
) -> std::io::Result<()> {
    let move_recovery_copies = journal
        .iter()
        .enumerate()
        .filter_map(|(index, mutation)| match &mutation.kind {
            MutationJournalKind::Move { from, .. } => Some(
                move_source_contents(mutation, from)
                    .and_then(|contents| create_move_recovery_copy(root, contents, index))
                    .map(|path| (index, path)),
            ),
            MutationJournalKind::Generic => None,
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;

    #[cfg(test)]
    if _lease.take_rollback_fault() {
        return Err(std::io::Error::other(
            "injected rollback restore failure after move recovery copies were retained",
        ));
    }

    let mut replacement_index = 0;
    let mut move_messages = Vec::new();
    for (mutation_index, mutation) in journal.iter().enumerate().rev() {
        match &mutation.kind {
            MutationJournalKind::Generic => {
                restore_journal_entries(root, &mutation.entries, &mut replacement_index)?;
            }
            MutationJournalKind::Move { from, to } => {
                let recovery_copy = move_recovery_copies
                    .get(&mutation_index)
                    .expect("every move journal has a retained recovery copy");
                let contents = move_source_contents(mutation, from)?;
                let source = root.join(from);
                validate_secure_path(root, from, true)?;
                let source_result = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&source)
                    .and_then(|mut file| {
                        file.write_all(contents)?;
                        file.sync_all()
                    });
                let source_status = match source_result {
                    Ok(()) => format!("source '{}' restored without replacement", from.display()),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => format!(
                        "source '{}' was already present and was not overwritten",
                        from.display()
                    ),
                    Err(error) => {
                        format!("source '{}' restoration failed: {error}", from.display())
                    }
                };
                move_messages.push(format!(
                    "{source_status}; move target '{}' retained because ownership cannot be proven atomically; original bytes retained at '{}'",
                    to.display(),
                    recovery_copy.display()
                ));
            }
        }
    }

    let protected_directories = move_target_directories(journal);
    for relative in created_parent_directories.iter().rev() {
        if protected_directories.contains(relative) {
            continue;
        }
        validate_secure_path(root, relative, false)?;
        let path = root.join(relative);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir(path)?,
            Ok(_) => return Err(std::io::Error::other("created parent is not a directory")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    if move_messages.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::other(move_messages.join("; ")))
    }
}

fn move_source_contents<'a>(
    mutation: &'a MutationJournal,
    from: &Path,
) -> std::io::Result<&'a [u8]> {
    mutation
        .entries
        .iter()
        .find_map(|entry| {
            if entry.relative_path != from {
                return None;
            }
            match &entry.before {
                BeforeImage::File(contents) => Some(contents.as_slice()),
                _ => None,
            }
        })
        .ok_or_else(|| std::io::Error::other("move journal has no source file before-image"))
}

fn create_move_recovery_copy(
    root: &Path,
    contents: &[u8],
    index: usize,
) -> std::io::Result<PathBuf> {
    validate_real_directory(root)?;
    for _ in 0..32 {
        let path = root.join(format!(
            ".yssbi-move-recovery-{index}-{}",
            uuid::Uuid::new_v4()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(contents)?;
                file.sync_all()?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a collision-safe move recovery file",
    ))
}

fn move_target_directories(journal: &[MutationJournal]) -> BTreeSet<PathBuf> {
    let mut protected = BTreeSet::new();
    for mutation in journal {
        let MutationJournalKind::Move { to, .. } = &mutation.kind else {
            continue;
        };
        let mut current = PathBuf::new();
        if let Some(parent) = to.parent() {
            for component in parent.components() {
                current.push(component.as_os_str());
                protected.insert(current.clone());
            }
        }
    }
    protected
}

fn restore_journal_entries(
    root: &Path,
    entries: &[JournalEntry],
    replacement_index: &mut usize,
) -> std::io::Result<()> {
    for entry in entries.iter().rev() {
        validate_secure_path(root, &entry.relative_path, true)?;
        let path = root.join(&entry.relative_path);
        match &entry.before {
            BeforeImage::Absent => remove_path_if_present(&path)?,
            BeforeImage::File(contents) => {
                remove_path_if_present(&path)?;
                let mut restored_parents = BTreeSet::new();
                create_missing_parents(root, &path, &mut restored_parents)?;
                let mut temporary = ReplacementFile::create(&path, *replacement_index)?;
                *replacement_index += 1;
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

fn cleanup_staging(staging_root: &Path, _lease: &ProjectFilesystemLeaseSet) -> std::io::Result<()> {
    #[cfg(test)]
    if _lease.take_fault(ProjectFilesystemFaultPoint::StagingCleanup) {
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
    MoveSourceRemoval,
    MoveTargetCleanup,
    MoveRestoration,
    StagingCleanup,
}
