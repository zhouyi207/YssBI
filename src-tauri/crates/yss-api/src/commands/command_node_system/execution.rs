use super::common::{RecoveryRequiredDetails, parse_graph_path, parse_opaque_u64};
use crate::commands::execution_dto::{ExecutionChannelEventDto, ExecutionDemandDto};
use crate::error::CommandError;
use serde::Serialize;
use tauri::{State, ipc::Channel};
use yss_application::execution::run_graph::{
    CancelRunOutcome, ExecutionApplicationError, RunApplicationEvent, RunGraphRequest,
    run_graph_with_sink,
};
use yss_project_identity::ProjectInstanceId;

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
        ExecutionApplicationError::ProjectSnapshot(error) => {
            crate::commands::project_failure::application_project_command_error(error)
        }
        ExecutionApplicationError::VariableBindings(error) => {
            CommandError::diagnosed("execution_resource_binding_failed", error)
        }
        ExecutionApplicationError::ProjectFacts(error) => {
            CommandError::diagnosed("execution_project_facts_failed", error)
        }
        ExecutionApplicationError::DatabaseCatalog(error) => {
            CommandError::diagnosed("execution_database_catalog_failed", error)
        }
        ExecutionApplicationError::CompiledDraftUnavailable => {
            CommandError::expected("graph_compile_required")
        }
        ExecutionApplicationError::GraphContract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        ExecutionApplicationError::GraphPackage(error) => {
            CommandError::diagnosed("graph_package_mapping_failed", error)
        }
        ExecutionApplicationError::PackagePreparation(error) => {
            CommandError::diagnosed("invalid_execution_plan", error)
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
        ExecutionApplicationError::RunFinalization(error) => {
            CommandError::diagnosed("execution_run_finalization_failed", error)
        }
        ExecutionApplicationError::StaleSession(error) => match error {
            yss_application::execution::SessionRevalidationError::Unavailable(error) => {
                session_capture_command_error(error)
            }
            yss_application::execution::SessionRevalidationError::Changed => {
                CommandError::expected("stale_project_lifecycle")
            }
        },
    }
}

fn session_capture_command_error(
    error: yss_application::execution::SessionCaptureError,
) -> CommandError {
    match error {
        yss_application::execution::SessionCaptureError::Inactive => {
            CommandError::expected("stale_project_lifecycle")
        }
        yss_application::execution::SessionCaptureError::Replacing => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        yss_application::execution::SessionCaptureError::Recovering => CommandError::expected(
            "project_recovery_required",
        )
        .with_details(RecoveryRequiredDetails {
            recovery_required: true,
        }),
    }
}

fn project_preparation_command_code(
    error: &yss_project::execution_authority::ProjectExecutionPreparationError,
) -> &'static str {
    use yss_project::execution_authority::ProjectExecutionPreparationError;

    match error {
        ProjectExecutionPreparationError::Unavailable
        | ProjectExecutionPreparationError::ProjectIdentityMismatch { .. }
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
    error: &yss_execution::state::ExecutePreparedError,
) -> &'static str {
    use yss_execution::state::ExecutePreparedError;

    match error {
        ExecutePreparedError::RuntimeGenerationMismatch { .. } => "stale_project_lifecycle",
        ExecutePreparedError::Admission(_) => "project_lifecycle_admission_closed",
        ExecutePreparedError::ResourcePreparation(_) => "execution_resource_unavailable",
        ExecutePreparedError::RunRegistry(_) => "execution_run_registry_failed",
        ExecutePreparedError::Cancelled { .. } => "run_cancelled",
        ExecutePreparedError::DeadlineExceeded { .. } => "run_deadline_exceeded",
        ExecutePreparedError::Kernel(_) => "run_failed",
        ExecutePreparedError::ResultIdentityExhausted
        | ExecutePreparedError::ResultTimestamp(_) => "execution_result_publication_failed",
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinPreviewGenerationDto {
    pub generation: u64,
}

#[tauri::command]
pub fn allocate_pin_preview_generation() -> Result<PinPreviewGenerationDto, CommandError> {
    yss_application::pin_preview_generation::allocate_pin_preview_generation()
        .map(|generation| PinPreviewGenerationDto { generation })
        .map_err(|_| CommandError::expected("pin_preview_generation_exhausted"))
}

#[tauri::command]
pub fn cancel_graph_run(
    state: State<'_, yss_application::execution::ApplicationState>,
    run_id: String,
) -> Result<bool, CommandError> {
    let run_id = parse_opaque_u64("runId", &run_id)?;
    let outcome = yss_application::execution::run_graph::cancel_run(
        state.inner(),
        yss_execution::run_registry::RunId::from_existing(run_id),
    )
    .map_err(map_application_execution_error)?;
    Ok(matches!(
        outcome,
        CancelRunOutcome::AlreadyCancelled | CancelRunOutcome::Requested
    ))
}

#[tauri::command]
pub async fn execute_graph_document(
    state: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    compiled_source_hash: String,
    demand: ExecutionDemandDto,
    on_event: Channel<ExecutionChannelEventDto>,
) -> Result<(), CommandError> {
    let graph_path = parse_graph_path(graph_path)?;
    let demand = crate::commands::execution_dto::execution_demand_to_application(demand)
        .map_err(|_| CommandError::expected("invalid_execution_demand"))?;
    let compiled_source_hash = parse_compiled_source_hash(&compiled_source_hash)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let channel = TauriExecutionChannelAdapter { channel: on_event };
        let mut delivery_failed = false;
        let request = RunGraphRequest::new(project_instance_id, graph_path, compiled_source_hash)
            .with_demand(demand);
        let execution = run_graph_with_sink(&state, request, |event| {
            let delivered = channel.deliver(event);
            if !delivered {
                delivery_failed = true;
            }
            delivered
        });
        if delivery_failed {
            Err(execution_channel_command_error())
        } else {
            match execution {
                Ok(_) => Ok(()),
                Err(error) => Err(map_application_execution_error(error)),
            }
        }
    })
    .await
    .map_err(CommandError::internal)?
}

fn parse_compiled_source_hash(value: &str) -> Result<[u8; 32], CommandError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CommandError::expected("invalid_compiled_source_hash"));
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| CommandError::expected("invalid_compiled_source_hash"))?;
    }
    Ok(bytes)
}
