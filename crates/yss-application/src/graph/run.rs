mod stages;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use thiserror::Error;

use super::finalization::{FinalizationError, finalize_successful_run};
use super::inputs::{GraphInputError, GraphResolutionContext};
use crate::session::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use yss_database_runtime::error::DatabaseError;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_execution::error::RunPhase;
use yss_graph_execution::graph_preparation::GraphExecutionScope;
use yss_graph_execution::package_preparation::PackagePreparationError;
use yss_graph_execution::plan::{
    InvalidPlanIdentity, PlanExecutionDemand, PlanOutputRef, PlanProjectSessionId,
    PlanRegistryFingerprint, PlanResourceId, PlanResourceObservedState, PlanResourceVersion,
};
use yss_graph_execution::run_registry::RunId;
use yss_graph_execution::state::{
    ExecutePreparedError, ExecutionAdmissionError, ExecutionCancelOutcome, PreparedExecutionEvent,
    RunExecutionControl,
};
use yss_project::execution_authority::{
    CandidateProjectEffects, ProjectEffectCommitControl, ProjectEffectCommitError,
    ProjectExecutionPreparationError, ProjectExecutionRequest, ProjectResourceAccess,
    ProjectResourceGrant, ProjectResourceId, ProjectResourceKind, ProjectResourcePresence,
    ProjectResourceRequirement,
};
use yss_project_identity::ProjectInstanceId;

/// A run demand is an Application-owned interpretation of the graph execution
/// request. It contains only Pure Leaf graph/plan identities, never transport
/// DTOs or a delivery target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunDemand {
    Default,
    Node {
        node_id: yss_graph_document::NodeId,
        mode: yss_graph_execution::plan::NodeExecutionMode,
    },
    Outputs {
        outputs: Box<[PlanOutputRef]>,
        include_default_results: bool,
        reuse_inputs: bool,
    },
}

/// A run request is intentionally owned by the Application seam. It carries
/// no transport data and no public RunId before execution has been admitted.
#[derive(Debug)]
pub struct RunGraphRequest {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    demand: RunDemand,
    required_resources: Box<[ProjectResourceRequirement]>,
    resource_authorizations: Option<Box<[RunResourceAuthorization]>>,
    inherit_graph_reads: bool,
    cancellation: Arc<AtomicBool>,
    deadline: Instant,
    document: GraphDocument,
    semantic_input_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct RunResourceAuthorization {
    pub resource: ProjectResourceId,
    pub access: ProjectResourceAccess,
    pub expected_version: Option<u64>,
}

impl RunGraphRequest {
    pub fn new(
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        document: GraphDocument,
        semantic_input_hash: [u8; 32],
    ) -> Self {
        Self {
            project_instance_id,
            graph_path,
            demand: RunDemand::Default,
            required_resources: Box::new([]),
            resource_authorizations: None,
            inherit_graph_reads: false,
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + std::time::Duration::from_secs(60),
            document,
            semantic_input_hash,
        }
    }

    pub fn with_demand(mut self, demand: RunDemand) -> Self {
        self.demand = demand;
        self
    }

    pub fn with_required_resources(
        mut self,
        resources: impl IntoIterator<Item = ProjectResourceRequirement>,
    ) -> Self {
        self.required_resources = resources.into_iter().collect();
        self
    }

    pub fn with_cancellation(mut self, cancellation: Arc<AtomicBool>) -> Self {
        self.cancellation = cancellation;
        self
    }

    pub fn with_resource_authorizations(
        mut self,
        authorizations: Vec<RunResourceAuthorization>,
    ) -> Self {
        self.resource_authorizations = Some(authorizations.into_boxed_slice());
        self
    }

    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = deadline;
        self
    }

