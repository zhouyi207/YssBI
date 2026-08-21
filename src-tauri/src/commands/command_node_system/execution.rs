use super::common::{RecoveryRequiredDetails, parse_graph_path, parse_opaque_u64};
use crate::application::graph_execution::{
    GraphExecutionDeliveryReport, GraphExecutionRequest, GraphExecutionStreamEvent,
    TerminalRunEventKind,
};
use crate::commands::node_system_execution_dto::{
    ExecutionChannelEventDto, ExecutionDemandDto, RunEventDtoError,
};
use crate::error::CommandError;
use crate::event::{Event, EventProject, emit_project_event};
use crate::project::{ProjectInstanceId, ProjectState};
use serde::Serialize;
use tauri::{AppHandle, State, ipc::Channel};

struct TauriExecutionChannelAdapter {
    channel: Channel<ExecutionChannelEventDto>,
}

impl TauriExecutionChannelAdapter {
    fn deliver(&self, event: GraphExecutionStreamEvent) -> bool {
        let Ok(event) = execution_channel_event_dto(event) else {
            return false;
        };
        self.channel.send(event).is_ok()
    }
}

pub(super) fn execution_channel_event_dto(
    event: GraphExecutionStreamEvent,
) -> Result<ExecutionChannelEventDto, RunEventDtoError> {
    match event {
        GraphExecutionStreamEvent::RunEvent(event) => event.try_into(),
        GraphExecutionStreamEvent::RunOutput(event) => Ok(event.into()),
    }
}

pub(super) fn execution_channel_command_error() -> CommandError {
    CommandError::diagnosed(
        "execution_channel_failed",
        "execution channel rejected a streamed event",
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalCompilationFailureCommandDetails<'a> {
    internal_compilation_failure: InternalCompilationFailureDetails<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InternalCompilationFailureDetails<'a> {
    stage: &'static str,
    code: &'a str,
    node_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalRunEventDetails {
    terminal_run_event_sent: bool,
}

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
    state: State<'_, ProjectState>,
    run_id: String,
) -> Result<bool, CommandError> {
    let run_id = parse_opaque_u64("runId", &run_id)?;
    Ok(state.cancel_graph_run(crate::node_system::runtime::RunId::new(run_id)))
}

#[tauri::command]
pub async fn execute_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    demand: ExecutionDemandDto,
    on_event: Channel<ExecutionChannelEventDto>,
) -> Result<(), CommandError> {
    let graph_path = parse_graph_path(graph_path)?;
    let demand = crate::node_system::plan::ExecutionDemand::try_from(demand)
        .map_err(|_| CommandError::expected("invalid_execution_demand"))?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let channel = TauriExecutionChannelAdapter { channel: on_event };
        let execution = crate::application::graph_execution::execute_graph(
            &state,
            GraphExecutionRequest {
                project_instance_id: project_instance_id.clone(),
                graph_path,
                demand,
            },
            |event| channel.deliver(event),
        );
        let delivery = match &execution {
            Ok(outcome) => outcome.delivery.clone(),
            Err(error) => error.delivery.clone(),
        };
        if let Ok(outcome) = &execution
            && let Some(result) = &outcome.resource_mutation
        {
            emit_project_event(
                &app,
                Event::Project(EventProject::ResourceMutationCommitted {
                    result: result.clone(),
                }),
            );
        }
        let result = if delivery.delivery_failed() {
            Err(execution_channel_command_error())
        } else {
            match execution {
                Ok(_) => Ok(()),
                Err(error) => Err(execution_command_error(
                    error.project_error,
                    &error.delivery,
                )),
            }
        };
        result
    })
    .await
    .map_err(CommandError::internal)?
}
