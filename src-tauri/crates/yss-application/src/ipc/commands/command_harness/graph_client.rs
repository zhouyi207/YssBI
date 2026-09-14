use super::{HarnessRuntimeState, parse_session_id};
use crate::ipc::channel::execution::execution_event_to_transport;
use crate::ipc::error::CommandError;
use crate::{
    automation::{AutomationGraphDraft, AutomationGraphUpdate, prepare_automation_graph_action},
    execution::ApplicationState,
};
use serde::Deserialize;
use tauri::{State, ipc::Channel};
use yss_automation_contract::{CapabilityFailure, CapabilityFailureCode};
use yss_graph_document::GraphDocument;
use yss_ipc_contract::execution::RunEventDto;
use yss_ipc_contract::harness_graph::{HarnessGraphToolRequestDto, HarnessGraphUpdateDto};
use yss_project_identity::ProjectInstanceId;

fn graph_update_to_transport(update: AutomationGraphUpdate) -> HarnessGraphUpdateDto {
    use crate::ipc::schema::graph_draft::{
        compile_graph_draft_to_transport, graph_draft_save_to_transport,
        graph_draft_transform_to_transport,
    };
    match update {
        AutomationGraphUpdate::None => HarnessGraphUpdateDto::None,
        AutomationGraphUpdate::Execution {
            terminal_event_sent,
            status,
        } => HarnessGraphUpdateDto::Execution {
            terminal_event_sent,
            status,
        },
        AutomationGraphUpdate::Draft(update) => {
            HarnessGraphUpdateDto::Draft(graph_draft_transform_to_transport(&update))
        }
        AutomationGraphUpdate::Compilation(update) => {
            HarnessGraphUpdateDto::Compilation(compile_graph_draft_to_transport(&update))
        }
        AutomationGraphUpdate::Saved(update) => {
            HarnessGraphUpdateDto::Saved(graph_draft_save_to_transport(&update))
        }
    }
}

fn command_failure(error: CapabilityFailure) -> CommandError {
    CommandError::expected("harness_graph_action_failed")
        .with_details(serde_json::json!({ "code": error.code }))
}

#[tauri::command]
pub fn subscribe_harness_graph_tools(
    runtime: State<'_, HarnessRuntimeState>,
    session_id: String,
    on_request: Channel<HarnessGraphToolRequestDto>,
) -> Result<String, CommandError> {
    Ok(runtime
        .graph_clients
        .subscribe(parse_session_id(session_id)?, on_request))
}

#[tauri::command]
pub fn unsubscribe_harness_graph_tools(
    runtime: State<'_, HarnessRuntimeState>,
    subscription_id: String,
) {
    runtime.graph_clients.unsubscribe(&subscription_id);
}

#[tauri::command]
pub fn complete_harness_graph_tool(
    runtime: State<'_, HarnessRuntimeState>,
    request_id: String,
    applied: bool,
) {
    runtime.graph_clients.complete(&request_id, applied);
}

#[tauri::command]
pub fn claim_harness_graph_tool(
    runtime: State<'_, HarnessRuntimeState>,
    request_id: String,
) -> bool {
    runtime.graph_clients.claim(&request_id)
}

#[tauri::command]
pub async fn prepare_harness_graph_tool(
    runtime: State<'_, HarnessRuntimeState>,
    application: State<'_, ApplicationState>,
    input: PrepareHarnessGraphToolDto,
    on_execution_event: Channel<RunEventDto>,
) -> Result<HarnessGraphUpdateDto, CommandError> {
    let PrepareHarnessGraphToolDto {
        request_id,
        project_instance_id,
        document,
        draft_generation,
        locale,
    } = input;
    let (context, request, control) = runtime
        .graph_clients
        .begin(&request_id, &project_instance_id)
        .map_err(command_failure)?;
    let application = application.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        prepare_automation_graph_action(
            &application,
            context,
            request,
            AutomationGraphDraft {
                document,
                generation: draft_generation,
                locale,
            },
            &control,
            |event| {
                execution_event_to_transport(event)
                    .is_ok_and(|event| on_execution_event.send(event).is_ok())
            },
        )
    })
    .await
    .unwrap_or_else(|_| {
        Err(CapabilityFailure::new(
            CapabilityFailureCode::InternalFailure,
        ))
    });
    runtime
        .graph_clients
        .stage(&request_id, result)
        .map(graph_update_to_transport)
        .map_err(command_failure)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareHarnessGraphToolDto {
    request_id: String,
    project_instance_id: ProjectInstanceId,
    document: GraphDocument,
    draft_generation: u64,
    locale: String,
}
