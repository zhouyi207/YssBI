//! Authoritative project state for normalized node-system graph documents.

use crate::database::DatabaseState;
use crate::event::GraphMutationResultDto;
use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::node_system::compiler::{GraphCompiler, ResourceSnapshot};
use crate::node_system::document::{
    EditorGraphMutationDto, GraphDeltaEvent, GraphDocumentPatch, GraphMutation, HistoryEntryId,
    HistoryMutation, HistoryStatusDto, MutationConflict, MutationRequest, ProjectDocumentState,
    ProjectHistory, ProjectHistoryTransaction, ResourceKey, RevisionedGraphStore,
};
use crate::project::{
    GraphLifecycleIntent, GraphLifecycleOperation, GraphLifecycleRegistry,
    GraphRenameOwnershipLease, GraphResourceDocument, GraphResourcePath, NormalizedProjectRoot,
    PreparedProjectActivation, ProjectData, ProjectFilesystemCoordinator, ProjectFilesystemError,
    ProjectFilesystemTransaction, ProjectInstanceId, ProjectSession, ProjectStore,
    ProjectTransactionContext, ResourceDocumentPatch, StagedFilesystemMutation,
    load_project_graph_from_file,
};
use crate::tabular::{is_variable_handle, normalize_variable_tabular, sync_variable_cache};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

#[cfg(test)]
type ProjectionTestHook = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;
#[cfg(test)]
type CommittedResourceCompletionTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type ProjectionEnvironmentCaptureTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type MutationPublicationTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type CompilePublicationTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type ExecutionTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type TraceQueryTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type VariableStagingTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
pub(crate) type ProjectActivationTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type ActivationPublicationTestHook = Arc<dyn Fn() + Send + Sync>;
#[cfg(test)]
type LifecycleLockTestHook = Arc<dyn Fn() + Send + Sync>;
type ActivationPanicPayload = Box<dyn std::any::Any + Send + 'static>;

enum CompileProductInvalidation {
    None,
    Graphs(Vec<GraphResourcePath>),
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VariablePresence {
    Present,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VariableRevisionEntry {
    pub(crate) revision: crate::node_system::document::ResourceRevision,
    pub(crate) presence: VariablePresence,
}

impl VariableRevisionEntry {
    pub(crate) const fn present(revision: crate::node_system::document::ResourceRevision) -> Self {
        Self {
            revision,
            presence: VariablePresence::Present,
        }
    }

    pub(crate) const fn deleted(revision: crate::node_system::document::ResourceRevision) -> Self {
        Self {
            revision,
            presence: VariablePresence::Deleted,
        }
    }

    pub(crate) const fn is_present(self) -> bool {
        matches!(self.presence, VariablePresence::Present)
    }
}

struct ActivationGarbage {
    _publication_project_instance_id: String,
    _path: Option<String>,
    _lifecycle: super::graph_lifecycle::GraphLifecycleState,
    _data: ProjectData,
    _store: ProjectStore,
    _graph_revisions: std::collections::HashMap<
        GraphResourcePath,
        crate::node_system::document::ResourceRevision,
    >,
    _variable_revisions:
        std::collections::HashMap<crate::variable::VariableId, VariableRevisionEntry>,
    _worksheet_revisions:
        std::collections::HashMap<String, crate::node_system::document::ResourceRevision>,
    _database_authority_revisions: std::collections::HashMap<String, u64>,
    _identity: ProjectionEnvironmentExpectation,
    _recovery_message: Option<String>,
    _history: ProjectHistory,
}

pub(super) struct PublishedProjectActivation {
    instance_id: ProjectInstanceId,
    garbage: ActivationGarbage,
    postcommit_panic: Option<ActivationPanicPayload>,
}

impl PublishedProjectActivation {
    pub(super) fn dispose(self) -> ProjectInstanceId {
        let Self {
            instance_id,
            garbage,
            postcommit_panic,
        } = self;
        drop(garbage);
        if let Some(payload) = postcommit_panic {
            std::panic::resume_unwind(payload);
        }
        instance_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectionEnvironmentExpectation {
    pub(super) project_instance_id: ProjectInstanceId,
    project_root: Option<NormalizedProjectRoot>,
    pub(super) project_session_id: crate::node_system::analysis::ProjectSessionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectionEnvironmentAuthorityBasis {
    pub(super) project_instance_id: String,
    pub(super) authority_generation: u64,
}

#[derive(Clone)]
pub(super) struct ProjectionEnvironmentSnapshot {
    pub(super) authority: ProjectionEnvironmentAuthorityBasis,
    pub(super) registry: Arc<crate::node_system::registry::NodeRegistry>,
    pub(super) catalog: Arc<crate::node_system::catalog::BuiltinCatalog>,
    pub(super) trace_sink: Arc<crate::node_system::analysis::BoundedTraceSink>,
    pub(super) project_session_id: crate::node_system::analysis::ProjectSessionId,
    pub(super) database_schemas:
        BTreeMap<crate::node_system::plan::ResourceId, Vec<crate::schema::ColumnInfoDTO>>,
    #[cfg(test)]
    projection_test_hook: Option<ProjectionTestHook>,
}

impl ProjectionEnvironmentSnapshot {
    pub(super) fn matches_publication(&self, publication: &MutationPublication) -> bool {
        self.authority.project_instance_id == publication.project_instance_id
            && self.authority.authority_generation == publication.authority_generation()
    }
}

#[derive(Clone)]
pub(super) struct ProjectionSourceSnapshot {
    pub(super) state: ProjectState,
    pub(super) data: ProjectData,
    pub(super) environment: ProjectionEnvironmentSnapshot,
    pub(super) project_instance_id: String,
    pub(super) authority_generation: u64,
}

struct CommittedGraphLoad {
    #[cfg(test)]
    resource: GraphResourceDocument,
    projection_source: Option<ProjectionSourceSnapshot>,
}

struct CommittedGraphMutation {
    delta: GraphDeltaEvent<GraphDocumentPatch>,
    projection_source: ProjectionSourceSnapshot,
    history: HistoryStatusDto,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct GraphMoveHistoryPayload {
    moved_before: GraphResourceDocument,
    moved_after: GraphResourceDocument,
    referenced_graphs_before: BTreeMap<GraphResourcePath, GraphResourceDocument>,
    referenced_graphs_after: BTreeMap<GraphResourcePath, GraphResourceDocument>,
    referenced_variables_before:
        BTreeMap<crate::variable::VariableId, crate::variable::VariableInstance>,
    referenced_variables_after:
        BTreeMap<crate::variable::VariableId, crate::variable::VariableInstance>,
}

struct GraphRenameDiskPlan {
    mutations: Vec<StagedFilesystemMutation>,
    referenced_graphs_before: BTreeMap<GraphResourcePath, GraphResourceDocument>,
    referenced_graphs_after: BTreeMap<GraphResourcePath, GraphResourceDocument>,
}

struct CommittedResourceMutation {
    operation_id: crate::node_system::document::OperationId,
    project_instance_id: String,
    publication_revision: u64,
    moves: Vec<crate::event::ResourceMoveDto>,
    deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
    history: HistoryStatusDto,
    projection_source: ProjectionSourceSnapshot,
    expected_graph_paths: Vec<String>,
    #[cfg(test)]
    completion_test_hook: Option<CommittedResourceCompletionTestHook>,
}

struct CommittedVariableEffects {
    variable_ids: Box<[crate::variable::VariableId]>,
    resource_mutation: Option<CommittedResourceMutation>,
}

pub(super) struct MutationPublication {
    pub(super) project_instance_id: String,
    pub(super) resource_revision: u64,
    authority_generation: u64,
}

pub(super) struct VariableStagingBasis {
    session: ProjectSession,
    authority_generation: u64,
}

impl Default for MutationPublication {
    fn default() -> Self {
        Self {
            project_instance_id: uuid::Uuid::new_v4().to_string(),
            resource_revision: 0,
            authority_generation: 0,
        }
    }
}

impl MutationPublication {
    pub(super) fn authority_generation(&self) -> u64 {
        self.authority_generation
    }

    pub(super) fn advance_authority_generation(&mut self) {
        self.authority_generation = self
            .authority_generation
            .checked_add(1)
            .expect("project authority generation overflowed");
    }

    pub(super) fn allocate_resource_revision(&mut self) -> u64 {
        self.resource_revision = self
            .resource_revision
            .checked_add(1)
            .expect("resource publication revision overflowed");
        self.advance_authority_generation();
        self.resource_revision
    }

    fn reset_to(&mut self, project_instance_id: String) -> String {
        let previous = std::mem::replace(&mut self.project_instance_id, project_instance_id);
        self.resource_revision = 0;
        self.authority_generation = 0;
        previous
    }
}

struct ActivationGenerationTransition {
    generation: Arc<std::sync::atomic::AtomicU64>,
    armed: bool,
}

impl ActivationGenerationTransition {
    fn begin(
        generation: &Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<Self, ProjectFilesystemError> {
        use std::sync::atomic::Ordering;

        let current = generation.load(Ordering::Acquire);
        let Some(changing) = current.checked_add(1) else {
            return Err(ProjectFilesystemError::FilesystemTransactionBusy {
                message: "project activation generation exhausted".into(),
            });
        };
        if current % 2 != 0
            || generation
                .compare_exchange(current, changing, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err(ProjectFilesystemError::FilesystemTransactionBusy {
                message: "project activation publication is already in progress".into(),
            });
        }
        Ok(Self {
            generation: Arc::clone(generation),
            armed: true,
        })
    }

    fn complete(mut self) {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::Release);
        self.armed = false;
    }
}

impl Drop for ActivationGenerationTransition {
    fn drop(&mut self) {
        if self.armed {
            self.generation
                .fetch_add(1, std::sync::atomic::Ordering::Release);
            self.armed = false;
        }
    }
}

fn graph_document_references_path(
    document: &crate::node_system::document::GraphDocument,
    target: &str,
) -> bool {
    document.nodes.values().any(|node| {
        node.parameters.values().any(|value| {
            value.as_str().is_some_and(|path| {
                crate::project::graph_resource_index::normalize_resource_path(path) == target
            })
        })
    })
}

fn variable_scope_references_path(scope: &crate::variable::VariableScope, target: &str) -> bool {
    match scope {
        crate::variable::VariableScope::Global => false,
        crate::variable::VariableScope::Event { event_path } => {
            crate::project::graph_resource_index::normalize_resource_path(event_path) == target
        }
        crate::variable::VariableScope::Function { function_path } => {
            crate::project::graph_resource_index::normalize_resource_path(function_path) == target
        }
    }
}

pub(super) fn validate_context_revisions(
    context: &ProjectTransactionContext,
    data: &ProjectData,
    graph_revisions: &std::collections::HashMap<
        GraphResourcePath,
        crate::node_system::document::ResourceRevision,
    >,
    variable_revisions: &std::collections::HashMap<
        crate::variable::VariableId,
        VariableRevisionEntry,
    >,
    worksheet_revisions: &std::collections::HashMap<
        String,
        crate::node_system::document::ResourceRevision,
    >,
) -> Result<(), ProjectFilesystemError> {
    for resource in &context.affected_resources {
        let expected = context.expected_revisions.get(resource).ok_or_else(|| {
            ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("missing expected revision for {resource:?}"),
            }
        })?;
        let actual = match resource {
            ResourceKey::Graph(path) => {
                GraphResourcePath::new(path.0.as_ref())
                    .ok()
                    .and_then(|path| {
                        data.graphs
                            .get(&path)
                            .map(|resource| resource.document.revision)
                            .or_else(|| graph_revisions.get(&path).copied())
                    })
            }
            ResourceKey::Function(path) => GraphResourcePath::new(path.0.as_ref())
                .ok()
                .and_then(|path| data.graphs.get(&path))
                .and_then(|resource| resource.function.as_ref())
                .map(|function| function.revision),
            ResourceKey::Variable(path) => path
                .0
                .strip_prefix("variables/")
                .or(Some(path.0.as_ref()))
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .map(crate::variable::VariableId::from)
                .and_then(|id| variable_revisions.get(&id).map(|entry| entry.revision)),
            ResourceKey::Database(_) => None,
            ResourceKey::Worksheet(path) => worksheet_revisions.get(path.0.as_ref()).copied(),
        };
        if actual != Some(*expected) {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!(
                    "revision for {resource:?} changed from {} to {}",
                    expected.get(),
                    actual
                        .map(|revision| revision.get().to_string())
                        .unwrap_or_else(|| "missing".into())
                ),
            });
        }
    }
    for resource in &context.expected_absent_resources {
        let present = match resource {
            ResourceKey::Graph(path) => GraphResourcePath::new(path.0.as_ref())
                .ok()
                .is_some_and(|path| data.graphs.contains_key(&path)),
            ResourceKey::Function(path) => GraphResourcePath::new(path.0.as_ref())
                .ok()
                .and_then(|path| data.graphs.get(&path))
                .is_some_and(|resource| resource.function.is_some()),
            ResourceKey::Variable(path) => path
                .0
                .strip_prefix("variables/")
                .or(Some(path.0.as_ref()))
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .map(crate::variable::VariableId::from)
                .is_some_and(|id| data.variables.contains_key(&id)),
            ResourceKey::Database(path) => path
                .0
                .strip_prefix("databases/")
                .is_some_and(|id| data.databases.contains_key(id)),
            ResourceKey::Worksheet(path) => data.worksheets.contains_key(path.0.as_ref()),
        };
        if present {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("expected {resource:?} to remain absent"),
            });
        }
    }
    Ok(())
}

fn canonical_graph_lifecycle_events(
    context: &ProjectTransactionContext,
    patch: &ResourceDocumentPatch,
) -> Vec<crate::node_system::document::ResourceDeltaEvent> {
    let graph_key = |path: &GraphResourcePath| {
        ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
            path.as_str().into(),
        ))
    };
    let lifecycle_state = |path: &GraphResourcePath, revision| {
        crate::node_system::document::GraphResourceLifecycleState {
            revision,
            path: path.as_str().into(),
            kind: match path
                .kind()
                .expect("validated graph resource path must have a kind")
            {
                crate::project::GraphDocumentKind::Event => {
                    crate::node_system::document::GraphResourceLifecycleKind::Event
                }
                crate::project::GraphDocumentKind::Function => {
                    crate::node_system::document::GraphResourceLifecycleKind::Function
                }
            },
        }
    };
    let lifecycle_delta = |path: &GraphResourcePath, revision, before, after| {
        crate::node_system::document::ResourceDeltaEvent {
            resource: graph_key(path),
            from_revision: revision,
            to_revision: revision.next(),
            caused_by: Some(context.operation_id),
            payload: crate::node_system::document::ResourceDocumentPatch::GraphResourceLifecycle(
                crate::node_system::document::GraphResourceLifecyclePatch { before, after },
            ),
        }
    };
    match patch {
        ResourceDocumentPatch::InsertGraph { path, resource } => {
            let revision = resource.document.revision;
            return vec![lifecycle_delta(
                path,
                revision,
                None,
                Some(lifecycle_state(path, revision)),
            )];
        }
        ResourceDocumentPatch::UnloadGraph { path } => {
            let revision = context.expected_revisions[&graph_key(path)];
            return vec![lifecycle_delta(
                path,
                revision,
                None,
                Some(lifecycle_state(path, revision)),
            )];
        }
        ResourceDocumentPatch::RemoveGraph { path, revision } => {
            let revision = *revision;
            return vec![lifecycle_delta(
                path,
                revision,
                Some(lifecycle_state(path, revision)),
                None,
            )];
        }
        ResourceDocumentPatch::MoveGraph { .. } => {}
        ResourceDocumentPatch::PatchVariables { .. }
        | ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. } => return Vec::new(),
    }
    let ResourceDocumentPatch::MoveGraph {
        from,
        to,
        moved,
        referenced_graphs,
        referenced_variables,
        ..
    } = patch
    else {
        unreachable!("non-move graph lifecycle patches returned above")
    };
    let graph_move_patch = || {
        crate::node_system::document::ResourceDocumentPatch::GraphResourceMove(
            crate::node_system::document::ResourcePathMovePatch {
                from: from.as_str().into(),
                to: to.as_str().into(),
            },
        )
    };
    let source_key = graph_key(from);
    let mut deltas = vec![crate::node_system::document::ResourceDeltaEvent {
        resource: graph_key(to),
        from_revision: context.expected_revisions[&source_key],
        to_revision: moved.document.revision,
        caused_by: Some(context.operation_id),
        payload: graph_move_patch(),
    }];
    deltas.extend(referenced_graphs.iter().map(|(path, resource)| {
        let key = graph_key(path);
        crate::node_system::document::ResourceDeltaEvent {
            from_revision: context.expected_revisions[&key],
            to_revision: resource.document.revision,
            resource: key,
            caused_by: Some(context.operation_id),
            payload: graph_move_patch(),
        }
    }));
    deltas.extend(referenced_variables.keys().map(|id| {
        let key = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
            format!("variables/{id}").into(),
        ));
        let from_revision = context.expected_revisions[&key];
        crate::node_system::document::ResourceDeltaEvent {
            resource: key,
            from_revision,
            to_revision: from_revision.next(),
            caused_by: Some(context.operation_id),
            payload: crate::node_system::document::ResourceDocumentPatch::VariableScopeMove(
                crate::node_system::document::ResourcePathMovePatch {
                    from: from.as_str().into(),
                    to: to.as_str().into(),
                },
            ),
        }
    }));
    deltas
}

fn patch_projection_paths(patch: &ResourceDocumentPatch) -> Vec<String> {
    let mut paths = std::collections::BTreeSet::new();
    match patch {
        ResourceDocumentPatch::InsertGraph { path, .. }
        | ResourceDocumentPatch::RemoveGraph { path, .. }
        | ResourceDocumentPatch::UnloadGraph { path } => {
            paths.insert(path.as_str().to_string());
        }
        ResourceDocumentPatch::MoveGraph {
            from,
            to,
            referenced_graphs,
            ..
        } => {
            paths.insert(from.as_str().to_string());
            paths.insert(to.as_str().to_string());
            paths.extend(
                referenced_graphs
                    .keys()
                    .map(|path| path.as_str().to_string()),
            );
        }
        ResourceDocumentPatch::PatchVariables { .. }
        | ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. } => {}
    }
    paths.into_iter().collect()
}

fn compile_product_invalidation_for_resource_patch(
    patch: &ResourceDocumentPatch,
    data: &ProjectData,
) -> CompileProductInvalidation {
    match patch {
        ResourceDocumentPatch::InsertGraph { path, resource } => {
            if resource.kind == crate::project::GraphDocumentKind::Function {
                CompileProductInvalidation::All
            } else {
                CompileProductInvalidation::Graphs(vec![path.clone()])
            }
        }
        ResourceDocumentPatch::RemoveGraph { path, .. }
        | ResourceDocumentPatch::UnloadGraph { path } => {
            let removes_function = data.graphs.get(path).is_some_and(|resource| {
                resource.kind == crate::project::GraphDocumentKind::Function
            });
            let removes_variables = data
                .variables
                .values()
                .any(|variable| variable_scope_references_path(&variable.scope, path.as_str()));
            if removes_function || removes_variables {
                CompileProductInvalidation::All
            } else {
                CompileProductInvalidation::Graphs(vec![path.clone()])
            }
        }
        ResourceDocumentPatch::MoveGraph {
            from,
            to,
            moved,
            referenced_graphs,
            referenced_variables,
            ..
        } => {
            if moved.kind == crate::project::GraphDocumentKind::Function
                || !referenced_variables.is_empty()
                || referenced_graphs
                    .values()
                    .any(|resource| resource.kind == crate::project::GraphDocumentKind::Function)
            {
                CompileProductInvalidation::All
            } else {
                let mut paths = vec![from.clone(), to.clone()];
                paths.extend(referenced_graphs.keys().cloned());
                CompileProductInvalidation::Graphs(paths)
            }
        }
        ResourceDocumentPatch::PatchVariables { .. } => CompileProductInvalidation::All,
        ResourceDocumentPatch::UpsertWorksheet { .. }
        | ResourceDocumentPatch::RemoveWorksheet { .. } => CompileProductInvalidation::None,
    }
}

