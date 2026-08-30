use crate::project::OperationId;
use crate::project::{
    DATABASE_DIR, EVENTS_DIR, FUNCTIONS_DIR, GLOBAL_VARIABLES_FILE, NormalizedProjectRoot,
    PROJECT_METADATA_FILE, PreparedProjectActivation, ProjectData, ProjectFilesystemError,
    ProjectFilesystemTransaction, ProjectInstanceId, ProjectRootBinding, ProjectRootIdentity,
    ProjectRootLifecycleGuard, ProjectSession, ProjectState, ProjectTransactionContext,
    StagedFilesystemMutation, WORKSHEETS_DIR, ensure_directory, read_project_source_tree,
    remove_directory_if_created, validate_deletion_root, validate_destination_policy,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
    activation: Option<crate::project::ProjectActivationToken>,
    lifecycle: Option<ProjectRootLifecycleGuard>,
    #[cfg(test)]
    run_drain: Option<crate::node_system::runtime::ProjectRunDrainGuard>,
}

impl PreparedProjectDeletion {
    pub fn post_activation_failed(&self) -> bool {
        self.post_activation_failed
    }
}

impl Drop for PreparedProjectDeletion {
    fn drop(&mut self) {
        #[cfg(test)]
        self.run_drain.take();
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
            context,
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
        let project_name = crate::project::normalize_project_name(name);
        let destination_binding = ProjectRootBinding::for_destination(destination)?;
        let destination_root = destination_binding.normalized().clone();
        validate_destination_policy(destination_root.as_path())?;
        let lease = self.filesystem().acquire(destination_root.clone())?;
        validate_destination_policy(destination_root.as_path())?;
        let mut root_guard = DestinationRootGuard::ensure(destination_root.as_path())?;
        let destination_binding = destination_binding.bind_existing()?;
        let mut data = ProjectData::new();
        data.metadata.project_name = project_name.clone();
        data.update_metadata();
        let context = lifecycle_context(
            ProjectSession {
                instance_id: ProjectInstanceId::new(),
                root: destination_root.clone(),
            },
            operation_id,
            self,
        );
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context,
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
        #[cfg(test)]
        let active = active_session_for_deletion(self, &normalized, expected_active_instance_id)?;
        #[cfg(not(test))]
        let _active = active_session_for_deletion(self, &normalized, expected_active_instance_id)?;
        #[cfg(test)]
        let run_snapshot = active.as_ref().map(|_| self.current_run_registry());
        lifecycle.release_initial_and_drain();
        #[cfg(test)]
        let run_drain = run_snapshot
            .as_ref()
            .map(|(runs, session_id)| runs.begin_drain(session_id));
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
            #[cfg(test)]
            run_drain,
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

#[cfg(test)]
type RecycleBinTestHook = std::sync::Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;

#[cfg(test)]
static RECYCLE_BIN_TEST_HOOK: std::sync::Mutex<Option<RecycleBinTestHook>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_recycle_bin_test_hook(hook: Option<RecycleBinTestHook>) {
    *RECYCLE_BIN_TEST_HOOK.lock().unwrap() = hook;
}

fn move_project_to_recycle_bin(root: &Path) -> Result<(), ProjectFilesystemError> {
    #[cfg(test)]
    if let Some(hook) = RECYCLE_BIN_TEST_HOOK.lock().unwrap().clone() {
        return hook(root).map_err(|error| recycle_bin_error(root, error));
    }

    #[cfg(test)]
    {
        std::fs::remove_dir_all(root).map_err(|error| recycle_bin_error(root, error))
    }

    #[cfg(not(test))]
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
    Ok(vec![
        create_directory(EVENTS_DIR),
        create_directory(FUNCTIONS_DIR),
        create_directory(WORKSHEETS_DIR),
        create_directory(DATABASE_DIR),
        write_mutation(
            PROJECT_METADATA_FILE,
            crate::project::serialize_project_manifest(data).map_err(prepare_error)?,
        ),
        write_mutation(
            GLOBAL_VARIABLES_FILE,
            crate::project::serialize_global_variables(data).map_err(prepare_error)?,
        ),
    ])
}

fn copy_mutations(
    source: &Path,
    authority: &ProjectData,
) -> Result<Vec<StagedFilesystemMutation>, ProjectFilesystemError> {
    let source_tree = read_project_source_tree(source)?;
    let mut directories = source_tree.directories;
    directories.extend([
        PathBuf::from(EVENTS_DIR),
        PathBuf::from(FUNCTIONS_DIR),
        PathBuf::from(WORKSHEETS_DIR),
        PathBuf::from(DATABASE_DIR),
    ]);
    let mut files = source_tree.files;
    files.remove(Path::new(PROJECT_METADATA_FILE));
    files.remove(Path::new(GLOBAL_VARIABLES_FILE));
    files.retain(|path, _| !path.starts_with(WORKSHEETS_DIR));
    files.insert(
        PathBuf::from(PROJECT_METADATA_FILE),
        crate::project::serialize_project_manifest(authority).map_err(prepare_error)?,
    );
    files.insert(
        PathBuf::from(GLOBAL_VARIABLES_FILE),
        crate::project::serialize_global_variables(authority).map_err(prepare_error)?,
    );
    for graph_path in authority.graphs.keys() {
        let (path, contents) = crate::project::serialize_graph_document(authority, graph_path)
            .map_err(prepare_error)?;
        files.insert(path, contents);
    }
    for (worksheet_path, worksheet) in &authority.worksheets {
        let (path, contents) = crate::project::serialize_worksheet(worksheet_path, worksheet)
            .map_err(prepare_error)?;
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
        return serde_json::from_slice::<crate::project::ProjectManifest>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yssbi-event" | "yssbi-function") => {
            serde_json::from_slice::<crate::project::GraphDocument>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some("yssbi-vars") => {
            serde_json::from_slice::<crate::project::GlobalVariablesDocument>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some("yssbi-worksheet") => {
            serde_json::from_slice::<crate::project::WorksheetDocument>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
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

#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use crate::graph::document::{
        GraphDocumentOperation, GraphDocumentPatch, MutationRequest, ResourceKey,
    };
    use crate::graph_document::GraphResourcePath;
    use crate::graph_document::{DocumentNode, NodeId, NodePosition, ParameterValues};
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, ProjectData, ProjectFilesystemFaultPoint,
        fixtures, load_project_from_file,
    };
    use crate::project::{OperationId, ResourceRevision};
    use std::time::Duration;
    use yss_data_contract::{DataType, DataValue};
    use yss_graph_protocol::NodeTypeId;
    use yss_variable_contract::VariableScope;

    fn root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "yssbi-project-lifecycle-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    #[cfg(windows)]
    fn link_directory(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_string_lossy().as_ref(),
                target.to_string_lossy().as_ref(),
            ])
            .status()
            .unwrap();
        assert!(status.success(), "failed to create test junction");
    }

    #[cfg(unix)]
    fn link_directory(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    fn active_state(label: &str) -> (ProjectState, PathBuf, ProjectInstanceId) {
        let root = root(label);
        std::fs::create_dir_all(&root).unwrap();
        let mut data = ProjectData::new();
        data.metadata.project_name = label.into();
        fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
        let instance_id = state.capture_project_session().unwrap().instance_id;
        (state, root, instance_id)
    }

    fn record_graph_transaction(state: &ProjectState, graph_path: &GraphResourcePath) {
        let node = DocumentNode {
            id: NodeId::new(),
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        state
            .apply_graph_patch(
                graph_path,
                MutationRequest::new(
                    ResourceKey::Graph(graph_path.clone()),
                    ResourceRevision::INITIAL,
                    OperationId::new(),
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node }]),
                ),
            )
            .unwrap();
    }

