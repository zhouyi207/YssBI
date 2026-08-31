use crate::{PreparedProjectActivation, ProjectSession, ProjectState, ProjectTransactionContext};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use yss_project_discovery::normalize_project_name;
use yss_project_filesystem::{
    NormalizedProjectRoot, ProjectFilesystemError, ProjectFilesystemTransaction,
    ProjectRootBinding, ProjectRootLifecycleGuard, StagedFilesystemMutation, ensure_directory,
    read_project_source_tree, remove_directory_if_created, validate_deletion_root,
    validate_destination_policy,
};
use yss_project_identity::{OperationId, ProjectInstanceId, ProjectRootIdentity};
use yss_project_layout::{
    GLOBAL_VARIABLES_FILE, PROJECT_CONTENT_DIRECTORIES, PROJECT_METADATA_FILE, WORKSHEET_EXTENSION,
    WORKSHEETS_DIR,
};
use yss_project_manifest::ProjectManifest;
use yss_project_model::ProjectData;
use yss_worksheet_document::WorksheetDocument;

pub struct PreparedProjectCopy {
    pub metadata_path: PathBuf,
    pub prepared_activation: PreparedProjectActivation,
}

impl std::fmt::Debug for PreparedProjectCopy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedProjectCopy")
            .field("metadata_path", &self.metadata_path)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct CreatedProject {
    pub metadata_path: PathBuf,
    pub project_name: String,
}

#[derive(Debug)]
pub struct ProjectDeletionResult {
    pub deleted_root: NormalizedProjectRoot,
    pub cleared_project_instance_id: Option<ProjectInstanceId>,
}

pub struct PreparedProjectDeletion {
    deleted_root: NormalizedProjectRoot,
    post_activation_failed: bool,
    active_project_instance_id: Option<ProjectInstanceId>,
    activation: Option<crate::ProjectActivationToken>,
    lifecycle: Option<ProjectRootLifecycleGuard>,
}

impl PreparedProjectDeletion {
    pub fn post_activation_failed(&self) -> bool {
        self.post_activation_failed
    }
}

impl Drop for PreparedProjectDeletion {
    fn drop(&mut self) {
        self.lifecycle.take();
        self.activation.take();
    }
}

struct DestinationRootGuard {
    root: PathBuf,
    remove_on_drop: bool,
}

impl DestinationRootGuard {
    fn ensure(root: &Path) -> Result<Self, ProjectFilesystemError> {
        Ok(Self {
            root: root.to_path_buf(),
            remove_on_drop: ensure_directory(root)?,
        })
    }

    fn disarm(&mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for DestinationRootGuard {
    fn drop(&mut self) {
        remove_directory_if_created(&self.root, self.remove_on_drop);
    }
}

impl ProjectState {
    pub fn save_project_as_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        destination: &Path,
        operation_id: OperationId,
    ) -> Result<PreparedProjectCopy, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(stale("save-as project instance is stale"));
        }
        let basis_before = self
            .capture_prepared_authority_basis(&session.root)?
            .ok_or_else(|| stale("save-as source authority is no longer active"))?;
        let (_, _, _, authority) = self.coherent_project_read_snapshot(&session)?;
        let authority_basis = self
            .capture_prepared_authority_basis(&session.root)?
            .filter(|basis| basis == &basis_before)
            .ok_or_else(|| stale("save-as authority changed during snapshot capture"))?;
        let destination_binding = ProjectRootBinding::for_destination(destination)?;
        let destination_root = destination_binding.normalized().clone();
        if session.root == destination_root {
            return Err(invalid_root(
                destination,
                "save-as destination equals the source project",
            ));
        }
        validate_destination_policy(destination_root.as_path())?;