fn affected_projection_paths(
    deltas: &[crate::node_system::document::ResourceDeltaEvent],
    data: &ProjectData,
) -> Vec<String> {
    let changed_functions = deltas
        .iter()
        .filter_map(|delta| match &delta.resource {
            crate::node_system::document::ResourceKey::Function(path) => Some(path.0.to_string()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut paths = deltas
        .iter()
        .filter_map(|delta| match &delta.resource {
            crate::node_system::document::ResourceKey::Graph(path) => Some(path.0.to_string()),
            crate::node_system::document::ResourceKey::Function(path) => Some(path.0.to_string()),
            crate::node_system::document::ResourceKey::Variable(_)
            | crate::node_system::document::ResourceKey::Database(_)
            | crate::node_system::document::ResourceKey::Worksheet(_) => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    if !changed_functions.is_empty() {
        for (graph_path, graph) in &data.graphs {
            let calls_changed_function = graph.document.nodes.values().any(|node| {
                node.node_type.as_str() == "yssbi.project.function.call"
                    && node.parameters.iter().any(|(key, value)| {
                        key.as_str() == "target"
                            && value
                                .as_str()
                                .is_some_and(|target| changed_functions.contains(target))
                    })
            });
            if calls_changed_function {
                paths.insert(graph_path.as_str().to_string());
            }
        }
    }
    paths.into_iter().collect()
}

impl ProjectionSourceSnapshot {
    fn replacements(
        &self,
        graph_paths: &[String],
        locale: &str,
    ) -> Result<Vec<crate::event::GraphProjectionReplacementDto>, String> {
        graph_paths
            .iter()
            .map(|path| {
                let graph_path = GraphResourcePath::new(path).map_err(|error| error.to_string())?;
                Ok(crate::event::GraphProjectionReplacementDto {
                    graph_path: graph_path.as_str().to_string(),
                    projection: self.graph_projection(&graph_path, locale)?,
                })
            })
            .collect()
    }

    fn graph_projection(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, String> {
        #[cfg(test)]
        if let Some(hook) = self.environment.projection_test_hook.as_ref() {
            hook()?;
        }
        let document = self
            .data
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.clone())
            .ok_or_else(|| format!("graph '{}' not loaded", graph_path))?;
        let (analysis, _) = self
            .state
            .get_or_compile_current_from_source(graph_path, self)?;
        EditorGraphProjectionDto::from_sources(
            graph_path.as_str(),
            &analysis.payload.analysis,
            &document,
            self.environment.registry.as_ref(),
            &self.environment.catalog.localization(locale),
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone)]
pub struct ProjectState {
    pub project_data: Arc<RwLock<ProjectData>>,
    project_path: Arc<RwLock<Option<String>>>,
    pub project_store: Arc<RwLock<ProjectStore>>,
    pub(super) history: Arc<RwLock<ProjectHistory>>,
    pub(super) project_activation: crate::project::ProjectActivationCoordinator,
    pub(super) mutation_publication: Arc<Mutex<MutationPublication>>,
    pub(super) compile_coordinator:
        Arc<RwLock<Arc<crate::node_system::compiler::ProjectCompileCoordinator>>>,
    filesystem: ProjectFilesystemCoordinator,
    graph_lifecycle: GraphLifecycleRegistry,
    pub(super) resource_operations:
        Arc<Mutex<crate::project::resource_mutations::ResourceOperationLedger>>,
    recovery_marker: crate::project::ProjectRecoveryMarker,
    activation_generation: Arc<std::sync::atomic::AtomicU64>,
    activation_identity: Arc<RwLock<ProjectionEnvironmentExpectation>>,
    pub(super) graph_revisions: Arc<
        RwLock<
            std::collections::HashMap<
                GraphResourcePath,
                crate::node_system::document::ResourceRevision,
            >,
        >,
    >,
    pub(super) variable_revisions:
        Arc<RwLock<std::collections::HashMap<crate::variable::VariableId, VariableRevisionEntry>>>,
    pub(super) worksheet_revisions: Arc<
        RwLock<std::collections::HashMap<String, crate::node_system::document::ResourceRevision>>,
    >,
    pub(super) database_authority_revisions: Arc<RwLock<std::collections::HashMap<String, u64>>>,

    #[cfg(test)]
    graph_rename_io_checkpoint: Arc<RwLock<Option<Arc<dyn Fn() + Send + Sync>>>>,
    #[cfg(test)]
    graph_move_history_io_checkpoint: Arc<RwLock<Option<Arc<dyn Fn() + Send + Sync>>>>,

    #[cfg(test)]
    function_load_checkpoint: Arc<
        RwLock<Option<Arc<dyn Fn(&crate::node_system::runtime::CancellationToken) + Send + Sync>>>,
    >,
    #[cfg(test)]
    production_relational_observer:
        Arc<RwLock<Option<Arc<crate::node_system::runtime::ProductionRelationalObserver>>>>,
    #[cfg(test)]
    project_resource_lease_observer:
        Arc<RwLock<Option<crate::node_system::runtime::ProjectResourceLeaseObserver>>>,
    #[cfg(test)]
    projection_test_hook: Arc<RwLock<Option<ProjectionTestHook>>>,
    #[cfg(test)]
    committed_resource_completion_test_hook:
        Arc<RwLock<Option<CommittedResourceCompletionTestHook>>>,
    #[cfg(test)]
    projection_environment_capture_test_hook:
        Arc<RwLock<Option<ProjectionEnvironmentCaptureTestHook>>>,
    #[cfg(test)]
    projection_environment_after_path_data_test_hook:
        Arc<RwLock<Option<ProjectionEnvironmentCaptureTestHook>>>,
    #[cfg(test)]
    mutation_publication_test_hook: Arc<RwLock<Option<MutationPublicationTestHook>>>,
    #[cfg(test)]
    compile_capture_after_environment_test_hook: Arc<RwLock<Option<CompilePublicationTestHook>>>,
    #[cfg(test)]
    compile_before_authority_gate_test_hook: Arc<RwLock<Option<CompilePublicationTestHook>>>,
    #[cfg(test)]
    compile_coalesced_before_wait_test_hook: Arc<RwLock<Option<CompilePublicationTestHook>>>,
    #[cfg(test)]
    execution_before_final_gate_test_hook: Arc<RwLock<Option<ExecutionTestHook>>>,
    #[cfg(test)]
    execution_before_run_test_hook: Arc<RwLock<Option<ExecutionTestHook>>>,
    #[cfg(test)]
    execution_before_commit_gate_test_hook: Arc<RwLock<Option<ExecutionTestHook>>>,
    #[cfg(test)]
    pub(super) trace_query_after_snapshot_test_hook: Arc<RwLock<Option<TraceQueryTestHook>>>,
    #[cfg(test)]
    variable_staging_test_hook: Arc<RwLock<Option<VariableStagingTestHook>>>,
    #[cfg(test)]
    project_activation_test_hook: Arc<RwLock<Option<ProjectActivationTestHook>>>,
    #[cfg(test)]
    activation_store_replaced_test_hook: Arc<RwLock<Option<ActivationPublicationTestHook>>>,
    #[cfg(test)]
    activation_publication_panic_test_hook: Arc<RwLock<Option<ActivationPublicationTestHook>>>,
    #[cfg(test)]
    activation_preparation_after_read_test_hook: Arc<RwLock<Option<ActivationPublicationTestHook>>>,
    #[cfg(test)]
    activation_final_rebuild_test_hook: Arc<RwLock<Option<ActivationPublicationTestHook>>>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommittedResourceMutation {
    fn complete(self, locale: &str) -> crate::event::ResourceMutationResultDto {
        let CommittedResourceMutation {
            operation_id,
            project_instance_id,
            publication_revision,
            moves,
            deltas,
            history,
            projection_source,
            expected_graph_paths,
            #[cfg(test)]
            completion_test_hook,
        } = self;
        #[cfg(test)]
        if let Some(hook) = completion_test_hook.as_ref() {
            hook();
        }

        let projection_replacements = projection_source.replacements(&expected_graph_paths, locale);

        match projection_replacements {
            Ok(projection_replacements) => crate::event::ResourceMutationResultDto {
                operation_id,
                project_instance_id,
                publication_revision,
                moves,
                deltas,
                worksheet_deltas: Vec::new(),
                projection_replacements,
                projection_status: crate::event::ProjectionStatusDto::Complete {
                    expected_graph_paths,
                },
                history,
            },
            Err(error) => {
                tauri_plugin_log::log::error!(
                    "committed resource mutation projection completion failed: {error}"
                );
                crate::event::ResourceMutationResultDto {
                    operation_id,
                    project_instance_id,
                    publication_revision,
                    moves,
                    deltas,
                    worksheet_deltas: Vec::new(),
                    projection_replacements: Vec::new(),
                    projection_status: crate::event::ProjectionStatusDto::Incomplete {
                        invalidated_graph_paths: expected_graph_paths,
                    },
                    history,
                }
            }
        }
    }
}

impl ProjectState {
    pub fn new() -> Self {
        Self::with_filesystem(ProjectFilesystemCoordinator::default())
    }

    #[cfg(test)]
    pub(crate) fn with_shared_filesystem_for_test(
        filesystem: ProjectFilesystemCoordinator,
    ) -> Self {
        Self::with_filesystem(filesystem)
    }

    fn with_filesystem(filesystem: ProjectFilesystemCoordinator) -> Self {
        let publication = MutationPublication::default();
        let store = ProjectStore::default();
        let activation_identity = ProjectionEnvironmentExpectation {
            project_instance_id: ProjectInstanceId::from_existing(
                publication.project_instance_id.clone(),
            ),
            project_root: None,
            project_session_id: store.project_session_id.clone(),
        };
        Self {
            project_data: Arc::new(RwLock::new(ProjectData::new())),
            project_path: Arc::new(RwLock::new(None)),
            project_store: Arc::new(RwLock::new(store)),
            history: Arc::new(RwLock::new(ProjectHistory::default())),
            project_activation: crate::project::ProjectActivationCoordinator::default(),
            mutation_publication: Arc::new(Mutex::new(publication)),
            compile_coordinator: Arc::new(RwLock::new(Arc::new(
                crate::node_system::compiler::ProjectCompileCoordinator::new(),
            ))),
            filesystem,
            graph_lifecycle: GraphLifecycleRegistry::default(),
            resource_operations: Arc::new(Mutex::new(
                crate::project::resource_mutations::ResourceOperationLedger::new(
                    activation_identity.project_instance_id.clone(),
                ),
            )),
            recovery_marker: crate::project::ProjectRecoveryMarker::default(),
            activation_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            activation_identity: Arc::new(RwLock::new(activation_identity)),
            graph_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            variable_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            worksheet_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            database_authority_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),

            #[cfg(test)]
            graph_rename_io_checkpoint: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            graph_move_history_io_checkpoint: Arc::new(RwLock::new(None)),

            #[cfg(test)]
            function_load_checkpoint: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            production_relational_observer: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            project_resource_lease_observer: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            projection_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            committed_resource_completion_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            projection_environment_capture_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            projection_environment_after_path_data_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            mutation_publication_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            compile_capture_after_environment_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            compile_before_authority_gate_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            compile_coalesced_before_wait_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            execution_before_final_gate_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            execution_before_run_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            execution_before_commit_gate_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            trace_query_after_snapshot_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            variable_staging_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            project_activation_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            activation_store_replaced_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            activation_publication_panic_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            activation_preparation_after_read_test_hook: Arc::new(RwLock::new(None)),
            #[cfg(test)]
            activation_final_rebuild_test_hook: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get_data(&self) -> Result<ProjectData, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        Ok(self.project_data.read().unwrap().clone())
    }

    pub(super) fn capture_variable_staging_basis(
        &self,
        publication: &MutationPublication,
    ) -> Result<VariableStagingBasis, ProjectFilesystemError> {
        let identity = self.activation_identity.read().unwrap();
        let root = identity.project_root.clone().ok_or_else(|| {
            ProjectFilesystemError::StaleProjectLifecycle {
                message: "no project is active while staging variable mutation".into(),
            }
        })?;
        Ok(VariableStagingBasis {
            session: ProjectSession {
                instance_id: identity.project_instance_id.clone(),
                root,
            },
            authority_generation: publication.authority_generation(),
        })
    }

    pub(super) fn validate_variable_staging_basis(
        &self,
        publication: &MutationPublication,
        basis: &VariableStagingBasis,
    ) -> Result<(), ProjectFilesystemError> {
        let identity = self.activation_identity.read().unwrap();
        if publication.authority_generation() != basis.authority_generation
            || publication.project_instance_id != basis.session.instance_id.as_str()
            || identity.project_instance_id != basis.session.instance_id
            || identity.project_root.as_ref() != Some(&basis.session.root)
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project session or authority changed while staging variable mutation"
                    .into(),
            });
        }
        Ok(())
    }

    pub fn project_instance_id(&self) -> String {
        self.mutation_publication
            .lock()
            .unwrap()
            .project_instance_id
            .clone()
    }

    pub fn history_status(&self) -> HistoryStatusDto {
        let _publication = self.mutation_publication.lock().unwrap();
        self.history.read().unwrap().status()
    }

    pub(super) fn current_projection_environment_expectation(
        &self,
    ) -> ProjectionEnvironmentExpectation {
        self.activation_identity.read().unwrap().clone()
    }

    fn projection_environment_expectation_for_session(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectionEnvironmentExpectation, String> {
        self.projection_environment_expectation_for_identity(
            session.instance_id.as_str(),
            &session.root,
        )
    }

    fn projection_environment_expectation_for_identity(
        &self,
        project_instance_id: &str,
        project_root: &NormalizedProjectRoot,
    ) -> Result<ProjectionEnvironmentExpectation, String> {
        let expected = self.current_projection_environment_expectation();
        if expected.project_instance_id.as_str() != project_instance_id
            || expected.project_root.as_ref() != Some(project_root)
        {
            return Err(
                "stale_project_lifecycle: project changed before projection environment capture"
                    .into(),
            );
        }
        Ok(expected)
    }

    fn capture_projection_environment_for_session(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.projection_environment_expectation_for_session(session)?;
        self.capture_projection_environment(&expected)
    }

    fn capture_projection_environment_for_execution_session(
        &self,
        session: &ProjectSession,
        expected_session_id: &crate::node_system::analysis::ProjectSessionId,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.projection_environment_expectation_for_session(session)?;
        if &expected.project_session_id != expected_session_id {
            return Err(
                "stale_project_lifecycle: execution session changed before projection environment capture"
                    .into(),
            );
        }
        self.capture_projection_environment(&expected)
    }

    pub(super) fn capture_projection_environment(
        &self,
        expected: &ProjectionEnvironmentExpectation,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        use crate::node_system::plan::ResourceId;
        use std::sync::atomic::Ordering;

        let mut capture_attempts = 0;
        loop {
            let generation_before = self.activation_generation.load(Ordering::Acquire);
            if generation_before % 2 != 0 {
                std::thread::yield_now();
                continue;
            }
            if capture_attempts == 3 {
                return Err(
                    "stale_project_lifecycle: authority changed repeatedly during projection environment capture"
                        .into(),
                );
            }
            capture_attempts += 1;

            let path = self.project_path.read().unwrap();
            #[cfg(test)]
            if let Some(hook) = self
                .projection_environment_capture_test_hook
                .read()
                .unwrap()
                .clone()
            {
                hook();
            }
            let project_path = path.clone();
            drop(path);
            let (authority, databases) = {
                let publication = self.mutation_publication.lock().unwrap();
                if publication.project_instance_id != expected.project_instance_id.as_str() {
                    return Err(
                        "stale_project_lifecycle: project changed before projection environment authority capture"
                            .into(),
                    );
                }
                let data = self.project_data.read().unwrap();
                (
                    ProjectionEnvironmentAuthorityBasis {
                        project_instance_id: publication.project_instance_id.clone(),
                        authority_generation: publication.authority_generation(),
                    },
                    data.databases.clone(),
                )
            };
            self.run_projection_environment_after_path_data_test_hook();

            let project_root = project_path
                .as_deref()
                .map(NormalizedProjectRoot::from_project_path)
                .transpose()
                .map_err(|error| error.to_string())?;
            let (registry, catalog, trace_sink, project_session_id, mut database_schemas) = {
                let store = self.project_store.read().unwrap();
                let schemas = store
                    .databases
                    .iter()
                    .filter_map(|(id, database)| {
                        if !databases.contains_key(id) {
                            return None;
                        }
                        let columns = match &database.state {
                            DatabaseState::DuckDb { columns, .. } => {
                                crate::application::database_schema::column_info_from_duckdb(
                                    columns,
                                )
                            }
                            DatabaseState::Loaded { dataframe, .. } => {
                                crate::application::database_schema::column_info_from_schema(
                                    dataframe.schema().as_ref(),
                                )
                            }
                            DatabaseState::Failed { .. } => return None,
                        };
                        Some((id.clone(), columns))
                    })
                    .collect::<BTreeMap<_, _>>();
                (
                    Arc::clone(&store.node_registry),
                    Arc::clone(&store.catalog),
                    Arc::clone(&store.trace_sink),
                    store.project_session_id.clone(),
                    schemas,
                )
            };
            let identity_after = self.activation_identity.read().unwrap().clone();
            let generation_after = self.activation_generation.load(Ordering::Acquire);
            if generation_before != generation_after || generation_after % 2 != 0 {
                if &identity_after != expected {
                    return Err("stale_project_lifecycle: project changed during projection environment capture"
                        .into());
                }
                continue;
            }
            if &identity_after != expected
                || project_root != expected.project_root
                || project_session_id != expected.project_session_id
            {
                return Err(
                    "stale_project_lifecycle: projection environment identity mismatch".into(),
                );
            }

            let mut metadata_error = None;
            for (id, declaration) in &databases {
                if database_schemas.contains_key(id) {
                    continue;
                }
                let crate::database::DatabaseEngine::DuckDb { path, table } = &declaration.engine
                else {
                    continue;
                };
                let Some(root) = project_root.as_ref() else {
                    metadata_error =
                        Some(format!("database '{id}' requires an active project path"));
                    break;
                };
                match crate::database::read_table_meta(&root.as_path().join(path), table) {
                    Ok(metadata) => {
                        database_schemas.insert(
                            id.clone(),
                            crate::application::database_schema::column_info_from_duckdb(
                                &metadata.columns,
                            ),
                        );
                    }
                    Err(error) => {
                        metadata_error = Some(error);
                        break;
                    }
                }
            }

            let final_generation = self.activation_generation.load(Ordering::Acquire);
            let final_identity = self.activation_identity.read().unwrap().clone();
            if final_generation != generation_after || final_generation % 2 != 0 {
                if &final_identity != expected {
                    return Err("stale_project_lifecycle: project changed during projection metadata capture"
                        .into());
                }
                continue;
            }
            if &final_identity != expected {
                return Err(
                    "stale_project_lifecycle: projection metadata identity mismatch".into(),
                );
            }
            let authority_is_current = {
                let publication = self.mutation_publication.lock().unwrap();
                authority.project_instance_id == publication.project_instance_id
                    && authority.authority_generation == publication.authority_generation()
            };
            if !authority_is_current {
                continue;
            }
            if let Some(error) = metadata_error {
                return Err(error);
            }

            let database_schemas = database_schemas
                .into_iter()
                .map(|(id, columns)| {
                    ResourceId::new(format!("databases/{id}"))
                        .map(|resource| (resource, columns))
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<_, _>>()?;
            return Ok(ProjectionEnvironmentSnapshot {
                authority,
                registry,
                catalog,
                trace_sink,
                project_session_id,
                database_schemas,
                #[cfg(test)]
                projection_test_hook: self.projection_test_hook.read().unwrap().clone(),
            });
        }
    }

    pub(super) fn projection_source_snapshot(
        &self,
        data: &ProjectData,
        environment: ProjectionEnvironmentSnapshot,
        project_instance_id: String,
        authority_generation: u64,
    ) -> ProjectionSourceSnapshot {
        ProjectionSourceSnapshot {
            state: self.clone(),
            data: data.clone(),
            environment,
            project_instance_id,
            authority_generation,
        }
    }

    #[cfg(test)]
    fn run_projection_environment_after_path_data_test_hook(&self) {
        if let Some(hook) = self
            .projection_environment_after_path_data_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_projection_environment_after_path_data_test_hook(&self) {}

    #[cfg(test)]
    fn run_mutation_publication_test_hook(&self) {
        if let Some(hook) = self.mutation_publication_test_hook.read().unwrap().clone() {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_mutation_publication_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn run_project_activation_test_hook(&self) {
        if let Some(hook) = self.project_activation_test_hook.read().unwrap().clone() {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_project_activation_test_hook(&self) {}

    #[cfg(test)]
    fn run_activation_publication_panic_test_hook(&self) -> Option<ActivationPanicPayload> {
        self.activation_publication_panic_test_hook
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .and_then(|hook| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook())).err()
            })
    }

    #[cfg(not(test))]
    fn run_activation_publication_panic_test_hook(&self) -> Option<ActivationPanicPayload> {
        None
    }

    #[cfg(test)]
    fn run_activation_store_replaced_test_hook(&self) -> Option<ActivationPanicPayload> {
        self.activation_store_replaced_test_hook
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .and_then(|hook| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook())).err()
            })
    }

    #[cfg(not(test))]
    fn run_activation_store_replaced_test_hook(&self) -> Option<ActivationPanicPayload> {
        None
    }

    #[cfg(test)]
    pub(super) fn run_variable_staging_test_hook(&self) {
        if let Some(hook) = self.variable_staging_test_hook.read().unwrap().clone() {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_variable_staging_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn run_activation_preparation_after_read_test_hook(&self) {
        if let Some(hook) = self
            .activation_preparation_after_read_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_activation_preparation_after_read_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn run_activation_final_rebuild_test_hook(&self) {
        if let Some(hook) = self
            .activation_final_rebuild_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_activation_final_rebuild_test_hook(&self) {}

    #[cfg(test)]
    pub(crate) fn set_production_relational_observer(
        &self,
        observer: Arc<crate::node_system::runtime::ProductionRelationalObserver>,
    ) {
        *self.production_relational_observer.write().unwrap() = Some(observer);
    }

    #[cfg(test)]
    pub(crate) fn set_project_resource_lease_observer(
        &self,
        observer: crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        *self.project_resource_lease_observer.write().unwrap() = Some(observer);
    }

    #[cfg(test)]
    pub(crate) fn set_projection_test_hook(&self, hook: ProjectionTestHook) {
        *self.projection_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_committed_resource_completion_test_hook(
        &self,
        hook: CommittedResourceCompletionTestHook,
    ) {
        *self
            .committed_resource_completion_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_projection_environment_capture_test_hook(
        &self,
        hook: ProjectionEnvironmentCaptureTestHook,
    ) {
        *self
            .projection_environment_capture_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_projection_environment_after_path_data_test_hook(
        &self,
        hook: ProjectionEnvironmentCaptureTestHook,
    ) {
        *self
            .projection_environment_after_path_data_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn capture_projection_environment_for_test(
        &self,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.current_projection_environment_expectation();
        self.capture_projection_environment(&expected)
    }

    #[cfg(test)]
    pub(super) fn capture_projection_environment_for_session_for_test(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.projection_environment_expectation_for_session(session)?;
        self.capture_projection_environment(&expected)
    }

    #[cfg(test)]
    pub(super) fn set_mutation_publication_test_hook(&self, hook: MutationPublicationTestHook) {
        *self.mutation_publication_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_compile_capture_after_environment_test_hook(
        &self,
        hook: CompilePublicationTestHook,
    ) {
        *self
            .compile_capture_after_environment_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_compile_before_authority_gate_test_hook(
        &self,
        hook: CompilePublicationTestHook,
    ) {
        *self
            .compile_before_authority_gate_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn run_compile_capture_after_environment_test_hook(&self) {
        if let Some(hook) = self
            .compile_capture_after_environment_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_compile_capture_after_environment_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn run_compile_before_authority_gate_test_hook(&self) {
        if let Some(hook) = self
            .compile_before_authority_gate_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_compile_before_authority_gate_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_compile_coalesced_before_wait_test_hook(
        &self,
        hook: CompilePublicationTestHook,
    ) {
        *self
            .compile_coalesced_before_wait_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn run_compile_coalesced_before_wait_test_hook(&self) {
        if let Some(hook) = self
            .compile_coalesced_before_wait_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_compile_coalesced_before_wait_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_execution_before_final_gate_test_hook(&self, hook: ExecutionTestHook) {
        *self.execution_before_final_gate_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_execution_before_final_gate_test_hook(&self) {
        if let Some(hook) = self
            .execution_before_final_gate_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_execution_before_final_gate_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_execution_before_run_test_hook(&self, hook: ExecutionTestHook) {
        *self.execution_before_run_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_execution_before_run_test_hook(&self) {
        if let Some(hook) = self.execution_before_run_test_hook.read().unwrap().clone() {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_execution_before_run_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_execution_before_commit_gate_test_hook(&self, hook: ExecutionTestHook) {
        *self.execution_before_commit_gate_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_execution_before_commit_gate_test_hook(&self) {
        if let Some(hook) = self
            .execution_before_commit_gate_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_execution_before_commit_gate_test_hook(&self) {}

    #[cfg(test)]
    pub(crate) fn append_history_head_for_test(&self) {
        let path = crate::node_system::document::GraphResourcePath(
            "events/ConcurrentHistory.yssbi-event".into(),
        );
        self.history.write().unwrap().record_committed_transaction(
            ProjectHistoryTransaction::graph_resource_move(
                crate::node_system::document::OperationId::new(),
                path.clone(),
                path,
                serde_json::Value::Null,
            ),
        );
    }

    #[cfg(test)]
    pub(crate) fn set_graph_move_history_io_checkpoint(&self, hook: Arc<dyn Fn() + Send + Sync>) {
        *self.graph_move_history_io_checkpoint.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_graph_move_history_io_checkpoint(&self) {
        if let Some(hook) = self
            .graph_move_history_io_checkpoint
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_graph_move_history_io_checkpoint(&self) {}

    #[cfg(test)]
    pub(crate) fn set_variable_staging_test_hook(&self, hook: VariableStagingTestHook) {
        *self.variable_staging_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_project_activation_test_hook(&self, hook: ProjectActivationTestHook) {
        *self.project_activation_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_activation_store_replaced_test_hook(
        &self,
        hook: ActivationPublicationTestHook,
    ) {
        *self.activation_store_replaced_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_activation_publication_panic_test_hook(
        &self,
        hook: ActivationPublicationTestHook,
    ) {
        *self.activation_publication_panic_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_activation_preparation_after_read_test_hook(
        &self,
        hook: ActivationPublicationTestHook,
    ) {
        *self
            .activation_preparation_after_read_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_activation_final_rebuild_test_hook(
        &self,
        hook: ActivationPublicationTestHook,
    ) {
        *self.activation_final_rebuild_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn poison_project_path_for_test(&self) {
        let _guard = self.project_path.write().unwrap();
        panic!("injected project path poison");
    }

    #[cfg(test)]
    pub(crate) fn authority_generation_for_test(&self) -> u64 {
        self.mutation_publication
            .lock()
            .unwrap()
            .authority_generation
    }

    pub(crate) fn activation_revision(&self) -> u64 {
        self.activation_generation
            .load(std::sync::atomic::Ordering::Acquire)
            / 2
    }

    #[cfg(test)]
    pub(crate) fn activation_generation_for_test(&self) -> u64 {
        self.activation_generation
            .load(std::sync::atomic::Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn revision_state_for_test(
        &self,
    ) -> (
        std::collections::HashMap<
            GraphResourcePath,
            crate::node_system::document::ResourceRevision,
        >,
        std::collections::HashMap<
            crate::variable::VariableId,
            crate::node_system::document::ResourceRevision,
        >,
        std::collections::HashMap<String, crate::node_system::document::ResourceRevision>,
    ) {
        (
            self.graph_revisions.read().unwrap().clone(),
            self.variable_revisions
                .read()
                .unwrap()
                .iter()
                .map(|(id, entry)| (*id, entry.revision))
                .collect(),
            self.worksheet_revisions.read().unwrap().clone(),
        )
    }

    #[cfg(test)]
    pub(crate) fn variable_revision_entry_for_test(
        &self,
        id: &crate::variable::VariableId,
    ) -> Option<VariableRevisionEntry> {
        self.variable_revisions.read().unwrap().get(id).copied()
    }

    #[cfg(test)]
    pub(crate) fn runtime_identity_sessions_for_test(
        &self,
    ) -> (
        crate::node_system::analysis::ProjectSessionId,
        crate::node_system::analysis::ProjectSessionId,
    ) {
        let _publication = self.mutation_publication.lock().unwrap();
        let runtime = self
            .project_store
            .read()
            .unwrap()
            .project_session_id
            .clone();
        let identity = self
            .activation_identity
            .read()
            .unwrap()
            .project_session_id
            .clone();
        (runtime, identity)
    }

    #[cfg(test)]
    pub(crate) fn try_current_pre_run_admission_for_test(&self) -> Option<bool> {
        let store = self.project_store.try_read().ok()?;
        let result = store.runs.track_pre_run(
            store.project_session_id.clone(),
            crate::node_system::runtime::CancellationToken::new(),
        );
        let accepted = result.is_ok();
        drop(result);
        Some(accepted)
    }

    #[cfg(test)]
    pub(crate) fn try_current_run_admission_for_test(&self) -> Option<bool> {
        let store = self.project_store.try_read().ok()?;
        let result = store.runs.track(
            store.project_session_id.clone(),
            crate::node_system::analysis::RunId::new(9_004),
            crate::node_system::runtime::CancellationToken::new(),
        );
        let accepted = result.is_ok();
        drop(result);
        Some(accepted)
    }

    #[cfg(test)]
    pub(crate) fn graph_lifecycle_entry_count(&self) -> usize {
        self.graph_lifecycle.entry_count()
    }

    #[cfg(test)]
    pub(crate) fn activation_publication_guards_are_available_for_test(&self) -> bool {
        self.mutation_publication.try_lock().is_ok()
            && self.project_path.try_write().is_ok()
            && self.graph_lifecycle.boundary_is_available()
            && self.project_data.try_write().is_ok()
            && self.project_store.try_write().is_ok()
            && self.graph_revisions.try_write().is_ok()
            && self.variable_revisions.try_write().is_ok()
            && self.worksheet_revisions.try_write().is_ok()
            && self.activation_identity.try_write().is_ok()
            && self.recovery_marker.boundary_is_available()
            && self.history.try_write().is_ok()
    }

    #[cfg(test)]
    pub(crate) fn set_graph_rename_io_checkpoint(&self, hook: LifecycleLockTestHook) {
        *self.graph_rename_io_checkpoint.write().unwrap() = Some(hook);
    }

    pub fn cancel_graph_run(&self, run_id: crate::node_system::analysis::RunId) -> bool {
        let (runs, project_session_id) = self.current_run_registry();
        runs.cancel_run(&project_session_id, run_id)
    }

    pub(super) fn current_run_registry(
        &self,
    ) -> (
        Arc<crate::node_system::runtime::ProjectRunRegistry>,
        crate::node_system::analysis::ProjectSessionId,
    ) {
        let store = self
            .project_store
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (Arc::clone(&store.runs), store.project_session_id.clone())
    }

    pub(super) fn read_activation_data(
        &self,
        root: &NormalizedProjectRoot,
    ) -> Result<ProjectData, ProjectFilesystemError> {
        crate::project::load_project_from_file(root.as_path().to_string_lossy().as_ref()).map_err(
            |error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            },
        )
    }

    pub(super) fn capture_prepared_authority_basis(
        &self,
        root: &NormalizedProjectRoot,
    ) -> Result<Option<crate::project::PreparedAuthorityBasis>, ProjectFilesystemError> {
        let publication = self.mutation_publication.lock().unwrap();
        let identity = self.activation_identity.read().unwrap();
        Ok((identity.project_root.as_ref() == Some(root)).then(|| {
            crate::project::PreparedAuthorityBasis {
                project_instance_id: ProjectInstanceId::from_existing(
                    publication.project_instance_id.clone(),
                ),
                project_root: root.clone(),
                publication_revision: publication.resource_revision,
                authority_generation: publication.authority_generation,
            }
        }))
    }

    pub(super) fn publish_project_activation(
        &self,
        prepared: PreparedProjectActivation,
    ) -> Result<PublishedProjectActivation, ProjectFilesystemError> {
        self.publish_project_activation_with_test_hooks(prepared, true)
    }

    pub(super) fn publish_project_activation_without_test_hooks(
        &self,
        prepared: PreparedProjectActivation,
    ) -> Result<PublishedProjectActivation, ProjectFilesystemError> {
        self.publish_project_activation_with_test_hooks(prepared, false)
    }

    fn publish_project_activation_with_test_hooks(
        &self,
        prepared: PreparedProjectActivation,
        run_test_hooks: bool,
    ) -> Result<PublishedProjectActivation, ProjectFilesystemError> {
        let PreparedProjectActivation {
            session_root: project_root,
            data,
            store,
            graph_revisions,
            variable_revisions,
            worksheet_revisions,
            authority_basis,
            requires_final_rebuild: _,
        } = prepared;
        let path = project_root
            .as_ref()
            .map(|root| root.as_path().to_string_lossy().into_owned());
        let next_instance_id = ProjectInstanceId::new();
        let next_publication_id = next_instance_id.to_string();
        let next_identity = ProjectionEnvironmentExpectation {
            project_instance_id: next_instance_id.clone(),
            project_root: project_root.clone(),
            project_session_id: store.project_session_id.clone(),
        };
        let database_authority_revisions =
            data.databases.keys().cloned().map(|id| (id, 0)).collect();
        let precommit_panic;
        let postcommit_panic;
        let mut garbage = None;

        {
            let (mut publication, publication_recovered) = match self.mutation_publication.lock() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut resource_operations, resource_operations_recovered) =
                match self.resource_operations.lock() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_path, path_recovered) = match self.project_path.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut lifecycle, lifecycle_recovered) = self.graph_lifecycle.boundary_recovering();
            let (mut current_data, data_recovered) = match self.project_data.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut current_store, store_recovered) = match self.project_store.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut current_database_authority_revisions, database_authority_revisions_recovered) =
                match self.database_authority_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_graph_revisions, graph_revisions_recovered) =
                match self.graph_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_variable_revisions, variable_revisions_recovered) =
                match self.variable_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_worksheet_revisions, worksheet_revisions_recovered) =
                match self.worksheet_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_identity, identity_recovered) = match self.activation_identity.write()
            {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut recovery, recovery_recovered) = self.recovery_marker.boundary_recovering();
            let (mut history, history_recovered) = match self.history.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };

            if authority_basis.as_ref().is_some_and(|basis| {
                publication.project_instance_id != basis.project_instance_id.as_str()
                    || publication.resource_revision != basis.publication_revision
                    || publication.authority_generation != basis.authority_generation
                    || current_identity.project_root.as_ref() != Some(&basis.project_root)
            }) {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "prepared project activation was superseded by committed authority"
                        .into(),
                });
            }

            let generation = ActivationGenerationTransition::begin(&self.activation_generation)?;
            precommit_panic = run_test_hooks
                .then(|| self.run_activation_publication_panic_test_hook())
                .flatten();

            if precommit_panic.is_none() {
                resource_operations.reset_for_project(next_instance_id.clone());
                let previous_publication_id = publication.reset_to(next_publication_id);
                garbage = Some(ActivationGarbage {
                    _publication_project_instance_id: previous_publication_id,
                    _path: std::mem::replace(&mut *current_path, path),
                    _lifecycle: lifecycle.take_state(),
                    _data: std::mem::replace(&mut *current_data, data),
                    _store: std::mem::replace(&mut *current_store, store),
                    _database_authority_revisions: std::mem::replace(
                        &mut *current_database_authority_revisions,
                        database_authority_revisions,
                    ),
                    _graph_revisions: std::mem::replace(
                        &mut *current_graph_revisions,
                        graph_revisions,
                    ),
                    _variable_revisions: std::mem::replace(
                        &mut *current_variable_revisions,
                        variable_revisions,
                    ),
                    _worksheet_revisions: std::mem::replace(
                        &mut *current_worksheet_revisions,
                        worksheet_revisions,
                    ),
                    _identity: std::mem::replace(&mut *current_identity, next_identity),
                    _recovery_message: std::mem::take(&mut *recovery),
                    _history: std::mem::take(&mut *history),
                });
                self.replace_compile_coordinator_generation();

                postcommit_panic = run_test_hooks
                    .then(|| self.run_activation_store_replaced_test_hook())
                    .flatten();
                generation.complete();

                if publication_recovered {
                    self.mutation_publication.clear_poison();
                }
                if resource_operations_recovered {
                    self.resource_operations.clear_poison();
                }
                if path_recovered {
                    self.project_path.clear_poison();
                }
                if lifecycle_recovered {
                    self.graph_lifecycle.clear_poison();
                }
                if data_recovered {
                    self.project_data.clear_poison();
                }
                if store_recovered {
                    self.project_store.clear_poison();
                }
                if database_authority_revisions_recovered {
                    self.database_authority_revisions.clear_poison();
                }
                if graph_revisions_recovered {
                    self.graph_revisions.clear_poison();
                }
                if variable_revisions_recovered {
                    self.variable_revisions.clear_poison();
                }
                if worksheet_revisions_recovered {
                    self.worksheet_revisions.clear_poison();
                }
                if identity_recovered {
                    self.activation_identity.clear_poison();
                }
                if recovery_recovered {
                    self.recovery_marker.clear_poison();
                }
                if history_recovered {
                    self.history.clear_poison();
                }
            } else {
                postcommit_panic = None;
            }
        }

        if let Some(payload) = precommit_panic {
            std::panic::resume_unwind(payload);
        }
        let garbage = garbage.ok_or_else(|| ProjectFilesystemError::TransactionCommitFailed {
            message: "activation commit did not retain previous authority".into(),
        })?;
        Ok(PublishedProjectActivation {
            instance_id: next_instance_id,
            garbage,
            postcommit_panic,
        })
    }

    pub fn get_path(&self) -> Option<String> {
        self.project_path.read().unwrap().clone()
    }

    pub(crate) fn filesystem(&self) -> &ProjectFilesystemCoordinator {
        &self.filesystem
    }

    pub(crate) fn project_recovery_marker(&self) -> crate::project::ProjectRecoveryMarker {
        self.recovery_marker.clone()
    }

    pub fn ensure_project_operational(&self) -> Result<(), ProjectFilesystemError> {
        match self.recovery_marker.error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn ensure_mutation_operational(&self) -> Result<(), MutationConflict> {
        self.ensure_project_operational()
            .map_err(|error| MutationConflict::RecoveryRequired(error.to_string().into()))
    }

    #[cfg(test)]
    pub(crate) fn initialize_worksheet_revision_for_test(&self, worksheet_id: &str) {
        self.worksheet_revisions.write().unwrap().insert(
            worksheet_id.to_string(),
            crate::node_system::document::ResourceRevision::INITIAL,
        );
    }

    pub fn localized_catalog_snapshot(
        &self,
        project_instance_id: &ProjectInstanceId,
        locale: &str,
    ) -> Result<crate::node_system::catalog::LocalizedCatalogDto, ProjectFilesystemError> {
        use std::sync::atomic::Ordering;

        self.ensure_project_operational()?;
        let generation = self.activation_generation.load(Ordering::Acquire);
        if generation % 2 != 0 {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project activation is changing during Catalog snapshot".into(),
            });
        }

        let (registry, catalog, publication_revision) = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            let store = self.project_store.read().unwrap();
            let identity = self.activation_identity.read().unwrap();
            if publication.project_instance_id != project_instance_id.as_str()
                || &identity.project_instance_id != project_instance_id
                || identity.project_session_id != store.project_session_id
                || path.is_none()
            {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project changed before Catalog snapshot".into(),
                });
            }
            (
                Arc::clone(&store.node_registry),
                Arc::clone(&store.catalog),
                publication.resource_revision,
            )
        };

        let captured_generation = self.activation_generation.load(Ordering::Acquire);
        if captured_generation != generation || captured_generation % 2 != 0 {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed during Catalog snapshot".into(),
            });
        }

        let registry_fingerprint = registry.fingerprint().to_string();
        let localized = catalog.localize(registry.as_ref(), locale);

        self.ensure_project_operational()?;
        let final_generation = self.activation_generation.load(Ordering::Acquire);
        let final_identity = self.activation_identity.read().unwrap();
        if final_generation != captured_generation
            || final_generation % 2 != 0
            || &final_identity.project_instance_id != project_instance_id
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed while localizing Catalog snapshot".into(),
            });
        }

        Ok(localized.into_dto(
            project_instance_id.as_str(),
            registry_fingerprint,
            publication_revision,
        ))
    }

    pub fn capture_project_session(&self) -> Result<ProjectSession, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let (instance_id, project_path) = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            let project_path =
                path.clone()
                    .ok_or_else(|| ProjectFilesystemError::StaleProjectLifecycle {
                        message: "no project is active".into(),
                    })?;
            (publication.project_instance_id.clone(), project_path)
        };
        let root = NormalizedProjectRoot::from_project_path(project_path)?;
        Ok(ProjectSession {
            instance_id: ProjectInstanceId::from_existing(instance_id),
            root,
        })
    }

    pub fn validate_project_session(
        &self,
        session: &ProjectSession,
    ) -> Result<(), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let project_path = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project instance changed".into(),
                });
            }
            path.clone()
                .ok_or_else(|| ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project was closed".into(),
                })?
        };
        let current_root = NormalizedProjectRoot::from_project_path(project_path)?;
        if current_root != session.root {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project root changed".into(),
            });
        }
        Ok(())
    }

    pub(crate) fn coherent_project_read_snapshot(
        &self,
        session: &ProjectSession,
    ) -> Result<(String, u64, HistoryStatusDto, ProjectData), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        let path = self.project_path.read().unwrap();
        let identity = self.activation_identity.read().unwrap();
        if publication.project_instance_id != session.instance_id.as_str()
            || identity.project_instance_id != session.instance_id
            || identity.project_root.as_ref() != Some(&session.root)
            || path.is_none()
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before read publication".into(),
            });
        }
        let data = self.project_data.read().unwrap().clone();
        let history = self.history.read().unwrap().status();
        self.ensure_project_operational()?;
        Ok((
            publication.project_instance_id.clone(),
            publication.resource_revision,
            history,
            data,
        ))
    }

    fn invalidate_graph_compile_products(&self, graph_path: &GraphResourcePath) {
        let coordinator = self.compile_coordinator.read().unwrap().clone();
        coordinator.invalidate(&crate::node_system::document::GraphResourcePath(
            graph_path.as_str().into(),
        ));
    }

    fn invalidate_graph_body_compile_products(
        &self,
        graph_path: &GraphResourcePath,
        kind: crate::project::GraphDocumentKind,
    ) {
        match kind {
            crate::project::GraphDocumentKind::Event => {
                self.invalidate_graph_compile_products(graph_path)
            }
            crate::project::GraphDocumentKind::Function => self.invalidate_all_compile_products(),
        }
    }

    pub(super) fn invalidate_all_compile_products(&self) {
        let coordinator = self.compile_coordinator.read().unwrap().clone();
        coordinator.invalidate_all();
    }

    fn apply_compile_product_invalidation(&self, invalidation: CompileProductInvalidation) {
        match invalidation {
            CompileProductInvalidation::None => {}
            CompileProductInvalidation::Graphs(paths) => {
                for path in paths {
                    self.invalidate_graph_compile_products(&path);
                }
            }
            CompileProductInvalidation::All => self.invalidate_all_compile_products(),
        }
    }

    fn replace_compile_coordinator_generation(&self) {
        let detached = {
            let mut current = self.compile_coordinator.write().unwrap();
            std::mem::replace(
                &mut *current,
                Arc::new(crate::node_system::compiler::ProjectCompileCoordinator::new()),
            )
        };
        detached.invalidate_all();
    }

    pub fn build_schema_provider(&self) -> crate::graph::core::SchemaProvider {
        let store = Arc::clone(&self.project_store);
        Arc::new(move |tabular_id: &str| {
            if is_variable_handle(tabular_id) {
                return store
                    .read()
                    .ok()?
                    .variable_tabular
                    .get(tabular_id)
                    .map(|entry| entry.schema.clone());
            }
            store
                .write()
                .ok()?
                .databases
                .get_mut(tabular_id)?
                .data_schema()
                .ok()
        })
    }

    pub fn insert_graph(
        &self,
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    ) -> Result<GraphResourceDocument, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let revision = resource.document.revision;
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        self.ensure_project_operational()?;
        let invalidation = if resource.kind == crate::project::GraphDocumentKind::Function {
            CompileProductInvalidation::All
        } else {
            CompileProductInvalidation::Graphs(vec![path.clone()])
        };
        let inserted = Self::insert_graph_locked(&mut data, path.clone(), resource);
        graph_revisions.insert(path, revision);
        publication.advance_authority_generation();
        self.apply_compile_product_invalidation(invalidation);
        Ok(inserted)
    }

    fn insert_graph_locked(
        data: &mut ProjectData,
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    ) -> GraphResourceDocument {
        data.graphs.insert(path, resource.clone());
        resource
    }

    pub fn apply_resource_document_patch(
        &self,
        context: &ProjectTransactionContext,
        patch: ResourceDocumentPatch,
    ) -> Result<crate::event::ResourceMutationResultDto, ProjectFilesystemError> {
        self.apply_resource_document_patch_internal(context, patch, None, None, None)
            .map(|receipt| receipt.complete("en-US"))
    }

    fn apply_resource_document_patch_with_environment(
        &self,
        context: &ProjectTransactionContext,
        patch: ResourceDocumentPatch,
        projection_environment: ProjectionEnvironmentSnapshot,
        rename_ownership: Option<&mut GraphRenameOwnershipLease>,
    ) -> Result<crate::event::ResourceMutationResultDto, ProjectFilesystemError> {
        self.apply_resource_document_patch_internal(
            context,
            patch,
            None,
            Some(projection_environment),
            rename_ownership,
        )
        .map(|receipt| receipt.complete("en-US"))
    }

    fn apply_resource_document_patch_internal(
        &self,
        context: &ProjectTransactionContext,
        patch: ResourceDocumentPatch,
        history_head: Option<(bool, HistoryEntryId)>,
        projection_environment: Option<ProjectionEnvironmentSnapshot>,
        mut rename_ownership: Option<&mut GraphRenameOwnershipLease>,
    ) -> Result<CommittedResourceMutation, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        self.validate_project_session(&context.session)?;
        let projection_environment = projection_environment
            .map(Ok)
            .unwrap_or_else(|| self.capture_projection_environment_for_session(&context.session))
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        let projection_paths = patch_projection_paths(&patch);
        let deltas = canonical_graph_lifecycle_events(context, &patch);
        let moves = match &patch {
            ResourceDocumentPatch::MoveGraph {
                from, to, moved, ..
            } => {
                vec![crate::event::ResourceMoveDto {
                    from: from.as_str().to_string(),
                    to: to.as_str().to_string(),
                    kind: match moved.kind {
                        crate::project::GraphDocumentKind::Event => "event",
                        crate::project::GraphDocumentKind::Function => "function",
                    }
                    .into(),
                    name: moved.name.clone(),
                }]
            }
            _ => Vec::new(),
        };
        let move_history = match &patch {
            ResourceDocumentPatch::MoveGraph {
                from,
                to,
                moved_before,
                moved,
                referenced_graphs_before,
                referenced_graphs,
                referenced_variables_before,
                referenced_variables,
                ..
            } => Some(
                crate::node_system::document::ProjectHistoryTransaction::graph_resource_move(
                    context.operation_id,
                    crate::node_system::document::GraphResourcePath(from.as_str().into()),
                    crate::node_system::document::GraphResourcePath(to.as_str().into()),
                    serde_json::to_value(GraphMoveHistoryPayload {
                        moved_before: moved_before.clone(),
                        moved_after: moved.clone(),
                        referenced_graphs_before: referenced_graphs_before.clone(),
                        referenced_graphs_after: referenced_graphs.clone(),
                        referenced_variables_before: referenced_variables_before.clone(),
                        referenced_variables_after: referenced_variables.clone(),
                    })
                    .map_err(|error| {
                        ProjectFilesystemError::TransactionPrepareFailed {
                            message: error.to_string(),
                        }
                    })?,
                ),
            ),
            _ => None,
        };
        let receipt = {
            let mut publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != context.session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project instance changed before patch publication".into(),
                });
            }
            if !projection_environment.matches_publication(&publication) {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "projection environment changed before patch publication".into(),
                });
            }
            let mut lifecycle = self.graph_lifecycle.boundary();
            let mut data = self.project_data.write().unwrap();
            let mut graph_revisions = self.graph_revisions.write().unwrap();
            let mut variable_revisions = self.variable_revisions.write().unwrap();
            let mut worksheet_revisions = self.worksheet_revisions.write().unwrap();
            let mut history = self.history.write().unwrap();
            self.ensure_project_operational()?;
            validate_context_revisions(
                context,
                &data,
                &graph_revisions,
                &variable_revisions,
                &worksheet_revisions,
            )?;
            let compile_invalidation =
                compile_product_invalidation_for_resource_patch(&patch, &data);
            if let Some((undo, expected_history_id)) = &history_head {
                let current = if *undo {
                    history.next_undo()
                } else {
                    history.next_redo()
                };
                if current.map(|entry| &entry.history_id) != Some(expected_history_id) {
                    return Err(ProjectFilesystemError::TransactionCommitFailed {
                        message: "history head changed during filesystem transaction".into(),
                    });
                }
            }

            if let Some(ownership) = rename_ownership.as_deref_mut() {
                ownership.commit_with_boundary(&mut lifecycle)?;
            }

            match patch {
                ResourceDocumentPatch::InsertGraph { path, resource } => {
                    graph_revisions.insert(path.clone(), resource.document.revision);
                    Self::insert_graph_locked(&mut data, path, resource);
                }
                ResourceDocumentPatch::RemoveGraph { path, .. } => {
                    data.graphs.remove(&path);
                    graph_revisions.remove(&path);
                    let removed_ids = data
                        .variables
                        .iter()
                        .filter(|(_, variable)| {
                            variable_scope_references_path(&variable.scope, path.as_str())
                        })
                        .map(|(id, _)| *id)
                        .collect::<Vec<_>>();
                    for id in removed_ids {
                        data.variables.remove(&id);
                        let revision = variable_revisions
                            .get(&id)
                            .map(|entry| entry.revision)
                            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL)
                            .next();
                        variable_revisions.insert(id, VariableRevisionEntry::deleted(revision));
                    }
                }
                ResourceDocumentPatch::UnloadGraph { path } => {
                    data.graphs.remove(&path);
                    data.variables.retain(|_, variable| {
                        !variable_scope_references_path(&variable.scope, path.as_str())
                    });
                }
                ResourceDocumentPatch::MoveGraph {
                    from,
                    to,
                    moved,
                    referenced_graphs,
                    loaded_referenced_graphs,
                    referenced_variables,
                    ..
                } => {
                    let was_loaded = data.graphs.remove(&from).is_some();
                    graph_revisions.remove(&from);
                    graph_revisions.insert(to.clone(), moved.document.revision);
                    if was_loaded {
                        Self::insert_graph_locked(&mut data, to, moved);
                    }
                    for (path, resource) in referenced_graphs {
                        graph_revisions.insert(path.clone(), resource.document.revision);
                        if loaded_referenced_graphs.contains(&path) {
                            Self::insert_graph_locked(&mut data, path, resource);
                        }
                    }
                    for (id, variable) in referenced_variables {
                        data.variables.insert(id, variable);
                        let revision = variable_revisions
                            .get(&id)
                            .map(|entry| entry.revision)
                            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL)
                            .next();
                        variable_revisions.insert(id, VariableRevisionEntry::present(revision));
                    }
                }
                ResourceDocumentPatch::PatchVariables { updates, removals } => {
                    for id in removals {
                        data.variables.remove(&id);
                        let revision = variable_revisions
                            .get(&id)
                            .map(|entry| entry.revision)
                            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL)
                            .next();
                        variable_revisions.insert(id, VariableRevisionEntry::deleted(revision));
                    }
                    for (id, variable) in updates {
                        data.variables.insert(id, variable);
                        let revision = variable_revisions
                            .get(&id)
                            .map(|entry| entry.revision)
                            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL)
                            .next();
                        variable_revisions.insert(id, VariableRevisionEntry::present(revision));
                    }
                }
                ResourceDocumentPatch::UpsertWorksheet { id, mut document } => {
                    let revision = worksheet_revisions
                        .get(&id)
                        .copied()
                        .map(|revision| revision.next())
                        .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL);
                    document.revision = revision;
                    data.worksheets.insert(id.clone(), document);
                    worksheet_revisions.insert(id, revision);
                }
                ResourceDocumentPatch::RemoveWorksheet { id } => {
                    data.worksheets.remove(&id);
                    worksheet_revisions.remove(&id);
                }
            }

            if let Some((undo, expected_history_id)) = history_head {
                history
                    .move_graph_resource_head(undo, &expected_history_id)
                    .map_err(|error| ProjectFilesystemError::TransactionCommitFailed {
                        message: error.to_string(),
                    })?;
            } else if let Some(transaction) = move_history {
                history.record_committed_transaction(transaction);
            } else if deltas.iter().any(|delta| {
                !matches!(
                    &delta.payload,
                    crate::node_system::document::ResourceDocumentPatch::GraphResourceLifecycle(_)
                )
            }) {
                let changes = deltas
                    .iter()
                    .filter(|delta| {
                        !matches!(
                            &delta.payload,
                            crate::node_system::document::ResourceDocumentPatch::GraphResourceLifecycle(_)
                        )
                    })
                    .map(|delta| crate::node_system::document::ResourcePatch {
                        resource: delta.resource.clone(),
                        before_revision: delta.from_revision,
                        after_revision: delta.to_revision,
                        forward: delta.payload.clone(),
                        inverse: delta.payload.inverse(),
                    })
                    .collect::<Vec<_>>();
                history.record_committed_transaction(
                    crate::node_system::document::ProjectHistoryTransaction::new(
                        context.operation_id,
                        changes,
                    ),
                );
            }
            let history = history.status();
            let publication_revision = publication.allocate_resource_revision();
            self.apply_compile_product_invalidation(compile_invalidation);
            let projection_source = self.projection_source_snapshot(
                &data,
                projection_environment,
                publication.project_instance_id.clone(),
                publication.authority_generation(),
            );
            #[cfg(test)]
            let completion_test_hook = self
                .committed_resource_completion_test_hook
                .read()
                .unwrap()
                .clone();
            CommittedResourceMutation {
                operation_id: context.operation_id,
                project_instance_id: publication.project_instance_id.clone(),
                publication_revision,
                moves,
                deltas,
                history,
                projection_source,
                expected_graph_paths: projection_paths,
                #[cfg(test)]
                completion_test_hook,
            }
        };

        Ok(receipt)
    }

    pub fn worksheet_creation_snapshot(
        &self,
    ) -> Result<(Vec<String>, Option<String>), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let data = self.project_data.read().unwrap();
        Ok((
            data.worksheets
                .values()
                .map(|worksheet| worksheet.name.clone())
                .collect(),
            data.databases.keys().next().cloned(),
        ))
    }

    pub fn upsert_worksheet_document(
        &self,
        document: crate::project::WorksheetDocument,
    ) -> Result<
        (
            crate::event::ResourceMutationResultDto,
            crate::project::WorksheetDocument,
        ),
        ProjectFilesystemError,
    > {
        let session = self.capture_project_session()?;
        let old = self
            .project_data
            .read()
            .unwrap()
            .worksheets
            .get(&document.id)
            .cloned();
        let worksheet_key = ResourceKey::Worksheet(
            crate::node_system::document::WorksheetResourceKey(document.id.clone().into()),
        );
        let current_revision = self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&document.id)
            .copied();
        if current_revision.is_none()
            && document.revision != crate::node_system::document::ResourceRevision::INITIAL
        {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!(
                    "new worksheet '{}' has non-initial revision {}",
                    document.id,
                    document.revision.get()
                ),
            });
        }
        let submitted_revision = document.revision;
        let mut committed_document = document;
        committed_document.revision = current_revision
            .map(|_| submitted_revision.next())
            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL);
        let context = ProjectTransactionContext {
            session: session.clone(),
            operation_id: crate::node_system::document::OperationId::new(),
            affected_resources: current_revision
                .map(|_| worksheet_key.clone())
                .into_iter()
                .collect(),
            expected_revisions: current_revision
                .map(|_| (worksheet_key.clone(), submitted_revision))
                .into_iter()
                .collect(),
            expected_absent_resources: current_revision
                .is_none()
                .then_some(worksheet_key)
                .into_iter()
                .collect(),
            recovery_marker: Some(self.project_recovery_marker()),
        };

        let next_path = crate::project::worksheet_relative_path(&committed_document);
        let mut mutations = Vec::new();
        if let Some(old) = old {
            let old_path = crate::project::worksheet_relative_path(&old);
            if old_path != next_path {
                mutations.push(StagedFilesystemMutation::RemoveFile {
                    relative_path: old_path,
                });
            }
        }
        let contents = serde_json::to_vec_pretty(&committed_document).map_err(|error| {
            ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            }
        })?;
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: next_path,
            contents,
        });
        let lease = self.filesystem.acquire(session.root.clone())?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            mutations,
            |_, contents| {
                serde_json::from_slice::<crate::project::WorksheetDocument>(contents)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
        )?;
        self.validate_project_session(&session)?;
        let projection_environment = self
            .capture_projection_environment_for_session(&session)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        let committed = prepared.commit()?;
        let authoritative_document = committed_document.clone();
        let publication = match self.apply_resource_document_patch_with_environment(
            &context,
            ResourceDocumentPatch::UpsertWorksheet {
                id: committed_document.id.clone(),
                document: committed_document,
            },
            projection_environment,
            None,
        ) {
            Ok(publication) => publication,
            Err(publication_error) => {
                return match committed.rollback() {
                    Ok(()) => Err(publication_error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        Ok((publication, authoritative_document))
    }

    pub fn remove_worksheet_document(
        &self,
        worksheet_id: &str,
    ) -> Result<
        (
            crate::event::ResourceMutationResultDto,
            crate::project::WorksheetDocument,
        ),
        ProjectFilesystemError,
    > {
        let session = self.capture_project_session()?;
        let document = self
            .project_data
            .read()
            .unwrap()
            .worksheets
            .get(worksheet_id)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::TransactionPrepareFailed {
                message: format!("worksheet '{worksheet_id}' not found"),
            })?;
        let worksheet_key = ResourceKey::Worksheet(
            crate::node_system::document::WorksheetResourceKey(worksheet_id.into()),
        );
        let expected_revision = self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(worksheet_id)
            .copied()
            .ok_or_else(|| ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("worksheet '{worksheet_id}' has no authoritative revision"),
            })?;
        let context = ProjectTransactionContext {
            session: session.clone(),
            operation_id: crate::node_system::document::OperationId::new(),
            affected_resources: vec![worksheet_key.clone()],
            expected_revisions: [(worksheet_key, expected_revision)].into_iter().collect(),
            expected_absent_resources: Default::default(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let lease = self.filesystem.acquire(session.root.clone())?;
        let prepared = ProjectFilesystemTransaction::prepare(
            context.clone(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: crate::project::worksheet_relative_path(&document),
            }],
        )?;
        self.validate_project_session(&session)?;
        let projection_environment = self
            .capture_projection_environment_for_session(&session)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        let committed = prepared.commit()?;
        let publication = match self.apply_resource_document_patch_with_environment(
            &context,
            ResourceDocumentPatch::RemoveWorksheet {
                id: worksheet_id.to_string(),
            },
            projection_environment,
            None,
        ) {
            Ok(publication) => publication,
            Err(publication_error) => {
                return match committed.rollback() {
                    Ok(()) => Err(publication_error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        Ok((publication, document))
    }

    pub(super) fn allocate_graph_path_from_snapshot(
        project_path: Option<&str>,
        data: &ProjectData,
        name: &str,
        kind: crate::project::GraphDocumentKind,
    ) -> Result<(GraphResourcePath, String), String> {
        let persisted = if let Some(path) = project_path {
            let root = crate::project::project_root_from_path(path);
            crate::project::scan_graph_resource_index(&root)
                .map_err(|error| error.to_string())?
                .entries()
                .iter()
                .filter(|entry| entry.kind == kind)
                .map(|entry| {
                    crate::project::load_project_graph_from_file(path, &entry.path)
                        .map(|resource| (entry.path.as_str().to_string(), resource.name))
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let existing_names = data
            .graphs
            .values()
            .filter(|graph| graph.kind == kind)
            .map(|graph| graph.name.clone())
            .chain(persisted.iter().map(|(_, name)| name.clone()))
            .collect::<Vec<_>>();
        let unique_name = crate::project::unique_name::unique_name(name.trim(), existing_names);
        let stem = sanitize_graph_name(&unique_name);
        let (directory, extension) = match kind {
            crate::project::GraphDocumentKind::Event => {
                (crate::project::EVENTS_DIR, crate::project::EVENT_EXTENSION)
            }
            crate::project::GraphDocumentKind::Function => (
                crate::project::FUNCTIONS_DIR,
                crate::project::FUNCTION_EXTENSION,
            ),
        };
        let used = data
            .graphs
            .keys()
            .map(|path| path.as_str().to_string())
            .chain(persisted.into_iter().map(|(path, _)| path))
            .collect::<std::collections::HashSet<_>>();
        for suffix in 0.. {
            let file_name = if suffix == 0 {
                format!("{stem}.{extension}")
            } else {
                format!("{stem} {suffix}.{extension}")
            };
            let candidate = format!("{directory}/{file_name}");
            if !used.contains(&candidate) {
                return Ok((
                    GraphResourcePath::new(candidate).map_err(|error| error.to_string())?,
                    unique_name,
                ));
            }
        }
        unreachable!("graph path allocation always finds a suffix")
    }

    pub fn unload_graph_resource(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<(), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let graph_path_text = graph_path.as_str();
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        self.ensure_project_operational()?;
        let removed = data.graphs.remove(graph_path);
        let variable_count = data.variables.len();
        data.variables.retain(|_, variable| match &variable.scope {
            crate::variable::VariableScope::Global => true,
            crate::variable::VariableScope::Event { event_path } => event_path != graph_path_text,
            crate::variable::VariableScope::Function { function_path } => {
                function_path != graph_path_text
            }
        });
        self.history.write().unwrap().clear();
        publication.advance_authority_generation();
        if removed
            .as_ref()
            .is_some_and(|resource| resource.kind == crate::project::GraphDocumentKind::Function)
            || data.variables.len() != variable_count
        {
            self.invalidate_all_compile_products();
        } else {
            self.invalidate_graph_compile_products(graph_path);
        }
        Ok(())
    }

    pub(super) fn rename_graph_resource_transaction_impl(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: crate::node_system::document::ResourceRevision,
        new_name: &str,
        lifecycle_token: u64,
        operation_id: crate::node_system::document::OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let mut ownership_lease = self.acquire_graph_rename_ownership(
            expected_project_instance_id,
            graph_path,
            lifecycle_token,
        )?;

        let ownership = ownership_lease.operation.clone();
        let root = ownership.session.root.clone();
        let project_path = root.as_path().to_string_lossy().into_owned();
        let filesystem_lease = self.filesystem().acquire(root.clone())?;
        self.validate_graph_lifecycle_operation(&ownership)?;

        let (loaded_source, loaded_source_variables, loaded_metadata) = {
            let data = self.project_data.read().unwrap();
            (
                data.graphs.get(graph_path).cloned(),
                data.variables
                    .iter()
                    .filter(|(_, variable)| {
                        variable_scope_references_path(&variable.scope, graph_path.as_str())
                    })
                    .map(|(id, variable)| (*id, variable.clone()))
                    .collect::<std::collections::HashMap<_, _>>(),
                data.graphs
                    .iter()
                    .map(|(path, resource)| (path.clone(), resource.name.clone(), resource.kind))
                    .collect::<Vec<_>>(),
            )
        };
        let source_was_loaded = loaded_source.is_some();

        let source_result = loaded_source.map_or_else(
            || {
                crate::project::project_io::load_project_graph_document_from_file(
                    &project_path,
                    graph_path,
                )
                .map(|document| {
                    let mut graph = document.document;
                    graph.revision = document.revision;
                    (
                        GraphResourceDocument {
                            name: document.name,
                            kind: document.kind,
                            document: graph,
                            function: document.function,
                        },
                        document.local_variables,
                    )
                })
            },
            |resource| Ok((resource, loaded_source_variables)),
        );
        let (mut moved, mut moved_local_variables) = match source_result {
            Ok(resource) => resource,
            Err(error) => {
                self.validate_graph_lifecycle_operation(&ownership)?;
                return Err(ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                });
            }
        };
        let mut allocation_data = ProjectData::new();
        for (path, name, kind) in loaded_metadata {
            allocation_data
                .graphs
                .insert(path, GraphResourceDocument::new(name, kind));
        }
        let allocation = Self::allocate_graph_path_from_snapshot(
            Some(&project_path),
            &allocation_data,
            new_name,
            moved.kind,
        );
        let (target, unique_name) = match allocation {
            Ok(value) => value,
            Err(message) => {
                self.validate_graph_lifecycle_operation(&ownership)?;
                return Err(ProjectFilesystemError::TransactionPrepareFailed { message });
            }
        };
        let moved_before = moved.clone();
        let source_revision = moved.document.revision;
        if source_revision != expected_revision {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("revision for '{}' changed", graph_path),
            });
        }
        moved.name = unique_name;
        moved.document.revision = source_revision.next();
        crate::project::resource_mutations::remap_graph_document_references(
            &mut moved.document,
            graph_path.as_str(),
            target.as_str(),
        );
        for variable in moved_local_variables.values_mut() {
            crate::project::resource_mutations::remap_variable_scope_path(
                &mut variable.scope,
                graph_path.as_str(),
                target.as_str(),
            );
        }

        let mut referenced_graphs_before = BTreeMap::new();
        let mut referenced_graphs = BTreeMap::new();
        let mut referenced_variables_before = BTreeMap::new();
        let mut referenced_variables = BTreeMap::new();
        let mut loaded_referenced_local_variables = BTreeMap::new();
        let mut expected_revisions = BTreeMap::new();
        let mut affected_resources = Vec::new();
        {
            let data = self.project_data.read().unwrap();
            let variable_revisions = self.variable_revisions.read().unwrap();
            let source_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                graph_path.as_str().into(),
            ));
            if data.graphs.contains_key(graph_path) {
                affected_resources.push(source_key.clone());
            }
            expected_revisions.insert(source_key, source_revision);
            for (path, resource) in &data.graphs {
                if path == graph_path
                    || !graph_document_references_path(&resource.document, graph_path.as_str())
                {
                    continue;
                }
                let mut changed = resource.clone();
                crate::project::resource_mutations::remap_graph_document_references(
                    &mut changed.document,
                    graph_path.as_str(),
                    target.as_str(),
                );
                changed.document.revision = changed.document.revision.next();
                let key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    path.as_str().into(),
                ));
                affected_resources.push(key.clone());
                expected_revisions.insert(key, resource.document.revision);
                referenced_graphs_before.insert(path.clone(), resource.clone());
                referenced_graphs.insert(path.clone(), changed);
            }
            for path in referenced_graphs.keys() {
                loaded_referenced_local_variables.insert(
                    path.clone(),
                    data.variables
                        .iter()
                        .filter(|(_, variable)| {
                            variable_scope_references_path(&variable.scope, path.as_str())
                        })
                        .map(|(id, variable)| (*id, variable.clone()))
                        .collect::<std::collections::HashMap<_, _>>(),
                );
            }
            for (id, variable) in &data.variables {
                if !variable_scope_references_path(&variable.scope, graph_path.as_str()) {
                    continue;
                }
                let mut changed = variable.clone();
                crate::project::resource_mutations::remap_variable_scope_path(
                    &mut changed.scope,
                    graph_path.as_str(),
                    target.as_str(),
                );
                let key = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                    format!("variables/{id}").into(),
                ));
                affected_resources.push(key.clone());
                expected_revisions.insert(
                    key,
                    variable_revisions
                        .get(id)
                        .map(|entry| entry.revision)
                        .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
                );
                referenced_variables_before.insert(*id, variable.clone());
                referenced_variables.insert(*id, changed);
            }
        }
        if source_was_loaded {
            moved_local_variables = referenced_variables
                .iter()
                .map(|(id, variable)| (*id, variable.clone()))
                .collect();
        }
        let loaded_referenced_graphs = referenced_graphs.keys().cloned().collect();
        let known_graph_revisions = self.graph_revisions.read().unwrap().clone();
        let disk_plan = match Self::graph_rename_mutations(
            root.as_path(),
            graph_path,
            &target,
            &moved,
            moved_local_variables,
            &loaded_referenced_graphs,
            &known_graph_revisions,
        ) {
            Ok(plan) => plan,
            Err(message) => {
                self.validate_graph_lifecycle_operation(&ownership)?;
                return Err(ProjectFilesystemError::TransactionPrepareFailed { message });
            }
        };
        for (path, before) in disk_plan.referenced_graphs_before {
            let key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                path.as_str().into(),
            ));
            affected_resources.push(key.clone());
            expected_revisions.insert(key, before.document.revision);
            referenced_graphs_before.insert(path, before);
        }
        referenced_graphs.extend(disk_plan.referenced_graphs_after);
        let context = ProjectTransactionContext {
            session: ProjectSession {
                instance_id: ownership.session.instance_id.clone(),
                root: root.clone(),
            },
            operation_id,
            affected_resources,
            expected_revisions,
            expected_absent_resources: [ResourceKey::Graph(
                crate::node_system::document::GraphResourcePath(target.as_str().into()),
            )]
            .into_iter()
            .collect(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let mut mutations = disk_plan.mutations;
        for path in &loaded_referenced_graphs {
            let resource = referenced_graphs
                .get(path)
                .expect("loaded referenced graph remains in the rename patch");
            let local_variables = loaded_referenced_local_variables
                .remove(path)
                .unwrap_or_default();
            let contents = crate::project::project_io::serialize_graph_resource_document(
                resource,
                local_variables,
            )
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?;
            mutations.push(StagedFilesystemMutation::Write {
                relative_path: path.as_str().into(),
                contents,
            });
        }
        self.validate_graph_lifecycle_operation(&ownership)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            filesystem_lease,
            mutations,
            |path, contents| {
                if path == std::path::Path::new(crate::project::GLOBAL_VARIABLES_FILE) {
                    serde_json::from_slice::<crate::project::project_io::GlobalVariablesDocument>(
                        contents,
                    )
                    .map(|_| ())
                    .map_err(|error| error.to_string())
                } else {
                    serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            },
        )?;
        self.validate_graph_lifecycle_operation(&ownership)?;
        let projection_environment = self
            .capture_projection_environment_for_session(&context.session)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        #[cfg(test)]
        crate::project::resource_mutations::run_resource_mutation_test_hook(
            crate::project::resource_mutations::ResourceMutationTestPoint::Prepared,
            Some(&target),
        );
        let committed = prepared.commit()?;
        #[cfg(test)]
        crate::project::resource_mutations::run_resource_mutation_test_hook(
            crate::project::resource_mutations::ResourceMutationTestPoint::Committed,
            Some(&target),
        );
        #[cfg(test)]
        if let Some(checkpoint) = self.graph_rename_io_checkpoint.read().unwrap().clone() {
            checkpoint();
        }
        #[cfg(test)]
        crate::project::resource_mutations::run_resource_mutation_test_hook(
            crate::project::resource_mutations::ResourceMutationTestPoint::BeforePublication,
            Some(&target),
        );
        let publication = self
            .validate_graph_lifecycle_operation(&ownership)
            .and_then(|_| {
                self.apply_resource_document_patch_with_environment(
                    &context,
                    ResourceDocumentPatch::MoveGraph {
                        from: graph_path.clone(),
                        to: target.clone(),
                        moved_before,
                        moved,
                        referenced_graphs_before,
                        referenced_graphs,
                        loaded_referenced_graphs,
                        referenced_variables_before,
                        referenced_variables,
                    },
                    projection_environment,
                    Some(&mut ownership_lease),
                )
            });
        match publication {
            Ok(result) => {
                committed.finalize();
                Ok(result)
            }
            Err(error) => match committed.rollback() {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(rollback_error),
            },
        }
    }

    fn graph_rename_mutations(
        root: &std::path::Path,
        source: &GraphResourcePath,
        target: &GraphResourcePath,
        moved: &GraphResourceDocument,
        moved_local_variables: std::collections::HashMap<
            crate::variable::VariableId,
            crate::variable::VariableInstance,
        >,
        excluded_graphs: &std::collections::BTreeSet<GraphResourcePath>,
        known_revisions: &std::collections::HashMap<
            GraphResourcePath,
            crate::node_system::document::ResourceRevision,
        >,
    ) -> Result<GraphRenameDiskPlan, String> {
        let mut plan = GraphRenameDiskPlan {
            mutations: Vec::new(),
            referenced_graphs_before: BTreeMap::new(),
            referenced_graphs_after: BTreeMap::new(),
        };
        for entry in crate::project::scan_graph_resource_index(root)
            .map_err(|error| error.to_string())?
            .entries()
        {
            if entry.path == *source || excluded_graphs.contains(&entry.path) {
                continue;
            }
            let relative_path = std::path::PathBuf::from(entry.path.as_str());
            let contents = crate::project::read_secure_project_file(root, &relative_path)
                .map_err(|error| error.to_string())?;
            let before: crate::project::project_io::GraphDocument =
                serde_json::from_slice(&contents).map_err(|error| error.to_string())?;
            let mut after = before.clone();
            let mut changed = crate::project::resource_mutations::remap_graph_document_references(
                &mut after.document,
                source.as_str(),
                target.as_str(),
            );
            for variable in after.local_variables.values_mut() {
                changed = crate::project::resource_mutations::remap_variable_scope_path(
                    &mut variable.scope,
                    source.as_str(),
                    target.as_str(),
                ) || changed;
            }
            if !changed {
                continue;
            }
            let before_revision = known_revisions
                .get(&entry.path)
                .copied()
                .unwrap_or(before.document.revision);
            after.document.revision = before_revision.next();
            let mut before_document = before.document;
            before_document.revision = before_revision;
            plan.referenced_graphs_before.insert(
                entry.path.clone(),
                GraphResourceDocument {
                    name: before.name,
                    kind: before.kind,
                    document: before_document,
                    function: before.function,
                },
            );
            plan.referenced_graphs_after.insert(
                entry.path.clone(),
                GraphResourceDocument {
                    name: after.name.clone(),
                    kind: after.kind,
                    document: after.document.clone(),
                    function: after.function.clone(),
                },
            );
            plan.mutations.push(StagedFilesystemMutation::Write {
                relative_path,
                contents: serde_json::to_vec_pretty(&after).map_err(|error| error.to_string())?,
            });
        }
        let variables = std::path::PathBuf::from(crate::project::GLOBAL_VARIABLES_FILE);
        match crate::project::read_secure_project_file(root, &variables) {
            Ok(contents) => {
                let mut document: crate::project::project_io::GlobalVariablesDocument =
                    serde_json::from_slice(&contents).map_err(|error| error.to_string())?;
                let changed = document
                    .variables
                    .values_mut()
                    .fold(false, |changed, variable| {
                        crate::project::resource_mutations::remap_variable_scope_path(
                            &mut variable.scope,
                            source.as_str(),
                            target.as_str(),
                        ) || changed
                    });
                if changed {
                    plan.mutations.push(StagedFilesystemMutation::Write {
                        relative_path: variables,
                        contents: serde_json::to_vec_pretty(&document)
                            .map_err(|error| error.to_string())?,
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        plan.mutations.push(StagedFilesystemMutation::Write {
            relative_path: target.as_str().into(),
            contents: crate::project::project_io::serialize_graph_resource_document(
                moved,
                moved_local_variables,
            )
            .map_err(|error| error.to_string())?,
        });
        plan.mutations.push(StagedFilesystemMutation::RemoveFile {
            relative_path: source.as_str().into(),
        });
        Ok(plan)
    }

    fn acquire_graph_rename_ownership(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        lifecycle_token: u64,
    ) -> Result<GraphRenameOwnershipLease, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_project_instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: format!(
                    "graph '{}' belongs to a different project instance",
                    graph_path
                ),
            });
        }
        let guard = self.graph_lifecycle.register(
            &session,
            graph_path,
            lifecycle_token,
            GraphLifecycleIntent::Rename,
        )?;
        drop(publication);
        let operation = GraphLifecycleOperation::from_guard(session, &guard);
        Ok(GraphRenameOwnershipLease::new(operation, guard))
    }

    fn validate_graph_lifecycle_operation(
        &self,
        operation: &GraphLifecycleOperation,
    ) -> Result<(), ProjectFilesystemError> {
        self.validate_project_session(&operation.session)?;
        self.graph_lifecycle.validate(&operation.owner)
    }

    fn register_graph_load_intent(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
    ) -> Result<
        (
            GraphLifecycleOperation,
            crate::project::GraphLifecycleGuard,
            Option<GraphResourceDocument>,
        ),
        ProjectFilesystemError,
    > {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: format!(
                    "graph '{}' belongs to a different project instance",
                    graph_path
                ),
            });
        }
        let guard = self.graph_lifecycle.register(
            &session,
            graph_path,
            token,
            GraphLifecycleIntent::Load,
        )?;
        let operation = GraphLifecycleOperation::from_guard(session, &guard);
        let cached = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .cloned();
        Ok((operation, guard, cached))
    }

    fn complete_graph_load(
        &self,
        operation: &GraphLifecycleOperation,
        guard: &mut crate::project::GraphLifecycleGuard,
        resource: GraphResourceDocument,
        local_variables: Option<
            std::collections::HashMap<
                crate::variable::VariableId,
                crate::variable::VariableInstance,
            >,
        >,
        include_projection: bool,
    ) -> Result<CommittedGraphLoad, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let projection_environment = if include_projection {
            let expected = self
                .projection_environment_expectation_for_identity(
                    operation.session.instance_id.as_str(),
                    &operation.session.root,
                )
                .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
            Some(
                self.capture_projection_environment(&expected)
                    .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed {
                        message,
                    })?,
            )
        } else {
            None
        };
        let mut publication = self.mutation_publication.lock().unwrap();
        let path = self.project_path.read().unwrap();
        if publication.project_instance_id != operation.session.instance_id.as_str()
            || path.is_none()
        {
            return Err(operation.stale_error());
        }
        if projection_environment
            .as_ref()
            .is_some_and(|environment| !environment.matches_publication(&publication))
        {
            return Err(ProjectFilesystemError::TransactionPrepareFailed {
                message:
                    "stale_project_lifecycle: projection environment changed before graph load commit"
                        .into(),
            });
        }
        let mut lifecycle = self.graph_lifecycle.boundary();
        lifecycle.validate(&operation.owner)?;
        self.ensure_project_operational()?;
        let revision = resource.document.revision;
        let invalidate_all = resource.kind == crate::project::GraphDocumentKind::Function
            || local_variables
                .as_ref()
                .is_some_and(|variables| !variables.is_empty());
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        let mut variable_revisions = self.variable_revisions.write().unwrap();
        let inserted =
            Self::insert_graph_locked(&mut data, operation.owner.graph_path.clone(), resource);
        graph_revisions.insert(operation.owner.graph_path.clone(), revision);
        if let Some(local_variables) = local_variables {
            for (id, variable) in local_variables {
                match variable_revisions.get(&id).copied() {
                    Some(entry) if !entry.is_present() => {}
                    Some(_) => {
                        data.variables.insert(id, variable);
                    }
                    None => {
                        data.variables.insert(id, variable);
                        variable_revisions.insert(
                            id,
                            VariableRevisionEntry::present(
                                crate::node_system::document::ResourceRevision::INITIAL,
                            ),
                        );
                    }
                }
            }
        }
        lifecycle.commit_guard(guard, GraphLifecycleIntent::Load)?;
        publication.advance_authority_generation();
        if invalidate_all {
            self.invalidate_all_compile_products();
        } else {
            self.invalidate_graph_compile_products(&operation.owner.graph_path);
        }
        let projection_source = include_projection.then(|| {
            self.projection_source_snapshot(
                &data,
                projection_environment.expect("projection environment was captured"),
                publication.project_instance_id.clone(),
                publication.authority_generation(),
            )
        });
        drop(variable_revisions);
        drop(graph_revisions);
        drop(data);
        drop(lifecycle);
        drop(path);
        drop(publication);
        #[cfg(not(test))]
        drop(inserted);
        Ok(CommittedGraphLoad {
            #[cfg(test)]
            resource: inserted,
            projection_source,
        })
    }

    fn load_graph_for_lifecycle_commit(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
        include_projection: bool,
        before_commit: Option<&dyn Fn() -> Result<(), ProjectFilesystemError>>,
    ) -> Result<CommittedGraphLoad, ProjectFilesystemError> {
        let (operation, guard, cached) =
            self.register_graph_load_intent(expected_project_instance_id, graph_path, token)?;
        self.load_graph_for_registered_lifecycle_commit(
            operation,
            guard,
            cached,
            include_projection,
            before_commit,
        )
    }

    fn load_graph_for_registered_lifecycle_commit(
        &self,
        operation: GraphLifecycleOperation,
        mut guard: crate::project::GraphLifecycleGuard,
        cached: Option<GraphResourceDocument>,
        include_projection: bool,
        before_commit: Option<&dyn Fn() -> Result<(), ProjectFilesystemError>>,
    ) -> Result<CommittedGraphLoad, ProjectFilesystemError> {
        if let Some(graph) = cached {
            if let Some(before_commit) = before_commit {
                before_commit()?;
            }
            return self.complete_graph_load(
                &operation,
                &mut guard,
                graph,
                None,
                include_projection,
            );
        }

        let filesystem_lease = self.filesystem().acquire(operation.session.root.clone())?;
        self.validate_graph_lifecycle_operation(&operation)?;
        let loaded = crate::project::project_io::load_project_graph_document_from_file(
            operation.session.root.as_path().to_string_lossy().as_ref(),
            &operation.owner.graph_path,
        );
        if let Err(error) = &loaded {
            self.validate_graph_lifecycle_operation(&operation)?;
            return Err(ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            });
        }
        let loaded = loaded.expect("checked graph read result");
        let mut graph = loaded.document;
        graph.revision = loaded.revision;
        let resource = GraphResourceDocument {
            name: loaded.name,
            kind: loaded.kind,
            document: graph,
            function: loaded.function,
        };
        drop(filesystem_lease);
        if let Some(before_commit) = before_commit {
            before_commit()?;
        }
        self.complete_graph_load(
            &operation,
            &mut guard,
            resource,
            Some(loaded.local_variables),
            include_projection,
        )
    }

    #[cfg(test)]
    pub(crate) fn load_graph_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
    ) -> Result<GraphResourceDocument, ProjectFilesystemError> {
        self.load_graph_for_lifecycle_commit(
            expected_project_instance_id,
            graph_path,
            token,
            false,
            None,
        )
        .map(|committed| committed.resource)
    }

    pub fn load_graph_projection(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        lifecycle_token: u64,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, ProjectFilesystemError> {
        let committed = self.load_graph_for_lifecycle_commit(
            expected_project_instance_id,
            graph_path,
            lifecycle_token,
            true,
            None,
        )?;
        committed
            .projection_source
            .as_ref()
            .expect("projection load requests a projection snapshot")
            .graph_projection(graph_path, locale)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })
    }

    pub fn unload_graph_resource_for_lifecycle(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
    ) -> Result<bool, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: format!(
                    "graph '{}' belongs to a different project instance",
                    graph_path
                ),
            });
        }
        let mut guard = self.graph_lifecycle.register(
            &session,
            graph_path,
            token,
            GraphLifecycleIntent::Unload,
        )?;
        let operation = GraphLifecycleOperation::from_guard(session, &guard);
        let mut publication = self.mutation_publication.lock().unwrap();
        let path = self.project_path.read().unwrap();
        if publication.project_instance_id != operation.session.instance_id.as_str()
            || path.is_none()
        {
            return Err(operation.stale_error());
        }
        let mut lifecycle = self.graph_lifecycle.boundary();
        lifecycle.validate(&operation.owner)?;
        self.ensure_project_operational()?;
        let graph_path_text = graph_path.as_str();
        let mut data = self.project_data.write().unwrap();
        let removed = data.graphs.remove(graph_path);
        let graph_removed = removed.is_some();
        let variable_count = data.variables.len();
        data.variables.retain(|_, variable| match &variable.scope {
            crate::variable::VariableScope::Global => true,
            crate::variable::VariableScope::Event { event_path } => event_path != graph_path_text,
            crate::variable::VariableScope::Function { function_path } => {
                function_path != graph_path_text
            }
        });
        self.history.write().unwrap().clear();
        lifecycle.commit_guard(&mut guard, GraphLifecycleIntent::Unload)?;
        let variables_removed = data.variables.len() != variable_count;
        let changed = graph_removed || variables_removed;
        if changed {
            publication.advance_authority_generation();
            if variables_removed
                || removed.as_ref().is_some_and(|resource| {
                    resource.kind == crate::project::GraphDocumentKind::Function
                })
            {
                self.invalidate_all_compile_products();
            } else {
                self.invalidate_graph_compile_products(graph_path);
            }
        }
        Ok(changed)
    }

    pub fn apply_editor_graph_mutation(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<EditorGraphMutationDto>,
    ) -> Result<GraphMutationResultDto, MutationConflict> {
        self.apply_editor_graph_mutation_observed(graph_path, locale, request, |_| {})
    }

    pub fn apply_editor_graph_mutation_observed(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<EditorGraphMutationDto>,
        observe: impl FnOnce(&GraphDeltaEvent<GraphDocumentPatch>),
    ) -> Result<GraphMutationResultDto, MutationConflict> {
        self.ensure_mutation_operational()?;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let expected_resource = ResourceKey::Graph(node_path.clone());
        if request.resource != expected_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: expected_resource,
            });
        }
        let document = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .map(|resource| resource.document.clone())
            .ok_or_else(|| MutationConflict::ResourceMismatch {
                requested: request.resource.clone(),
                store: ResourceKey::Graph(node_path.clone()),
            })?;
        if request.base_revision != document.revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision: document.revision,
            });
        }
        let registry = {
            let store = self.project_store.read().unwrap();
            Arc::clone(&store.node_registry)
        };
        let patch = request
            .payload
            .into_patch(&node_path, &document, registry.as_ref())?;
        let committed = self.commit_graph_patch(
            graph_path,
            MutationRequest::new(
                request.resource,
                request.base_revision,
                request.operation_id,
                patch,
            ),
        )?;
        observe(&committed.delta);
        let projection_replacement = crate::event::GraphProjectionReplacementDto {
            graph_path: graph_path.as_str().to_string(),
            projection: committed
                .projection_source
                .graph_projection(graph_path, locale)
                .map_err(|error| MutationConflict::Projection(error.into()))?,
        };
        Ok(GraphMutationResultDto {
            delta: committed.delta,
            projection_replacement,
            history: committed.history,
        })
    }

    pub fn apply_graph_mutation(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphMutation>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        self.ensure_mutation_operational()?;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        self.ensure_mutation_operational()?;
        let resource = data.graphs.get(graph_path).cloned().ok_or_else(|| {
            MutationConflict::ResourceMismatch {
                requested: request.resource.clone(),
                store: ResourceKey::Graph(node_path.clone()),
            }
        })?;
        let mut planner = RevisionedGraphStore::new(node_path.clone(), resource.document.clone());
        let event = planner.apply_mutation(request)?;
        let mut documents = ProjectDocumentState::new(
            data.graphs
                .iter()
                .map(|(path, graph)| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        graph.document.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let transaction = ProjectHistoryTransaction::graph(
            event
                .caused_by
                .expect("mutation events carry operation IDs"),
            node_path,
            event.from_revision,
            event.payload.clone(),
        );
        self.history
            .write()
            .unwrap()
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.run_mutation_publication_test_hook();
        for (path, graph) in &mut data.graphs {
            let key = crate::node_system::document::GraphResourcePath(path.as_str().into());
            if let Some(document) = documents.graphs.remove(&key) {
                graph.document = document;
            }
        }
        publication.advance_authority_generation();
        self.invalidate_graph_body_compile_products(graph_path, resource.kind);
        Ok(event)
    }

    pub fn apply_graph_patch(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphDocumentPatch>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        self.commit_graph_patch(graph_path, request)
            .map(|committed| committed.delta)
    }

    fn commit_graph_patch(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphDocumentPatch>,
    ) -> Result<CommittedGraphMutation, MutationConflict> {
        self.ensure_mutation_operational()?;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let expected_resource = ResourceKey::Graph(node_path.clone());
        if request.resource != expected_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: expected_resource,
            });
        }
        let expected_session = self.current_projection_environment_expectation();
        let projection_environment = self
            .capture_projection_environment(&expected_session)
            .map_err(|error| MutationConflict::Projection(error.into()))?;
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_session.project_instance_id.as_str() {
            return Err(MutationConflict::Projection(
                "stale_project_lifecycle: project changed before graph authority commit".into(),
            ));
        }
        if !projection_environment.matches_publication(&publication) {
            return Err(MutationConflict::Projection(
                "stale_project_lifecycle: projection environment changed before graph authority commit"
                    .into(),
            ));
        }
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        self.ensure_mutation_operational()?;
        let resource =
            data.graphs
                .get(graph_path)
                .ok_or_else(|| MutationConflict::ResourceMismatch {
                    requested: expected_resource.clone(),
                    store: expected_resource.clone(),
                })?;
        if resource.document.revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision: resource.document.revision,
            });
        }
        let graph_kind = resource.kind;
        let mut documents = ProjectDocumentState::new(
            data.graphs
                .iter()
                .map(|(path, graph)| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        graph.document.clone(),
                    )
                })
                .collect(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let transaction = ProjectHistoryTransaction::graph(
            request.operation_id,
            node_path,
            request.base_revision,
            request.payload.clone(),
        );
        let mut history = self.history.write().unwrap();
        history
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.run_mutation_publication_test_hook();
        let updated = documents
            .graphs
            .remove(&crate::node_system::document::GraphResourcePath(
                graph_path.as_str().into(),
            ))
            .expect("patched graph remains present");
        let to_revision = updated.revision;
        data.graphs
            .get_mut(graph_path)
            .expect("graph remains loaded")
            .document = updated;
        graph_revisions.insert(graph_path.clone(), to_revision);
        let history = history.status();
        publication.advance_authority_generation();
        self.invalidate_graph_body_compile_products(graph_path, graph_kind);
        let projection_source = self.projection_source_snapshot(
            &data,
            projection_environment,
            publication.project_instance_id.clone(),
            publication.authority_generation(),
        );
        Ok(CommittedGraphMutation {
            delta: GraphDeltaEvent {
                graph_path: crate::node_system::document::GraphResourcePath(
                    graph_path.as_str().into(),
                ),
                from_revision: request.base_revision,
                to_revision,
                caused_by: Some(request.operation_id),
                payload: request.payload,
            },
            projection_source,
            history,
        })
    }

    pub fn update_function_signature_observed(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<crate::node_system::document::FunctionDocumentPatch>,
        observe: impl FnOnce(&crate::event::ResourceMutationResultDto),
    ) -> Result<crate::event::ResourceMutationResultDto, MutationConflict> {
        let receipt = self.commit_function_signature(graph_path, request)?;
        let result = receipt.complete(locale);
        observe(&result);
        Ok(result)
    }

    fn commit_function_signature(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<crate::node_system::document::FunctionDocumentPatch>,
    ) -> Result<CommittedResourceMutation, MutationConflict> {
        self.ensure_mutation_operational()?;
        let function_key =
            crate::node_system::document::FunctionResourceKey(graph_path.as_str().into());
        let expected_resource = ResourceKey::Function(function_key.clone());
        if request.resource != expected_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: expected_resource,
            });
        }
        let expected_session = self.current_projection_environment_expectation();
        let projection_environment = self
            .capture_projection_environment(&expected_session)
            .map_err(|error| MutationConflict::Projection(error.into()))?;
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_session.project_instance_id.as_str() {
            return Err(MutationConflict::Projection(
                "stale_project_lifecycle: project changed before signature authority commit".into(),
            ));
        }
        if !projection_environment.matches_publication(&publication) {
            return Err(MutationConflict::Projection(
                "stale_project_lifecycle: projection environment changed before signature authority commit"
                    .into(),
            ));
        }
        let mut data = self.project_data.write().unwrap();
        self.ensure_mutation_operational()?;
        let function = data
            .graphs
            .get(graph_path)
            .and_then(|resource| resource.function.as_ref())
            .ok_or_else(|| MutationConflict::ResourceMismatch {
                requested: expected_resource.clone(),
                store: expected_resource.clone(),
            })?;
        if function.revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision: function.revision,
            });
        }
        if function.signature != request.payload.before {
            return Err(MutationConflict::History(
                "function patch before-state does not match the current signature".into(),
            ));
        }
        let from_revision = function.revision;
        let mut revisions = self.variable_revisions.write().unwrap();
        let mut documents = project_documents(&data, &revisions);
        let transaction = crate::node_system::document::ProjectHistoryTransaction::new(
            request.operation_id,
            vec![crate::node_system::document::ResourcePatch::function(
                function_key,
                from_revision,
                request.payload.clone(),
            )],
        );
        let mut history = self.history.write().unwrap();
        history
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.run_mutation_publication_test_hook();
        let to_revision = documents.functions[match &expected_resource {
            ResourceKey::Function(key) => key,
            _ => unreachable!(),
        }]
        .revision;
        replace_project_documents(&mut data, &mut revisions, documents);
        let deltas = vec![crate::node_system::document::ResourceDeltaEvent {
            resource: expected_resource,
            from_revision,
            to_revision,
            caused_by: Some(request.operation_id),
            payload: crate::node_system::document::ResourceDocumentPatch::Function(request.payload),
        }];
        let expected_graph_paths = affected_projection_paths(&deltas, &data);
        let publication_revision = publication.allocate_resource_revision();
        self.invalidate_all_compile_products();
        let projection_source = self.projection_source_snapshot(
            &data,
            projection_environment,
            publication.project_instance_id.clone(),
            publication.authority_generation(),
        );
        #[cfg(test)]
        let completion_test_hook = self
            .committed_resource_completion_test_hook
            .read()
            .unwrap()
            .clone();
        Ok(CommittedResourceMutation {
            operation_id: request.operation_id,
            project_instance_id: publication.project_instance_id.clone(),
            publication_revision,
            moves: Vec::new(),
            deltas,
            history: history.status(),
            projection_source,
            expected_graph_paths,
            #[cfg(test)]
            completion_test_hook,
        })
    }

    pub fn undo_last_transaction_observed(
        &self,
        locale: &str,
        request: MutationRequest<HistoryMutation>,
        observe: impl FnOnce(&crate::event::ResourceMutationResultDto),
    ) -> Result<crate::event::ResourceMutationResultDto, MutationConflict> {
        let receipt = self.commit_history_direction(true, request)?;
        let result = receipt.complete(locale);
        observe(&result);
        Ok(result)
    }

    pub fn redo_last_transaction_observed(
        &self,
        locale: &str,
        request: MutationRequest<HistoryMutation>,
        observe: impl FnOnce(&crate::event::ResourceMutationResultDto),
    ) -> Result<crate::event::ResourceMutationResultDto, MutationConflict> {
        let receipt = self.commit_history_direction(false, request)?;
        let result = receipt.complete(locale);
        observe(&result);
        Ok(result)
    }

    fn commit_history_direction(
        &self,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<CommittedResourceMutation, MutationConflict> {
        self.ensure_mutation_operational()?;
        let next_transaction = {
            let history = self.history.read().unwrap();
            if undo {
                history.next_undo()
            } else {
                history.next_redo()
            }
            .cloned()
        };
        if let Some(transaction) = next_transaction {
            match transaction.persistence {
                crate::node_system::document::HistoryPersistencePolicy::DurableResourceMove => {
                    return self.commit_graph_move_history_direction(undo, request, transaction);
                }
                crate::node_system::document::HistoryPersistencePolicy::DurableVariableEffects => {
                    return self.commit_variable_effect_history_direction(
                        undo,
                        request,
                        transaction,
                    );
                }
                crate::node_system::document::HistoryPersistencePolicy::InMemoryUntilSave => {}
            }
        }
        let expected_session = self.current_projection_environment_expectation();
        let projection_environment = self
            .capture_projection_environment(&expected_session)
            .map_err(|error| MutationConflict::Projection(error.into()))?;
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_session.project_instance_id.as_str() {
            return Err(MutationConflict::Projection(
                "stale_project_lifecycle: project changed before history authority commit".into(),
            ));
        }
        if !projection_environment.matches_publication(&publication) {
            return Err(MutationConflict::Projection(
                "stale_project_lifecycle: projection environment changed before history authority commit"
                    .into(),
            ));
        }
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        self.ensure_mutation_operational()?;
        let mut documents = project_documents(&data, &revisions);
        let current_revision = try_project_document_revision(&documents, &request.resource)
            .ok_or_else(|| {
                MutationConflict::History(
                    format!(
                        "history anchor resource {:?} was not found",
                        request.resource
                    )
                    .into(),
                )
            })?;
        if current_revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision,
            });
        }

        let before = documents.clone();
        let mut history = self.history.write().unwrap();
        let transaction = if undo {
            history.undo(&mut documents)
        } else {
            history.redo(&mut documents)
        }
        .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.run_mutation_publication_test_hook();
        let deltas = transaction
            .changes
            .iter()
            .map(|change| crate::node_system::document::ResourceDeltaEvent {
                resource: change.resource.clone(),
                from_revision: project_document_revision(&before, &change.resource),
                to_revision: project_document_revision(&documents, &change.resource),
                caused_by: Some(request.operation_id),
                payload: if undo {
                    change.inverse.clone()
                } else {
                    change.forward.clone()
                },
            })
            .collect::<Vec<_>>();
        replace_project_documents(&mut data, &mut revisions, documents);
        for (path, graph) in &data.graphs {
            graph_revisions.insert(path.clone(), graph.document.revision);
        }
        let expected_graph_paths = affected_projection_paths(&deltas, &data);
        let publication_revision = publication.allocate_resource_revision();
        self.invalidate_all_compile_products();
        let projection_source = self.projection_source_snapshot(
            &data,
            projection_environment,
            publication.project_instance_id.clone(),
            publication.authority_generation(),
        );
        #[cfg(test)]
        let completion_test_hook = self
            .committed_resource_completion_test_hook
            .read()
            .unwrap()
            .clone();
        Ok(CommittedResourceMutation {
            operation_id: request.operation_id,
            project_instance_id: publication.project_instance_id.clone(),
            publication_revision,
            moves: Vec::new(),
            deltas,
            history: history.status(),
            projection_source,
            expected_graph_paths,
            #[cfg(test)]
            completion_test_hook,
        })
    }

    fn commit_variable_effect_history_direction(
        &self,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
        transaction: ProjectHistoryTransaction,
    ) -> Result<CommittedResourceMutation, MutationConflict> {
        let history_id = transaction.history_id.clone();
        let session = self
            .capture_project_session()
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let expected_project_path = self.get_path().ok_or_else(|| {
            MutationConflict::History("no project is active for variable persistence".into())
        })?;
        let projection_environment = self
            .capture_projection_environment_for_session(&session)
            .map_err(|error| MutationConflict::History(error.into()))?;
        let filesystem_lease = self
            .filesystem()
            .acquire(session.root.clone())
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.validate_project_session(&session)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;

        let (data_snapshot, graph_revisions, variable_revisions, history_snapshot) = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != session.instance_id.as_str()
                || path.as_deref() != Some(expected_project_path.as_str())
            {
                return Err(MutationConflict::History(
                    "project changed before durable history snapshot".into(),
                ));
            }
            (
                self.project_data.read().unwrap().clone(),
                self.graph_revisions.read().unwrap().clone(),
                self.variable_revisions.read().unwrap().clone(),
                self.history.read().unwrap().clone(),
            )
        };
        let mut documents = project_documents(&data_snapshot, &variable_revisions);
        let current_revision = try_project_document_revision(&documents, &request.resource)
            .ok_or_else(|| {
                MutationConflict::History(
                    format!(
                        "history anchor resource {:?} was not found",
                        request.resource
                    )
                    .into(),
                )
            })?;
        if current_revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision,
            });
        }
        let before = documents.clone();
        let mut proposed_history = history_snapshot;
        let applied = if undo {
            proposed_history.undo(&mut documents)
        } else {
            proposed_history.redo(&mut documents)
        }
        .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        if applied.history_id != history_id {
            return Err(MutationConflict::History(
                crate::node_system::document::HistoryError::HistoryHeadChanged
                    .to_string()
                    .into(),
            ));
        }
        let mut proposed_data = data_snapshot.clone();
        let mut proposed_revisions = variable_revisions.clone();
        replace_project_documents(
            &mut proposed_data,
            &mut proposed_revisions,
            documents.clone(),
        );
        let ids = install_variable_effect_snapshots(&mut proposed_data, &transaction, undo)
            .map_err(|error| MutationConflict::History(error.into()))?;
        let _cache_updates = variable_cache_updates(&proposed_data, &ids)
            .map_err(|error| MutationConflict::History(error.into()))?;

        let mut expected_revisions = BTreeMap::new();
        for change in &transaction.changes {
            expected_revisions.insert(
                change.resource.clone(),
                project_document_revision(&before, &change.resource),
            );
        }
        for id in &ids {
            let scope = variable_history_scope(&proposed_data, &transaction, *id, undo)
                .map_err(|error| MutationConflict::History(error.into()))?;
            if let Some(graph_path) = variable_scope_graph_path(&scope)
                .map_err(|error| MutationConflict::History(error.into()))?
            {
                let revision = graph_revisions.get(&graph_path).copied().ok_or_else(|| {
                    MutationConflict::History(
                        format!("local variable graph '{graph_path}' is not loaded").into(),
                    )
                })?;
                expected_revisions.insert(
                    ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                        graph_path.as_str().into(),
                    )),
                    revision,
                );
            }
        }
        let mutations =
            variable_effect_filesystem_mutations(&proposed_data, &ids, &transaction, undo)
                .map_err(|error| MutationConflict::History(error.into()))?;
        let context = ProjectTransactionContext {
            session,
            operation_id: request.operation_id,
            affected_resources: expected_revisions.keys().cloned().collect(),
            expected_revisions,
            expected_absent_resources: Default::default(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            filesystem_lease,
            mutations,
            validate_variable_effect_document,
        )
        .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let committed_filesystem = prepared
            .commit()
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;

        let authority_result = (|| {
            let mut publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != context.session.instance_id.as_str()
                || path.as_deref() != Some(expected_project_path.as_str())
            {
                return Err(MutationConflict::History(
                    "project changed before durable history authority commit".into(),
                ));
            }
            if !projection_environment.matches_publication(&publication) {
                return Err(MutationConflict::History(
                    "projection environment changed before durable history authority commit".into(),
                ));
            }
            let mut data = self.project_data.write().unwrap();
            let mut store = self.project_store.write().unwrap();
            let graph_revisions = self.graph_revisions.read().unwrap();
            let mut revisions = self.variable_revisions.write().unwrap();
            validate_context_revisions(
                &context,
                &data,
                &graph_revisions,
                &revisions,
                &self.worksheet_revisions.read().unwrap(),
            )
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
            self.run_mutation_publication_test_hook();
            let current_history = self.history.read().unwrap();
            let current_head = if undo {
                current_history.next_undo()
            } else {
                current_history.next_redo()
            };
            if current_head.map(|entry| &entry.history_id) != Some(&history_id) {
                return Err(MutationConflict::History(
                    crate::node_system::document::HistoryError::HistoryHeadChanged
                        .to_string()
                        .into(),
                ));
            }
            let mut next_history = current_history.clone();
            drop(current_history);
            let mut current_documents = project_documents(&data, &revisions);
            let before = current_documents.clone();
            let applied = if undo {
                next_history.undo(&mut current_documents)
            } else {
                next_history.redo(&mut current_documents)
            }
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
            if applied.history_id != history_id {
                return Err(MutationConflict::History(
                    crate::node_system::document::HistoryError::HistoryHeadChanged
                        .to_string()
                        .into(),
                ));
            }
            let mut next_data = data.clone();
            let mut next_revisions = revisions.clone();
            replace_project_documents(
                &mut next_data,
                &mut next_revisions,
                current_documents.clone(),
            );
            let installed_ids =
                install_variable_effect_snapshots(&mut next_data, &transaction, undo)
                    .map_err(|error| MutationConflict::History(error.into()))?;
            let cache_updates = variable_cache_updates(&next_data, &installed_ids)
                .map_err(|error| MutationConflict::History(error.into()))?;
            let deltas = transaction
                .changes
                .iter()
                .map(|change| crate::node_system::document::ResourceDeltaEvent {
                    resource: change.resource.clone(),
                    from_revision: project_document_revision(&before, &change.resource),
                    to_revision: project_document_revision(&current_documents, &change.resource),
                    caused_by: Some(request.operation_id),
                    payload: if undo {
                        change.inverse.clone()
                    } else {
                        change.forward.clone()
                    },
                })
                .collect::<Vec<_>>();
            let expected_graph_paths = affected_projection_paths(&deltas, &next_data);
            *data = next_data;
            *revisions = next_revisions;
            apply_variable_cache_updates(&mut store, cache_updates);
            let history_status = next_history.status();
            *self.history.write().unwrap() = next_history;
            drop(store);
            let publication_revision = publication.allocate_resource_revision();
            self.invalidate_all_compile_products();
            let projection_source = self.projection_source_snapshot(
                &data,
                projection_environment,
                publication.project_instance_id.clone(),
                publication.authority_generation(),
            );
            #[cfg(test)]
            let completion_test_hook = self
                .committed_resource_completion_test_hook
                .read()
                .unwrap()
                .clone();
            Ok(CommittedResourceMutation {
                operation_id: request.operation_id,
                project_instance_id: publication.project_instance_id.clone(),
                publication_revision,
                moves: Vec::new(),
                deltas,
                history: history_status,
                projection_source,
                expected_graph_paths,
                #[cfg(test)]
                completion_test_hook,
            })
        })();

        match authority_result {
            Ok(result) => {
                committed_filesystem.finalize();
                Ok(result)
            }
            Err(error) => {
                let rollback = committed_filesystem.rollback();
                Err(rollback
                    .err()
                    .map(|rollback| MutationConflict::History(rollback.to_string().into()))
                    .unwrap_or(error))
            }
        }
    }

    fn commit_graph_move_history_direction(
        &self,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
        transaction: ProjectHistoryTransaction,
    ) -> Result<CommittedResourceMutation, MutationConflict> {
        let history_id = transaction.history_id.clone();
        let move_patch = transaction.graph_resource_move.ok_or_else(|| {
            MutationConflict::History("graph move history patch is missing".into())
        })?;
        let payload: GraphMoveHistoryPayload = serde_json::from_value(move_patch.payload)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let source = GraphResourcePath::new(if undo {
            move_patch.to.0.as_ref()
        } else {
            move_patch.from.0.as_ref()
        })
        .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let target = GraphResourcePath::new(if undo {
            move_patch.from.0.as_ref()
        } else {
            move_patch.to.0.as_ref()
        })
        .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let mut desired_moved = if undo {
            payload.moved_before.clone()
        } else {
            payload.moved_after.clone()
        };
        let desired_graphs = if undo {
            payload.referenced_graphs_before.clone()
        } else {
            payload.referenced_graphs_after.clone()
        };
        let desired_variables = if undo {
            payload.referenced_variables_before.clone()
        } else {
            payload.referenced_variables_after.clone()
        };

        let session = self
            .capture_project_session()
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let projection_environment = self
            .capture_projection_environment_for_session(&session)
            .map_err(|error| MutationConflict::History(error.into()))?;
        let filesystem_lease = self
            .filesystem()
            .acquire(session.root.clone())
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.validate_project_session(&session)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let loaded_source = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(&source)
            .cloned();
        let current_moved = loaded_source
            .clone()
            .map_or_else(
                || {
                    load_project_graph_from_file(
                        session.root.as_path().to_string_lossy().as_ref(),
                        &source,
                    )
                },
                Ok,
            )
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        if request.resource
            != ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                source.as_str().into(),
            ))
        {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    source.as_str().into(),
                )),
            });
        }
        let current_revision = loaded_source
            .as_ref()
            .map(|resource| resource.document.revision)
            .or_else(|| self.graph_revisions.read().unwrap().get(&source).copied())
            .unwrap_or(current_moved.document.revision);
        if current_revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision,
            });
        }
        desired_moved.document.revision = current_revision.next();

        let mut referenced_graphs_before = BTreeMap::new();
        let mut referenced_graphs = BTreeMap::new();
        let mut referenced_variables_before = BTreeMap::new();
        let mut referenced_variables = BTreeMap::new();
        let mut affected_resources = Vec::new();
        let mut expected_revisions = BTreeMap::new();
        let source_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
            source.as_str().into(),
        ));
        if loaded_source.is_some() {
            affected_resources.push(source_key.clone());
        }
        expected_revisions.insert(source_key, current_revision);
        {
            let data = self.project_data.read().unwrap();
            let variable_revisions = self.variable_revisions.read().unwrap();
            for (path, desired) in desired_graphs {
                let Some(current) = data.graphs.get(&path) else {
                    continue;
                };
                let mut next = desired;
                next.document.revision = current.document.revision.next();
                let key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    path.as_str().into(),
                ));
                affected_resources.push(key.clone());
                expected_revisions.insert(key, current.document.revision);
                referenced_graphs_before.insert(path.clone(), current.clone());
                referenced_graphs.insert(path, next);
            }
            for (id, desired) in desired_variables {
                let Some(current) = data.variables.get(&id) else {
                    continue;
                };
                let key = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                    format!("variables/{id}").into(),
                ));
                affected_resources.push(key.clone());
                expected_revisions.insert(
                    key,
                    variable_revisions
                        .get(&id)
                        .map(|entry| entry.revision)
                        .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
                );
                referenced_variables_before.insert(id, current.clone());
                referenced_variables.insert(id, desired);
            }
        }
        let loaded_referenced_graphs = referenced_graphs.keys().cloned().collect();
        let known_graph_revisions = self.graph_revisions.read().unwrap().clone();
        let disk_plan = Self::graph_rename_mutations(
            session.root.as_path(),
            &source,
            &target,
            &desired_moved,
            referenced_variables
                .values()
                .cloned()
                .map(|variable| (variable.id, variable))
                .collect(),
            &loaded_referenced_graphs,
            &known_graph_revisions,
        )
        .map_err(|error| MutationConflict::History(error.into()))?;
        for (path, before) in disk_plan.referenced_graphs_before {
            let key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                path.as_str().into(),
            ));
            affected_resources.push(key.clone());
            expected_revisions.insert(key, before.document.revision);
            referenced_graphs_before.insert(path, before);
        }
        referenced_graphs.extend(disk_plan.referenced_graphs_after);
        let context = ProjectTransactionContext {
            session,
            operation_id: request.operation_id,
            affected_resources,
            expected_revisions,
            expected_absent_resources: [ResourceKey::Graph(
                crate::node_system::document::GraphResourcePath(target.as_str().into()),
            )]
            .into_iter()
            .collect(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let mutations = disk_plan.mutations;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            filesystem_lease,
            mutations,
            |path, contents| {
                if path == std::path::Path::new(crate::project::GLOBAL_VARIABLES_FILE) {
                    serde_json::from_slice::<crate::project::project_io::GlobalVariablesDocument>(
                        contents,
                    )
                    .map(|_| ())
                    .map_err(|error| error.to_string())
                } else {
                    serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            },
        )
        .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let committed_filesystem = prepared
            .commit()
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.run_graph_move_history_io_checkpoint();
        let publication = self.apply_resource_document_patch_internal(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: source,
                to: target,
                moved_before: current_moved,
                moved: desired_moved,
                referenced_graphs_before,
                referenced_graphs,
                loaded_referenced_graphs,
                referenced_variables_before,
                referenced_variables,
            },
            Some((undo, history_id)),
            Some(projection_environment),
            None,
        );
        match publication {
            Ok(receipt) => {
                committed_filesystem.finalize();
                Ok(receipt)
            }
            Err(error) => {
                let rollback = committed_filesystem.rollback();
                Err(MutationConflict::History(
                    rollback.err().unwrap_or(error).to_string().into(),
                ))
            }
        }
    }

    pub fn graph_projection(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, String> {
        self.ensure_project_operational()
            .map_err(|error| format!("{}: {error}", error.code()))?;
        self.capture_projection_source(graph_path)?
            .graph_projection(graph_path, locale)
    }

    #[cfg(test)]
    pub(super) fn set_function_load_checkpoint(
        &self,
        checkpoint: Arc<dyn Fn(&crate::node_system::runtime::CancellationToken) + Send + Sync>,
    ) {
        *self.function_load_checkpoint.write().unwrap() = Some(checkpoint);
    }

    fn load_function_resources(
        &self,
        cancellation: &crate::node_system::runtime::CancellationToken,
    ) -> Result<(), String> {
        cancellation.check().map_err(|error| error.to_string())?;
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let loaded_paths = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let function_paths = self
            .read_project_index(&session.instance_id)
            .map_err(|error| error.to_string())?
            .graphs
            .into_iter()
            .filter(|entry| entry.graph_type == crate::project::GraphDocumentKind::Function)
            .map(|entry| GraphResourcePath::new(entry.path).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        for path in function_paths {
            if loaded_paths.contains(&path) {
                continue;
            }
            cancellation.check().map_err(|error| error.to_string())?;
            let guard = self
                .graph_lifecycle
                .allocate_and_register(&session, &path, GraphLifecycleIntent::Load)
                .map_err(|error| error.to_string())?;
            let operation = GraphLifecycleOperation::from_guard(session.clone(), &guard);
            let cached = self.project_data.read().unwrap().graphs.get(&path).cloned();
            let before_commit = || {
                cancellation.check().map_err(|error| {
                    ProjectFilesystemError::StaleProjectLifecycle {
                        message: error.to_string(),
                    }
                })?;
                #[cfg(test)]
                if let Some(checkpoint) = self.function_load_checkpoint.read().unwrap().clone() {
                    checkpoint(cancellation);
                }
                cancellation.check().map_err(|error| {
                    ProjectFilesystemError::StaleProjectLifecycle {
                        message: error.to_string(),
                    }
                })
            };
            self.load_graph_for_registered_lifecycle_commit(
                operation,
                guard,
                cached,
                false,
                Some(&before_commit),
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn result_source_descriptor(
        &self,
        source_id: crate::node_system::runtime::ResultSourceId,
    ) -> Result<Option<crate::node_system::runtime::ResultSourceDescriptor>, ProjectFilesystemError>
    {
        self.ensure_project_operational()?;
        Ok(self
            .project_store
            .read()
            .unwrap()
            .results
            .descriptor(source_id))
    }

    pub fn result_source_value(
        &self,
        source_id: crate::node_system::runtime::ResultSourceId,
    ) -> Result<Option<Arc<crate::node_system::runtime::ArtifactSnapshot>>, ProjectFilesystemError>
    {
        self.ensure_project_operational()?;
        Ok(self.project_store.read().unwrap().results.value(source_id))
    }

    pub fn result_source_page(
        &self,
        source_id: crate::node_system::runtime::ResultSourceId,
        offset: usize,
        limit: usize,
    ) -> Result<Option<crate::node_system::runtime::ResultSourcePage>, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        Ok(self
            .project_store
            .read()
            .unwrap()
            .results
            .page(source_id, offset, limit))
    }

    pub fn release_result_source(
        &self,
        source_id: crate::node_system::runtime::ResultSourceId,
    ) -> Result<bool, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        Ok(self
            .project_store
            .read()
            .unwrap()
            .results
            .release(source_id))
    }

    pub fn release_run_result_sources(
        &self,
        run_id: crate::node_system::analysis::RunId,
    ) -> Result<usize, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        Ok(self
            .project_store
            .read()
            .unwrap()
            .results
            .release_run_sources(run_id))
    }

    fn capture_execution_snapshot(
        &self,
        graph_path: &GraphResourcePath,
        compilation: &super::compile_publication::CurrentCompilation,
    ) -> Result<ExecutionSnapshot, String> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != compilation.authority.project_instance_id
            || publication.authority_generation() != compilation.authority.authority_generation
        {
            return Err(
                "stale_project_lifecycle: execution authority changed before snapshot".into(),
            );
        }
        let data = self.project_data.read().unwrap().clone();
        let variable_revisions = self.variable_revisions.read().unwrap().clone();
        let store = self.project_store.read().unwrap();
        let database_instances = store.databases.clone();
        let registry = Arc::clone(&store.node_registry);
        let kernels = Arc::clone(&store.kernels);
        let functions = Arc::clone(&store.function_plans);
        let results = store.results.clone();
        let runs = Arc::clone(&store.runs);
        let session_id = store.project_session_id.clone();
        drop(store);
        let identity = self.current_projection_environment_expectation();
        if !self.execution_authority_matches(&publication, &compilation.authority)
            || session_id != compilation.authority.project_session_id
        {
            return Err(
                "stale_project_lifecycle: execution authority changed before snapshot".into(),
            );
        }
        let document = data
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.clone())
            .ok_or_else(|| format!("graph '{}' not loaded", graph_path))?;
        Ok(ExecutionSnapshot {
            document,
            data,
            database_instances,
            variable_revisions,
            project_root: identity.project_root,
            database_schemas: compilation.source.environment.database_schemas.clone(),
            registry,
            kernels,
            functions,
            results,
            runs,
            session_id,
        })
    }

    fn validate_execution_authority(
        &self,
        authority: &super::compile_publication::ExecutionAuthorityToken,
    ) -> Result<(), String> {
        let publication = self.mutation_publication.lock().unwrap();
        self.execution_authority_matches(&publication, authority)
            .then_some(())
            .ok_or_else(|| "stale_project_lifecycle: execution authority changed before run".into())
    }

    pub fn execute_graph(
        &self,
        graph_path: &GraphResourcePath,
        events: &dyn crate::node_system::runtime::RunEventSink,
    ) -> Result<crate::node_system::runtime::RunResult, String> {
        self.ensure_project_operational()
            .map_err(|error| format!("{}: {error}", error.code()))?;
        let cancellation = crate::node_system::runtime::CancellationToken::new();
        let store = self.project_store.read().unwrap();
        let runs = Arc::clone(&store.runs);
        let session_id = store.project_session_id.clone();
        let trace_sink = Arc::clone(&store.trace_sink);
        let pre_run = runs
            .track_pre_run(session_id.clone(), cancellation.clone())
            .map_err(|error| error.to_string())?;
        drop(store);

        self.load_function_resources(&cancellation)?;
        let compilation =
            self.get_or_compile_current(graph_path, &session_id, trace_sink.as_ref())?;
        let plan = compilation
            .plan
            .as_ref()
            .map(|projection| projection.payload.clone())
            .ok_or_else(|| {
                let codes = compilation
                    .analysis
                    .payload
                    .analysis
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("execution refused because graph has blocking diagnostics: {codes}")
            })?;
        cancellation.check().map_err(|error| error.to_string())?;
        let execution = self.capture_execution_snapshot(graph_path, &compilation)?;
        let compile_resources =
            compile_resources_from_data(&execution.data, execution.database_schemas.clone())?;
        if compile_resources.versions != compilation.authority.basis.resource_versions {
            return Err("stale_project_lifecycle: execution resource basis changed".into());
        }
        let resource_snapshot = snapshot_execution_resources(&execution, compile_resources)?;
        let compile_cancellation =
            crate::node_system::compiler::CompileCancellationToken::from_shared(
                cancellation.shared_flag(),
            );
        let mut compiled_parameters = crate::node_system::runtime::CompiledParameterStore::new();
        let function_generation = publish_function_plans(
            execution.registry.as_ref(),
            execution.functions.as_ref(),
            &resource_snapshot.compile,
            execution.session_id.clone(),
            trace_sink.as_ref(),
            &compile_cancellation,
            &mut compiled_parameters,
        )?;
        #[cfg(test)]
        let production_relational_observer =
            self.production_relational_observer.read().unwrap().clone();
        #[cfg(test)]
        if let Some(observer) = &production_relational_observer {
            observer.observe_plan(&plan);
        }
        #[cfg(test)]
        let mut resources =
            crate::node_system::runtime::ProjectResourceProvider::new(resource_snapshot.runtime);
        #[cfg(not(test))]
        let resources =
            crate::node_system::runtime::ProjectResourceProvider::new(resource_snapshot.runtime);
        #[cfg(test)]
        if let Some(observer) = self.project_resource_lease_observer.read().unwrap().clone() {
            resources.set_lease_observer(observer);
        }
        build_run_parameters(&mut compiled_parameters, &execution.document, &plan)?;
        let mut relational_backends = crate::node_system::runtime::RelationalBackendRegistry::new();
        #[cfg(test)]
        let production_relational_backend = production_relational_observer
            .map(crate::node_system::runtime::ProductionRelationalBackend::with_observer)
            .unwrap_or_default();
        #[cfg(not(test))]
        let production_relational_backend =
            crate::node_system::runtime::ProductionRelationalBackend::default();
        relational_backends
            .register(
                crate::node_system::plan::RelationalBackendId::new("relational.default")
                    .map_err(|error| error.to_string())?,
                production_relational_backend,
            )
            .map_err(|error| error.to_string())?;
        self.run_execution_before_final_gate_test_hook();
        self.validate_execution_authority(&compilation.authority)?;
        self.run_execution_before_run_test_hook();
        let finalize =
            |result: &mut crate::node_system::runtime::RunResult,
             cancellation: &crate::node_system::runtime::CancellationToken| {
                self.run_execution_before_commit_gate_test_hook();
                let _finalization = pre_run.begin_finalization(cancellation).map_err(|error| {
                    crate::node_system::runtime::RunError::ProjectDraining(error.to_string().into())
                })?;
                cancellation.check()?;
                let effects = resources.snapshot().variable_effects();
                let committed = self
                    .commit_variable_effects(&execution.session_id, effects)
                    .map_err(|error| {
                        crate::node_system::runtime::RunError::ResourceSnapshotMismatch(
                            error.to_string().into(),
                        )
                    })?;
                result.committed_variable_ids = committed.variable_ids;
                result.resource_mutation = committed.resource_mutation;
                Ok(())
            };
        crate::node_system::runtime::RunExecutor::new(
            execution.kernels.as_ref(),
            &resources,
            &function_generation,
        )
        .with_relational_backends(&relational_backends)
        .with_compiled_parameters(&compiled_parameters)
        .with_run_registry(execution.runs.as_ref())
        .with_trace_sink(trace_sink.as_ref())
        .with_event_sink(events)
        .with_result_store(&execution.results)
        .with_success_finalizer(&finalize)
        .run(&plan, cancellation)
        .map_err(|error| error.to_string())
    }

    pub(super) fn commit_variable_effects(
        &self,
        expected_session_id: &crate::node_system::analysis::ProjectSessionId,
        effects: Vec<crate::node_system::runtime::VariableWriteEffect>,
    ) -> Result<VariableEffectCommitResult, VariableEffectCommitError> {
        let committed = self.commit_variable_effects_receipt(expected_session_id, effects)?;
        Ok(VariableEffectCommitResult {
            variable_ids: committed.variable_ids,
            resource_mutation: committed
                .resource_mutation
                .map(|receipt| receipt.complete("en-US")),
        })
    }

    fn commit_variable_effects_receipt(
        &self,
        expected_session_id: &crate::node_system::analysis::ProjectSessionId,
        effects: Vec<crate::node_system::runtime::VariableWriteEffect>,
    ) -> Result<CommittedVariableEffects, VariableEffectCommitError> {
        let current_session_id = self
            .project_store
            .read()
            .unwrap()
            .project_session_id
            .clone();
        if &current_session_id != expected_session_id {
            return Err(VariableEffectCommitError::SessionChanged {
                expected: expected_session_id.clone(),
                current: current_session_id,
            });
        }
        if effects.is_empty() {
            return Ok(CommittedVariableEffects {
                variable_ids: Box::new([]),
                resource_mutation: None,
            });
        }

        let session = self
            .capture_project_session()
            .map_err(variable_effect_persistence_error)?;
        let expected_project_path = self.get_path().ok_or_else(|| {
            variable_effect_persistence_error("no project is active for variable persistence")
        })?;
        let projection_environment = self
            .capture_projection_environment_for_execution_session(&session, expected_session_id)
            .map_err(variable_effect_persistence_error)?;
        let (data_snapshot, graph_revisions, variable_revisions, history_snapshot) = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != session.instance_id.as_str()
                || path.as_deref() != Some(expected_project_path.as_str())
            {
                return Err(variable_effect_persistence_error(
                    "project changed before variable persistence snapshot",
                ));
            }
            (
                self.project_data.read().unwrap().clone(),
                self.graph_revisions.read().unwrap().clone(),
                self.variable_revisions.read().unwrap().clone(),
                self.history.read().unwrap().clone(),
            )
        };

        let mut expected_revisions = BTreeMap::new();
        let mut changes = Vec::with_capacity(effects.len());
        let mut history_before = BTreeMap::new();
        let mut history_after = BTreeMap::new();
        let mut ids = Vec::with_capacity(effects.len());
        let mut local_graph_paths = std::collections::HashSet::new();
        let mut writes_globals = false;
        for effect in &effects {
            let id = variable_effect_id(effect)?;
            let resource_key = ResourceKey::Variable(
                crate::node_system::document::VariableResourceKey(effect.resource.as_str().into()),
            );
            let current = data_snapshot.variables.get(&id).ok_or_else(|| {
                VariableEffectCommitError::Conflict {
                    resource: resource_key.clone(),
                    expected_revision: effect.expected_revision,
                    current_revision: None,
                }
            })?;
            let revision = variable_revisions
                .get(&id)
                .map(|entry| entry.revision)
                .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL);
            if revision != effect.expected_revision
                || serde_json::to_value(current).map_err(variable_effect_invalid_error)?
                    != serde_json::to_value(&effect.before)
                        .map_err(variable_effect_invalid_error)?
            {
                return Err(VariableEffectCommitError::Conflict {
                    resource: resource_key,
                    expected_revision: effect.expected_revision,
                    current_revision: Some(revision),
                });
            }
            expected_revisions.insert(resource_key, revision);
            match &current.scope {
                crate::variable::VariableScope::Global => writes_globals = true,
                crate::variable::VariableScope::Event { event_path }
                | crate::variable::VariableScope::Function {
                    function_path: event_path,
                } => {
                    let graph_path = GraphResourcePath::new(event_path)
                        .map_err(variable_effect_invalid_error)?;
                    let graph_revision =
                        graph_revisions.get(&graph_path).copied().ok_or_else(|| {
                            variable_effect_persistence_error(format!(
                                "local variable graph '{}' is not loaded",
                                graph_path
                            ))
                        })?;
                    expected_revisions.insert(
                        ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                            graph_path.as_str().into(),
                        )),
                        graph_revision,
                    );
                    local_graph_paths.insert(graph_path);
                }
            }
            let variable_key =
                crate::node_system::document::VariableResourceKey(effect.resource.as_str().into());
            let mut canonical_after = current.clone();
            canonical_after.data_value = effect.after.clone();
            normalize_variable_tabular(&mut canonical_after)
                .map_err(variable_effect_invalid_error)?;
            changes.push(crate::node_system::document::ResourcePatch::variable(
                variable_key.clone(),
                revision,
                crate::node_system::document::VariableDocumentPatch::new(
                    Some(serde_json::to_value(current).map_err(variable_effect_invalid_error)?),
                    Some(
                        serde_json::to_value(&canonical_after)
                            .map_err(variable_effect_invalid_error)?,
                    ),
                ),
            ));
            history_before.insert(
                variable_key.clone(),
                Some(serde_json::to_value(current).map_err(variable_effect_invalid_error)?),
            );
            history_after.insert(
                variable_key,
                Some(serde_json::to_value(canonical_after).map_err(variable_effect_invalid_error)?),
            );
            ids.push(id);
        }

        let transaction = ProjectHistoryTransaction::durable_variable_effects(
            crate::node_system::document::OperationId::new(),
            changes,
            crate::node_system::document::VariableEffectHistorySnapshots {
                before: history_before,
                after: history_after,
            },
        );
        let deltas = transaction
            .changes
            .iter()
            .map(|change| crate::node_system::document::ResourceDeltaEvent {
                resource: change.resource.clone(),
                from_revision: change.before_revision,
                to_revision: change.after_revision,
                caused_by: Some(transaction.caused_by),
                payload: change.forward.clone(),
            })
            .collect::<Vec<_>>();
        let mut proposed_data = data_snapshot.clone();
        let mut proposed_revisions = variable_revisions.clone();
        let mut proposed_documents = project_documents(&proposed_data, &proposed_revisions);
        let mut proposed_history = history_snapshot;
        proposed_history
            .apply_transaction(&mut proposed_documents, transaction.clone())
            .map_err(|error| VariableEffectCommitError::History {
                message: error.to_string().into(),
            })?;
        replace_project_documents(
            &mut proposed_data,
            &mut proposed_revisions,
            proposed_documents,
        );
        install_variable_effect_snapshots(&mut proposed_data, &transaction, false)
            .map_err(variable_effect_invalid_error)?;
        let mut validation_store = ProjectStore::default();
        for id in &ids {
            let variable = proposed_data
                .variables
                .get_mut(id)
                .expect("effect variable exists");
            normalize_variable_tabular(variable).map_err(variable_effect_invalid_error)?;
            sync_variable_cache(&mut validation_store, variable)
                .map_err(variable_effect_invalid_error)?;
        }

        let mut mutations = Vec::new();
        if writes_globals {
            let variables = proposed_data
                .variables
                .iter()
                .filter(|(_, variable)| {
                    matches!(variable.scope, crate::variable::VariableScope::Global)
                })
                .map(|(id, variable)| (*id, variable.clone()))
                .collect();
            mutations.push(StagedFilesystemMutation::Write {
                relative_path: crate::project::GLOBAL_VARIABLES_FILE.into(),
                contents: serde_json::to_vec_pretty(
                    &crate::project::project_io::GlobalVariablesDocument {
                        schema_version: crate::project::project_io::SCHEMA_VERSION,
                        variables,
                    },
                )
                .map_err(variable_effect_invalid_error)?,
            });
        }
        for graph_path in &local_graph_paths {
            let graph = proposed_data.graphs.get(graph_path).ok_or_else(|| {
                variable_effect_persistence_error(format!(
                    "local variable graph '{}' is not loaded",
                    graph_path
                ))
            })?;
            let local_variables = proposed_data
                .variables
                .iter()
                .filter(|(_, variable)| variable_scope_matches_graph(&variable.scope, graph_path))
                .map(|(id, variable)| (*id, variable.clone()))
                .collect();
            mutations.push(StagedFilesystemMutation::Write {
                relative_path: graph_path.as_str().into(),
                contents: crate::project::project_io::serialize_graph_resource_document(
                    graph,
                    local_variables,
                )
                .map_err(variable_effect_persistence_error)?,
            });
        }

        let context = ProjectTransactionContext {
            session,
            operation_id: transaction.caused_by,
            affected_resources: expected_revisions.keys().cloned().collect(),
            expected_revisions,
            expected_absent_resources: Default::default(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let filesystem_lease = self
            .filesystem()
            .acquire(context.session.root.clone())
            .map_err(variable_effect_persistence_error)?;
        self.validate_project_session(&context.session)
            .map_err(variable_effect_persistence_error)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            filesystem_lease,
            mutations,
            |path, contents| {
                if path == std::path::Path::new(crate::project::GLOBAL_VARIABLES_FILE) {
                    serde_json::from_slice::<crate::project::project_io::GlobalVariablesDocument>(
                        contents,
                    )
                    .map(|_| ())
                    .map_err(|error| error.to_string())
                } else {
                    serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            },
        )
        .map_err(variable_effect_persistence_error)?;
        let committed_filesystem = prepared
            .commit()
            .map_err(variable_effect_persistence_error)?;

        let authority_result = (|| {
            let mut publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != context.session.instance_id.as_str()
                || path.as_deref() != Some(expected_project_path.as_str())
            {
                return Err(variable_effect_persistence_error(
                    "project changed before variable authority commit",
                ));
            }
            if !projection_environment.matches_publication(&publication) {
                return Err(variable_effect_persistence_error(
                    "projection environment changed before variable authority commit",
                ));
            }
            let mut data = self.project_data.write().unwrap();
            let mut store = self.project_store.write().unwrap();
            if &store.project_session_id != expected_session_id {
                return Err(VariableEffectCommitError::SessionChanged {
                    expected: expected_session_id.clone(),
                    current: store.project_session_id.clone(),
                });
            }
            let graph_revisions = self.graph_revisions.read().unwrap();
            let mut revisions = self.variable_revisions.write().unwrap();
            validate_context_revisions(
                &context,
                &data,
                &graph_revisions,
                &revisions,
                &self.worksheet_revisions.read().unwrap(),
            )
            .map_err(variable_effect_persistence_error)?;
            let mut documents = project_documents(&data, &revisions);
            let mut next_history = self.history.read().unwrap().clone();
            next_history
                .apply_transaction(&mut documents, transaction.clone())
                .map_err(|error| VariableEffectCommitError::History {
                    message: error.to_string().into(),
                })?;
            let mut next_data = data.clone();
            let mut next_revisions = revisions.clone();
            replace_project_documents(&mut next_data, &mut next_revisions, documents);
            let snapshot_ids =
                install_variable_effect_snapshots(&mut next_data, &transaction, false)
                    .map_err(variable_effect_invalid_error)?;
            let cache_updates = variable_cache_updates(&next_data, &snapshot_ids)
                .map_err(variable_effect_invalid_error)?;
            self.run_mutation_publication_test_hook();
            let expected_graph_paths = affected_projection_paths(&deltas, &next_data);
            *data = next_data;
            *revisions = next_revisions;
            apply_variable_cache_updates(&mut store, cache_updates);
            let history_status = next_history.status();
            *self.history.write().unwrap() = next_history;
            drop(store);
            let publication_revision = publication.allocate_resource_revision();
            self.invalidate_all_compile_products();
            let projection_source = self.projection_source_snapshot(
                &data,
                projection_environment,
                publication.project_instance_id.clone(),
                publication.authority_generation(),
            );
            #[cfg(test)]
            let completion_test_hook = self
                .committed_resource_completion_test_hook
                .read()
                .unwrap()
                .clone();
            let resource_mutation = Some(CommittedResourceMutation {
                operation_id: transaction.caused_by,
                project_instance_id: publication.project_instance_id.clone(),
                publication_revision,
                moves: Vec::new(),
                deltas: deltas.clone(),
                history: history_status,
                projection_source,
                expected_graph_paths,
                #[cfg(test)]
                completion_test_hook,
            });
            Ok(CommittedVariableEffects {
                variable_ids: ids.into_boxed_slice(),
                resource_mutation,
            })
        })();

        match authority_result {
            Ok(result) => {
                committed_filesystem.finalize();
                Ok(result)
            }
            Err(error) => {
                let rollback = committed_filesystem.rollback();
                Err(rollback
                    .err()
                    .map(variable_effect_persistence_error)
                    .unwrap_or(error))
            }
        }
    }
}

