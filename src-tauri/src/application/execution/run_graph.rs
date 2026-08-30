use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use thiserror::Error;

use super::finalization::{FinalizationError, finalize_successful_run};
use super::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::application::catalog_query::capture_localized_project_facts;
use crate::application::graph_contracts::{build_resource_catalog, execution_package_from_graph};
use crate::database::error::DatabaseError;
use crate::database::session_api::catalog_snapshot;
use crate::project::ProjectFilesystemError;
use crate::project::execution_authority::{
    CandidateProjectEffects, ProjectEffectCommitControl, ProjectEffectCommitError,
    ProjectExecutionPreparationError, ProjectExecutionRequest, ProjectResourceAccess,
    ProjectResourceGrant, ProjectResourceId, ProjectResourceKind, ProjectResourcePresence,
    ProjectResourceRequirement,
};
use yss_execution::error::RunPhase;
use yss_execution::package_preparation::PackagePreparationError;
use yss_execution::plan::{
    CanonicalDecimalError, InvalidPlanIdentity, InvalidPlanParameterId, PlanGraphId, PlanOutputRef,
    PlanProjectSessionId, PlanRegistryFingerprint, PlanResourceId, PlanResourceObservedState,
    PlanResourceVersion,
};
use yss_execution::run_registry::{RunId, RunState};
use yss_execution::state::{
    ExecutePreparedError, ExecutionAdmissionError, ExecutionCancelOutcome, RunExecutionControl,
};
use yss_graph_compiler::{GraphCompilationInput, compile};
use yss_graph_document::GraphResourcePath;
use yss_project_identity::ProjectInstanceId;
use yss_project_model::ProjectData;

/// A run demand is an Application-owned interpretation of the graph execution
/// request. It contains only Pure Leaf graph/plan identities, never transport
/// DTOs or a delivery target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunDemand {
    Default,
    Outputs {
        outputs: Box<[PlanOutputRef]>,
        include_default_results: bool,
    },
    PinPreview {
        output: PlanOutputRef,
        generation: u64,
    },
}

/// A run request is intentionally owned by the Application seam. It carries
/// no transport data and no public RunId before execution has been admitted.
#[derive(Debug)]
pub(crate) struct RunGraphRequest {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    demand: RunDemand,
    required_resources: Box<[ProjectResourceRequirement]>,
    cancellation: Arc<AtomicBool>,
    deadline: Instant,
}

impl RunGraphRequest {
    pub(crate) fn new(
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
    ) -> Self {
        Self {
            project_instance_id,
            graph_path,
            demand: RunDemand::Default,
            required_resources: Box::new([]),
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + std::time::Duration::from_secs(60),
        }
    }

    pub(crate) fn with_demand(mut self, demand: RunDemand) -> Self {
        self.demand = demand;
        self
    }

    pub(crate) fn with_required_resources(
        mut self,
        resources: impl IntoIterator<Item = ProjectResourceRequirement>,
    ) -> Self {
        self.required_resources = resources.into_iter().collect();
        self
    }

    pub(crate) fn with_cancellation(mut self, cancellation: Arc<AtomicBool>) -> Self {
        self.cancellation = cancellation;
        self
    }

    pub(crate) fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = deadline;
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
pub(crate) struct RunIdentity {
    project_session_id: PlanProjectSessionId,
    graph_path: GraphResourcePath,
    run_id: RunId,
}

impl RunIdentity {
    fn new(
        project_session_id: PlanProjectSessionId,
        graph_path: GraphResourcePath,
        run_id: RunId,
    ) -> Self {
        Self {
            project_session_id,
            graph_path,
            run_id,
        }
    }

    pub(crate) fn project_session_id(&self) -> &PlanProjectSessionId {
        &self.project_session_id
    }