    /// The caller has project read access and has authorized execution of this graph.
    /// Only the selected graph's actual shared dependencies inherit that read access.
    pub(crate) fn with_inherited_graph_reads(mut self) -> Self {
        self.inherit_graph_reads = true;
        self
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation.load(Ordering::Acquire)
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunIdentity {
    execution_session_id: yss_graph_execution::identity::ExecutionSessionId,
    graph_path: GraphResourcePath,
    run_id: RunId,
    semantic_input_hash: [u8; 32],
}

impl RunIdentity {
    fn new(
        execution_session_id: yss_graph_execution::identity::ExecutionSessionId,
        graph_path: GraphResourcePath,
        run_id: RunId,
        semantic_input_hash: [u8; 32],
    ) -> Self {
        Self {
            execution_session_id,
            graph_path,
            run_id,
            semantic_input_hash,
        }
    }

    pub fn execution_session_id(&self) -> &yss_graph_execution::identity::ExecutionSessionId {
        &self.execution_session_id
    }

    pub fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub const fn run_id(&self) -> RunId {
        self.run_id
    }

    pub const fn semantic_input_hash(&self) -> &[u8; 32] {
        &self.semantic_input_hash
    }
}

/// References captured from this run's committed handoff, before delivering terminal events.
#[derive(Clone, Debug)]
pub struct RunGraphReceipt {
    pub identity: RunIdentity,
    pub results: Box<[RunResultReference]>,
}

#[derive(Clone, Debug)]
pub struct RunResultReference {
    pub result_id: yss_graph_execution::result::ResultId,
    pub output: PlanOutputRef,
    pub category: yss_graph_execution::plan::ResultCategory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunApplicationEventKind {
    RunStarted {
        outputs: Box<[PlanOutputRef]>,
    },
    RunCompleted,
    RunCancelled,
    RunErrored {
        failure: yss_graph_execution::error::RunFailure,
    },
    ResultInspectionRequested {
        result_id: yss_graph_execution::result::ResultId,
        source: yss_graph_execution::plan::PlanSourceIdentity,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunApplicationEvent {
    identity: RunIdentity,
    result_revision: u64,
    kind: RunApplicationEventKind,
}

impl RunApplicationEvent {
    fn new(
        execution: &yss_graph_execution::state::ExecutionRuntimeState,
        identity: RunIdentity,
        kind: RunApplicationEventKind,
    ) -> Self {
        Self {
            identity,
            result_revision: execution.result_revision(),
            kind,
        }
    }

    pub fn identity(&self) -> &RunIdentity {
        &self.identity
    }

    pub fn kind(&self) -> &RunApplicationEventKind {
        &self.kind
    }

    pub const fn result_revision(&self) -> u64 {
        self.result_revision
    }
}

#[derive(Debug, Error)]
pub enum ExecutionApplicationError {
    #[error("application session capture failed")]
    SessionCapture(#[source] SessionCaptureError),
    #[error("execution admission failed")]
    Admission(#[source] ExecutionAdmissionError),
    #[error("execution run was cancelled before the public RunId was published")]
    Cancelled,
    #[error("execution run deadline elapsed before the public RunId was published")]
    DeadlineExceeded,
    #[error("project execution preparation failed")]
    ProjectPreparation(#[source] ProjectExecutionPreparationError),
    #[error("project resource binding failed")]
    ResourceBindings(#[source] ResourceBindingError),
    #[error("project facts could not be captured for execution")]
    ProjectFacts(#[source] crate::graph::catalog::ProjectCatalogReadError),
    #[error("database catalog snapshot failed")]
    DatabaseCatalog(#[source] DatabaseError),
    #[error("graph draft is invalid")]
    InvalidDocument(#[source] yss_graph_document_edit::DocumentError),
    #[error("graph draft or its dependencies changed")]
    DraftChanged,
    #[error("graph has blocking diagnostics")]
    GraphNotReady,
    #[error("graph resolution failed: {code}")]
    GraphResolutionFailed { code: Box<str> },
    #[error("graph execution plan preparation failed")]
    GraphPlan(#[source] yss_graph_execution::graph_preparation::GraphPlanError),
    #[error("graph contract mapping failed")]
    GraphContract(#[source] crate::graph::inputs::GraphContractMappingError),
    #[error("execution package preparation failed")]
    PackagePreparation(#[source] PackagePreparationError),
    #[error("prepared execution failed")]
    PreparedExecution(#[source] ExecutePreparedError),
    #[error("project effect preparation failed")]
    ProjectEffectPreparation(#[source] ProjectEffectCommitError),
    #[error("project effect finalization failed")]
    ProjectEffectFinalization(#[source] ProjectEffectCommitError),
    #[error("execution finalization failed")]
    Finalization(#[source] FinalizationError),
    #[error("execution run terminal publication failed")]
    RunFinalization(#[source] yss_graph_execution::run_registry::RunRegistryError),
    #[error("captured application session is stale")]
    StaleSession(#[source] SessionRevalidationError),
}

#[derive(Debug, Error)]
pub enum ResourceBindingError {
    #[error("resource {resource:?} requires {access:?} access")]
    ScopeDenied {
        resource: ProjectResourceId,
        access: ProjectResourceAccess,
    },
    #[error("resource {resource:?} version changed: expected {expected}, actual {actual:?}")]
    VersionChanged {
        resource: ProjectResourceId,
        expected: u64,
        actual: Option<u64>,
    },
    #[error("project dataset snapshot is unavailable")]
    Dataset(#[source] yss_database_runtime::error::DatabaseError),
    #[error("present resource has no version")]
    MissingVersion { resource: ProjectResourceId },
    #[error("project value contains an invalid Execution identity")]
    Identity(#[source] InvalidPlanIdentity),
}

impl From<GraphInputError> for ExecutionApplicationError {
    fn from(error: GraphInputError) -> Self {
        match error {
            GraphInputError::Catalog(error) => Self::ProjectFacts(error),
            GraphInputError::Database(error) => Self::DatabaseCatalog(error),
            GraphInputError::Contract(error) => Self::GraphContract(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelRunOutcome {
    NotFound,
    AlreadyCancelled,
    AlreadyTerminal,
    Requested,
}

/// Execute one graph through the session-bound Application/Graph/Execution
/// owners. The no-sink overload is kept for non-transport callers; commands
/// use `run_graph_with_sink` to attach the ordered run channel.
pub fn run_graph(
    state: &ApplicationState,
    request: RunGraphRequest,
) -> Result<RunId, ExecutionApplicationError> {
    run_graph_with_sink(state, request, |_| true).map(|receipt| receipt.identity.run_id())
}

pub fn run_graph_with_sink<D>(
    state: &ApplicationState,
    mut request: RunGraphRequest,
    mut deliver: D,
) -> Result<RunGraphReceipt, ExecutionApplicationError>
where
    D: FnMut(RunApplicationEvent) -> bool + Send,
{
    let captured = state
        .capture_session()
        .map_err(ExecutionApplicationError::SessionCapture)?;
    let mut deliver = |event: RunApplicationEvent| {
        captured.publish_graph_activity(super::editing::GraphActivity::Execution(event.clone()));
        deliver(event)
    };
    let _execution_admission = captured
        .execution()
        .admit()
        .map_err(ExecutionApplicationError::Admission)?;

    check_control(&request)?;

    yss_graph_document_edit::validate_graph_document(&request.document)
        .map_err(ExecutionApplicationError::InvalidDocument)?;
    let context = GraphResolutionContext::capture(&captured, &request.document)?;
    let analysis = context.resolve(&captured, &request.graph_path, &request.document, "en-US");
    if analysis.semantic_input_hash() != &request.semantic_input_hash {
        return Err(ExecutionApplicationError::DraftChanged);
    }
    let plan_demand = match &request.demand {
        RunDemand::Default => PlanExecutionDemand::Default,
        RunDemand::Node { node_id, mode } => PlanExecutionDemand::Node {
            node: yss_graph_execution::plan::PlanNodeId::from_existing(node_id.to_string().into()),
            mode: mode.clone(),
        },
        RunDemand::Outputs {
            outputs,
            include_default_results,
            reuse_inputs,
        } => PlanExecutionDemand::Outputs {
            outputs: outputs.clone(),
            include_default_results: *include_default_results,
            reuse_inputs: *reuse_inputs,
        },
    };
    let semantics = analysis.semantic_snapshot();
    let scope = GraphExecutionScope::select(&request.graph_path, semantics, &plan_demand)
        .map_err(ExecutionApplicationError::GraphPlan)?;
    if let yss_graph_analysis::GraphResolutionOutcome::InternalFailure { code, node_id, .. } =
        semantics.outcome()
        && node_id.is_none_or(|node| scope.nodes().contains(&node))
    {
        return Err(ExecutionApplicationError::GraphResolutionFailed { code: code.clone() });
    }
    stages::select_stage(
        &request.graph_path,
        &analysis,
        &scope,
        &plan_demand,
        captured.execution(),
        None,
    )?;
    let graph_reads = graph_resource_requirements(semantics, &scope)?;
    if request.inherit_graph_reads
        && let Some(allowed) = &mut request.resource_authorizations
    {
        let mut grants = allowed.to_vec();
        for dependency in &graph_reads {
            if dependency.access() == ProjectResourceAccess::Shared
                && !grants
                    .iter()
                    .any(|entry| &entry.resource == dependency.resource())
            {
                grants.push(RunResourceAuthorization {
                    resource: dependency.resource().clone(),
                    access: ProjectResourceAccess::Shared,
                    expected_version: None,
                });
            }
        }
        *allowed = grants.into_boxed_slice();
    }
    let required_resources =
        merge_resource_requirements(request.required_resources.iter().cloned(), graph_reads);
    if let Some(allowed) = &request.resource_authorizations {
        for required in &required_resources {
            if !allowed.iter().any(|grant| {
                &grant.resource == required.resource()
                    && (required.access() == ProjectResourceAccess::Shared
                        || grant.access == ProjectResourceAccess::Exclusive)
            }) {
                return Err(ExecutionApplicationError::ResourceBindings(
                    ResourceBindingError::ScopeDenied {
                        resource: required.resource().clone(),
                        access: required.access(),
                    },
                ));
            }
        }
    }

    let project_request = ProjectExecutionRequest::new(
        request.project_instance_id.clone(),
        request.graph_path.clone(),
    )
    .with_required_resources(required_resources.iter().cloned());
    let prepared_project = captured
        .project()
        .prepare_execution(project_request)
        .map_err(ExecutionApplicationError::ProjectPreparation)?;

    if let Some(allowed) = &request.resource_authorizations {
        for grant in prepared_project.resources().grants() {
            let permitted = allowed
                .iter()
                .find(|entry| &entry.resource == grant.resource())
                .ok_or(ExecutionApplicationError::ResourceBindings(
                    ResourceBindingError::ScopeDenied {
                        resource: grant.resource().clone(),
                        access: grant.access(),
                    },
                ))?;
            if permitted
                .expected_version
                .is_some_and(|version| grant.version().map(|actual| actual.get()) != Some(version))
            {
                return Err(ExecutionApplicationError::ResourceBindings(
                    ResourceBindingError::VersionChanged {
                        resource: grant.resource().clone(),
                        expected: permitted.expected_version.expect("version was checked"),
                        actual: grant.version().map(|version| version.get()),
                    },
                ));
            }
        }
    }
    let deferred_outputs = analysis
        .semantic_snapshot()
        .nodes()
        .iter()
        .flat_map(|node| &node.ports)
        .filter(|port| {
            matches!(
                port.schema_state,
                yss_graph_analysis::GraphSchemaState::Deferred
                    | yss_graph_analysis::GraphSchemaState::Observed { .. }
            )
        })
        .map(|port| port.address.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let result_revision = captured.execution().result_revision();
    let outcome = stages::execute_stages(
        state,
        &captured,
        &request,
        stages::PreparedGraphRun {
            context,
            analysis,
            scope,
            demand: plan_demand,
            project: prepared_project,
        },
        &mut deliver,
    );
    if !deferred_outputs.is_empty()
        && captured.execution().result_revision() != result_revision
        && let Ok(editing) = captured
            .project()
            .read_graph_editing(captured.project_instance_id(), &request.graph_path)
    {
        captured.publish_graph_activity(super::editing::GraphActivity::Changed {
            graph_path: request.graph_path.as_str().into(),
            editing: editing.state,
        });
    }
    outcome
}

pub fn cancel_run(
    state: &ApplicationState,
    execution_session_id: yss_graph_execution::identity::ExecutionSessionId,
    run_id: RunId,
) -> Result<CancelRunOutcome, ExecutionApplicationError> {
    let captured = state
        .capture_session()
        .map_err(ExecutionApplicationError::SessionCapture)?;
    if captured.execution_session_id() != execution_session_id {
        return Err(ExecutionApplicationError::StaleSession(
            SessionRevalidationError::Changed,
        ));
    }
    Ok(match captured.execution().cancel_run(run_id) {
        ExecutionCancelOutcome::NotFound => CancelRunOutcome::NotFound,
        ExecutionCancelOutcome::AlreadyCancelled => CancelRunOutcome::AlreadyCancelled,
        ExecutionCancelOutcome::AlreadyTerminal => CancelRunOutcome::AlreadyTerminal,
        ExecutionCancelOutcome::Requested => CancelRunOutcome::Requested,
    })
}

fn check_control(request: &RunGraphRequest) -> Result<(), ExecutionApplicationError> {
    if request.is_cancelled() {
        return Err(ExecutionApplicationError::Cancelled);
    }
    if request.is_expired() {
        return Err(ExecutionApplicationError::DeadlineExceeded);
    }
    Ok(())
}

fn terminal_kind_for_effect_error(error: &ProjectEffectCommitError) -> RunApplicationEventKind {
    match error {
        ProjectEffectCommitError::Cancelled => RunApplicationEventKind::RunCancelled,
        _ => RunApplicationEventKind::RunErrored {
            failure: finalization_failure(),
        },
    }
}

fn finalization_failure() -> yss_graph_execution::error::RunFailure {
    yss_graph_execution::error::RunFailure {
        code: yss_graph_execution::error::RunFailureCode::FinalizationFailed,
        phase: RunPhase::Finalization,
        source: None,
        groups: Box::new([]),
    }
}

fn publish_run_failure<D>(
    execution: &yss_graph_execution::state::ExecutionRuntimeState,
    run_id: RunId,
    identity: &RunIdentity,
    deliver: &mut D,
    terminal: RunApplicationEventKind,
) where
    D: FnMut(RunApplicationEvent) -> bool + Send,
{
    if matches!(&terminal, RunApplicationEventKind::RunCancelled) {
        let _ = execution.finalize_run_cancelled(run_id);
    } else {
        let _ = execution.finalize_run_failure(run_id);
    }
    let _ = deliver(RunApplicationEvent::new(
        execution,
        identity.clone(),
        terminal,
    ));
}

fn revalidate_final_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
) -> Result<(), ExecutionApplicationError> {
    state
        .revalidate_captured_session(captured)
        .map_err(ExecutionApplicationError::StaleSession)
}

fn merge_resource_requirements(
    first: impl IntoIterator<Item = ProjectResourceRequirement>,
    second: impl IntoIterator<Item = ProjectResourceRequirement>,
) -> Vec<ProjectResourceRequirement> {
    let mut resources = BTreeMap::new();
    for requirement in first.into_iter().chain(second) {
        resources.insert(requirement.resource().as_str().to_owned(), requirement);
    }
    resources.into_values().collect()
}

fn graph_resource_requirements(
    semantics: &yss_graph_analysis::GraphSemanticSnapshot,
    scope: &GraphExecutionScope,
) -> Result<Vec<ProjectResourceRequirement>, ExecutionApplicationError> {
    semantics
        .resources_for_nodes(scope.nodes())
        .into_iter()
        .map(|identity| {
            let kind = if identity.starts_with("databases/") {
                ProjectResourceKind::DataFrame
            } else {
                ProjectResourceKind::File
            };
            let resource = ProjectResourceId::new(identity).map_err(|_| {
                ExecutionApplicationError::ResourceBindings(ResourceBindingError::Identity(
                    InvalidPlanIdentity::Empty,
                ))
            })?;
            Ok(ProjectResourceRequirement::new(
                resource,
                kind,
                ProjectResourceAccess::Shared,
                false,
            ))
        })
        .collect()
}

fn plan_basis(
    captured: &ApplicationSession,
    grants: &[ProjectResourceGrant],
) -> Result<yss_graph_execution::plan::PlanBasis, ExecutionApplicationError> {
    let mut observations = BTreeMap::new();
    for grant in grants {
        let resource = PlanResourceId::new(grant.resource().as_str().to_owned().into_boxed_str())
            .map_err(|_| {
            ExecutionApplicationError::ResourceBindings(ResourceBindingError::Identity(
                InvalidPlanIdentity::Empty,
            ))
        })?;
        let version = grant
            .version()
            .map(|version| PlanResourceVersion::from_existing(version.get().to_string().into()));
        observations.insert(
            resource,
            match grant.presence() {
                ProjectResourcePresence::Present => {
                    PlanResourceObservedState::Present(version.ok_or_else(|| {
                        ExecutionApplicationError::ResourceBindings(
                            ResourceBindingError::MissingVersion {
                                resource: grant.resource().clone(),
                            },
                        )
                    })?)
                }
                ProjectResourcePresence::Absent => PlanResourceObservedState::Absent(version),
            },
        );
    }
    Ok(yss_graph_execution::plan::PlanBasis::new(
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
        PlanRegistryFingerprint::from_bytes(captured.graph().registry_fingerprint()),
        captured.execution().kernels().fingerprint(),
        observations,
    ))
}

fn map_project_resource_facts(
    captured: &ApplicationSession,
    grants: &[ProjectResourceGrant],
) -> Result<yss_graph_execution::resource_preparation::RunResourceBindings, ResourceBindingError> {
    let mut requirements = Vec::new();
    let mut bindings = Vec::new();
    for grant in grants {
        let resource = PlanResourceId::new(grant.resource().as_str().to_owned().into_boxed_str())
            .map_err(|_| ResourceBindingError::Identity(InvalidPlanIdentity::Empty))?;
        let kind = match grant.kind() {
            ProjectResourceKind::DatabaseConnection => {
                yss_graph_execution::plan::ResourceKind::DatabaseConnection
            }
            ProjectResourceKind::DataFrame => yss_graph_execution::plan::ResourceKind::DataFrame,
            ProjectResourceKind::File => yss_graph_execution::plan::ResourceKind::File,
            ProjectResourceKind::Plot => yss_graph_execution::plan::ResourceKind::Plot,
        };
        let access = match grant.access() {
            ProjectResourceAccess::Shared => yss_graph_execution::plan::ResourceAccess::Shared,
            ProjectResourceAccess::Exclusive => {
                yss_graph_execution::plan::ResourceAccess::Exclusive
            }
        };
        let requirement = yss_graph_execution::plan::PlanResourceRequirement::new(
            resource.clone(),
            kind,
            access,
            grant.optional(),
        );
        requirements.push(requirement.clone());
        if grant.presence() != ProjectResourcePresence::Present {
            continue;
        }
        let version = grant
            .version()
            .ok_or_else(|| ResourceBindingError::MissingVersion {
                resource: grant.resource().clone(),
            })?;
        let value = if grant.kind() == ProjectResourceKind::DataFrame {
            let id = resource
                .as_str()
                .strip_prefix("databases/")
                .filter(|id| !id.is_empty())
                .ok_or(ResourceBindingError::Identity(InvalidPlanIdentity::Empty))?;
            let relation = captured
                .database()
                .capture_relation(
                    &yss_database_contract::DatabaseId::from_existing(id.into()),
                    version.get(),
                )
                .map_err(ResourceBindingError::Dataset)?;
            yss_node_kernel::RuntimeValue::Relation(relation)
        } else {
            yss_node_kernel::RuntimeValue::Resource(resource.as_str().into())
        };
        bindings.push(
            yss_graph_execution::resource_preparation::RunResourceBinding::new(
                requirement,
                PlanResourceVersion::from_existing(version.get().to_string().into()),
                value,
            ),
        );
    }
    Ok(
        yss_graph_execution::resource_preparation::RunResourceBindings::new(
            PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
            requirements,
            bindings,
        ),
    )
}

#[cfg(test)]
mod tests {
    mod ols_configuration;
    use super::*;
    use crate::session::ApplicationSessionEpoch;
    use std::num::NonZeroU64;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
        DatabaseId, DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
    use yss_graph_execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use yss_graph_execution::resource_preparation::ResourceProviderFactory;
    use yss_graph_execution::state::ExecutionRuntimeState;
    use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};
    use yss_node_catalog::build_builtin_node_system;
    use yss_project::ProjectState;
    use yss_project_identity::ProjectSessionId;

    fn session(epoch: u64) -> Arc<ApplicationSession> {
        let project_session_id = ProjectSessionId::new(format!("session-{epoch}"));
        let execution_session_id = ExecutionSessionId::new(uuid::Uuid::from_u128(epoch as u128));
        let project = Arc::new(ProjectState::new());
        let builtin = build_builtin_node_system().expect("test built-ins are valid");
        let graph = Arc::new(
            GraphRuntimeState::from_components(
                GraphRuntimeEpoch::from_existing(epoch),
                GraphRuntimeComponents {
                    registry: builtin.registry,
                    catalog: builtin.catalog,
                },
            )
            .unwrap(),
        );
        let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
            DatabaseId,
            DatabaseDeclarationObservation,
        )>())
        .expect("empty observation set is valid");
        let database = Arc::new(
            DatabaseRuntimeRegistry::new()
                .open_session(DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(project_session_id.as_str().into()),
                    NonZeroU64::new(1).expect("non-zero test generation"),
                    Vec::<DatabaseDecl>::new().into(),
                    observations,
                ))
                .expect("empty database session is valid"),
        );
        let execution = Arc::new(ExecutionRuntimeState::new(
            execution_session_id,
            RuntimeGeneration::from_existing(epoch),
            yss_node_kernel::KernelRegistry::default().into(),
            yss_database_runtime::dataset_query_engine().unwrap(),
        ));
        let resource_provider_factory = Arc::new(ResourceProviderFactory::new(
            project_session_id.as_str().into(),
        ));
        Arc::new(ApplicationSession::new_for_test(
            ApplicationSessionEpoch::from_existing(epoch),
            ProjectInstanceId::from_existing(format!("project-{epoch}")),
            project_session_id,
            execution_session_id,
            RuntimeGeneration::from_existing(epoch),
            project,
            graph,
            execution,
            database,
            resource_provider_factory,
        ))
    }

    #[test]
    fn stale_captured_session_is_rejected_at_the_final_gate() {
        let slot = Arc::new(crate::session::ApplicationSessionSlot::new(
            crate::session::NodeComponents::builtins().unwrap(),
        ));
        let first = session(1);
        slot.publish_for_test(Arc::clone(&first));
        let state = ApplicationState::new(Arc::clone(&slot));
        let captured = state.capture_session().expect("session is active");
        slot.publish_for_test(session(2));

        assert!(matches!(
            revalidate_final_session(&state, &captured),
            Err(ExecutionApplicationError::StaleSession(
                SessionRevalidationError::Changed
            ))
        ));
    }

    #[test]
    fn cancellation_rejects_a_replaced_execution_sessions_same_run_id() {
        let slot = Arc::new(crate::session::ApplicationSessionSlot::new(
            crate::session::NodeComponents::builtins().unwrap(),
        ));
        let first = session(1);
        let run_id = RunId::from_existing(1);
        first.execution().runs().admit(run_id).unwrap();
        slot.publish_for_test(Arc::clone(&first));
        let state = ApplicationState::new(Arc::clone(&slot));

        let successor = session(2);
        successor.execution().runs().admit(run_id).unwrap();
        slot.publish_for_test(Arc::clone(&successor));

        let outcome = cancel_run(&state, first.execution_session_id(), run_id);
        assert!(
            matches!(
                outcome,
                Err(ExecutionApplicationError::StaleSession(
                    SessionRevalidationError::Changed
                ))
            ),
            "old execution session must not cancel the successor's same-ID run: {outcome:?}"
        );
        assert!(matches!(
            cancel_run(&state, successor.execution_session_id(), run_id),
            Ok(CancelRunOutcome::Requested)
        ));
    }

    #[test]
    fn anonymous_admission_and_cancellation_leave_no_public_run() {
        let slot = Arc::new(crate::session::ApplicationSessionSlot::new(
            crate::session::NodeComponents::builtins().unwrap(),
        ));
        let active = session(1);
        slot.publish_for_test(Arc::clone(&active));
        let state = ApplicationState::new(slot);
        let run_id = RunId::from_existing(41);
        let cancellation = Arc::new(AtomicBool::new(true));
        let request = RunGraphRequest::new(
            active.project_instance_id().clone(),
            GraphResourcePath::new("events/cancel.yssbi-event").expect("valid graph path"),
            GraphDocument::default(),
            [1; 32],
        )
        .with_cancellation(cancellation);

        assert!(matches!(
            run_graph(&state, request),
            Err(ExecutionApplicationError::Cancelled)
        ));
        assert_eq!(active.execution().runs().state(run_id), None);

        active.execution().close_admission();
        let request = RunGraphRequest::new(
            active.project_instance_id().clone(),
            GraphResourcePath::new("events/admission.yssbi-event").expect("valid graph path"),
            GraphDocument::default(),
            [1; 32],
        );
        assert!(matches!(
            run_graph(&state, request),
            Err(ExecutionApplicationError::Admission(
                ExecutionAdmissionError::Closed
            ))
        ));
        assert_eq!(active.execution().runs().state(run_id), None);
    }
}
