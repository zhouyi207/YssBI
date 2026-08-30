use crate::project::resource_patch::ResourceDocumentPatch;
use crate::project::{
    GraphDocument, ProjectData, ProjectFilesystemError, ProjectFilesystemTransaction,
    ProjectSession, ProjectState, ProjectTransactionContext, StagedFilesystemMutation,
};
#[cfg(test)]
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use yss_data_contract::{DataType, DataValue};
use yss_graph_document::GraphResourcePath;
use yss_project_history::{
    FunctionResourceKey, HistoryStatusDto, ResourceDeltaEvent, ResourceKey, ResourceLifecycleKind,
    ResourceLifecyclePatch, ResourceLifecycleState, ResourcePathMovePatch, VariableResourceKey,
    WorksheetDocumentPatch, WorksheetDocumentState, WorksheetResourceKey,
};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};
use yss_project_layout::{PROJECT_METADATA_FILE, WORKSHEET_EXTENSION};
use yss_project_manifest::ProjectManifest;
use yss_resource_naming::{ResourceName, allocate_unique_resource_name};
use yss_variable_contract::{VariableId, VariableInstance, VariableScope};
use yss_variable_value::default_value_for;
use yss_worksheet_document::{WorksheetDocument, WorksheetResourcePath};

#[path = "project_writers/graph.rs"]
mod graph;
#[path = "project_writers/variables.rs"]
mod variables;
#[path = "project_writers/worksheets.rs"]
mod worksheets;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectSaveResult {
    pub(crate) project_instance_id: ProjectInstanceId,
    pub(crate) operation_id: OperationId,
    pub(crate) publication_revision: u64,
    pub(crate) affected_resources: Box<[ResourceKey]>,
    pub(crate) index_invalidated: bool,
    pub(crate) history: ProjectHistoryStatus,
}

impl ProjectSaveResult {
    pub(crate) fn into_parts(
        self,
    ) -> (
        ProjectInstanceId,
        OperationId,
        u64,
        Box<[ResourceKey]>,
        bool,
        ProjectHistoryStatus,
    ) {
        (
            self.project_instance_id,
            self.operation_id,
            self.publication_revision,
            self.affected_resources,
            self.index_invalidated,
            self.history,
        )
    }
}

#[cfg(test)]
pub(crate) use crate::schema::project::ProjectSaveResultDto;

pub struct GlobalVariableMutationResult {
    pub variable: VariableInstance,
    pub(crate) mutation: ProjectResourceMutationFacts,
    #[cfg(test)]
    pub result: crate::schema::application_event::ResourceMutationResultDto,
}

#[derive(Debug)]
pub(crate) struct ProjectVariableMutationReceipt {
    variable: VariableInstance,
    mutation: ProjectResourceMutationFacts,
}