        let lease = self
            .filesystem()
            .acquire_many([session.root.clone(), destination_root.clone()])?;
        self.validate_project_session(&session)?;
        if self.capture_prepared_authority_basis(&session.root)? != Some(authority_basis.clone()) {
            return Err(stale(
                "save-as authority changed while waiting for root leases",
            ));
        }
        validate_destination_policy(destination_root.as_path())?;
        let mut root_guard = DestinationRootGuard::ensure(destination_root.as_path())?;
        let destination_binding = destination_binding.bind_existing()?;
        let mutations = copy_mutations(session.root.as_path(), &authority)?;
        let context = lifecycle_context(
            ProjectSession {
                instance_id: session.instance_id.clone(),
                root: destination_root.clone(),
            },
            operation_id,
            self,
        );
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
            lease,
            mutations,
            validate_project_copy_file,
        )?;
        self.validate_project_session(&session)?;
        destination_binding.revalidate()?;
        let committed = prepared.commit()?;
        destination_binding.revalidate()?;
        self.validate_project_session(&session)?;
        if self.capture_prepared_authority_basis(&session.root)? != Some(authority_basis.clone()) {
            return Err(stale(
                "save-as authority changed before destination publication",
            ));
        }
        let activation_data = self.read_activation_data(&destination_root)?;
        let prepared_activation = PreparedProjectActivation::from_data(
            Some(destination_root.clone()),
            activation_data,
            Some(authority_basis),
            false,
        )?;
        committed.finalize();
        root_guard.disarm();
        Ok(PreparedProjectCopy {
            metadata_path: destination_root.as_path().join(PROJECT_METADATA_FILE),
            prepared_activation,
        })
    }

    pub fn create_project_transaction(
        &self,
        name: &str,
        destination: &Path,
        operation_id: OperationId,
    ) -> Result<CreatedProject, ProjectFilesystemError> {
        let project_name = normalize_project_name(name);
        let destination_binding = ProjectRootBinding::for_destination(destination)?;
        let destination_root = destination_binding.normalized().clone();
        validate_destination_policy(destination_root.as_path())?;
        let lease = self.filesystem().acquire(destination_root.clone())?;
        validate_destination_policy(destination_root.as_path())?;
        let mut root_guard = DestinationRootGuard::ensure(destination_root.as_path())?;
        let destination_binding = destination_binding.bind_existing()?;
        let mut data = ProjectData::new();
        data.metadata.project_name = project_name.clone();
        data.metadata.export_time = current_export_time();
        let context = lifecycle_context(
            ProjectSession {
                instance_id: ProjectInstanceId::new(),
                root: destination_root.clone(),
            },
            operation_id,
            self,
        );
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
            lease,
            new_project_mutations(&data)?,
            validate_project_copy_file,
        )?;
        destination_binding.revalidate()?;
        let committed = prepared.commit()?;
        destination_binding.revalidate()?;
        committed.finalize();
        root_guard.disarm();
        Ok(CreatedProject {
            metadata_path: destination_root.as_path().join(PROJECT_METADATA_FILE),
            project_name,
        })
    }

    pub fn prepare_project_deletion(
        &self,
        root: &Path,
        expected_root_identity: Option<&ProjectRootIdentity>,
        expected_active_instance_id: Option<&ProjectInstanceId>,
    ) -> Result<PreparedProjectDeletion, ProjectFilesystemError> {
        let root_binding = ProjectRootBinding::for_existing(root)?;
        if expected_root_identity.is_some_and(|expected| root_binding.identity() != Some(expected))
        {
            return Err(stale("registered project root identity changed"));
        }
        let normalized = root_binding.normalized().clone();
        let activation = self.project_activation.acquire();
        let mut lifecycle = self.filesystem().begin_root_lifecycle(normalized.clone())?;
        root_binding.revalidate()?;
        validate_deletion_root(&normalized)?;
        let _active = active_session_for_deletion(self, &normalized, expected_active_instance_id)?;
        lifecycle.release_initial_and_drain();
        lifecycle.acquire_final()?;
        root_binding.revalidate()?;
        validate_deletion_root(&normalized)?;
        let active = active_session_for_deletion(self, &normalized, expected_active_instance_id)?;
        let cleared_project_instance_id =
            active.as_ref().map(|session| session.instance_id.clone());
        let cleared_activation = cleared_project_instance_id
            .as_ref()
            .map(|_| PreparedProjectActivation::from_data(None, ProjectData::new(), None, false))
            .transpose()?;
        root_binding.revalidate()?;
        move_project_to_recycle_bin(normalized.as_path())?;
        let post_activation_failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(cleared) = cleared_activation {
                let published = self
                    .publish_project_activation_without_test_hooks(cleared)
                    .map_err(|_| ())?;
                published.dispose();
            }
            Ok::<(), ()>(())
        }))
        .map_or(true, |result| result.is_err());
        Ok(PreparedProjectDeletion {
            deleted_root: normalized,
            post_activation_failed,
            active_project_instance_id: cleared_project_instance_id,
            activation: Some(activation),
            lifecycle: Some(lifecycle),
        })
    }

    pub fn commit_project_deletion(
        &self,
        prepared: PreparedProjectDeletion,
    ) -> ProjectDeletionResult {
        let cleared_project_instance_id = prepared.active_project_instance_id.clone();
        ProjectDeletionResult {
            deleted_root: prepared.deleted_root.clone(),
            cleared_project_instance_id,
        }
    }

    pub fn delete_project_transaction(
        &self,
        root: &Path,
        expected_root_identity: Option<&ProjectRootIdentity>,
        expected_active_instance_id: Option<&ProjectInstanceId>,
    ) -> Result<ProjectDeletionResult, ProjectFilesystemError> {
        let prepared = self.prepare_project_deletion(
            root,
            expected_root_identity,
            expected_active_instance_id,
        )?;
        Ok(self.commit_project_deletion(prepared))
    }
}