fn install_variable_effect_snapshots(
    data: &mut ProjectData,
    transaction: &ProjectHistoryTransaction,
    undo: bool,
) -> Result<Vec<crate::variable::VariableId>, String> {
    let snapshots = transaction
        .variable_effect_snapshots
        .as_ref()
        .ok_or_else(|| "durable variable-effect history is missing snapshots".to_string())?;
    let selected = if undo {
        &snapshots.before
    } else {
        &snapshots.after
    };
    let mut ids = Vec::with_capacity(selected.len());
    for (key, snapshot) in selected {
        let id = key
            .0
            .strip_prefix("variables/")
            .ok_or_else(|| format!("invalid variable history resource '{}'", key.0))
            .and_then(|value| uuid::Uuid::parse_str(value).map_err(|error| error.to_string()))
            .map(crate::variable::VariableId::from)?;
        match snapshot {
            Some(snapshot) => {
                let variable: crate::variable::VariableInstance =
                    serde_json::from_value(snapshot.clone()).map_err(|error| error.to_string())?;
                if variable.id != id {
                    return Err(format!(
                        "variable history snapshot does not match resource '{}'",
                        key.0
                    ));
                }
                data.variables.insert(id, variable);
            }
            None => {
                data.variables.remove(&id);
            }
        }
        ids.push(id);
    }
    Ok(ids)
}

