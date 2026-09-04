use crate::{GraphResourceFile, ProjectSession, ProjectState, ProjectTransactionContext};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_data_contract::{DataType, DataValue};
use yss_graph_document::GraphResourcePath;
use yss_project_filesystem::{
    ProjectFilesystemError, ProjectFilesystemTransaction, StagedFilesystemMutation,
};
use yss_project_history::{
    ChartResourceKey, FunctionResourceKey, ResourceKey, VariableResourceKey,
};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};
use yss_project_layout::{CHART_EXTENSION, PROJECT_METADATA_FILE};
use yss_project_manifest::ProjectManifest;
use yss_project_model::{ProjectData, ProjectDataPatch};
use yss_resource_naming::{ResourceName, allocate_unique_resource_name};
use yss_variable_contract::{VariableId, VariableInstance, VariableScope};
use yss_variable_value::default_value_for;

#[path = "project_writers/charts.rs"]
mod charts;
#[path = "project_writers/graph.rs"]
mod graph;
#[path = "project_writers/variables.rs"]
mod variables;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSaveResult {
    pub(crate) project_instance_id: ProjectInstanceId,
    pub(crate) operation_id: OperationId,
    pub(crate) publication_revision: u64,
    pub(crate) affected_resources: Box<[ResourceKey]>,
    pub(crate) index_invalidated: bool,
    pub(crate) history: ProjectHistoryStatus,
}

