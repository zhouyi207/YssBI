#![allow(
    dead_code,
    reason = "staged until the execution cutover installs its consumer"
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use thiserror::Error;

use super::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::data_contract::DataValue;
use crate::execution::plan::{
    CanonicalDecimal, CanonicalDecimalError, InvalidPlanIdentity, InvalidPlanParameterId,
    PlanParameterFieldId, PlanParameterScalar, PlanParameterValue, PlanResourceId,
};
use crate::execution::run_registry::{RunId, RunState};
use crate::execution::state::ExecutionAdmissionError;
use crate::graph_document::GraphResourcePath;
use crate::project::execution_authority::{
    ProjectExecutionPreparationError, ProjectExecutionRequest, ProjectResourceGrant,
    ProjectResourceId, ProjectResourceKind, ProjectResourcePresence, ProjectResourceRequirement,
    ProjectResourceVersion,
};
use crate::project::{ProjectData, ProjectFilesystemError, ProjectInstanceId};

/// A run request is intentionally owned by the Application seam. It carries
/// no transport data and no public RunId before execution has been admitted.
#[derive(Debug)]
pub(crate) struct RunGraphRequest {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
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
            required_resources: Box::new([]),
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + std::time::Duration::from_secs(60),
        }
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunResourceBinding {
    resource: ProjectResourceId,
    version: ProjectResourceVersion,
    value: PlanParameterValue,
}

impl RunResourceBinding {
    pub(crate) fn resource(&self) -> &ProjectResourceId {
        &self.resource
    }

    pub(crate) const fn version(&self) -> ProjectResourceVersion {
        self.version
    }