fn variable_cache_updates(
    data: &ProjectData,
    ids: &[crate::variable::VariableId],
) -> Result<Vec<(String, Option<crate::tabular::VariableTabularCache>)>, String> {
    ids.iter()
        .map(|id| {
            let entry = data
                .variables
                .get(id)
                .and_then(|variable| variable.tabular.as_ref())
                .map(crate::tabular::build_variable_cache_entry)
                .transpose()?;
            Ok((crate::tabular::variable_handle(id), entry))
        })
        .collect()
}

fn apply_variable_cache_updates(
    store: &mut ProjectStore,
    updates: Vec<(String, Option<crate::tabular::VariableTabularCache>)>,
) {
    for (handle, entry) in updates {
        if let Some(entry) = entry {
            store.variable_tabular.insert(handle, entry);
        } else {
            store.variable_tabular.remove(&handle);
        }
    }
}

fn variable_effect_id(
    effect: &crate::node_system::runtime::VariableWriteEffect,
) -> Result<crate::variable::VariableId, VariableEffectCommitError> {
    effect
        .resource
        .as_str()
        .strip_prefix("variables/")
        .ok_or_else(|| VariableEffectCommitError::InvalidEffect {
            message: format!("invalid variable resource '{}'", effect.resource.as_str()).into(),
        })
        .and_then(|value| {
            uuid::Uuid::parse_str(value).map_err(|error| VariableEffectCommitError::InvalidEffect {
                message: error.to_string().into(),
            })
        })
        .map(crate::variable::VariableId::from)
}