impl ProjectVariableMutationReceipt {
    pub(crate) fn into_parts(self) -> (VariableInstance, ProjectResourceMutationFacts) {
        (self.variable, self.mutation)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectResourceMutationFacts {
    operation_id: OperationId,
    project_instance_id: ProjectInstanceId,
    publication_revision: u64,
    moves: Box<[ProjectResourceMove]>,
    deltas: Box<[yss_project_history::ResourceDeltaEvent]>,
    projection_status: ProjectProjectionStatus,
    history: ProjectHistoryStatus,
}

impl ProjectResourceMutationFacts {
    pub(crate) fn new(
        operation_id: OperationId,
        project_instance_id: ProjectInstanceId,
        publication_revision: u64,
        moves: impl Into<Box<[ProjectResourceMove]>>,
        deltas: impl Into<Box<[yss_project_history::ResourceDeltaEvent]>>,
        projection_status: ProjectProjectionStatus,
        history: ProjectHistoryStatus,
    ) -> Self {
        Self {
            operation_id,
            project_instance_id,
            publication_revision,
            moves: moves.into(),
            deltas: deltas.into(),
            projection_status,
            history,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectResourceMove {
    pub(crate) from: Box<str>,
    pub(crate) to: Box<str>,
    pub(crate) kind: yss_project_history::ResourceLifecycleKind,
    pub(crate) name: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectProjectionStatus {
    Complete {
        expected_graph_paths: Box<[GraphResourcePath]>,
    },
    Incomplete {
        invalidated_graph_paths: Box<[GraphResourcePath]>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProjectHistoryStatus {
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
}

impl ProjectResourceMutationFacts {
    pub(crate) fn into_parts(
        self,
    ) -> (
        OperationId,
        ProjectInstanceId,
        u64,
        Box<[ProjectResourceMove]>,
        Box<[yss_project_history::ResourceDeltaEvent]>,
        ProjectProjectionStatus,
        ProjectHistoryStatus,
    ) {
        (
            self.operation_id,
            self.project_instance_id,
            self.publication_revision,
            self.moves,
            self.deltas,
            self.projection_status,
            self.history,
        )
    }

    #[cfg(test)]
    pub(crate) fn into_transport(
        self,
    ) -> crate::schema::application_event::ResourceMutationResultDto {
        crate::schema::application_event::ResourceMutationResultDto {
            operation_id: self.operation_id,
            project_instance_id: self.project_instance_id.to_string(),
            publication_revision: self.publication_revision,
            moves: self
                .moves
                .into_vec()
                .into_iter()
                .map(|value| crate::schema::application_event::ResourceMoveDto {
                    from: value.from.to_string(),
                    to: value.to.to_string(),
                    kind: value.kind,
                    name: value.name.to_string(),
                })
                .collect(),
            deltas: self.deltas.into_vec(),
            projection_replacements: Vec::new(),
            projection_status: match self.projection_status {
                ProjectProjectionStatus::Complete {
                    expected_graph_paths,
                } => crate::schema::application_event::ProjectionStatusDto::Complete {
                    expected_graph_paths: expected_graph_paths
                        .into_vec()
                        .into_iter()
                        .map(|path| path.as_str().to_owned())
                        .collect(),
                },
                ProjectProjectionStatus::Incomplete {
                    invalidated_graph_paths,
                } => crate::schema::application_event::ProjectionStatusDto::Incomplete {
                    invalidated_graph_paths: invalidated_graph_paths
                        .into_vec()
                        .into_iter()
                        .map(|path| path.as_str().to_owned())
                        .collect(),
                },
            },
            history: yss_project_history::HistoryStatusDto {
                can_undo: self.history.can_undo,
                can_redo: self.history.can_redo,
            },
        }
    }
}

impl GlobalVariableMutationResult {
    pub(crate) fn into_application_receipt(
        self,
    ) -> Result<ProjectVariableMutationReceipt, ProjectFilesystemError> {
        Ok(ProjectVariableMutationReceipt {
            variable: self.variable,
            mutation: self.mutation,
        })
    }
}

fn worksheet_document_state(
    document: &WorksheetDocument,
) -> yss_project_history::WorksheetDocumentState {
    yss_project_history::WorksheetDocumentState {
        database_id: document.database_id.clone(),
        chart_type: document.chart_type.clone(),
        encodings: document.encodings.clone(),
    }
}

fn worksheet_lifecycle_state(
    path: &WorksheetResourcePath,
    revision: ResourceRevision,
) -> yss_project_history::ResourceLifecycleState {
    yss_project_history::ResourceLifecycleState {
        revision,
        path: path.as_str().into(),
        kind: yss_project_history::ResourceLifecycleKind::Worksheet,
        name: path.display_name().as_str().to_string(),
    }
}

fn worksheet_move_delta(
    from: &WorksheetResourcePath,
    to: &WorksheetResourcePath,
    operation_id: OperationId,
    from_revision: ResourceRevision,
    to_revision: ResourceRevision,
) -> yss_project_history::ResourceDeltaEvent {
    yss_project_history::ResourceDeltaEvent {
        resource: worksheet_key(to),
        from_revision,
        to_revision,
        caused_by: Some(operation_id),
        payload: yss_project_history::ResourceDocumentPatch::ResourceMove(
            yss_project_history::ResourcePathMovePatch {
                from: from.as_str().into(),
                to: to.as_str().into(),
            },
        ),
    }
}

fn worksheet_resource_delta(
    path: &WorksheetResourcePath,
    operation_id: OperationId,
    retained_revision: Option<ResourceRevision>,
    before: Option<&WorksheetDocument>,
    after: Option<&WorksheetDocument>,
) -> Result<yss_project_history::ResourceDeltaEvent, ProjectFilesystemError> {
    let (from_revision, to_revision, payload) = match (before, after) {
        (Some(before), Some(after)) => (
            before.revision,
            after.revision,
            yss_project_history::ResourceDocumentPatch::Worksheet(
                yss_project_history::WorksheetDocumentPatch {
                    before: worksheet_document_state(before),
                    after: worksheet_document_state(after),
                },
            ),
        ),
        (None, Some(after)) => (
            retained_revision.unwrap_or(after.revision),
            after.revision,
            yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                yss_project_history::ResourceLifecyclePatch {
                    before: None,
                    after: Some(worksheet_lifecycle_state(path, after.revision)),
                },
            ),
        ),
        (Some(before), None) => (
            before.revision,
            super::project_state::checked_resource_revision(path.as_str(), before.revision)?,
            yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                yss_project_history::ResourceLifecyclePatch {
                    before: Some(worksheet_lifecycle_state(path, before.revision)),
                    after: None,
                },
            ),
        ),
        (None, None) => unreachable!("a worksheet resource delta must change a document"),
    };
    Ok(yss_project_history::ResourceDeltaEvent {
        resource: worksheet_key(path),
        from_revision,
        to_revision,
        caused_by: Some(operation_id),
        payload,
    })
}

#[cfg(test)]
static WRITER_SNAPSHOT_TEST_HOOK: std::sync::Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
fn set_writer_snapshot_test_hook(hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
    *WRITER_SNAPSHOT_TEST_HOOK.lock().unwrap() = hook;
}

struct WriterSnapshot {
    session: ProjectSession,
    data: ProjectData,
    variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        crate::project::project_state::VariableRevisionEntry,
    >,
    authority_generation: u64,
}

enum GlobalVariableMutation {
    Create {
        scope: VariableScope,
        name: String,
        data_type: DataType,
        data_value: DataValue,
        description: String,
        tags: Vec<String>,
    },
    Update {
        id: VariableId,
        expected_revision: ResourceRevision,
        name: Option<String>,
        data_type: Option<DataType>,
        data_value: Option<DataValue>,
        description: Option<String>,
        tags: Option<Vec<String>>,
    },
    Delete {
        id: VariableId,
        expected_revision: ResourceRevision,
    },
}

enum StagedGlobalVariableMutation {
    Create {
        variable: VariableInstance,
        history_patch: yss_project_history::ResourcePatch,
    },
    Update {
        variable: VariableInstance,
        expected_revision: ResourceRevision,
        history_patch: yss_project_history::ResourcePatch,
    },
    Delete {
        variable: VariableInstance,
        expected_revision: ResourceRevision,
        history_patch: yss_project_history::ResourcePatch,
    },
}

impl StagedGlobalVariableMutation {
    fn variable(&self) -> &VariableInstance {
        match self {
            Self::Create { variable, .. }
            | Self::Update { variable, .. }
            | Self::Delete { variable, .. } => variable,
        }
    }

    fn expected_revision(&self) -> Option<ResourceRevision> {
        match self {
            Self::Create { .. } => None,
            Self::Update {
                expected_revision, ..
            }
            | Self::Delete {
                expected_revision, ..
            } => Some(*expected_revision),
        }
    }

    fn history_patch(&self) -> &yss_project_history::ResourcePatch {
        match self {
            Self::Create { history_patch, .. }
            | Self::Update { history_patch, .. }
            | Self::Delete { history_patch, .. } => history_patch,
        }
    }

    fn is_create(&self) -> bool {
        matches!(self, Self::Create { .. })
    }

    fn is_delete(&self) -> bool {
        matches!(self, Self::Delete { .. })
    }

    fn into_variable(self) -> VariableInstance {
        match self {
            Self::Create { variable, .. }
            | Self::Update { variable, .. }
            | Self::Delete { variable, .. } => variable,
        }
    }
}

struct CommittedProjectSave {
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    publication_revision: u64,
    affected_resources: Vec<ResourceKey>,
    history: ProjectHistoryStatus,
}

impl CommittedProjectSave {
    fn complete(self) -> ProjectSaveResult {
        ProjectSaveResult {
            project_instance_id: self.project_instance_id,
            operation_id: self.operation_id,
            publication_revision: self.publication_revision,
            affected_resources: self.affected_resources.into_boxed_slice(),
            index_invalidated: true,
            history: self.history,
        }
    }
}

fn graph_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Graph(path.clone())
}

fn function_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Function(FunctionResourceKey(path.as_str().into()))
}

fn variable_key(id: &yss_variable_contract::VariableId) -> ResourceKey {
    ResourceKey::Variable(VariableResourceKey(format!("variables/{id}").into()))
}

fn worksheet_key(path: &WorksheetResourcePath) -> ResourceKey {
    ResourceKey::Worksheet(WorksheetResourceKey(path.as_str().into()))
}

fn context(
    state: &ProjectState,
    session: ProjectSession,
    operation_id: OperationId,
    expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
    expected_absent_resources: BTreeSet<ResourceKey>,
) -> ProjectTransactionContext {
    ProjectTransactionContext {
        affected_resources: expected_revisions.keys().cloned().collect(),
        session,
        operation_id,
        expected_revisions,
        expected_absent_resources,
        recovery_marker: Some(state.project_recovery_marker()),
    }
}

fn prepare_error(error: impl ToString) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

fn validate_document(path: &Path, contents: &[u8]) -> Result<(), String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yssbi-event" | "yssbi-function") => serde_json::from_slice::<GraphDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Some("yssbi-vars") => {
            serde_json::from_slice::<crate::project::GlobalVariablesDocument>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some(WORKSHEET_EXTENSION) => serde_json::from_slice::<WorksheetDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        _ if path == Path::new(PROJECT_METADATA_FILE) => {
            serde_json::from_slice::<ProjectManifest>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        _ => Err(format!(
            "unsupported project document target '{}'",
            path.display()
        )),
    }
}

impl ProjectState {
    fn capture_writer_snapshot(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<WriterSnapshot, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "writer project instance is stale".into(),
            });
        }
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed during writer snapshot".into(),
            });
        }
        let data = self.project_data.read().unwrap().clone();
        let variable_revisions = self.variable_revisions.read().unwrap().clone();
        let snapshot = WriterSnapshot {
            session,
            data,
            variable_revisions,
            authority_generation: publication.authority_generation(),
        };
        drop(publication);
        #[cfg(test)]
        if let Some(hook) = WRITER_SNAPSHOT_TEST_HOOK.lock().unwrap().clone() {
            hook();
        }
        Ok(snapshot)
    }

    fn validate_writer_context(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
    ) -> Result<(), ProjectFilesystemError> {
        self.validate_project_session(&context.session)?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != context.session.instance_id.as_str()
            || publication.authority_generation() != authority_generation
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project authority changed while writer was waiting".into(),
            });
        }
        let data = self.project_data.read().unwrap();
        let graph_revisions = self.graph_revisions.read().unwrap();
        let variable_revisions = self.variable_revisions.read().unwrap();
        let worksheet_revisions = self.worksheet_revisions.read().unwrap();
        super::project_state::validate_context_revisions(
            context,
            &data,
            &graph_revisions,
            &variable_revisions,
            &worksheet_revisions,
        )
    }

    fn publish_project_save(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
    ) -> Result<CommittedProjectSave, ProjectFilesystemError> {
        self.validate_writer_context(context, authority_generation)?;
        let publication = self.mutation_publication.lock().unwrap();
        let history = self.history.read().unwrap().status();
        Ok(CommittedProjectSave {
            project_instance_id: ProjectInstanceId::from_existing(
                publication.project_instance_id.clone(),
            ),
            operation_id: context.operation_id,
            publication_revision: publication.resource_revision,
            affected_resources: context.affected_resources.clone(),
            history: ProjectHistoryStatus {
                can_undo: history.can_undo,
                can_redo: history.can_redo,
            },
        })
    }

    fn execute_save(
        &self,
        snapshot: &WriterSnapshot,
        context: ProjectTransactionContext,
        mutations: Vec<StagedFilesystemMutation>,
    ) -> Result<ProjectSaveResult, ProjectFilesystemError> {
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            mutations,
            validate_document,
        )?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        let receipt = match self.publish_project_save(&context, snapshot.authority_generation) {
            Ok(receipt) => receipt,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        Ok(receipt.complete())
    }
}