impl ProjectSaveResult {
    pub fn into_parts(
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

pub struct VariableMutationResult {
    variable: VariableInstance,
    mutation: ProjectResourceMutationFacts,
}

pub struct CreateVariableRequest {
    pub project_instance_id: ProjectInstanceId,
    pub name: String,
    pub data_type: DataType,
    pub data_value: DataValue,
    pub description: String,
    pub scope: VariableScope,
    pub tags: Vec<String>,
    pub expected_collection_revision: u64,
    pub operation_id: OperationId,
}

pub struct UpdateVariableRequest {
    pub project_instance_id: ProjectInstanceId,
    pub id: VariableId,
    pub name: Option<String>,
    pub data_type: Option<DataType>,
    pub data_value: Option<DataValue>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub expected_revision: ResourceRevision,
    pub operation_id: OperationId,
}

pub struct DeleteVariableRequest {
    pub project_instance_id: ProjectInstanceId,
    pub id: VariableId,
    pub expected_revision: ResourceRevision,
    pub operation_id: OperationId,
}

#[derive(Debug, Clone)]
pub struct ProjectResourceMutationFacts {
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
pub struct ProjectResourceMove {
    pub from: Box<str>,
    pub to: Box<str>,
    pub kind: yss_project_history::ResourceLifecycleKind,
    pub name: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectProjectionStatus {
    Complete {
        expected_graph_paths: Box<[GraphResourcePath]>,
    },
    Incomplete {
        invalidated_graph_paths: Box<[GraphResourcePath]>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectHistoryStatus {
    pub can_undo: bool,
    pub can_redo: bool,
}

impl ProjectResourceMutationFacts {
    pub fn into_parts(self) -> ProjectResourceMutationParts {
        ProjectResourceMutationParts {
            operation_id: self.operation_id,
            project_instance_id: self.project_instance_id,
            publication_revision: self.publication_revision,
            moves: self.moves,
            deltas: self.deltas,
            projection_status: self.projection_status,
            history: self.history,
        }
    }
}

pub struct ProjectResourceMutationParts {
    pub operation_id: OperationId,
    pub project_instance_id: ProjectInstanceId,
    pub publication_revision: u64,
    pub moves: Box<[ProjectResourceMove]>,
    pub deltas: Box<[yss_project_history::ResourceDeltaEvent]>,
    pub projection_status: ProjectProjectionStatus,
    pub history: ProjectHistoryStatus,
}

impl VariableMutationResult {
    pub fn into_parts(self) -> (VariableInstance, ProjectResourceMutationFacts) {
        (self.variable, self.mutation)
    }
}

fn chart_document_state(document: &ChartDocument) -> yss_project_history::ChartDocumentState {
    yss_project_history::ChartDocumentState {
        database_id: document.database_id.clone(),
        chart_type: document.chart_type.clone(),
        encodings: document.encodings.clone(),
    }
}

fn chart_lifecycle_state(
    path: &ChartResourcePath,
    revision: ResourceRevision,
) -> yss_project_history::ResourceLifecycleState {
    yss_project_history::ResourceLifecycleState {
        revision,
        path: path.as_str().into(),
        kind: yss_project_history::ResourceLifecycleKind::Chart,
        name: path.display_name().as_str().to_string(),
    }
}

fn chart_move_delta(
    from: &ChartResourcePath,
    to: &ChartResourcePath,
    operation_id: OperationId,
    from_revision: ResourceRevision,
    to_revision: ResourceRevision,
) -> yss_project_history::ResourceDeltaEvent {
    yss_project_history::ResourceDeltaEvent {
        resource: chart_key(to),
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

fn chart_resource_delta(
    path: &ChartResourcePath,
    operation_id: OperationId,
    retained_revision: Option<ResourceRevision>,
    before: Option<&ChartDocument>,
    after: Option<&ChartDocument>,
) -> Result<yss_project_history::ResourceDeltaEvent, ProjectFilesystemError> {
    let (from_revision, to_revision, payload) = match (before, after) {
        (Some(before), Some(after)) => (
            before.revision,
            after.revision,
            yss_project_history::ResourceDocumentPatch::Chart(
                yss_project_history::ChartDocumentPatch {
                    before: chart_document_state(before),
                    after: chart_document_state(after),
                },
            ),
        ),
        (None, Some(after)) => (
            retained_revision.unwrap_or(after.revision),
            after.revision,
            yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                yss_project_history::ResourceLifecyclePatch {
                    before: None,
                    after: Some(chart_lifecycle_state(path, after.revision)),
                },
            ),
        ),
        (Some(before), None) => (
            before.revision,
            super::project_state::checked_resource_revision(path.as_str(), before.revision)?,
            yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                yss_project_history::ResourceLifecyclePatch {
                    before: Some(chart_lifecycle_state(path, before.revision)),
                    after: None,
                },
            ),
        ),
        (None, None) => {
            return Err(prepare_error(
                "chart resource delta has neither a before nor an after document",
            ));
        }
    };
    Ok(yss_project_history::ResourceDeltaEvent {
        resource: chart_key(path),
        from_revision,
        to_revision,
        caused_by: Some(operation_id),
        payload,
    })
}

struct WriterSnapshot {
    session: ProjectSession,
    data: ProjectData,
    graph_resource_revisions: std::collections::HashMap<GraphResourcePath, ResourceRevision>,
    variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        crate::project_state::VariableRevisionEntry,
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

fn chart_key(path: &ChartResourcePath) -> ResourceKey {
    ResourceKey::Chart(ChartResourceKey(path.as_str().into()))
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
        Some("yssbi-event" | "yssbi-function") => {
            serde_json::from_slice::<GraphResourceFile>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some("yssbi-vars") => serde_json::from_slice::<crate::GlobalVariablesDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Some(CHART_EXTENSION) => serde_json::from_slice::<ChartDocument>(contents)
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
        let graph_resource_revisions = self.graph_resource_revisions.read().unwrap().clone();
        let variable_revisions = self.variable_revisions.read().unwrap().clone();
        let snapshot = WriterSnapshot {
            session,
            data,
            graph_resource_revisions,
            variable_revisions,
            authority_generation: publication.authority_generation(),
        };
        drop(publication);
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
        let graph_resource_revisions = self.graph_resource_revisions.read().unwrap();
        let variable_revisions = self.variable_revisions.read().unwrap();
        let chart_revisions = self.chart_revisions.read().unwrap();
        super::project_state::validate_context_revisions(
            context,
            &data,
            &graph_resource_revisions,
            &variable_revisions,
            &chart_revisions,
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
            context.filesystem_context(),
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