fn variable_effect_invalid_error(error: impl ToString) -> VariableEffectCommitError {
    VariableEffectCommitError::InvalidEffect {
        message: error.to_string().into(),
    }
}

fn variable_effect_persistence_error(error: impl ToString) -> VariableEffectCommitError {
    VariableEffectCommitError::Persistence {
        message: error.to_string().into(),
    }
}

fn variable_scope_graph_path(
    scope: &crate::variable::VariableScope,
) -> Result<Option<GraphResourcePath>, String> {
    match scope {
        crate::variable::VariableScope::Global => Ok(None),
        crate::variable::VariableScope::Event { event_path }
        | crate::variable::VariableScope::Function {
            function_path: event_path,
        } => GraphResourcePath::new(event_path)
            .map(Some)
            .map_err(|error| error.to_string()),
    }
}

fn variable_history_scope(
    data: &ProjectData,
    transaction: &ProjectHistoryTransaction,
    id: crate::variable::VariableId,
    undo: bool,
) -> Result<crate::variable::VariableScope, String> {
    if let Some(variable) = data.variables.get(&id) {
        return Ok(variable.scope.clone());
    }
    let snapshots = transaction
        .variable_effect_snapshots
        .as_ref()
        .ok_or_else(|| "durable variable-effect history is missing snapshots".to_string())?;
    let opposite = if undo {
        &snapshots.after
    } else {
        &snapshots.before
    };
    let key = crate::node_system::document::VariableResourceKey(format!("variables/{id}").into());
    let snapshot = opposite
        .get(&key)
        .and_then(Option::as_ref)
        .ok_or_else(|| format!("variable history cannot recover scope for '{id}'"))?;
    let variable: crate::variable::VariableInstance =
        serde_json::from_value(snapshot.clone()).map_err(|error| error.to_string())?;
    if variable.id != id {
        return Err(format!(
            "variable history snapshot does not match resource 'variables/{id}'"
        ));
    }
    Ok(variable.scope)
}