    pub(crate) fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub(crate) const fn run_id(&self) -> RunId {
        self.run_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunApplicationEventKind {
    RunStarted,
    RunCompleted,
    RunCancelled,
    RunErrored {
        phase: RunPhase,
    },
    PinPreviewResultReady {
        output: PlanOutputRef,
        generation: u64,
        result_id: yss_execution::result::ResultId,
    },
    ResultInspectionRequested {
        result_id: yss_execution::result::ResultId,
        source: yss_execution::plan::PlanSourceIdentity,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunApplicationEvent {
    identity: RunIdentity,
    kind: RunApplicationEventKind,
}

impl RunApplicationEvent {
    fn new(identity: RunIdentity, kind: RunApplicationEventKind) -> Self {
        Self { identity, kind }
    }

    pub(crate) fn identity(&self) -> &RunIdentity {
        &self.identity
    }

    pub(crate) fn kind(&self) -> &RunApplicationEventKind {
        &self.kind
    }
}

#[derive(Debug, Error)]
pub(crate) enum ExecutionApplicationError {
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
    #[error("project snapshot failed")]
    ProjectSnapshot(#[source] ProjectFilesystemError),
    #[error("project variable binding failed")]
    VariableBindings(#[source] VariableBindingError),
    #[error("project facts could not be captured for execution")]
    ProjectFacts(#[source] crate::application::catalog_query::ProjectCatalogReadError),
    #[error("database catalog snapshot failed")]
    DatabaseCatalog(#[source] DatabaseError),
    #[error("graph compilation failed")]
    GraphCompilation(#[source] yss_graph_compiler::GraphCompileError),
    #[error("graph contract mapping failed")]
    GraphContract(#[source] crate::application::graph_contracts::GraphContractMappingError),
    #[error("graph execution package mapping failed")]
    GraphPackage(#[source] crate::application::graph_contracts::GraphPackageMappingError),
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
    RunFinalization(#[source] yss_execution::run_registry::RunRegistryError),
    #[error("captured application session is stale")]
    StaleSession(#[source] SessionRevalidationError),
}

#[derive(Debug, Error)]
pub(crate) enum VariableBindingError {
    #[error("variable resource identity is invalid")]
    InvalidResource { resource: ProjectResourceId },
    #[error("variable resource has no present value")]
    MissingValue { resource: ProjectResourceId },
    #[error("present variable resource has no version")]
    MissingVersion { resource: ProjectResourceId },
    #[error("variable value identity does not match its resource")]
    IdentityMismatch { resource: ProjectResourceId },
    #[error("project value cannot be represented by Execution")]
    Value(#[source] CanonicalDecimalError),
    #[error("project value cannot be represented by the runtime")]
    RuntimeValue(#[source] yss_execution::value::RuntimeValueError),
    #[error("project value contains an invalid Execution identity")]
    Identity(#[source] InvalidPlanIdentity),
    #[error("project value contains an invalid record field")]
    Field(#[source] InvalidPlanParameterId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CancelRunOutcome {
    NotFound,
    AlreadyCancelled,
    AlreadyTerminal,
    Requested,
}

/// Execute one graph through the session-bound Application/Graph/Execution
/// owners. The no-sink overload is kept for non-transport callers; commands
/// use `run_graph_with_sink` to attach the ordered run channel.
pub(crate) fn run_graph(
    state: &ApplicationState,
    request: RunGraphRequest,
) -> Result<RunId, ExecutionApplicationError> {
    run_graph_with_sink(state, request, |_| true)
}

pub(crate) fn run_graph_with_sink<D>(
    state: &ApplicationState,
    request: RunGraphRequest,
    mut deliver: D,
) -> Result<RunId, ExecutionApplicationError>
where
    D: FnMut(RunApplicationEvent) -> bool + Send,
{
    let captured = state
        .capture_session()
        .map_err(ExecutionApplicationError::SessionCapture)?;
    let _execution_admission = captured
        .execution()
        .admit()
        .map_err(ExecutionApplicationError::Admission)?;

    check_control(&request)?;

    let initial_data = captured
        .project()
        .get_data()
        .map_err(ExecutionApplicationError::ProjectSnapshot)?;
    let required_resources = merge_resource_requirements(
        request.required_resources.iter().cloned(),
        graph_resource_requirements(&initial_data, &request.graph_path)?,
    );

    let project_request = ProjectExecutionRequest::new(
        request.project_instance_id.clone(),
        request.graph_path.clone(),
    )
    .with_required_resources(required_resources.iter().cloned());
    let prepared_project = captured
        .project()
        .prepare_execution(project_request)
        .map_err(ExecutionApplicationError::ProjectPreparation)?;

    check_control(&request)?;

    let project_data = captured
        .project()
        .get_data()
        .map_err(ExecutionApplicationError::ProjectSnapshot)?;
    let project_facts = capture_localized_project_facts(&captured)
        .map_err(ExecutionApplicationError::ProjectFacts)?;
    let database_facts = catalog_snapshot(captured.database())
        .map_err(ExecutionApplicationError::DatabaseCatalog)?;
    let _validated_graph_catalog =
        build_resource_catalog(project_facts.resources().graph(), &database_facts)
            .map_err(ExecutionApplicationError::GraphContract)?;
    let graph_document = prepared_project.graph();
    let basis = plan_basis(
        &captured,
        prepared_project.authority().graph_revision(),
        prepared_project.resources().grants(),
    )?;
    let graph_package = compile(GraphCompilationInput::new(
        graph_document,
        yss_graph_document::GraphRevision::new(prepared_project.authority().graph_revision().get()),
        request.graph_path.clone(),
        yss_graph_analysis_contract::CompileId::new(
            prepared_project.authority().graph_revision().get(),
        ),
    ))
    .map_err(ExecutionApplicationError::GraphCompilation)?;
    let package = execution_package_from_graph(graph_package, basis)
        .map_err(ExecutionApplicationError::GraphPackage)?;
    let prepared_plan = captured
        .execution()
        .prepare_compiled_package(package, captured.runtime_generation())
        .map_err(ExecutionApplicationError::PackagePreparation)?;
    let bindings = map_project_resource_facts(
        captured.project_session_id().as_str(),
        &project_data,
        prepared_project.resources().grants(),
    )
    .map_err(ExecutionApplicationError::VariableBindings)?;
    drop(project_data);

    check_control(&request)?;
    revalidate_final_session(state, &captured)?;

    let control =
        RunExecutionControl::with_cancellation(Arc::clone(&request.cancellation), request.deadline);
    let mut started_identity = None;
    let executed = match captured.execution().execute_prepared_handoff(
        &prepared_plan,
        bindings,
        captured.resource_provider_factory(),
        &control,
        |run_id| {
            let identity = RunIdentity::new(
                PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
                request.graph_path.clone(),
                run_id,
            );
            started_identity = Some(identity.clone());
            let _ = deliver(RunApplicationEvent::new(
                identity,
                RunApplicationEventKind::RunStarted,
            ));
        },
    ) {
        Ok(executed) => executed,
        Err(error) => {
            if let Some(identity) = started_identity {
                let kind = if matches!(&error, ExecutePreparedError::Cancelled { .. }) {
                    RunApplicationEventKind::RunCancelled
                } else {
                    RunApplicationEventKind::RunErrored {
                        phase: RunPhase::Execution,
                    }
                };
                let _ = deliver(RunApplicationEvent::new(identity, kind));
            }
            return Err(ExecutionApplicationError::PreparedExecution(error));
        }
    };
    let run_id = executed.run_id();
    let identity = started_identity.unwrap_or_else(|| {
        RunIdentity::new(
            PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
            request.graph_path.clone(),
            run_id,
        )
    });

    let prepared_effects = match captured.project().prepare_execution_effects(
        prepared_project.authority(),
        CandidateProjectEffects::empty(),
    ) {
        Ok(effects) => effects,
        Err(error) => {
            publish_run_failure(
                captured.execution(),
                run_id,
                &identity,
                &mut deliver,
                terminal_kind_for_effect_error(&error),
            );
            return Err(ExecutionApplicationError::ProjectEffectPreparation(error));
        }
    };
    let committed_effects = match captured.project().finalize_execution_effects(
        prepared_effects,
        &ProjectEffectCommitControl::new(Arc::clone(&request.cancellation), request.deadline),
    ) {
        Ok(effects) => effects,
        Err(error) => {
            publish_run_failure(
                captured.execution(),
                run_id,
                &identity,
                &mut deliver,
                terminal_kind_for_effect_error(&error),
            );
            return Err(ExecutionApplicationError::ProjectEffectFinalization(error));
        }
    };
    let outcome = match finalize_successful_run(
        executed.into_handoff(),
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
        PlanGraphId::from_existing(request.graph_path.as_str().into()),
        run_id,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            publish_run_failure(
                captured.execution(),
                run_id,
                &identity,
                &mut deliver,
                RunApplicationEventKind::RunErrored {
                    phase: RunPhase::Finalization,
                },
            );
            return Err(ExecutionApplicationError::Finalization(error));
        }
    };
    captured
        .execution()
        .publish_committed_results(outcome.handoff());
    if let Err(error) = captured.execution().finalize_run_success(run_id) {
        publish_run_failure(
            captured.execution(),
            run_id,
            &identity,
            &mut deliver,
            RunApplicationEventKind::RunErrored {
                phase: RunPhase::Finalization,
            },
        );
        return Err(ExecutionApplicationError::RunFinalization(error));
    }

    for inspection in outcome.inspection_requests() {
        let _ = deliver(RunApplicationEvent::new(
            identity.clone(),
            RunApplicationEventKind::ResultInspectionRequested {
                result_id: inspection.result_id(),
                source: inspection.requester().clone(),
            },
        ));
    }
    if let RunDemand::PinPreview { output, generation } = &request.demand
        && let Some(result) = outcome.results().first()
    {
        let _ = deliver(RunApplicationEvent::new(
            identity.clone(),
            RunApplicationEventKind::PinPreviewResultReady {
                output: output.clone(),
                generation: *generation,
                result_id: result.result_id(),
            },
        ));
    }
    let _ = deliver(RunApplicationEvent::new(
        identity,
        RunApplicationEventKind::RunCompleted,
    ));
    drop(committed_effects);
    Ok(run_id)
}

pub(crate) fn cancel_run(
    state: &ApplicationState,
    run_id: RunId,
) -> Result<CancelRunOutcome, ExecutionApplicationError> {
    let captured = state
        .capture_session()
        .map_err(ExecutionApplicationError::SessionCapture)?;
    match captured.execution().runs().state(run_id) {
        None => Ok(CancelRunOutcome::NotFound),
        Some(RunState::Cancelled) => Ok(CancelRunOutcome::AlreadyCancelled),
        Some(RunState::Succeeded | RunState::Failed) => Ok(CancelRunOutcome::AlreadyTerminal),
        Some(RunState::Admitted | RunState::Running | RunState::Finalizing) => {
            Ok(match captured.execution().cancel_run(run_id) {
                ExecutionCancelOutcome::NotFound => CancelRunOutcome::NotFound,
                ExecutionCancelOutcome::AlreadyCancelled => CancelRunOutcome::AlreadyCancelled,
                ExecutionCancelOutcome::AlreadyTerminal => CancelRunOutcome::AlreadyTerminal,
                ExecutionCancelOutcome::Requested => CancelRunOutcome::Requested,
            })
        }
    }
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
            phase: RunPhase::Finalization,
        },
    }
}

fn publish_run_failure<D>(
    execution: &yss_execution::state::ExecutionRuntimeState,
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
    let _ = deliver(RunApplicationEvent::new(identity.clone(), terminal));
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
    data: &ProjectData,
    graph_path: &GraphResourcePath,
) -> Result<Vec<ProjectResourceRequirement>, ExecutionApplicationError> {
    let Some(graph) = data.graphs.get(graph_path) else {
        return Ok(Vec::new());
    };
    let mut requirements = Vec::new();
    for value in graph
        .document
        .nodes
        .values()
        .flat_map(|node| node.parameters.values())
    {
        collect_resource_requirements(value, &mut requirements)?;
    }
    Ok(requirements)
}

fn collect_resource_requirements(
    value: &serde_json::Value,
    requirements: &mut Vec<ProjectResourceRequirement>,
) -> Result<(), ExecutionApplicationError> {
    match value {
        serde_json::Value::String(value) => {
            let kind = if value.starts_with("variables/") {
                ProjectResourceKind::Variable
            } else if value.starts_with("databases/") {
                ProjectResourceKind::DataFrame
            } else if value.starts_with("events/") || value.starts_with("functions/") {
                ProjectResourceKind::File
            } else {
                return Ok(());
            };
            let resource =
                ProjectResourceId::new(value.clone().into_boxed_str()).map_err(|_| {
                    ExecutionApplicationError::VariableBindings(VariableBindingError::Identity(
                        InvalidPlanIdentity::Empty,
                    ))
                })?;
            requirements.push(ProjectResourceRequirement::new(
                resource,
                kind,
                ProjectResourceAccess::Shared,
                false,
            ));
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_resource_requirements(value, requirements)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                collect_resource_requirements(value, requirements)?;
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
    Ok(())
}

fn plan_basis(
    captured: &ApplicationSession,
    graph_revision: yss_graph_document::GraphRevision,
    grants: &[ProjectResourceGrant],
) -> Result<yss_execution::plan::PlanCompilationBasis, ExecutionApplicationError> {
    let mut versions = BTreeMap::new();
    let mut observations = BTreeMap::new();
    for grant in grants {
        let resource = PlanResourceId::new(grant.resource().as_str().to_owned().into_boxed_str())
            .map_err(|_| {
            ExecutionApplicationError::VariableBindings(VariableBindingError::Identity(
                InvalidPlanIdentity::Empty,
            ))
        })?;
        let version = grant
            .version()
            .map(|version| PlanResourceVersion::from_existing(version.get().to_string().into()));
        if let Some(version) = version.clone() {
            versions.insert(resource.clone(), version.clone());
        }
        observations.insert(
            resource,
            match grant.presence() {
                ProjectResourcePresence::Present => {
                    PlanResourceObservedState::Present(version.ok_or_else(|| {
                        ExecutionApplicationError::VariableBindings(
                            VariableBindingError::MissingVersion {
                                resource: grant.resource().clone(),
                            },
                        )
                    })?)
                }
                ProjectResourcePresence::Absent => PlanResourceObservedState::Absent(version),
            },
        );
    }
    Ok(yss_execution::plan::PlanCompilationBasis::new(
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
        yss_execution::plan::PlanGraphRevision::from_existing(graph_revision.get()),
        PlanRegistryFingerprint::from_bytes(captured.graph().registry_fingerprint()),
        versions,
        observations,
    ))
}

fn map_project_resource_facts(
    project_session_id: &str,
    project_data: &ProjectData,
    grants: &[ProjectResourceGrant],
) -> Result<yss_execution::resource_preparation::RunResourceBindings, VariableBindingError> {
    let mut requirements = Vec::new();
    let mut bindings = Vec::new();
    for grant in grants {
        let resource = PlanResourceId::new(grant.resource().as_str().to_owned().into_boxed_str())
            .map_err(|_| VariableBindingError::Identity(InvalidPlanIdentity::Empty))?;
        let kind = match grant.kind() {
            ProjectResourceKind::DatabaseConnection => {
                yss_execution::plan::ResourceKind::DatabaseConnection
            }
            ProjectResourceKind::DataFrame => yss_execution::plan::ResourceKind::DataFrame,
            ProjectResourceKind::File => yss_execution::plan::ResourceKind::File,
            ProjectResourceKind::Variable => yss_execution::plan::ResourceKind::Variable,
            ProjectResourceKind::Plot => yss_execution::plan::ResourceKind::Plot,
        };
        let access = match grant.access() {
            ProjectResourceAccess::Shared => yss_execution::plan::ResourceAccess::Shared,
            ProjectResourceAccess::Exclusive => yss_execution::plan::ResourceAccess::Exclusive,
        };
        let requirement = yss_execution::plan::PlanResourceRequirement::new(
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
            .ok_or_else(|| VariableBindingError::MissingVersion {
                resource: grant.resource().clone(),
            })?;
        let value = if grant.kind() == ProjectResourceKind::Variable {
            let variable_id = variable_id_from_resource(grant.resource())?;
            let variable = project_data.variables.get(&variable_id).ok_or_else(|| {
                VariableBindingError::MissingValue {
                    resource: grant.resource().clone(),
                }
            })?;
            if variable.id != variable_id {
                return Err(VariableBindingError::IdentityMismatch {
                    resource: grant.resource().clone(),
                });
            }
            yss_execution::value::RuntimeValue::try_from(&variable.data_value)
                .map_err(VariableBindingError::RuntimeValue)?
        } else {
            yss_execution::value::RuntimeValue::Resource(resource.as_str().into())
        };
        bindings.push(
            yss_execution::resource_preparation::RunResourceBinding::new(
                requirement,
                PlanResourceVersion::from_existing(version.get().to_string().into()),
                value,
            ),
        );
    }
    Ok(
        yss_execution::resource_preparation::RunResourceBindings::new(
            PlanProjectSessionId::from_existing(project_session_id.into()),
            requirements,
            bindings,
        ),
    )
}

fn variable_id_from_resource(
    resource: &ProjectResourceId,
) -> Result<yss_variable_contract::VariableId, VariableBindingError> {
    let value = resource
        .as_str()
        .strip_prefix("variables/")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| VariableBindingError::InvalidResource {
            resource: resource.clone(),
        })?;
    yss_variable_contract::VariableId::try_from(value).map_err(|_| {
        VariableBindingError::InvalidResource {
            resource: resource.clone(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::execution::ApplicationSessionEpoch;
    use crate::database::runtime::DatabaseRuntimeRegistry;
    use crate::project::ProjectState;
    use std::num::NonZeroU64;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
        DatabaseId, DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use yss_execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use yss_execution::resource_preparation::ResourceProviderFactory;
    use yss_execution::state::ExecutionRuntimeState;
    use yss_graph_catalog::build_builtin_node_system;
    use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};
    use yss_project_identity::ProjectSessionId;

    fn session(epoch: u64) -> Arc<ApplicationSession> {
        let project_session_id = ProjectSessionId::new(format!("session-{epoch}"));
        let execution_session_id = ExecutionSessionId::new(uuid::Uuid::from_u128(epoch as u128));
        let project = Arc::new(ProjectState::new());
        let builtin = build_builtin_node_system().expect("test built-ins are valid");
        let graph = Arc::new(GraphRuntimeState::from_components(
            GraphRuntimeEpoch::from_existing(epoch),
            GraphRuntimeComponents {
                registry: builtin.registry,
                catalog: builtin.catalog,
            },
        ));
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
                    None,
                    Vec::<DatabaseDecl>::new().into(),
                    observations,
                ))
                .expect("empty database session is valid"),
        );
        let execution = Arc::new(ExecutionRuntimeState::new(
            execution_session_id,
            RuntimeGeneration::from_existing(epoch),
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
        let slot = Arc::new(super::super::session_slot::ApplicationSessionSlot::new());
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
    fn anonymous_admission_and_cancellation_leave_no_public_run() {
        let slot = Arc::new(super::super::session_slot::ApplicationSessionSlot::new());
        let active = session(1);
        slot.publish_for_test(Arc::clone(&active));
        let state = ApplicationState::new(slot);
        let run_id = RunId::from_existing(41);
        let cancellation = Arc::new(AtomicBool::new(true));
        let request = RunGraphRequest::new(
            active.project_instance_id().clone(),
            GraphResourcePath::new("events/cancel.yssbi-event").expect("valid graph path"),
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
