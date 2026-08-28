use super::common::{RecoveryRequiredDetails, parse_graph_path, parse_opaque_u64};
use crate::application::execution::run_graph::{
    CancelRunOutcome, ExecutionApplicationError, RunApplicationEvent, RunGraphRequest,
    run_graph_with_sink,
};
use crate::commands::node_system_execution_dto::{ExecutionChannelEventDto, ExecutionDemandDto};
use crate::error::CommandError;
use crate::project::ProjectInstanceId;
use serde::Serialize;
use tauri::{State, ipc::Channel};

#[cfg(test)]
use crate::application::graph_execution::{
    GraphExecutionDeliveryReport, GraphExecutionStreamEvent, TerminalRunEventKind,
};
#[cfg(test)]
use crate::commands::node_system_execution_dto::RunEventDtoError;

#[cfg(test)]
pub(super) fn execution_channel_event_dto(
    event: GraphExecutionStreamEvent,
) -> Result<ExecutionChannelEventDto, RunEventDtoError> {
    match event {
        GraphExecutionStreamEvent::RunEvent(event) => event.try_into(),
        GraphExecutionStreamEvent::RunOutput(event) => Ok(event.into()),
    }
}

struct TauriExecutionChannelAdapter {
    channel: Channel<ExecutionChannelEventDto>,
}

impl TauriExecutionChannelAdapter {
    fn deliver(&self, event: RunApplicationEvent) -> bool {
        let Ok(event) = ExecutionChannelEventDto::try_from(event) else {
            return false;
        };
        self.channel.send(event).is_ok()
    }
}

pub(super) fn execution_channel_command_error() -> CommandError {
    CommandError::diagnosed(
        "execution_channel_failed",
        "execution channel rejected a streamed event",
    )
}

#[derive(Serialize)]
#[cfg(test)]
#[serde(rename_all = "camelCase")]
struct InternalCompilationFailureCommandDetails<'a> {
    internal_compilation_failure: InternalCompilationFailureDetails<'a>,
}

#[derive(Serialize)]
#[cfg(test)]
#[serde(rename_all = "camelCase")]
struct InternalCompilationFailureDetails<'a> {
    stage: &'static str,
    code: &'a str,
    node_id: Option<String>,
}

#[derive(Serialize)]
#[cfg(test)]
#[serde(rename_all = "camelCase")]
struct TerminalRunEventDetails {
    terminal_run_event_sent: bool,
}

#[cfg(test)]
pub(super) fn execution_command_error(
    error: crate::project::ProjectExecutionError,
    delivery: &GraphExecutionDeliveryReport,
) -> CommandError {
    use crate::project::ProjectExecutionErrorKind;

    match error.kind() {
        ProjectExecutionErrorKind::StaleProjectLifecycle => {
            CommandError::expected("stale_project_lifecycle")
        }
        ProjectExecutionErrorKind::RecoveryRequired => CommandError::expected(
            "project_recovery_required",
        )
        .with_details(RecoveryRequiredDetails {
            recovery_required: true,
        }),
        ProjectExecutionErrorKind::InvalidDemand => {
            CommandError::expected("invalid_execution_demand")
        }
        ProjectExecutionErrorKind::InternalCompilation => {
            let failure = error
                .internal_compilation_failure()
                .expect("internal compilation errors carry failure details");
            let stage = match failure.stage {
                crate::node_system::compiler::CompilationStage::Analysis => "analysis",
                crate::node_system::compiler::CompilationStage::Lowering => "lowering",
            };
            let details = InternalCompilationFailureCommandDetails {
                internal_compilation_failure: InternalCompilationFailureDetails {
                    stage,
                    code: failure.code.as_ref(),
                    node_id: failure.node_id.map(|node_id| node_id.to_string()),
                },
            };
            CommandError::diagnosed("internal_compilation_failure", &error).with_details(details)
        }
        ProjectExecutionErrorKind::Internal => CommandError::internal(error),
        ProjectExecutionErrorKind::Run => {
            let command_error = match delivery.delivered_terminal_kind() {
                Some(TerminalRunEventKind::Cancelled) => CommandError::expected("run_cancelled"),
                Some(TerminalRunEventKind::Errored) => {
                    let code = error
                        .run_error()
                        .map(crate::node_system::runtime::RunErrorCode::from)
                        .and_then(relational_run_command_error_code)
                        .unwrap_or("run_failed");
                    CommandError::diagnosed(code, error)
                }
                Some(TerminalRunEventKind::Completed) | None => {
                    return CommandError::internal(error);
                }
            };
            command_error.with_details(TerminalRunEventDetails {
                terminal_run_event_sent: true,
            })
        }
    }
}