fn variable_effect_filesystem_mutations(
    data: &ProjectData,
    ids: &[crate::variable::VariableId],
    transaction: &ProjectHistoryTransaction,
    undo: bool,
) -> Result<Vec<StagedFilesystemMutation>, String> {
    let mut writes_globals = false;
    let mut local_graph_paths = std::collections::BTreeSet::new();
    for id in ids {
        let scope = variable_history_scope(data, transaction, *id, undo)?;
        match variable_scope_graph_path(&scope)? {
            Some(path) => {
                local_graph_paths.insert(path);
            }
            None => writes_globals = true,
        }
    }

    let mut mutations = Vec::new();
    if writes_globals {
        let variables = data
            .variables
            .iter()
            .filter(|(_, variable)| {
                matches!(variable.scope, crate::variable::VariableScope::Global)
            })
            .map(|(id, variable)| (*id, variable.clone()))
            .collect();
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: crate::project::GLOBAL_VARIABLES_FILE.into(),
            contents: serde_json::to_vec_pretty(
                &crate::project::project_io::GlobalVariablesDocument {
                    schema_version: crate::project::project_io::SCHEMA_VERSION,
                    variables,
                },
            )
            .map_err(|error| error.to_string())?,
        });
    }
    for graph_path in local_graph_paths {
        let graph = data
            .graphs
            .get(&graph_path)
            .ok_or_else(|| format!("local variable graph '{graph_path}' is not loaded"))?;
        let local_variables = data
            .variables
            .iter()
            .filter(|(_, variable)| variable_scope_matches_graph(&variable.scope, &graph_path))
            .map(|(id, variable)| (*id, variable.clone()))
            .collect();
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: graph_path.as_str().into(),
            contents: crate::project::project_io::serialize_graph_resource_document(
                graph,
                local_variables,
            )
            .map_err(|error| error.to_string())?,
        });
    }
    Ok(mutations)
}