    fn assert_lifecycle_unload_snapshot(
        state: &ProjectState,
        graph_path: &GraphResourcePath,
        before_data: &serde_json::Value,
        before_history: crate::project::HistoryStatusDto,
        before_lengths: (usize, usize),
        before_head: Option<crate::project::HistoryEntryId>,
        before_revisions: &(
            std::collections::HashMap<GraphResourcePath, crate::graph_document::GraphRevision>,
            std::collections::HashMap<yss_variable_contract::VariableId, ResourceRevision>,
            std::collections::HashMap<crate::project::WorksheetResourcePath, ResourceRevision>,
        ),
        before_generation: u64,
    ) {
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            *before_data
        );
        assert!(state.get_data().unwrap().graphs.contains_key(graph_path));
        assert_eq!(state.history_status(), before_history);
        assert_eq!(state.history_lengths_for_test(), before_lengths);
        assert_eq!(state.history_head_id_for_test(true), before_head);
        assert_eq!(&state.revision_state_for_test(), before_revisions);
        assert_eq!(state.authority_generation_for_test(), before_generation);
    }

    #[test]
    fn lifecycle_unload_retains_exact_graph_revision() {
        let (state, root, instance_id) = active_state("unload-revision");
        let graph_path = state
            .create_graph_resource_fixture("Lifecycle Revision", GraphDocumentKind::Event)
            .unwrap();
        state
            .load_graph_projection(&instance_id, &graph_path, 1, "en-US")
            .unwrap();

        let node = DocumentNode {
            id: NodeId::new(),
            node_type: NodeTypeId::new("yssbi.test.reference").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        state
            .apply_graph_patch(
                &graph_path,
                MutationRequest::new(
                    ResourceKey::Graph(graph_path.clone()),
                    ResourceRevision::INITIAL,
                    OperationId::new(),
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node }]),
                ),
            )
            .unwrap();

        assert!(
            state
                .unload_graph_resource_for_lifecycle(&instance_id, &graph_path, 2)
                .unwrap()
        );
        assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
        assert_eq!(
            state.revision_state_for_test().0.get(&graph_path),
            Some(&crate::graph_document::GraphRevision::new(1))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lifecycle_graph_cache_unload_preserves_complete_project_history() {
        let (state, root, instance_id) = active_state("history-unload");
        let unloaded = state
            .create_graph_resource_fixture("Unloaded", GraphDocumentKind::Event)
            .unwrap();
        let retained = state
            .create_graph_resource_fixture("Retained", GraphDocumentKind::Event)
            .unwrap();
        state
            .load_graph_projection(&instance_id, &unloaded, 1, "en-US")
            .unwrap();
        state
            .load_graph_projection(&instance_id, &retained, 1, "en-US")
            .unwrap();
        let local_variable = state
            .add_variable(
                "Unloaded local",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Event {
                    event_path: unloaded.as_str().into(),
                },
                Vec::new(),
            )
            .unwrap();
        record_graph_transaction(&state, &unloaded);
        record_graph_transaction(&state, &retained);
        fixtures::write_state_graph(&state, &unloaded).unwrap();
        state.graph_projection(&unloaded, "en-US").unwrap();
        state.graph_projection(&retained, "en-US").unwrap();
        let coordinator = state.compile_coordinator.read().unwrap().clone();
        let document_path = unloaded.clone();
        let retained_document_path = retained.clone();
        assert!(coordinator.contains_slot_for_test(&document_path));
        assert!(coordinator.contains_slot_for_test(&retained_document_path));
        let before_status = state.history_status();
        let before_lengths = state.history_lengths_for_test();
        let before_head = state.history_head_id_for_test(true);
        let before_revisions = state.revision_state_for_test();
        let before_generation = state.authority_generation_for_test();
        assert_eq!(before_lengths, (2, 0));

        assert!(
            state
                .unload_graph_resource_for_lifecycle(&instance_id, &unloaded, 2)
                .unwrap()
        );

        let data = state.get_data().unwrap();
        assert!(!data.graphs.contains_key(&unloaded));
        assert!(data.graphs.contains_key(&retained));
        assert!(!data.variables.contains_key(&local_variable.id));
        assert_eq!(state.history_status(), before_status);
        assert_eq!(state.history_lengths_for_test(), before_lengths);
        assert_eq!(state.history_head_id_for_test(true), before_head);
        assert_eq!(state.revision_state_for_test(), before_revisions);
        assert_eq!(state.authority_generation_for_test(), before_generation + 1);
        assert!(!coordinator.contains_slot_for_test(&document_path));
        assert!(coordinator.contains_slot_for_test(&retained_document_path));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_lifecycle_unload_token_preserves_history_and_residency() {
        let (state, root, instance_id) = active_state("stale-unload-token");
        let unloaded = state
            .create_graph_resource_fixture("Unloaded", GraphDocumentKind::Event)
            .unwrap();
        let retained = state
            .create_graph_resource_fixture("Retained", GraphDocumentKind::Event)
            .unwrap();
        state
            .load_graph_projection(&instance_id, &unloaded, 1, "en-US")
            .unwrap();
        state
            .load_graph_projection(&instance_id, &retained, 1, "en-US")
            .unwrap();
        state
            .load_graph_projection(&instance_id, &unloaded, 3, "en-US")
            .unwrap();
        record_graph_transaction(&state, &unloaded);
        record_graph_transaction(&state, &retained);
        let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let before_history = state.history_status();
        let before_lengths = state.history_lengths_for_test();
        let before_head = state.history_head_id_for_test(true);
        let before_revisions = state.revision_state_for_test();
        let before_generation = state.authority_generation_for_test();

        let error = state
            .unload_graph_resource_for_lifecycle(&instance_id, &unloaded, 2)
            .unwrap_err();

        assert_eq!(error.code(), "stale_resource_lifecycle");
        assert_lifecycle_unload_snapshot(
            &state,
            &unloaded,
            &before_data,
            before_history,
            before_lengths,
            before_head,
            &before_revisions,
            before_generation,
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_project_lifecycle_unload_preserves_history_and_residency() {
        let (state, root, stale_instance_id) = active_state("stale-unload-project");
        let unloaded = GraphResourcePath::new("events/Unloaded.yssbi-event").unwrap();
        let retained = GraphResourcePath::new("events/Retained.yssbi-event").unwrap();
        let mut replacement = ProjectData::new();
        replacement.graphs.insert(
            unloaded.clone(),
            GraphResourceDocument::new("Unloaded", GraphDocumentKind::Event),
        );
        replacement.graphs.insert(
            retained.clone(),
            GraphResourceDocument::new("Retained", GraphDocumentKind::Event),
        );
        state.activate_project_fixture(root.to_string_lossy().into_owned(), replacement);
        let current_instance_id = state.capture_project_session().unwrap().instance_id;
        assert_ne!(current_instance_id, stale_instance_id);
        record_graph_transaction(&state, &unloaded);
        record_graph_transaction(&state, &retained);
        let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let before_history = state.history_status();
        let before_lengths = state.history_lengths_for_test();
        let before_head = state.history_head_id_for_test(true);
        let before_revisions = state.revision_state_for_test();
        let before_generation = state.authority_generation_for_test();

        let error = state
            .unload_graph_resource_for_lifecycle(&stale_instance_id, &unloaded, 2)
            .unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_lifecycle_unload_snapshot(
            &state,
            &unloaded,
            &before_data,
            before_history,
            before_lengths,
            before_head,
            &before_revisions,
            before_generation,
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn save_as_rejects_redirect_root_before_target_write() {
        let (state, source, instance_id) = active_state("save-as-redirect-source");
        let target = root("save-as-redirect-target");
        let redirect = root("save-as-redirect-link");
        std::fs::create_dir_all(&target).unwrap();
        link_directory(&redirect, &target);

        let error = state
            .save_project_as_transaction(&instance_id, &redirect, OperationId::new())
            .unwrap_err();

        assert_eq!(error.code(), "invalid_project_root");
        assert!(std::fs::read_dir(&target).unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(redirect);
        let _ = std::fs::remove_dir_all(target);
    }

    #[test]
    fn create_project_rejects_redirect_root_before_target_write() {
        let state = ProjectState::new();
        let target = root("create-redirect-target");
        let redirect = root("create-redirect-link");
        std::fs::create_dir_all(&target).unwrap();
        link_directory(&redirect, &target);

        let error = state
            .create_project_transaction("Redirected", &redirect, OperationId::new())
            .unwrap_err();

        assert_eq!(error.code(), "invalid_project_root");
        assert!(std::fs::read_dir(&target).unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(redirect);
        let _ = std::fs::remove_dir_all(target);
    }

    #[test]
    fn delete_project_rejects_redirect_root_before_target_recycle() {
        let target = root("delete-redirect-target");
        let redirect = root("delete-redirect-link");
        std::fs::create_dir_all(&target).unwrap();
        fixtures::write_project(&ProjectData::new(), target.to_string_lossy().as_ref()).unwrap();
        link_directory(&redirect, &target);
        let state = ProjectState::new();

        let error = state
            .delete_project_transaction(&redirect, None, None)
            .unwrap_err();

        assert_eq!(error.code(), "invalid_project_root");
        assert!(target.join(crate::project::PROJECT_METADATA_FILE).is_file());
        let _ = std::fs::remove_dir_all(redirect);
        let _ = std::fs::remove_dir_all(target);
    }

    #[test]
    fn inactive_registered_identity_cannot_delete_same_path_replacement() {
        let project_root = root("inactive-replacement");
        std::fs::create_dir_all(&project_root).unwrap();
        fixtures::write_project(&ProjectData::new(), project_root.to_string_lossy().as_ref())
            .unwrap();
        let registered_identity = ProjectRootBinding::for_existing(&project_root)
            .unwrap()
            .identity()
            .unwrap()
            .clone();
        std::fs::remove_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(&project_root).unwrap();
        fixtures::write_project(&ProjectData::new(), project_root.to_string_lossy().as_ref())
            .unwrap();

        let error = ProjectState::new()
            .delete_project_transaction(&project_root, Some(&registered_identity), None)
            .unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert!(project_root.join(PROJECT_METADATA_FILE).is_file());
        let _ = std::fs::remove_dir_all(project_root);
    }

    #[test]
    fn save_as_builds_destination_from_one_authoritative_snapshot_and_publishes_after_commit() {
        let (state, source, instance_id) = active_state("save-as-authority");
        let graph_path = GraphResourcePath::new("events/Authority.yssbi-event").unwrap();
        let mut authority = state.get_data().unwrap();
        authority.metadata.project_name = "Authoritative Copy".into();
        authority.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Authority", GraphDocumentKind::Event),
        );
        let variable = yss_variable_contract::VariableInstance {
            id: yss_variable_contract::VariableId::new(),
            name: "authoritative_global".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(42),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: Vec::new(),
        };
        authority.variables.insert(variable.id, variable);
        *state.project_data.write().unwrap() = authority;
        let destination = root("save-as-authority-destination");

        let prepared = state
            .save_project_as_transaction(&instance_id, &destination, OperationId::new())
            .unwrap();

        assert_eq!(
            state.capture_project_session().unwrap().instance_id,
            instance_id,
            "save-as must not publish before destination commit completes"
        );
        let committed =
            load_project_from_file(prepared.metadata_path.to_string_lossy().as_ref()).unwrap();
        assert_eq!(committed.metadata.project_name, "Authoritative Copy");
        assert!(
            committed
                .variables
                .values()
                .any(|value| value.name == "authoritative_global")
        );
        assert!(destination.join(graph_path.as_str()).is_file());
        state
            .activate_prepared_project(prepared.prepared_activation)
            .unwrap();
        assert_eq!(
            state.capture_project_session().unwrap().root,
            NormalizedProjectRoot::from_project_path(&destination).unwrap()
        );
        let source_disk = load_project_from_file(source.to_string_lossy().as_ref()).unwrap();
        assert_eq!(source_disk.metadata.project_name, "save-as-authority");
        assert!(
            source_disk.variables.is_empty(),
            "save-as flushed its source"
        );
        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(destination);
    }

    #[test]
    fn failed_save_as_leaves_source_session_and_destination_unchanged() {
        let (state, source, instance_id) = active_state("save-as-failure");
        let destination = root("save-as-failure-destination");
        std::fs::create_dir_all(&destination).unwrap();
        let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let before_path = state.get_path();
        state.set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::StagedSerialization));

        let result =
            state.save_project_as_transaction(&instance_id, &destination, OperationId::new());

        state.set_project_filesystem_fault(None);
        assert!(result.is_err());
        assert_eq!(state.get_path(), before_path);
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            before_data
        );
        assert!(std::fs::read_dir(&destination).unwrap().next().is_none());
        assert_eq!(
            state.capture_project_session().unwrap().instance_id,
            instance_id
        );
        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(destination);
    }

    #[test]
    fn registered_project_deletion_excludes_index_load_save_rename_and_worksheet_operations() {
        let (state, root, instance_id) = active_state("delete-exclusion");
        let load_path = GraphResourcePath::new("events/Load.yssbi-event").unwrap();
        let rename_path = GraphResourcePath::new("events/Rename.yssbi-event").unwrap();
        state
            .insert_graph(
                load_path.clone(),
                GraphResourceDocument::new("Load", GraphDocumentKind::Event),
            )
            .unwrap();
        state
            .insert_graph(
                rename_path.clone(),
                GraphResourceDocument::new("Rename", GraphDocumentKind::Event),
            )
            .unwrap();
        let (worksheet_path, worksheet) = fixtures::worksheet("Sheet", "database");
        state
            .project_data
            .write()
            .unwrap()
            .worksheets
            .insert(worksheet_path.clone(), worksheet.clone());
        state.initialize_worksheet_revision_for_test(&worksheet_path);
        let data = state.get_data().unwrap();
        fixtures::write_graph(&data, root.to_string_lossy().as_ref(), &load_path).unwrap();
        fixtures::write_graph(&data, root.to_string_lossy().as_ref(), &rename_path).unwrap();
        fixtures::write_worksheet(&root, &worksheet_path, &worksheet).unwrap();
        state
            .unload_graph_resource_for_lifecycle(&instance_id, &load_path, 1)
            .unwrap();

        let destination = std::env::temp_dir().join(format!(
            "yssbi-project-lifecycle-delete-exclusion-copy-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&destination).unwrap();
        let destination_root = NormalizedProjectRoot::from_project_path(&destination).unwrap();
        let held_destination = state
            .filesystem()
            .acquire(destination_root.clone())
            .unwrap();
        let ordinary_waiting = state.filesystem().observe_next_wait();
        let ordinary_coordinator = state.filesystem().clone();
        let source_root = NormalizedProjectRoot::from_project_path(&root).unwrap();
        let source_root_for_diagnostics = source_root.clone();
        let (ordinary_done_tx, ordinary_done_rx) = std::sync::mpsc::channel();
        let ordinary = std::thread::spawn(move || {
            let lease = ordinary_coordinator
                .acquire_many([source_root, destination_root])
                .unwrap();
            ordinary_done_tx.send(()).unwrap();
            drop(lease);
        });
        ordinary_waiting
            .recv_timeout(Duration::from_secs(2))
            .unwrap();

        let deletion_draining = state.filesystem().observe_next_lifecycle_drain();
        let delete_state = state.clone();
        let delete_root = root.clone();
        let delete_instance = instance_id.clone();
        let (delete_done_tx, delete_done_rx) = std::sync::mpsc::channel();
        let deletion = std::thread::spawn(move || {
            let result =
                delete_state.delete_project_transaction(&delete_root, None, Some(&delete_instance));
            delete_done_tx.send(result).unwrap();
        });
        if deletion_draining
            .recv_timeout(Duration::from_secs(2))
            .is_err()
        {
            if let Ok(result) = delete_done_rx.try_recv() {
                panic!("deletion returned before drain checkpoint: {result:?}");
            }
            panic!(
                "deletion blocked before drain checkpoint: {:?}",
                state
                    .filesystem()
                    .lifecycle_state_for_test(&source_root_for_diagnostics)
            );
        }

        let assert_rejected = |result: Result<(), ProjectFilesystemError>| {
            assert_eq!(
                result.unwrap_err().code(),
                "project_lifecycle_admission_closed"
            );
        };
        assert_rejected(state.read_project_index(&instance_id).map(|_| ()));
        assert_rejected(
            state
                .load_graph_projection(&instance_id, &load_path, 2, "en-US")
                .map(|_| ()),
        );
        assert_rejected(
            state
                .flush_project_documents(&instance_id, OperationId::new())
                .map(|_| ()),
        );
        assert_rejected(
            state
                .rename_graph_resource_transaction(
                    &instance_id,
                    &rename_path,
                    crate::project::ResourceRevision::INITIAL,
                    "Renamed",
                    1,
                    OperationId::new(),
                )
                .map(|_| ()),
        );
        assert_rejected(
            state
                .save_worksheet_document(
                    &instance_id,
                    &worksheet_path,
                    crate::project::ResourceRevision::INITIAL,
                    OperationId::new(),
                    worksheet,
                )
                .map(|_| ()),
        );

        assert!(!deletion.is_finished());
        drop(held_destination);
        ordinary_done_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        ordinary.join().unwrap();
        let result = delete_done_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        deletion.join().unwrap();
        assert_eq!(result.cleared_project_instance_id, Some(instance_id));
        assert!(!root.exists());
        let _ = std::fs::remove_dir_all(destination);
    }

    #[test]
    fn deletion_commits_without_retaining_local_cleanup_artifact() {
        let (state, project_root, instance_id) = active_state("delete-recycle-commit");
        let identity = ProjectRootBinding::for_existing(&project_root)
            .unwrap()
            .identity()
            .unwrap()
            .clone();

        let result = state
            .delete_project_transaction(&project_root, Some(&identity), Some(&instance_id))
            .unwrap();

        assert_eq!(result.cleared_project_instance_id, Some(instance_id));
        assert!(!project_root.exists());
        assert!(state.capture_project_session().is_err());
    }

    #[test]
    fn stale_active_identity_cannot_delete_replacement_project() {
        let (state, root, stale_instance_id) = active_state("delete-stale");
        let mut replacement = ProjectData::new();
        replacement.metadata.project_name = "Replacement".into();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), replacement);
        let replacement_session = state.capture_project_session().unwrap();

        let error = state
            .delete_project_transaction(&root, None, Some(&stale_instance_id))
            .unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        assert!(root.join(crate::project::PROJECT_METADATA_FILE).is_file());
        assert_eq!(
            state.capture_project_session().unwrap().instance_id,
            replacement_session.instance_id
        );
        assert_eq!(
            state.get_data().unwrap().metadata.project_name,
            "Replacement"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