#[cfg(test)]
fn relational_run_command_error_code(
    code: crate::node_system::runtime::RunErrorCode,
) -> Option<&'static str> {
    use crate::node_system::runtime::RunErrorCode;

    match code {
        RunErrorCode::RelationalBackendNotFound => Some("relational_backend_not_found"),
        RunErrorCode::RelationalOperatorInvalid => Some("relational_operator_invalid"),
        RunErrorCode::RelationalColumnMissing => Some("relational_column_missing"),
        RunErrorCode::RelationalTypeMismatch => Some("relational_type_mismatch"),
        RunErrorCode::RelationalInputShapeInvalid => Some("relational_input_shape_invalid"),
        RunErrorCode::RelationalHintInvalid => Some("relational_hint_invalid"),
        _ => None,
    }
}

fn map_application_execution_error(error: ExecutionApplicationError) -> CommandError {
    match error {
        ExecutionApplicationError::SessionCapture(error) => session_capture_command_error(error),
        ExecutionApplicationError::Admission(_) => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        ExecutionApplicationError::Cancelled => CommandError::expected("run_cancelled"),
        ExecutionApplicationError::DeadlineExceeded => {
            CommandError::expected("run_deadline_exceeded")
        }
        ExecutionApplicationError::ProjectPreparation(error) => {
            CommandError::expected(project_preparation_command_code(&error))
        }
        ExecutionApplicationError::ProjectSnapshot(error) => CommandError::from(error),
        ExecutionApplicationError::VariableBindings(error) => {
            CommandError::diagnosed("execution_resource_binding_failed", error)
        }
        ExecutionApplicationError::ProjectFacts(error) => {
            CommandError::diagnosed("execution_project_facts_failed", error)
        }
        ExecutionApplicationError::DatabaseCatalog(error) => {
            CommandError::diagnosed("execution_database_catalog_failed", error)
        }
        ExecutionApplicationError::GraphCompilation(error) => {
            let code = match &error {
                crate::graph::error::GraphCompileError::Catalog(_) => "graph_catalog_invalid",
                crate::graph::error::GraphCompileError::InvalidGraph { .. } => "invalid_graph",
                crate::graph::error::GraphCompileError::Internal(_) => {
                    "internal_compilation_failure"
                }
            };
            CommandError::diagnosed(code, error)
        }
        ExecutionApplicationError::GraphContract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        ExecutionApplicationError::PackagePreparation(error) => {
            CommandError::diagnosed("invalid_execution_plan", error)
        }
        ExecutionApplicationError::PackageUnavailable => {
            CommandError::internal("compiled graph did not produce an execution package")
        }
        ExecutionApplicationError::PreparedExecution(error) => {
            let code = prepared_execution_command_code(&error);
            CommandError::diagnosed(code, error)
        }
        ExecutionApplicationError::ProjectEffectPreparation(error)
        | ExecutionApplicationError::ProjectEffectFinalization(error) => {
            CommandError::diagnosed("execution_effect_commit_failed", error)
        }
        ExecutionApplicationError::Finalization(error) => {
            CommandError::diagnosed("execution_finalization_failed", error)
        }
        ExecutionApplicationError::StaleSession(error) => match error {
            crate::application::execution::SessionRevalidationError::Unavailable(error) => {
                session_capture_command_error(error)
            }
            crate::application::execution::SessionRevalidationError::Changed => {
                CommandError::expected("stale_project_lifecycle")
            }
        },
    }
}

fn session_capture_command_error(
    error: crate::application::execution::SessionCaptureError,
) -> CommandError {
    match error {
        crate::application::execution::SessionCaptureError::Inactive => {
            CommandError::expected("stale_project_lifecycle")
        }
        crate::application::execution::SessionCaptureError::Replacing => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        crate::application::execution::SessionCaptureError::Recovering => CommandError::expected(
            "project_recovery_required",
        )
        .with_details(RecoveryRequiredDetails {
            recovery_required: true,
        }),
    }
}