fn validate_variable_effect_document(
    path: &std::path::Path,
    contents: &[u8],
) -> Result<(), String> {
    if path == std::path::Path::new(crate::project::GLOBAL_VARIABLES_FILE) {
        serde_json::from_slice::<crate::project::project_io::GlobalVariablesDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn variable_scope_matches_graph(
    scope: &crate::variable::VariableScope,
    graph_path: &GraphResourcePath,
) -> bool {
    match scope {
        crate::variable::VariableScope::Event { event_path } => event_path == graph_path.as_str(),
        crate::variable::VariableScope::Function { function_path } => {
            function_path == graph_path.as_str()
        }
        crate::variable::VariableScope::Global => false,
    }
}

#[derive(Debug)]
pub(super) struct VariableEffectCommitResult {
    pub variable_ids: Box<[crate::variable::VariableId]>,
    pub resource_mutation: Option<crate::event::ResourceMutationResultDto>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum VariableEffectCommitError {
    SessionChanged {
        expected: crate::node_system::analysis::ProjectSessionId,
        current: crate::node_system::analysis::ProjectSessionId,
    },
    Conflict {
        resource: crate::node_system::document::ResourceKey,
        expected_revision: crate::node_system::document::ResourceRevision,
        current_revision: Option<crate::node_system::document::ResourceRevision>,
    },
    InvalidEffect {
        message: Box<str>,
    },
    History {
        message: Box<str>,
    },
    Persistence {
        message: Box<str>,
    },
}

impl std::fmt::Display for VariableEffectCommitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionChanged { expected, current } => write!(
                formatter,
                "project session changed from '{}' to '{}' before variable effects committed",
                expected.as_str(),
                current.as_str()
            ),
            Self::Conflict {
                resource,
                expected_revision,
                current_revision,
            } => write!(
                formatter,
                "variable effect conflict for {resource:?}: expected revision {}, current revision {:?}",
                expected_revision.get(),
                current_revision.map(|revision| revision.get())
            ),
            Self::InvalidEffect { message }
            | Self::History { message }
            | Self::Persistence { message } => formatter.write_str(message),
        }
    }
}

#[derive(Clone)]
pub(super) struct CompileResourceSnapshot {
    pub(super) versions: crate::node_system::analysis::ResourceVersionSet,
    functions: BTreeMap<
        crate::node_system::document::GraphResourcePath,
        crate::node_system::document::FunctionDocument,
    >,
    function_graphs: BTreeMap<
        crate::node_system::document::GraphResourcePath,
        crate::node_system::document::GraphDocument,
    >,
    database_schemas:
        BTreeMap<crate::node_system::plan::ResourceId, Vec<crate::schema::ColumnInfoDTO>>,
}

impl CompileResourceSnapshot {
    pub(super) fn schema_resolvers(&self) -> crate::node_system::compiler::SchemaResolverSet {
        let mut resolvers = crate::node_system::compiler::SchemaResolverSet::new();
        resolvers.insert(
            crate::node_system::protocol::SchemaResolverId::new(
                crate::node_system::catalog::DATAFRAME_RESOURCE_SCHEMA_RESOLVER,
            )
            .expect("built-in dataframe schema resolver ID is valid"),
            ProjectDatabaseSchemaResolver {
                schemas: self.database_schemas.clone(),
            },
        );
        resolvers
    }
}

struct ProjectDatabaseSchemaResolver {
    schemas: BTreeMap<crate::node_system::plan::ResourceId, Vec<crate::schema::ColumnInfoDTO>>,
}

impl crate::node_system::compiler::SchemaResolver for ProjectDatabaseSchemaResolver {
    fn resolve(
        &self,
        context: &crate::node_system::compiler::SchemaResolutionContext<'_>,
    ) -> Result<
        crate::node_system::compiler::SchemaFact,
        crate::node_system::compiler::SchemaResolutionError,
    > {
        let resource = context
            .parameters
            .iter()
            .find(|(key, _)| key.as_str() == "dataframe")
            .and_then(|(_, value)| value.as_str())
            .ok_or_else(|| {
                crate::node_system::compiler::SchemaResolutionError::new(
                    "dataframe source requires a database resource",
                )
            })?;
        let resource = crate::node_system::plan::ResourceId::new(resource).map_err(|error| {
            crate::node_system::compiler::SchemaResolutionError::new(error.to_string())
        })?;
        let fields = self.schemas.get(&resource).ok_or_else(|| {
            crate::node_system::compiler::SchemaResolutionError::new(format!(
                "database resource '{}' has no compiled schema",
                resource.as_str()
            ))
        })?;
        Ok(crate::node_system::compiler::SchemaFact::new(
            crate::node_system::protocol::SchemaExpr::Input(
                crate::node_system::protocol::PortKey::new("dataframe").unwrap(),
            ),
            fields.iter().map(|column| {
                crate::node_system::protocol::SchemaColumnRef(column.name.clone().into())
            }),
        ))
    }
}

impl ResourceSnapshot for CompileResourceSnapshot {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        self.versions.clone()
    }

    fn function_document(
        &self,
        path: &crate::node_system::document::GraphResourcePath,
    ) -> Option<&crate::node_system::document::FunctionDocument> {
        self.functions.get(path)
    }

    fn function_graph_document(
        &self,
        path: &crate::node_system::document::GraphResourcePath,
    ) -> Option<&crate::node_system::document::GraphDocument> {
        self.function_graphs.get(path)
    }
}

pub(super) struct ProductionPlotSink;

impl crate::node_system::runtime::PlotSink for ProductionPlotSink {
    fn publish(
        &self,
        _kind: crate::node_system::runtime::PlotKind,
        payload: &str,
    ) -> Result<Box<str>, crate::node_system::runtime::PlotPublishError> {
        Ok(payload.into())
    }
}

pub(super) struct ProductionResourceSnapshots {
    compile: CompileResourceSnapshot,
    pub(super) runtime: crate::node_system::runtime::ProjectResourceSnapshot,
}

struct ExecutionSnapshot {
    document: crate::node_system::document::GraphDocument,
    data: ProjectData,
    database_instances: std::collections::HashMap<String, crate::database::DatabaseInstance>,
    variable_revisions:
        std::collections::HashMap<crate::variable::VariableId, VariableRevisionEntry>,
    project_root: Option<NormalizedProjectRoot>,
    database_schemas:
        BTreeMap<crate::node_system::plan::ResourceId, Vec<crate::schema::ColumnInfoDTO>>,
    registry: Arc<crate::node_system::registry::NodeRegistry>,
    kernels: Arc<crate::node_system::runtime::KernelRegistry>,
    functions: Arc<crate::node_system::runtime::FunctionPlanStore>,
    results: crate::node_system::runtime::ResultStore,
    runs: Arc<crate::node_system::runtime::ProjectRunRegistry>,
    session_id: crate::node_system::analysis::ProjectSessionId,
}

pub(super) fn compile_resources_from_projection_snapshot(
    source: &ProjectionSourceSnapshot,
) -> Result<CompileResourceSnapshot, String> {
    compile_resources_from_data(&source.data, source.environment.database_schemas.clone())
}