#[cfg(all(test, any()))]
mod tests {
    use super::set_writer_snapshot_test_hook;
    use crate::project::{
        GraphDocument, GraphDocumentKind, GraphResourceDocument, ProjectData,
        ProjectFilesystemFaultPoint, ProjectState, fixtures,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use yss_data_contract::{DataType, DataValue};
    use yss_graph_document::GraphResourcePath;
    use yss_project_history::{FunctionResourceKey, ResourceKey, VariableResourceKey};
    use yss_project_identity::{OperationId, ResourceRevision};
    use yss_project_layout::{GLOBAL_VARIABLES_FILE, WORKSHEETS_DIR};
    use yss_resource_naming::ResourceName;
    use yss_variable_contract::VariableScope;

    fn worksheet_files(project: &TestProject) -> Vec<std::path::PathBuf> {
        let worksheets = project.root.join(WORKSHEETS_DIR);
        let Ok(entries) = std::fs::read_dir(worksheets) else {
            return Vec::new();
        };
        let mut paths = entries
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension().and_then(|value| value.to_str()) == Some(WORKSHEET_EXTENSION)
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn assert_two_distinct_worksheets_on_disk(
        project: &TestProject,
        first: &WorksheetResourcePath,
        second: &WorksheetResourcePath,
    ) {
        let files = worksheet_files(project);
        assert_eq!(
            files.len(),
            2,
            "each authoritative worksheet needs its own file"
        );
        assert!(project.root.join(first.relative_path()).is_file());
        assert!(project.root.join(second.relative_path()).is_file());
    }

    struct TestProject {
        root: std::path::PathBuf,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "yssbi-project-writer-{label}-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn active_state(project: &TestProject, data: ProjectData) -> ProjectState {
        let state = ProjectState::new();
        state.activate_project_fixture(project.root.to_string_lossy().into_owned(), data);
        state
    }

    fn graph_key(path: &GraphResourcePath) -> ResourceKey {
        ResourceKey::Graph(path.clone())
    }

    #[test]
    fn graph_save_revalidates_revision_after_waiting_for_rename() {
        let project = TestProject::new("graph-revision-wait");
        let path = GraphResourcePath::new("events/Before.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Before", GraphDocumentKind::Event),
        );
        let state = Arc::new(active_state(&project, data));
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let worker_state = Arc::clone(&state);
        let worker_path = path.clone();
        let worker_session = session.clone();
        let worker = std::thread::spawn(move || {
            worker_state.save_graph_document(
                &worker_session.instance_id,
                &worker_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
        });

        {
            let mut authority = state.project_data.write().unwrap();
            let graph = authority.graphs.get_mut(&path).unwrap();
            graph.name = "After".into();
            graph.document.revision = yss_graph_document::GraphRevision::new(1);
        }
        drop(lease);

        let error = worker.join().unwrap().unwrap_err();
        assert_eq!(error.code(), "resource_revision_conflict");
        assert!(!project.root.join(path.as_str()).exists());
    }

    #[test]
    fn flush_writes_one_coherent_authoritative_snapshot_without_recreating_removed_graphs() {
        let project = TestProject::new("coherent-flush");
        let loaded = GraphResourcePath::new("events/Loaded.yssbi-event").unwrap();
        let removed = GraphResourcePath::new("events/Removed.yssbi-event").unwrap();
        let unknown = project.root.join("events/Unknown.yssbi-event");
        std::fs::create_dir_all(unknown.parent().unwrap()).unwrap();
        std::fs::write(&unknown, b"unknown-resource").unwrap();

        let mut data = ProjectData::new();
        data.metadata.project_name = "coherent-authority".into();
        data.graphs.insert(
            loaded.clone(),
            GraphResourceDocument::new("Loaded", GraphDocumentKind::Event),
        );
        data.graphs.insert(
            removed.clone(),
            GraphResourceDocument::new("Removed", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_graph(
            &data,
            project.root.to_string_lossy().as_ref(),
            &removed,
        )
        .unwrap();
        let state = Arc::new(active_state(&project, data));
        let session = state.capture_project_session().unwrap();
        let (captured_tx, captured_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let resume_rx = std::sync::Mutex::new(resume_rx);
        set_writer_snapshot_test_hook(Some(Arc::new(move || {
            captured_tx.send(()).unwrap();
            resume_rx.lock().unwrap().recv().unwrap();
        })));
        let worker_state = Arc::clone(&state);
        let worker_instance_id = session.instance_id.clone();
        let worker = std::thread::spawn(move || {
            worker_state.flush_project_documents(&worker_instance_id, OperationId::new())
        });
        captured_rx.recv().unwrap();
        state.unload_graph_resource(&removed).unwrap();
        std::fs::remove_file(project.root.join(removed.as_str())).unwrap();
        resume_tx.send(()).unwrap();
        let stale_error = worker.join().unwrap().unwrap_err();
        set_writer_snapshot_test_hook(None);
        assert_eq!(stale_error.code(), "stale_project_lifecycle");

        let result = state
            .flush_project_documents(&session.instance_id, OperationId::new())
            .unwrap();

        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(project.root.join(PROJECT_METADATA_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["projectName"], "coherent-authority");
        assert!(project.root.join(loaded.as_str()).is_file());
        assert!(!project.root.join(removed.as_str()).exists());
        assert_eq!(std::fs::read(unknown).unwrap(), b"unknown-resource");
        assert_eq!(result.project_instance_id, session.instance_id.as_str());
    }

    #[test]
    fn global_variable_writer_cannot_be_overwritten_by_rename_rollback() {
        let project = TestProject::new("global-narrow-write");
        let metadata = project.root.join(PROJECT_METADATA_FILE);
        std::fs::write(&metadata, br#"{\"sentinel\":true}"#).unwrap();
        let graph_path = GraphResourcePath::new("events/Before.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Before", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_graph(
            &data,
            project.root.to_string_lossy().as_ref(),
            &graph_path,
        )
        .unwrap();
        let state = active_state(&project, data);
        let session = state.capture_project_session().unwrap();
        state
            .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));
        let rename_error = state
            .rename_graph_resource_fixture(session.instance_id.as_str(), &graph_path, "After")
            .unwrap_err();
        state.set_project_filesystem_fault(None);
        assert_eq!(rename_error.code(), "transaction_commit_failed");
        assert!(project.root.join(graph_path.as_str()).is_file());
        assert!(!project.root.join("events/After.yssbi-event").exists());

        let variable = state
            .add_variable(
                "global",
                DataType::Int64,
                DataValue::Int64(7),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        let key = ResourceKey::Variable(VariableResourceKey(
            format!("variables/{}", variable.id).into(),
        ));
        let result = state
            .persist_global_variables(
                &session.instance_id,
                BTreeMap::from([(key.clone(), ResourceRevision::INITIAL)]),
                OperationId::new(),
            )
            .unwrap();

        let globals: crate::project::GlobalVariablesDocument = serde_json::from_slice(
            &std::fs::read(project.root.join(GLOBAL_VARIABLES_FILE)).unwrap(),
        )
        .unwrap();
        let persisted = globals.variables.get(&variable.id).unwrap();
        assert_eq!(persisted.name, variable.name);
        assert_eq!(persisted.data_value, variable.data_value);
        assert_eq!(std::fs::read(metadata).unwrap(), br#"{\"sentinel\":true}"#);
        assert_eq!(result.affected_resources, vec![key]);
    }

    #[test]
    fn function_save_persists_signature_and_graph_at_one_revision() {
        let project = TestProject::new("function-revision");
        let path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();
        let mut function = GraphResourceDocument::new("Shared", GraphDocumentKind::Function);
        function.document.revision = yss_graph_document::GraphRevision::new(4);
        function.function.as_mut().unwrap().revision = ResourceRevision::new(4);
        let mut data = ProjectData::new();
        data.graphs.insert(path.clone(), function);
        let state = active_state(&project, data);
        let session = state.capture_project_session().unwrap();
        let result = state
            .save_graph_document(
                &session.instance_id,
                &path,
                ResourceRevision::new(4),
                OperationId::new(),
            )
            .unwrap();

        let persisted: GraphDocument =
            serde_json::from_slice(&std::fs::read(project.root.join(path.as_str())).unwrap())
                .unwrap();
        assert_eq!(persisted.revision, ResourceRevision::new(4));
        assert_eq!(
            persisted.function.unwrap().revision,
            ResourceRevision::new(4)
        );
        assert_eq!(
            result.affected_resources,
            vec![
                graph_key(&path),
                ResourceKey::Function(FunctionResourceKey(path.as_str().into()))
            ]
        );
    }

    #[test]
    fn worksheet_create_rechecks_unique_name_under_root_lease() {
        let project = TestProject::new("worksheet-name-wait");
        let state = Arc::new(active_state(&project, ProjectData::new()));
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let worker_state = Arc::clone(&state);
        let worker_session = session.clone();
        let worker = std::thread::spawn(move || {
            worker_state.create_worksheet_resource_transaction(
                &worker_session.instance_id,
                &ResourceName::parse("Analysis").unwrap(),
                None,
                OperationId::new(),
            )
        });

        let (existing_path, existing) = fixtures::worksheet("Analysis", "");
        state
            .project_data
            .write()
            .unwrap()
            .worksheets
            .insert(existing_path, existing);
        drop(lease);

        let created = worker.join().unwrap().unwrap();
        assert_eq!(
            worksheet_path_from_lifecycle_result(&created)
                .display_name()
                .as_str(),
            "Analysis 2"
        );
    }

    #[test]
    fn worksheet_create_duplicate_name_keeps_distinct_authority_and_disk_documents() {
        let project = TestProject::new("worksheet-create-duplicate");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let first = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let second = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let first_path = worksheet_path_from_lifecycle_result(&first);
        let second_path = worksheet_path_from_lifecycle_result(&second);

        assert_ne!(first_path, second_path);
        assert_two_distinct_worksheets_on_disk(&project, &first_path, &second_path);
    }

    #[test]
    fn worksheet_rename_moves_authority_file_revision_and_common_publication() {
        let project = TestProject::new("worksheet-rename-authority");
        let (source, document) = fixtures::worksheet("Report", "database");
        let target = WorksheetResourcePath::parse("worksheets/Renamed.yssbi-worksheet").unwrap();
        let mut data = ProjectData::new();
        data.worksheets.insert(source.clone(), document.clone());
        let state = active_state(&project, data);
        fixtures::write_worksheet(&project.root, &source, &document).unwrap();
        state.initialize_worksheet_revision_for_test(&source);
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();

        let result = state
            .rename_worksheet_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                &ResourceName::parse("Renamed").unwrap(),
                1,
                operation_id,
            )
            .unwrap();

        let authority = state.get_data().unwrap();
        assert!(!authority.worksheets.contains_key(&source));
        assert_eq!(
            authority.worksheets[&target].revision,
            ResourceRevision::new(1)
        );
        assert!(!project.root.join(source.relative_path()).exists());
        assert!(project.root.join(target.relative_path()).is_file());
        assert_eq!(result.moves.len(), 1);
        assert_eq!(result.moves[0].from, source.as_str());
        assert_eq!(result.moves[0].to, target.as_str());
        assert_eq!(result.moves[0].name, "Renamed");
        assert_eq!(
            result.moves[0].kind,
            yss_project_history::ResourceLifecycleKind::Worksheet
        );
        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].resource, super::worksheet_key(&target));
        assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(result.deltas[0].to_revision, ResourceRevision::new(1));
        assert_eq!(result.deltas[0].caused_by, Some(operation_id));
        assert_eq!(
            result.history,
            yss_project_history::HistoryStatusDto {
                can_undo: true,
                can_redo: false,
            }
        );
    }

    #[test]
    fn worksheet_rename_rejects_exact_portable_conflict_without_suffixing() {
        let project = TestProject::new("worksheet-rename-conflict");
        let (source, source_document) = fixtures::worksheet("Source", "database");
        let (conflict, conflict_document) = fixtures::worksheet("Report", "database");
        let mut data = ProjectData::new();
        data.worksheets
            .insert(source.clone(), source_document.clone());
        data.worksheets
            .insert(conflict.clone(), conflict_document.clone());
        let state = active_state(&project, data);
        fixtures::write_worksheet(&project.root, &source, &source_document).unwrap();
        fixtures::write_worksheet(&project.root, &conflict, &conflict_document).unwrap();
        state.initialize_worksheet_revision_for_test(&source);
        state.initialize_worksheet_revision_for_test(&conflict);
        let session = state.capture_project_session().unwrap();

        let error = state
            .rename_worksheet_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                &ResourceName::parse("report").unwrap(),
                1,
                OperationId::new(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "resource_name_conflict");
        let authority = state.get_data().unwrap();
        assert!(authority.worksheets.contains_key(&source));
        assert!(authority.worksheets.contains_key(&conflict));
        assert_eq!(worksheet_files(&project).len(), 2);
    }

    #[test]
    fn worksheet_save_never_overwrites_another_path() {
        let project = TestProject::new("worksheet-save-distinct-path");
        let (first_path, first) = fixtures::worksheet("Report", "database");
        let (second_path, mut second) = fixtures::worksheet("Other", "database");
        let mut data = ProjectData::new();
        data.worksheets.insert(first_path.clone(), first.clone());
        data.worksheets.insert(second_path.clone(), second.clone());
        let state = active_state(&project, data);
        fixtures::write_worksheet(&project.root, &first_path, &first).unwrap();
        fixtures::write_worksheet(&project.root, &second_path, &second).unwrap();
        state.initialize_worksheet_revision_for_test(&first_path);
        state.initialize_worksheet_revision_for_test(&second_path);
        let session = state.capture_project_session().unwrap();

        second.chart_type = "line".into();
        state
            .save_worksheet_document(
                &session.instance_id,
                &second_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
                second,
            )
            .unwrap();

        assert_two_distinct_worksheets_on_disk(&project, &first_path, &second_path);
        let persisted: WorksheetDocument = serde_json::from_slice(
            &std::fs::read(project.root.join(first_path.relative_path())).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted, first);
    }

    #[test]
    fn worksheet_delete_removes_only_its_canonical_path() {
        let project = TestProject::new("worksheet-delete-canonical");
        let (first_path, first) = fixtures::worksheet("First", "database");
        let (second_path, second) = fixtures::worksheet("Second", "database");
        let first_file = project.root.join(first_path.relative_path());
        let second_file = project.root.join(second_path.relative_path());
        let mut data = ProjectData::new();
        data.worksheets.insert(first_path.clone(), first.clone());
        data.worksheets.insert(second_path.clone(), second.clone());
        fixtures::write_worksheet(&project.root, &first_path, &first).unwrap();
        fixtures::write_worksheet(&project.root, &second_path, &second).unwrap();
        let state = active_state(&project, data);
        state.initialize_worksheet_revision_for_test(&first_path);
        state.initialize_worksheet_revision_for_test(&second_path);
        let session = state.capture_project_session().unwrap();

        state
            .remove_worksheet_resource_transaction(
                &session.instance_id,
                &first_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();

        assert!(!first_file.exists());
        assert!(second_file.is_file());
        assert!(
            !state
                .get_data()
                .unwrap()
                .worksheets
                .contains_key(&first_path)
        );
        assert!(
            state
                .get_data()
                .unwrap()
                .worksheets
                .contains_key(&second_path)
        );
    }

    #[test]
    fn worksheet_create_rejects_invalid_resource_names_without_writing() {
        let project = TestProject::new("worksheet-invalid-name");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        for name in ["A/B", "A\\B"] {
            assert!(
                ResourceName::parse(name).is_err()
                    || state
                        .create_worksheet_resource_transaction(
                            &session.instance_id,
                            &ResourceName::parse(name).unwrap(),
                            None,
                            OperationId::new(),
                        )
                        .is_err()
            );
        }

        assert!(worksheet_files(&project).is_empty());
    }

    #[test]
    fn worksheet_casefold_collision_uses_portable_unique_suffix() {
        let project = TestProject::new("worksheet-casefold-collision");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let first = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let second = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let first_path = worksheet_path_from_lifecycle_result(&first);
        let second_path = worksheet_path_from_lifecycle_result(&second);

        assert_eq!(second_path.display_name().as_str(), "report 2");
        assert_two_distinct_worksheets_on_disk(&project, &first_path, &second_path);
    }

    #[test]
    fn worksheet_commit_failure_restores_file_and_nested_directory_topology() {
        let project = TestProject::new("worksheet-rollback");
        let nested = project.root.join("worksheets/nested/deeper");
        std::fs::create_dir_all(&nested).unwrap();
        let sentinel = nested.join("sentinel.txt");
        std::fs::write(&sentinel, b"untouched").unwrap();
        let (worksheet_path, mut document) = fixtures::worksheet("Original", "database");
        fixtures::write_worksheet(&project.root, &worksheet_path, &document).unwrap();
        let canonical_path = project.root.join(worksheet_path.relative_path());
        let original_bytes = std::fs::read(&canonical_path).unwrap();
        let mut data = ProjectData::new();
        data.worksheets
            .insert(worksheet_path.clone(), document.clone());
        let state = active_state(&project, data);
        state.initialize_worksheet_revision_for_test(&worksheet_path);
        let session = state.capture_project_session().unwrap();
        document.chart_type = "line".into();
        state.set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::FirstLiveReplacement));

        let error = state
            .save_worksheet_document(
                &session.instance_id,
                &worksheet_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
                document,
            )
            .unwrap_err();
        state.set_project_filesystem_fault(None);

        assert_eq!(error.code(), "transaction_commit_failed");
        assert_eq!(std::fs::read(canonical_path).unwrap(), original_bytes);
        assert_eq!(std::fs::read(sentinel).unwrap(), b"untouched");
        assert!(nested.is_dir());
    }

    fn worksheet_path_from_lifecycle_result(
        result: &crate::schema::application_event::ResourceMutationResultDto,
    ) -> WorksheetResourcePath {
        let delta = result.deltas.first().expect("worksheet lifecycle delta");
        let yss_project_history::ResourceDocumentPatch::ResourceLifecycle(patch) = &delta.payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        WorksheetResourcePath::parse(
            patch
                .after
                .as_ref()
                .expect("created worksheet lifecycle state")
                .path
                .as_ref(),
        )
        .unwrap()
    }

    #[test]
    fn worksheet_create_publishes_resource_lifecycle_delta() {
        let project = TestProject::new("worksheet-authoritative-create");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        let name = ResourceName::parse("Report").unwrap();

        let result = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                Some("database".into()),
                operation_id,
            )
            .unwrap();

        let path = WorksheetResourcePath::parse("worksheets/Report.yssbi-worksheet").unwrap();
        assert_eq!(worksheet_path_from_lifecycle_result(&result), path);
        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(result.deltas[0].to_revision, ResourceRevision::INITIAL);
        let yss_project_history::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
            &result.deltas[0].payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        assert_eq!(
            lifecycle.after.as_ref().unwrap().revision,
            ResourceRevision::INITIAL
        );
        assert!(state.get_data().unwrap().worksheets.contains_key(&path));
        assert!(project.root.join(path.relative_path()).is_file());
    }

    #[test]
    fn worksheet_duplicate_allocates_first_free_authoritative_path() {
        let project = TestProject::new("worksheet-authoritative-duplicate");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let name = ResourceName::parse("Report").unwrap();
        let first = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                Some("database".into()),
                OperationId::new(),
            )
            .unwrap();
        let first_path = worksheet_path_from_lifecycle_result(&first);
        let second = state
            .duplicate_worksheet_resource_transaction(
                &session.instance_id,
                &first_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();
        let second_path = worksheet_path_from_lifecycle_result(&second);
        let third = state
            .duplicate_worksheet_resource_transaction(
                &session.instance_id,
                &first_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();
        let third_path = worksheet_path_from_lifecycle_result(&third);

        assert_eq!(first_path.display_name().as_str(), "Report");
        assert_eq!(second_path.display_name().as_str(), "Report 2");
        assert_eq!(third_path.display_name().as_str(), "Report 3");
        assert_eq!(state.get_data().unwrap().worksheets.len(), 3);
        assert_eq!(worksheet_files(&project).len(), 3);
    }

    #[test]
    fn worksheet_save_publishes_document_delta() {
        let project = TestProject::new("worksheet-authoritative-save");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let created = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                Some("database".into()),
                OperationId::new(),
            )
            .unwrap();
        let path = worksheet_path_from_lifecycle_result(&created);
        let mut document = state.get_data().unwrap().worksheets[&path].clone();
        document.chart_type = "line".into();
        let operation_id = OperationId::new();

        let result = state
            .save_worksheet_document(
                &session.instance_id,
                &path,
                ResourceRevision::INITIAL,
                operation_id,
                document,
            )
            .unwrap();

        assert_eq!(result.operation_id, operation_id);
        assert!(matches!(
            result.deltas.as_slice(),
            [yss_project_history::ResourceDeltaEvent {
                from_revision,
                to_revision,
                payload: yss_project_history::ResourceDocumentPatch::Worksheet(_),
                ..
            }] if *from_revision == ResourceRevision::INITIAL
                && *to_revision == ResourceRevision::new(1)
        ));
        assert_eq!(
            state.get_data().unwrap().worksheets[&path].revision,
            ResourceRevision::new(1)
        );
    }

    #[test]
    fn worksheet_remove_publishes_resource_lifecycle_delta() {
        let project = TestProject::new("worksheet-authoritative-remove");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let created = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let path = worksheet_path_from_lifecycle_result(&created);
        let operation_id = OperationId::new();

        let result = state
            .remove_worksheet_resource_transaction(
                &session.instance_id,
                &path,
                ResourceRevision::INITIAL,
                operation_id,
            )
            .unwrap();

        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(result.deltas[0].to_revision, ResourceRevision::new(1));
        let yss_project_history::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
            &result.deltas[0].payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        assert_eq!(
            lifecycle.before.as_ref().unwrap().revision,
            ResourceRevision::INITIAL
        );
        assert!(lifecycle.after.is_none());
        assert!(!state.get_data().unwrap().worksheets.contains_key(&path));
        assert!(!project.root.join(path.relative_path()).exists());
    }

    #[test]
    fn worksheet_delete_recreate_preserves_tombstone_revision() {
        let project = TestProject::new("worksheet-authoritative-aba");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let name = ResourceName::parse("Reusable").unwrap();
        let created = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                None,
                OperationId::new(),
            )
            .unwrap();
        let path = worksheet_path_from_lifecycle_result(&created);
        state
            .remove_worksheet_resource_transaction(
                &session.instance_id,
                &path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();

        let recreated = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                None,
                OperationId::new(),
            )
            .unwrap();

        assert_eq!(worksheet_path_from_lifecycle_result(&recreated), path);
        assert_eq!(recreated.deltas[0].from_revision, ResourceRevision::new(1));
        assert_eq!(recreated.deltas[0].to_revision, ResourceRevision::new(2));
        let yss_project_history::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
            &recreated.deltas[0].payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        assert_eq!(
            lifecycle.after.as_ref().unwrap().revision,
            ResourceRevision::new(2)
        );
        assert_eq!(
            state.get_data().unwrap().worksheets[&path].revision,
            ResourceRevision::new(2)
        );
    }

    #[test]
    fn worksheet_mutation_failures_have_zero_authoritative_effects() {
        let project = TestProject::new("worksheet-authoritative-failure");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        let name = ResourceName::parse("Report").unwrap();
        state.set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::FirstLiveReplacement));

        let error = state
            .create_worksheet_resource_transaction(&session.instance_id, &name, None, operation_id)
            .unwrap_err();

        state.set_project_filesystem_fault(None);
        assert_eq!(error.code(), "transaction_commit_failed");
        assert!(state.get_data().unwrap().worksheets.is_empty());
        assert!(state.worksheet_revisions.read().unwrap().is_empty());
        assert!(worksheet_files(&project).is_empty());
        state
            .create_worksheet_resource_transaction(&session.instance_id, &name, None, operation_id)
            .unwrap();
    }