fn project_preparation_command_code(
    error: &crate::project::execution_authority::ProjectExecutionPreparationError,
) -> &'static str {
    use crate::project::execution_authority::ProjectExecutionPreparationError;

    match error {
        ProjectExecutionPreparationError::Unavailable
        | ProjectExecutionPreparationError::ProjectIdentityMismatch { .. }
        | ProjectExecutionPreparationError::GraphRevisionUnavailable { .. }
        | ProjectExecutionPreparationError::ResourceRevisionUnavailable { .. } => {
            "stale_project_lifecycle"
        }
        ProjectExecutionPreparationError::GraphUnavailable { .. } => "graph_not_loaded",
        ProjectExecutionPreparationError::InvalidGraph { .. } => "invalid_graph",
        ProjectExecutionPreparationError::DuplicateResourceRequirement { .. } => {
            "invalid_execution_resource"
        }
        ProjectExecutionPreparationError::InvalidResourceIdentity { .. } => {
            "invalid_execution_resource"
        }
        ProjectExecutionPreparationError::ResourceUnavailable { .. } => {
            "execution_resource_unavailable"
        }
        ProjectExecutionPreparationError::ResourceKindMismatch { .. }
        | ProjectExecutionPreparationError::UnsupportedResourceKind { .. } => {
            "invalid_execution_resource"
        }
    }
}

fn prepared_execution_command_code(
    error: &crate::execution::state::ExecutePreparedError,
) -> &'static str {
    use crate::execution::state::ExecutePreparedError;

    match error {
        ExecutePreparedError::RuntimeGenerationMismatch { .. } => "stale_project_lifecycle",
        ExecutePreparedError::Admission(_) => "project_lifecycle_admission_closed",
        ExecutePreparedError::ResourcePreparation(_) => "execution_resource_unavailable",
        ExecutePreparedError::RunRegistry(_) => "execution_run_registry_failed",
        ExecutePreparedError::Cancelled { .. } => "run_cancelled",
        ExecutePreparedError::DeadlineExceeded { .. } => "run_deadline_exceeded",
        ExecutePreparedError::KernelUnavailable | ExecutePreparedError::Kernel(_) => "run_failed",
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinPreviewGenerationDto {
    pub generation: u64,
}

#[tauri::command]
pub fn allocate_pin_preview_generation() -> Result<PinPreviewGenerationDto, CommandError> {
    crate::application::pin_preview_generation::allocate_pin_preview_generation()
        .map(|generation| PinPreviewGenerationDto { generation })
        .map_err(|_| CommandError::expected("pin_preview_generation_exhausted"))
}

#[tauri::command]
pub fn cancel_graph_run(
    state: State<'_, crate::application::execution::ApplicationState>,
    run_id: String,
) -> Result<bool, CommandError> {
    let run_id = parse_opaque_u64("runId", &run_id)?;
    let outcome = crate::application::execution::run_graph::cancel_run(
        state.inner(),
        crate::execution::run_registry::RunId::from_existing(run_id),
    )
    .map_err(map_application_execution_error)?;
    Ok(matches!(
        outcome,
        CancelRunOutcome::AlreadyCancelled | CancelRunOutcome::Requested
    ))
}

#[tauri::command]
pub async fn execute_graph_document(
    state: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    demand: ExecutionDemandDto,
    on_event: Channel<ExecutionChannelEventDto>,
) -> Result<(), CommandError> {
    let graph_path = parse_graph_path(graph_path)?;
    let demand =
        crate::commands::node_system_execution_dto::execution_demand_to_application(demand)
            .map_err(|_| CommandError::expected("invalid_execution_demand"))?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let channel = TauriExecutionChannelAdapter { channel: on_event };
        let mut delivery_failed = false;
        let execution = run_graph_with_sink(
            &state,
            RunGraphRequest::new(project_instance_id, graph_path).with_demand(demand),
            |event| {
                let delivered = channel.deliver(event);
                if !delivered {
                    delivery_failed = true;
                }
                delivered
            },
        );
        let result = if delivery_failed {
            Err(execution_channel_command_error())
        } else {
            match execution {
                Ok(_) => Ok(()),
                Err(error) => Err(map_application_execution_error(error)),
            }
        };
        result
    })
    .await
    .map_err(CommandError::internal)?
}