fn current_export_time() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(any(test, feature = "test-support"))]
pub type RecycleBinTestHook = std::sync::Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;

#[cfg(any(test, feature = "test-support"))]
static RECYCLE_BIN_TEST_HOOK: std::sync::Mutex<Option<RecycleBinTestHook>> =
    std::sync::Mutex::new(None);

#[cfg(any(test, feature = "test-support"))]
pub fn set_recycle_bin_test_hook(hook: Option<RecycleBinTestHook>) {
    *RECYCLE_BIN_TEST_HOOK.lock().unwrap() = hook;
}

fn move_project_to_recycle_bin(root: &Path) -> Result<(), ProjectFilesystemError> {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(hook) = RECYCLE_BIN_TEST_HOOK.lock().unwrap().clone() {
        return hook(root).map_err(|error| recycle_bin_error(root, error));
    }

    #[cfg(any(test, feature = "test-support"))]
    {
        std::fs::remove_dir_all(root).map_err(|error| recycle_bin_error(root, error))
    }

    #[cfg(not(any(test, feature = "test-support")))]
    {
        trash::delete(root).map_err(|error| recycle_bin_error(root, error))
    }
}

fn recycle_bin_error(root: &Path, error: impl ToString) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionCommitFailed {
        message: format!(
            "failed to move project root '{}' to the system recycle bin: {}",
            root.display(),
            error.to_string()
        ),
    }
}

fn lifecycle_context(
    session: ProjectSession,
    operation_id: OperationId,
    state: &ProjectState,
) -> ProjectTransactionContext {
    ProjectTransactionContext {
        session,
        operation_id,
        affected_resources: Vec::new(),
        expected_revisions: BTreeMap::new(),
        expected_absent_resources: BTreeSet::new(),
        recovery_marker: Some(state.project_recovery_marker()),
    }
}

fn active_session_for_deletion(
    state: &ProjectState,
    root: &NormalizedProjectRoot,
    expected: Option<&ProjectInstanceId>,
) -> Result<Option<ProjectSession>, ProjectFilesystemError> {
    let current = state.capture_project_session().ok();
    let active = current.filter(|session| &session.root == root);
    match (active, expected) {
        (Some(session), Some(expected)) if &session.instance_id == expected => Ok(Some(session)),
        (Some(_), _) => Err(stale("active project identity does not authorize deletion")),
        (None, Some(_)) => Err(stale(
            "expected active project is no longer active at this root",
        )),
        (None, None) => Ok(None),
    }
}