    pub(crate) fn value(&self) -> &PlanParameterValue {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunResourceBindings {
    project_session_id: Box<str>,
    bindings: Box<[RunResourceBinding]>,
}

impl RunResourceBindings {
    pub(crate) fn project_session_id(&self) -> &str {
        &self.project_session_id
    }

    pub(crate) fn as_slice(&self) -> &[RunResourceBinding] {
        &self.bindings
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum ExecutionStagedError {
    #[error("the execution prepared-plan consumer is not installed")]
    PreparedPlanConsumerUnavailable,
    #[error("the public run cancellation consumer is not installed")]
    RunCancellationConsumerUnavailable,
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
    #[error("captured application session is stale")]
    StaleSession(#[source] SessionRevalidationError),
    #[error("execution remains staged")]
    Staged(#[source] ExecutionStagedError),
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
}

/// Staged coordinator entry point.
///
/// The current Graph/Execution seam does not expose a consumer for the
/// immutable prepared execution handle. The coordinator therefore performs
/// every safe pre-plan step and returns a typed staged result. It never
/// registers a public RunId on this path.
pub(crate) fn run_graph(
    state: &ApplicationState,
    request: RunGraphRequest,
) -> Result<(), ExecutionApplicationError> {
    let captured = state
        .capture_session()
        .map_err(ExecutionApplicationError::SessionCapture)?;
    let _execution_admission = captured
        .execution()
        .admit()
        .map_err(ExecutionApplicationError::Admission)?;

    check_control(&request)?;

    let project_request = ProjectExecutionRequest::new(
        request.project_instance_id.clone(),
        request.graph_path.clone(),
    )
    .with_required_resources(request.required_resources.iter().cloned());
    let prepared_project = captured
        .project()
        .prepare_execution(project_request)
        .map_err(ExecutionApplicationError::ProjectPreparation)?;

    check_control(&request)?;

    // This is a short authoritative snapshot. The lock held by ProjectState is
    // released when get_data returns, before any later runtime/backend work.
    let project_data = captured
        .project()
        .get_data()
        .map_err(ExecutionApplicationError::ProjectSnapshot)?;
    let _bindings = map_project_variable_facts(
        captured.project_session_id().as_str(),
        &project_data,
        prepared_project.resources().grants(),
    )
    .map_err(ExecutionApplicationError::VariableBindings)?;
    drop(project_data);

    check_control(&request)?;
    revalidate_final_session(state, &captured)?;

    // The plan is deliberately not fabricated or reconstructed here. A later
    // slice must provide the Graph-produced PreparedExecutionPlan consumer and
    // the Execution execute_prepared seam before RunId publication is allowed.
    drop(prepared_project);
    Err(ExecutionApplicationError::Staged(
        ExecutionStagedError::PreparedPlanConsumerUnavailable,
    ))
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
            Err(ExecutionApplicationError::Staged(
                ExecutionStagedError::RunCancellationConsumerUnavailable,
            ))
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

fn revalidate_final_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
) -> Result<(), ExecutionApplicationError> {
    state
        .revalidate_captured_session(captured)
        .map_err(ExecutionApplicationError::StaleSession)
}

fn map_project_variable_facts(
    project_session_id: &str,
    project_data: &ProjectData,
    grants: &[ProjectResourceGrant],
) -> Result<RunResourceBindings, VariableBindingError> {
    let mut bindings = Vec::new();
    for grant in grants {
        if grant.kind() != ProjectResourceKind::Variable
            || grant.presence() != ProjectResourcePresence::Present
        {
            continue;
        }
        let resource = grant.resource().clone();
        let variable_id = variable_id_from_resource(&resource)?;
        let version = grant
            .version()
            .ok_or_else(|| VariableBindingError::MissingVersion {
                resource: resource.clone(),
            })?;
        let variable = project_data.variables.get(&variable_id).ok_or_else(|| {
            VariableBindingError::MissingValue {
                resource: resource.clone(),
            }
        })?;
        if variable.id != variable_id {
            return Err(VariableBindingError::IdentityMismatch { resource });
        }
        let value = data_value_to_binding(&variable.data_value)?;
        bindings.push(RunResourceBinding {
            resource,
            version,
            value,
        });
    }
    Ok(RunResourceBindings {
        project_session_id: project_session_id.into(),
        bindings: bindings.into_boxed_slice(),
    })
}

fn data_value_to_binding(value: &DataValue) -> Result<PlanParameterValue, VariableBindingError> {
    match value {
        DataValue::Null => Ok(PlanParameterValue::Scalar(PlanParameterScalar::Null)),
        DataValue::Boolean(value) => Ok(PlanParameterValue::Scalar(PlanParameterScalar::Bool(
            *value,
        ))),
        DataValue::Int64(value) => Ok(PlanParameterValue::Scalar(PlanParameterScalar::Integer(
            *value,
        ))),
        DataValue::Float64(value) => CanonicalDecimal::try_new(*value)
            .map(PlanParameterScalar::Decimal)
            .map(PlanParameterValue::Scalar)
            .map_err(VariableBindingError::Value),
        DataValue::String(value) => Ok(PlanParameterValue::Scalar(PlanParameterScalar::String(
            value.clone().into_boxed_str(),
        ))),
        DataValue::Array(values) => values
            .iter()
            .map(data_value_to_binding)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| PlanParameterValue::List(values.into_boxed_slice())),
        DataValue::Object(values) => values
            .iter()
            .map(|(field, value)| {
                Ok((
                    PlanParameterFieldId::new(field.clone().into_boxed_str())
                        .map_err(VariableBindingError::Field)?,
                    data_value_to_binding(value)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, VariableBindingError>>()
            .map(PlanParameterValue::Record),
        DataValue::DataFrame(resource) => PlanResourceId::new(resource.clone().into_boxed_str())
            .map(PlanParameterValue::Resource)
            .map_err(VariableBindingError::Identity),
        DataValue::DataSeries(series) => PlanResourceId::new(series.id.clone().into_boxed_str())
            .map(PlanParameterValue::Resource)
            .map_err(VariableBindingError::Identity),
        DataValue::Struct { handle_id, .. } => {
            PlanResourceId::new(handle_id.clone().into_boxed_str())
                .map(PlanParameterValue::Resource)
                .map_err(VariableBindingError::Identity)
        }
    }
}

fn variable_id_from_resource(
    resource: &ProjectResourceId,
) -> Result<crate::variable::VariableId, VariableBindingError> {
    let value = resource
        .as_str()
        .strip_prefix("variables/")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| VariableBindingError::InvalidResource {
            resource: resource.clone(),
        })?;
    crate::variable::VariableId::try_from(value).map_err(|_| {
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
    use crate::database_contract::{
        DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
        DatabaseId, DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use crate::execution::resource_preparation::ResourceProviderFactory;
    use crate::execution::state::ExecutionRuntimeState;
    use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
    use crate::graph::runtime_state::{
        GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState,
    };
    use crate::node_system::ProjectSessionId;
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::compiler::ProjectCompileCoordinator;
    use crate::project::ProjectState;
    use std::collections::BTreeMap;
    use std::num::NonZeroU64;

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
                compiler: Arc::new(ProjectCompileCoordinator::new()),
                resource_catalog: Arc::new(ResourceCatalogSnapshot::new(
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                    ResourceCatalogFingerprint::from_bytes([epoch as u8; 32]),
                )),
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