pub(super) fn compile_resources_from_data(
    data: &ProjectData,
    database_schemas: BTreeMap<
        crate::node_system::plan::ResourceId,
        Vec<crate::schema::ColumnInfoDTO>,
    >,
) -> Result<CompileResourceSnapshot, String> {
    use crate::node_system::analysis::{ResourceKey as AnalysisResourceKey, ResourceVersion};

    let functions = data
        .graphs
        .iter()
        .filter_map(|(path, graph)| {
            graph.function.clone().map(|function| {
                (
                    crate::node_system::document::GraphResourcePath(path.as_str().into()),
                    function,
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let function_graphs = data
        .graphs
        .iter()
        .filter_map(|(path, graph)| {
            graph.function.as_ref().map(|_| {
                (
                    crate::node_system::document::GraphResourcePath(path.as_str().into()),
                    graph.document.clone(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut versions = crate::node_system::analysis::ResourceVersionSet::new();
    for (path, function) in &functions {
        let graph_path =
            GraphResourcePath::new(path.0.as_ref()).map_err(|error| error.to_string())?;
        let graph_document = &data
            .graphs
            .get(&graph_path)
            .ok_or_else(|| format!("function '{}' graph is not loaded", graph_path))?
            .document;
        let version = serde_json::to_string(&(function, graph_document))
            .map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(path.0.as_ref()),
            ResourceVersion::new(version),
        );
    }
    for (id, variable) in &data.variables {
        let key = format!("variables/{id}");
        let version = serde_json::to_string(variable).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }
    for (id, declaration) in &data.databases {
        let key = format!("databases/{id}");
        let resource = crate::node_system::plan::ResourceId::new(key.as_str())
            .map_err(|error| error.to_string())?;
        let schema = database_schemas
            .get(&resource)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let version =
            serde_json::to_string(&(declaration, schema)).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }

    Ok(CompileResourceSnapshot {
        versions,
        functions,
        function_graphs,
        database_schemas,
    })
}

fn snapshot_execution_resources(
    snapshot: &ExecutionSnapshot,
    compile: CompileResourceSnapshot,
) -> Result<ProductionResourceSnapshots, String> {
    use crate::node_system::plan::ResourceId;

    let mut runtime = crate::node_system::runtime::ProjectResourceSnapshot::new(
        snapshot.session_id.clone(),
        compile.versions.clone(),
    )
    .with_plot_sink(Arc::new(ProductionPlotSink));
    for (id, variable) in &snapshot.data.variables {
        runtime = runtime.with_variable_revision(
            ResourceId::new(format!("variables/{id}")).map_err(|error| error.to_string())?,
            Arc::new(variable.clone()),
            snapshot
                .variable_revisions
                .get(id)
                .map(|entry| entry.revision)
                .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
        );
    }
    for (id, instance) in &snapshot.database_instances {
        if !snapshot.data.databases.contains_key(id) {
            continue;
        }
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        match &instance.state {
            DatabaseState::Loaded { dataframe, .. } => {
                runtime = runtime.with_database(resource, Arc::clone(dataframe));
            }
            DatabaseState::DuckDb { .. } => {
                let crate::database::DatabaseEngine::DuckDb { path, table } = &instance.decl.engine
                else {
                    return Err(format!("database '{id}' runtime/declaration mismatch"));
                };
                let root = snapshot
                    .project_root
                    .as_ref()
                    .ok_or_else(|| format!("database '{id}' requires an active project path"))?;
                runtime = runtime.with_duckdb_database(
                    resource,
                    root.as_path().join(path).to_string_lossy().into_owned(),
                    table.clone(),
                );
            }
            DatabaseState::Failed { .. } => {}
        }
    }
    Ok(ProductionResourceSnapshots { compile, runtime })
}

#[cfg(test)]
pub(super) fn snapshot_project_resources(
    state: &ProjectState,
    variables: std::collections::HashMap<
        crate::variable::VariableId,
        crate::variable::VariableInstance,
    >,
    databases: std::collections::HashMap<String, crate::database::DatabaseDecl>,
) -> Result<ProductionResourceSnapshots, String> {
    use crate::node_system::analysis::{ResourceKey as AnalysisResourceKey, ResourceVersion};
    use crate::node_system::plan::ResourceId;

    let (session_id, loaded_databases) = {
        let store = state.project_store.read().unwrap();
        let loaded = store
            .databases
            .iter()
            .filter_map(|(id, database)| match &database.state {
                DatabaseState::Loaded { dataframe, .. } => Some((
                    id.clone(),
                    Arc::clone(dataframe),
                    crate::application::database_schema::column_info_from_schema(
                        dataframe.schema().as_ref(),
                    ),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        (store.project_session_id.clone(), loaded)
    };

    let (function_resources, function_graphs) = {
        let data = state.project_data.read().unwrap();
        let resources = data
            .graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph.function.clone().map(|function| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        function,
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let graphs = data
            .graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph.function.as_ref().map(|_| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        graph.document.clone(),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        (resources, graphs)
    };
    let mut versions = crate::node_system::analysis::ResourceVersionSet::new();
    for (path, function) in &function_resources {
        let graph = function_graphs
            .get(path)
            .ok_or_else(|| format!("function '{}' graph is not loaded", path.0))?;
        let version =
            serde_json::to_string(&(function, graph)).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(path.0.as_ref()),
            ResourceVersion::new(version),
        );
    }
    for (id, variable) in &variables {
        let key = format!("variables/{id}");
        let version = serde_json::to_string(variable).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }
    for (id, declaration) in &databases {
        let key = format!("databases/{id}");
        let version = serde_json::to_string(declaration).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }

    let project_root = state
        .get_path()
        .map(|path| crate::project::project_root_from_path(&path));
    let mut database_schemas = BTreeMap::new();
    let variable_revisions = state.variable_revisions.read().unwrap().clone();
    let mut runtime =
        crate::node_system::runtime::ProjectResourceSnapshot::new(session_id, versions.clone())
            .with_plot_sink(Arc::new(ProductionPlotSink));
    for (id, variable) in variables {
        runtime = runtime.with_variable_revision(
            ResourceId::new(format!("variables/{id}")).map_err(|error| error.to_string())?,
            Arc::new(variable),
            variable_revisions
                .get(&id)
                .map(|entry| entry.revision)
                .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
        );
    }
    for (id, dataframe, columns) in loaded_databases {
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        database_schemas.insert(resource.clone(), columns);
        runtime = runtime.with_database(resource, dataframe);
    }
    for (id, declaration) in databases {
        let crate::database::DatabaseEngine::DuckDb { path, table } = declaration.engine else {
            continue;
        };
        let root = project_root
            .as_ref()
            .ok_or_else(|| format!("database '{id}' requires an active project path"))?;
        let absolute = root.join(path);
        let metadata = crate::database::read_table_meta(&absolute, &table)?;
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        database_schemas.insert(
            resource.clone(),
            crate::application::database_schema::column_info_from_duckdb(&metadata.columns),
        );
        runtime =
            runtime.with_duckdb_database(resource, absolute.to_string_lossy().into_owned(), table);
    }

    Ok(ProductionResourceSnapshots {
        compile: CompileResourceSnapshot {
            versions,
            functions: function_resources,
            function_graphs,
            database_schemas,
        },
        runtime,
    })
}

fn project_documents(
    data: &ProjectData,
    variable_revisions: &std::collections::HashMap<
        crate::variable::VariableId,
        VariableRevisionEntry,
    >,
) -> ProjectDocumentState {
    ProjectDocumentState::new(
        data.graphs
            .iter()
            .map(|(path, graph)| {
                (
                    crate::node_system::document::GraphResourcePath(path.as_str().into()),
                    graph.document.clone(),
                )
            })
            .collect(),
        data.graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph.function.clone().map(|function| {
                    (
                        crate::node_system::document::FunctionResourceKey(path.as_str().into()),
                        function,
                    )
                })
            })
            .collect(),
        variable_revisions
            .iter()
            .filter_map(|(id, entry)| {
                let value = if entry.is_present() {
                    Some(
                        serde_json::to_value(data.variables.get(id)?)
                            .expect("variable documents are serializable"),
                    )
                } else {
                    None
                };
                Some((
                    crate::node_system::document::VariableResourceKey(
                        format!("variables/{id}").into(),
                    ),
                    crate::node_system::document::VariableDocument {
                        revision: entry.revision,
                        value,
                    },
                ))
            })
            .collect(),
    )
}

fn try_project_document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> Option<crate::node_system::document::ResourceRevision> {
    match resource {
        ResourceKey::Graph(path) => documents.graphs.get(path).map(|document| document.revision),
        ResourceKey::Function(key) => documents
            .functions
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Variable(key) => documents
            .variables
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Database(_) | ResourceKey::Worksheet(_) => None,
    }
}

fn project_document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> crate::node_system::document::ResourceRevision {
    try_project_document_revision(documents, resource)
        .expect("history transaction resource remains present")
}

fn replace_project_documents(
    data: &mut ProjectData,
    variable_revisions: &mut std::collections::HashMap<
        crate::variable::VariableId,
        VariableRevisionEntry,
    >,
    mut documents: ProjectDocumentState,
) {
    for (path, graph) in &mut data.graphs {
        let key = crate::node_system::document::GraphResourcePath(path.as_str().into());
        if let Some(document) = documents.graphs.remove(&key) {
            graph.document = document;
        }
        let function_key = crate::node_system::document::FunctionResourceKey(path.as_str().into());
        if let Some(function) = documents.functions.remove(&function_key) {
            graph.function = Some(function);
        }
    }
    for (key, document) in documents.variables {
        let Some(id) = key.0.strip_prefix("variables/") else {
            continue;
        };
        let Ok(uuid) = uuid::Uuid::parse_str(id) else {
            continue;
        };
        let variable_id = crate::variable::VariableId::from(uuid);
        let presence = match document.value {
            Some(value) => {
                let variable = serde_json::from_value(value)
                    .expect("history retains valid variable documents");
                data.variables.insert(variable_id, variable);
                VariablePresence::Present
            }
            None => {
                data.variables.remove(&variable_id);
                VariablePresence::Deleted
            }
        };
        variable_revisions.insert(
            variable_id,
            VariableRevisionEntry {
                revision: document.revision,
                presence,
            },
        );
    }
}

pub(super) fn publish_function_plans(
    registry: &crate::node_system::registry::NodeRegistry,
    store: &crate::node_system::runtime::FunctionPlanStore,
    resources: &CompileResourceSnapshot,
    session_id: crate::node_system::analysis::ProjectSessionId,
    trace_sink: &dyn crate::node_system::analysis::TraceSink,
    cancellation: &crate::node_system::compiler::CompileCancellationToken,
    parameters: &mut crate::node_system::runtime::CompiledParameterStore,
) -> Result<crate::node_system::runtime::FunctionPlanGeneration, String> {
    let compiler = GraphCompiler::with_resolvers(
        registry,
        resources,
        resources.schema_resolvers(),
        crate::node_system::compiler::build_builtin_interface_resolvers(),
    )
    .with_observability(session_id, trace_sink);
    let mut entries = Vec::with_capacity(resources.function_graphs.len());
    for (document_path, document) in &resources.function_graphs {
        let snapshot = compiler.snapshot(document_path.clone(), document);
        let products = compiler
            .compile_snapshot(&snapshot, cancellation)
            .map_err(|error| error.to_string())?;
        let abi = products.function_abi.clone().ok_or_else(|| {
            format!(
                "function '{}' did not produce an Entry/Return ABI",
                document_path.0
            )
        })?;
        let plan = products.plan.ok_or_else(|| {
            let diagnostics = products
                .analysis
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "function '{}' has blocking diagnostics and cannot be published: {}",
                document_path.0, diagnostics
            )
        })?;
        build_run_parameters(parameters, &document, &plan)?;
        let resource_key = crate::node_system::analysis::ResourceKey::new(document_path.0.as_ref());
        let version = resources
            .versions
            .get(&resource_key)
            .cloned()
            .ok_or_else(|| format!("function '{}' has no resource version", document_path.0))?;
        entries.push((
            document_path.clone(),
            version,
            Arc::new(plan),
            Arc::new(abi),
        ));
    }
    store
        .generation(
            registry.fingerprint().clone(),
            resources.versions(),
            entries,
        )
        .map_err(|error| error.to_string())
}

fn sanitize_graph_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches([' ', '.']).trim();
    if sanitized.is_empty() {
        "Untitled".into()
    } else {
        sanitized.into()
    }
}

fn build_run_parameters(
    parameters: &mut crate::node_system::runtime::CompiledParameterStore,
    document: &crate::node_system::document::GraphDocument,
    plan: &crate::node_system::plan::ExecutionPlan,
) -> Result<(), String> {
    for operation in &plan.operations {
        let node_type = operation.source_node_type_id.as_str();
        let node = document
            .nodes
            .get(&operation.source_node_id)
            .ok_or_else(|| format!("plan source node '{}' is missing", operation.source_node_id))?;
        if matches!(
            node_type,
            "yssbi.project.variable.get" | "yssbi.project.variable.set"
        ) {
            let resource = node
                .parameters
                .iter()
                .find(|(key, _)| key.as_str() == "variable")
                .and_then(|(_, value)| value.as_str())
                .ok_or_else(|| format!("variable node '{}' has no binding", node.id))?;
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::BuiltinVariableParameters::new(
                        crate::node_system::plan::ResourceId::new(resource)
                            .map_err(|error| error.to_string())?,
                    ),
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if node_type.starts_with("yssbi.statistics.") {
            let parameter = |name: &str| {
                node.parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == name)
                    .map(|(_, value)| value)
            };
            let positive_integer = |name: &str| {
                parameter(name)
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as usize)
            };
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::StatisticsKernelParameters {
                        lags: positive_integer("lags"),
                        max_lags: positive_integer("max_lags"),
                        rank: positive_integer("rank"),
                        regression: parameter("regression")
                            .and_then(serde_json::Value::as_str)
                            .map(Into::into),
                        trend: parameter("trend")
                            .and_then(serde_json::Value::as_str)
                            .map(Into::into),
                    },
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if node_type.starts_with("yssbi.dataframe.") {
            let parameter = |name: &str| {
                node.parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == name)
                    .map(|(_, value)| value)
            };
            let resource = parameter("dataframe")
                .and_then(serde_json::Value::as_str)
                .map(crate::node_system::plan::ResourceId::new)
                .transpose()
                .map_err(|error| error.to_string())?;
            let column = parameter("column")
                .and_then(serde_json::Value::as_str)
                .map(Into::into);
            let order = parameter("order")
                .or_else(|| parameter("window"))
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as usize);
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::DataframeKernelParameters {
                        resource,
                        column,
                        order,
                    },
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if !node_type.starts_with("yssbi.constant.") {
            continue;
        }
        let node = document
            .nodes
            .get(&operation.source_node_id)
            .ok_or_else(|| format!("plan source node '{}' is missing", operation.source_node_id))?;
        let value = node
            .parameters
            .iter()
            .find(|(key, _)| key.as_str() == "value")
            .map(|(_, value)| json_to_protocol_value(value))
            .transpose()?
            .unwrap_or(crate::node_system::protocol::Value::Null);
        parameters
            .insert(
                operation.params.clone(),
                crate::node_system::runtime::BuiltinConstantParameters::new(value),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn json_to_protocol_value(
    value: &serde_json::Value,
) -> Result<crate::node_system::protocol::Value, String> {
    use crate::node_system::protocol::{CanonicalDecimal, Value};
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::Number(value) if value.is_i64() => {
            Value::Integer(value.as_i64().expect("checked i64"))
        }
        serde_json::Value::Number(value) if value.is_u64() => {
            Value::Unsigned(value.as_u64().expect("checked u64"))
        }
        serde_json::Value::Number(value) => Value::Decimal(
            CanonicalDecimal::new(value.to_string()).map_err(|error| error.to_string())?,
        ),
        serde_json::Value::String(value) => Value::String(value.as_str().into()),
        serde_json::Value::Array(values) => Value::List(
            values
                .iter()
                .map(json_to_protocol_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        serde_json::Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.as_str().into(), json_to_protocol_value(value)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

#[cfg(test)]
mod run_parameter_tests {
    use super::*;
    use crate::node_system::document::{DocumentNode, GraphDocument, NodeId, NodePosition};
    use crate::node_system::plan::{
        CompiledParameterHandle, ExecutionPlan, PlannedKernel, PlannedOperation,
    };
    use crate::node_system::protocol::{NodeTypeId, ParameterKey, Value};
    use crate::project::GraphDocumentKind;
    use std::collections::BTreeMap;

    fn catalog_defaults(node_type: &NodeTypeId) -> BTreeMap<ParameterKey, serde_json::Value> {
        let registry = crate::node_system::catalog::build_builtin_registry();
        registry
            .get(node_type)
            .unwrap()
            .protocol
            .parameters
            .parameters
            .iter()
            .filter_map(|parameter| {
                let value = match &parameter.default_value.as_ref()?.value {
                    Value::Integer(value) => serde_json::json!(value),
                    Value::Unsigned(value) => serde_json::json!(value),
                    Value::String(value) => serde_json::json!(value),
                    other => panic!("unsupported test catalog default: {other:?}"),
                };
                Some((parameter.key.clone(), value))
            })
            .collect()
    }

    fn parameter_plan(node: &DocumentNode, params: CompiledParameterHandle) -> ExecutionPlan {
        use crate::node_system::analysis::{
            CompilationBasis, CompileId, CompileProvenance, ProjectSessionId,
        };
        use crate::node_system::document::{GraphResourcePath, GraphRevision};
        use crate::node_system::plan::StructuredControlRegion;
        use crate::node_system::registry::RegistryFingerprint;

        ExecutionPlan {
            provenance: CompileProvenance {
                project_session_id: ProjectSessionId::new("run-parameter-test"),
                graph_path: GraphResourcePath("events/test".into()),
                basis: CompilationBasis {
                    graph_revision: GraphRevision::new(1),
                    registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                    resource_versions: BTreeMap::new(),
                },
                compile_id: CompileId::new(1),
            },
            value_count: 0,
            value_sources: Box::new([]),
            operations: Box::new([PlannedOperation {
                source_node_id: node.id,
                source_node_type_id: node.node_type.clone(),
                kernel: PlannedKernel::Native(
                    crate::node_system::plan::KernelHandle::new("test.kernel").unwrap(),
                ),
                inputs: Box::new([]),
                outputs: Box::new([]),
                params,
            }]),
            value_dependencies: Box::new([]),
            root_region: StructuredControlRegion::Sequence(Box::new([])),
            effect_dependencies: Box::new([]),
            relational_subplans: Box::new([]),
            resources: Box::new([]),
            results: Box::new([]),
        }
    }

    #[test]
    fn function_graph_replacement_changes_the_coherent_compile_resource_version() {
        let path = GraphResourcePath::new("functions/replaced.yssbi-function").unwrap();
        let analysis_path = crate::node_system::document::GraphResourcePath(path.as_str().into());
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Replaced", GraphDocumentKind::Function),
        );
        let first = compile_resources_from_data(&data, BTreeMap::new()).unwrap();
        let first_version = first
            .versions
            .get(&crate::node_system::analysis::ResourceKey::new(
                path.as_str(),
            ))
            .unwrap()
            .clone();

        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(91));
        data.graphs.get_mut(&path).unwrap().document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.project.function.entry").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        );
        let replaced = compile_resources_from_data(&data, BTreeMap::new()).unwrap();
        let replaced_version = replaced
            .versions
            .get(&crate::node_system::analysis::ResourceKey::new(
                path.as_str(),
            ))
            .unwrap();

        assert_ne!(&first_version, replaced_version);
        assert_eq!(
            replaced
                .function_graph_document(&analysis_path)
                .unwrap()
                .nodes
                .len(),
            1
        );
    }

    #[test]
    fn function_plan_publication_uses_only_the_compile_resource_snapshot() {
        let registry = crate::node_system::catalog::build_builtin_registry();
        let session = crate::node_system::analysis::ProjectSessionId::new("coherent-run");
        let store = crate::node_system::runtime::FunctionPlanStore::new(session.clone(), 64);
        let resources = CompileResourceSnapshot {
            versions: crate::node_system::analysis::ResourceVersionSet::new(),
            functions: BTreeMap::new(),
            function_graphs: BTreeMap::new(),
            database_schemas: BTreeMap::new(),
        };
        let mut parameters = crate::node_system::runtime::CompiledParameterStore::new();

        let generation = publish_function_plans(
            &registry,
            &store,
            &resources,
            session,
            &crate::node_system::analysis::NOOP_TRACE_SINK,
            &crate::node_system::compiler::CompileCancellationToken::new(),
            &mut parameters,
        )
        .unwrap();

        assert_eq!(generation.plan_count(), 0);
    }

    #[test]
    fn adf_catalog_regression_builds_the_production_kernel_parameter() {
        let node_type = NodeTypeId::new("yssbi.statistics.adf.test").unwrap();
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(1));
        let mut parameters = catalog_defaults(&node_type);
        assert_eq!(
            parameters.get(&ParameterKey::new("regression").unwrap()),
            Some(&serde_json::json!("constant"))
        );
        parameters.insert(
            ParameterKey::new("regression").unwrap(),
            serde_json::json!("none"),
        );
        let node = DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters,
            user_label: None,
        };
        let mut document = GraphDocument::default();
        document.nodes.insert(node_id, node.clone());
        let handle = CompiledParameterHandle::new("adf").unwrap();
        let plan = parameter_plan(&node, handle.clone());
        let mut store = crate::node_system::runtime::CompiledParameterStore::new();

        build_run_parameters(&mut store, &document, &plan).unwrap();

        let parameters = store
            .get::<crate::node_system::runtime::StatisticsKernelParameters>(&handle)
            .unwrap()
            .unwrap();
        assert_eq!(parameters.regression.as_deref(), Some("none"));
        assert_eq!(parameters.trend, None);
    }

    struct NoResources;

    impl crate::node_system::runtime::ResourceProvider for NoResources {
        fn acquire(
            &self,
            _: &crate::node_system::plan::CompiledResourceRequirement,
        ) -> Result<
            Box<dyn crate::node_system::runtime::ResourceLease>,
            crate::node_system::runtime::ResourceError,
        > {
            unreachable!("statistics parameter test has no resources")
        }
    }

    struct NoFunctions;

    impl crate::node_system::runtime::FunctionPlanProvider for NoFunctions {
        fn get_function(
            &self,
            _: &crate::node_system::plan::FunctionPlanHandle,
        ) -> Result<
            Option<std::sync::Arc<crate::node_system::runtime::PublishedFunctionPlan>>,
            Box<str>,
        > {
            Ok(None)
        }
    }

    struct SeriesKernel(Value);

    impl crate::node_system::runtime::Kernel for SeriesKernel {
        fn execute(
            &self,
            _: &crate::node_system::runtime::KernelContext<'_>,
            _: &[crate::node_system::runtime::RuntimeValue],
        ) -> Result<
            Vec<crate::node_system::runtime::RuntimeValue>,
            crate::node_system::runtime::KernelError,
        > {
            Ok(vec![self.0.clone().into()])
        }
    }

    #[test]
    fn adf_regression_reaches_adapter_through_the_production_run_chain() {
        use crate::node_system::plan::{
            ControlStep, OperationIndex, PlanResult, PlannedInput, PlannedOutput,
            StructuredControlRegion, ValueRef,
        };
        use crate::node_system::protocol::{InputConsumption, OutputProduction};
        use crate::node_system::runtime::{CancellationToken, RunError, RunExecutor, RuntimeValue};

        let series_values =
            serde_json::json!([1, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3, 2.7, 3.4]);
        let series = [1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4];
        let source_id = NodeId::from_uuid(uuid::Uuid::from_u128(10));
        let adf_id = NodeId::from_uuid(uuid::Uuid::from_u128(11));
        let adf_type = NodeTypeId::new("yssbi.statistics.adf.test").unwrap();
        let regression_key = ParameterKey::new("regression").unwrap();
        let mut adf_parameters = catalog_defaults(&adf_type);
        assert_eq!(
            adf_parameters.get(&regression_key),
            Some(&serde_json::json!("constant"))
        );
        adf_parameters.insert(regression_key.clone(), serde_json::json!("trend"));

        let source_node = DocumentNode {
            id: source_id,
            node_type: NodeTypeId::new("yssbi.test.adf.series").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        };
        let adf_node = DocumentNode {
            id: adf_id,
            node_type: adf_type.clone(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: adf_parameters,
            user_label: None,
        };
        let mut document = GraphDocument::default();
        document.nodes.insert(source_id, source_node.clone());
        document.nodes.insert(adf_id, adf_node.clone());

        let mut plan = parameter_plan(
            &adf_node,
            CompiledParameterHandle::new("adf-production-chain").unwrap(),
        );
        plan.value_count = 2;
        plan.operations = Box::new([
            PlannedOperation {
                source_node_id: source_id,
                source_node_type_id: source_node.node_type,
                kernel: PlannedKernel::Native(
                    crate::node_system::plan::KernelHandle::new("test.adf.series").unwrap(),
                ),
                inputs: Box::new([]),
                outputs: Box::new([PlannedOutput {
                    value: ValueRef::new(0),
                    production: OutputProduction::FullyMaterialized,
                }]),
                params: CompiledParameterHandle::new("adf-series").unwrap(),
            },
            PlannedOperation {
                source_node_id: adf_id,
                source_node_type_id: adf_type,
                kernel: PlannedKernel::Native(
                    crate::node_system::plan::KernelHandle::new("yssbi.statistics.adf.test")
                        .unwrap(),
                ),
                inputs: Box::new([PlannedInput {
                    value: ValueRef::new(0),
                    consumption: InputConsumption::FullyMaterialized,
                }]),
                outputs: Box::new([PlannedOutput {
                    value: ValueRef::new(1),
                    production: OutputProduction::FullyMaterialized,
                }]),
                params: CompiledParameterHandle::new("adf-production-chain").unwrap(),
            },
        ]);
        plan.root_region = StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ]));
        plan.results = Box::new([PlanResult {
            name: "adf".into(),
            value: ValueRef::new(1),
        }]);

        let mut kernels = crate::node_system::runtime::build_builtin_kernel_registry();
        kernels
            .register(
                crate::node_system::plan::KernelHandle::new("test.adf.series").unwrap(),
                SeriesKernel(json_to_protocol_value(&series_values).unwrap()),
            )
            .unwrap();
        let run = |document: &GraphDocument| {
            let mut store = crate::node_system::runtime::CompiledParameterStore::new();
            build_run_parameters(&mut store, document, &plan).unwrap();
            RunExecutor::new(&kernels, &NoResources, &NoFunctions)
                .with_compiled_parameters(&store)
                .run(&plan, CancellationToken::new())
        };

        let trend_result = run(&document).unwrap();
        let RuntimeValue::Scalar(Value::Object(actual)) = &trend_result.values["adf"] else {
            panic!("ADF result must be an object");
        };
        let expected_trend =
            crate::sci::api::node_statistics::augmented_dickey_fuller(&series, 1, "trend").unwrap();
        let constant_result =
            crate::sci::api::node_statistics::augmented_dickey_fuller(&series, 1, "constant")
                .unwrap();
        let protocol_number = |value: &Value| match value {
            Value::Integer(value) => *value as f64,
            Value::Unsigned(value) => *value as f64,
            Value::Decimal(value) => value.as_str().parse::<f64>().unwrap(),
            other => panic!("expected numeric protocol value, got {other:?}"),
        };
        let actual_statistic = protocol_number(&actual["statistic"]);
        let trend_statistic = expected_trend["statistic"].as_f64().unwrap();
        let constant_statistic = constant_result["statistic"].as_f64().unwrap();
        assert!((actual_statistic - trend_statistic).abs() < f64::EPSILON);
        assert!((actual_statistic - constant_statistic).abs() > f64::EPSILON);

        document
            .nodes
            .get_mut(&adf_id)
            .unwrap()
            .parameters
            .insert(regression_key, serde_json::json!("unexpected"));
        let error = run(&document).unwrap_err();
        assert!(matches!(
            error,
            RunError::KernelFailed { operation, ref message }
                if operation == OperationIndex::new(1)
                    && message.as_ref() == "unsupported ADF regression 'unexpected'"
        ));
    }

    #[test]
    fn var_summary_catalog_lags_reach_the_production_kernel() {
        use crate::node_system::plan::{
            ControlStep, OperationIndex, PlanResult, PlannedInput, PlannedOutput,
            StructuredControlRegion, ValueRef,
        };
        use crate::node_system::protocol::{InputConsumption, OutputProduction};
        use crate::node_system::runtime::{CancellationToken, RunExecutor, RuntimeValue};

        let var_type = NodeTypeId::new("yssbi.statistics.var.summary").unwrap();
        let mut var_parameters = catalog_defaults(&var_type);
        let lags_key = ParameterKey::new("lags").unwrap();
        assert_eq!(var_parameters.get(&lags_key), Some(&serde_json::json!(1)));
        var_parameters.insert(lags_key, serde_json::json!(2));

        let node_specs = [
            (1_u128, "yssbi.test.series.a"),
            (2_u128, "yssbi.test.series.b"),
        ];
        let mut document = GraphDocument::default();
        let mut constant_ids = Vec::new();
        for (raw_id, node_type) in node_specs {
            let id = NodeId::from_uuid(uuid::Uuid::from_u128(raw_id));
            constant_ids.push(id);
            document.nodes.insert(
                id,
                DocumentNode {
                    id,
                    node_type: NodeTypeId::new(node_type).unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: BTreeMap::new(),
                    user_label: None,
                },
            );
        }
        let var_id = NodeId::from_uuid(uuid::Uuid::from_u128(3));
        document.nodes.insert(
            var_id,
            DocumentNode {
                id: var_id,
                node_type: var_type.clone(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: var_parameters,
                user_label: None,
            },
        );

        let mut plan = parameter_plan(
            document.nodes.get(&var_id).unwrap(),
            CompiledParameterHandle::new("var").unwrap(),
        );
        let constant_operation = |index: usize| PlannedOperation {
            source_node_id: constant_ids[index],
            source_node_type_id: NodeTypeId::new(format!("yssbi.test.series.{index}")).unwrap(),
            kernel: PlannedKernel::Native(
                crate::node_system::plan::KernelHandle::new(format!("test.series.{index}"))
                    .unwrap(),
            ),
            inputs: Box::new([]),
            outputs: Box::new([PlannedOutput {
                value: ValueRef::new(index as u32),
                production: OutputProduction::FullyMaterialized,
            }]),
            params: CompiledParameterHandle::new(format!("series-{index}")).unwrap(),
        };
        let var_operation = PlannedOperation {
            source_node_id: var_id,
            source_node_type_id: var_type,
            kernel: PlannedKernel::Native(
                crate::node_system::plan::KernelHandle::new("yssbi.statistics.var.summary")
                    .unwrap(),
            ),
            inputs: Box::new([
                PlannedInput {
                    value: ValueRef::new(0),
                    consumption: InputConsumption::FullyMaterialized,
                },
                PlannedInput {
                    value: ValueRef::new(1),
                    consumption: InputConsumption::FullyMaterialized,
                },
            ]),
            outputs: Box::new([
                PlannedOutput {
                    value: ValueRef::new(2),
                    production: OutputProduction::FullyMaterialized,
                },
                PlannedOutput {
                    value: ValueRef::new(3),
                    production: OutputProduction::FullyMaterialized,
                },
            ]),
            params: CompiledParameterHandle::new("var").unwrap(),
        };
        plan.value_count = 4;
        plan.operations = Box::new([constant_operation(0), constant_operation(1), var_operation]);
        plan.root_region = StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ]));
        plan.results = Box::new([PlanResult {
            name: "var".into(),
            value: ValueRef::new(2),
        }]);
        let mut store = crate::node_system::runtime::CompiledParameterStore::new();
        build_run_parameters(&mut store, &document, &plan).unwrap();

        let mut kernels = crate::node_system::runtime::build_builtin_kernel_registry();
        for (index, values) in [
            serde_json::json!([1, 1.2, 0.9, 1.1, 1.4, 1, 0.8, 1.3, 1.1, 0.9, 1.2, 1.5]),
            serde_json::json!([0.5, 0.7, 0.6, 0.9, 0.8, 1, 0.7, 0.6, 0.9, 1.1, 0.8, 0.7]),
        ]
        .into_iter()
        .enumerate()
        {
            kernels
                .register(
                    crate::node_system::plan::KernelHandle::new(format!("test.series.{index}"))
                        .unwrap(),
                    SeriesKernel(json_to_protocol_value(&values).unwrap()),
                )
                .unwrap();
        }
        let result = RunExecutor::new(&kernels, &NoResources, &NoFunctions)
            .with_compiled_parameters(&store)
            .run(&plan, CancellationToken::new())
            .unwrap();
        let RuntimeValue::Scalar(Value::Object(result)) = &result.values["var"] else {
            panic!("VAR result must be an object");
        };
        let Value::List(coefficients) = &result["coefficients"] else {
            panic!("VAR coefficients must be grouped by equation");
        };
        let Value::List(labels) = &result["coef_labels"] else {
            panic!("VAR coefficient labels must be grouped by equation");
        };
        for (coefficients, labels) in coefficients.iter().zip(labels) {
            let Value::List(coefficients) = coefficients else {
                panic!("equation coefficients must be a list");
            };
            let Value::List(labels) = labels else {
                panic!("equation labels must be a list");
            };
            assert_eq!(coefficients.len(), 5);
            assert!(
                labels.iter().any(|label| {
                    matches!(label, Value::String(label) if label.contains("L2."))
                })
            );
        }
    }
}