fn new_project_mutations(
    data: &ProjectData,
) -> Result<Vec<StagedFilesystemMutation>, ProjectFilesystemError> {
    let mut mutations = PROJECT_CONTENT_DIRECTORIES
        .into_iter()
        .map(create_directory)
        .collect::<Vec<_>>();
    mutations.extend([
        write_mutation(
            PROJECT_METADATA_FILE,
            crate::serialize_project_manifest(data).map_err(prepare_error)?,
        ),
        write_mutation(
            GLOBAL_VARIABLES_FILE,
            crate::serialize_global_variables(data).map_err(prepare_error)?,
        ),
    ]);
    Ok(mutations)
}

fn copy_mutations(
    source: &Path,
    authority: &ProjectData,
) -> Result<Vec<StagedFilesystemMutation>, ProjectFilesystemError> {
    let source_tree = read_project_source_tree(source)?;
    let mut directories = source_tree.directories;
    directories.extend(PROJECT_CONTENT_DIRECTORIES.map(PathBuf::from));
    let mut files = source_tree.files;
    files.remove(Path::new(PROJECT_METADATA_FILE));
    files.remove(Path::new(GLOBAL_VARIABLES_FILE));
    files.retain(|path, _| !path.starts_with(WORKSHEETS_DIR));
    files.insert(
        PathBuf::from(PROJECT_METADATA_FILE),
        crate::serialize_project_manifest(authority).map_err(prepare_error)?,
    );
    files.insert(
        PathBuf::from(GLOBAL_VARIABLES_FILE),
        crate::serialize_global_variables(authority).map_err(prepare_error)?,
    );
    for graph_path in authority.graphs.keys() {
        let (path, contents) =
            crate::serialize_graph_document(authority, graph_path).map_err(prepare_error)?;
        files.insert(path, contents);
    }
    for (worksheet_path, worksheet) in &authority.worksheets {
        let (path, contents) =
            crate::serialize_worksheet(worksheet_path, worksheet).map_err(prepare_error)?;
        files.insert(path, contents);
    }
    let mut mutations = directories
        .into_iter()
        .map(|relative_path| StagedFilesystemMutation::CreateDirectory { relative_path })
        .collect::<Vec<_>>();
    mutations.extend(files.into_iter().map(|(relative_path, contents)| {
        StagedFilesystemMutation::Write {
            relative_path,
            contents,
        }
    }));
    Ok(mutations)
}

fn validate_project_copy_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    if path == Path::new(PROJECT_METADATA_FILE) {
        return serde_json::from_slice::<ProjectManifest>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yssbi-event" | "yssbi-function") => {
            serde_json::from_slice::<crate::GraphDocument>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some("yssbi-vars") => serde_json::from_slice::<crate::GlobalVariablesDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Some(WORKSHEET_EXTENSION) => serde_json::from_slice::<WorksheetDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        _ => Ok(()),
    }
}

fn create_directory(path: impl Into<PathBuf>) -> StagedFilesystemMutation {
    StagedFilesystemMutation::CreateDirectory {
        relative_path: path.into(),
    }
}

fn write_mutation(path: impl Into<PathBuf>, contents: Vec<u8>) -> StagedFilesystemMutation {
    StagedFilesystemMutation::Write {
        relative_path: path.into(),
        contents,
    }
}

fn invalid_root(path: impl AsRef<Path>, message: impl Into<String>) -> ProjectFilesystemError {
    ProjectFilesystemError::InvalidRoot {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}

fn stale(message: impl Into<String>) -> ProjectFilesystemError {
    ProjectFilesystemError::StaleProjectLifecycle {
        message: message.into(),
    }
}

fn prepare_error(error: impl ToString) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}