    #[test]
    fn global_update_revalidates_caller_revision_after_waiting_for_root_lease() {
        let project = TestProject::new("global-revision-wait");
        let state = std::sync::Arc::new(active_state(&project, ProjectData::new()));
        let session = state.capture_project_session().unwrap();
        let variable = state
            .add_variable(
                "global",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        state
            .persist_global_variables(
                &session.instance_id,
                state.global_variable_revision_snapshot(),
                OperationId::new(),
            )
            .unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let (staged_tx, staged_rx) = std::sync::mpsc::channel();
        set_writer_snapshot_test_hook(Some(std::sync::Arc::new(move || {
            staged_tx.send(()).unwrap();
        })));
        let worker_state = std::sync::Arc::clone(&state);
        let project_instance_id = session.instance_id.clone();
        let worker = std::thread::spawn(move || {
            worker_state.update_global_variable_transaction(
                &project_instance_id,
                variable.id,
                Some("stale".into()),
                None,
                None,
                None,
                None,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
        });
        staged_rx.recv().unwrap();
        state.variable_revisions.write().unwrap().insert(
            variable.id,
            crate::project::project_state::VariableRevisionEntry::present(ResourceRevision::new(1)),
        );
        drop(lease);

        let error = match worker.join().unwrap() {
            Ok(_) => panic!("stale variable update unexpectedly committed"),
            Err(error) => error,
        };
        set_writer_snapshot_test_hook(None);
        assert_eq!(error.code(), "resource_revision_conflict");
        assert_eq!(
            state.get_variable(&variable.id).unwrap().unwrap().name,
            "global"
        );
    }

    #[test]
    fn stale_writer_emits_no_result_or_event() {
        let project = TestProject::new("stale-writer");
        let state = active_state(&project, ProjectData::new());
        let stale = state.capture_project_session().unwrap();
        state.activate_project_fixture(
            project.root.to_string_lossy().into_owned(),
            ProjectData::new(),
        );

        let mut events = Vec::new();
        let error = crate::commands::command_project::lifecycle::flush_project_with_emitter(
            &state,
            stale.instance_id,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(error.code(), "stale_project_lifecycle");
        assert!(events.is_empty());
    }
}
