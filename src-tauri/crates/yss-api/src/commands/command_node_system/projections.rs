use tauri::State;
use tauri::ipc::Channel;

use crate::GraphProjectionRuntime;
use crate::error::CommandError;
use crate::graph_projection_runtime::{GraphProjectionRuntimeError, ResolveGraphProjectionRequest};
use crate::schema::graph_projection_channel::{
    GraphProjectionChannelEventDto, GraphProjectionSnapshotDto, GraphProjectionSubscriptionDto,
};
use yss_application::execution::ApplicationState;

#[tauri::command]
pub fn subscribe_graph_projections(
    application: State<'_, ApplicationState>,
    runtime: State<'_, GraphProjectionRuntime>,
    project_instance_id: String,
    on_events: Channel<GraphProjectionChannelEventDto>,
) -> Result<GraphProjectionSubscriptionDto, CommandError> {
    validate_current_project(&application, &project_instance_id)?;
    runtime
        .subscribe(project_instance_id, on_events)
        .map_err(graph_projection_runtime_error)
}

#[tauri::command]
pub fn get_graph_projection_snapshot(
    application: State<'_, ApplicationState>,
    runtime: State<'_, GraphProjectionRuntime>,
    project_instance_id: String,
) -> Result<GraphProjectionSnapshotDto, CommandError> {
    if project_instance_id.is_empty() {
        return Err(CommandError::expected("invalid_project_instance_id"));
    }
    validate_current_project(&application, &project_instance_id)?;
    Ok(runtime.snapshot(&project_instance_id))
}

fn validate_current_project(
    application: &ApplicationState,
    project_instance_id: &str,
) -> Result<(), CommandError> {
    let captured = application
        .capture_session()
        .map_err(|_| CommandError::expected("stale_project_lifecycle"))?;
    if captured.project_instance_id().as_str() != project_instance_id {
        return Err(CommandError::expected("stale_project_lifecycle"));
    }
    Ok(())
}

#[tauri::command]
pub fn unsubscribe_graph_projections(
    runtime: State<'_, GraphProjectionRuntime>,
    subscription_id: String,
) -> Result<(), CommandError> {
    runtime
        .unsubscribe(&subscription_id)
        .map_err(graph_projection_runtime_error)
}

pub(super) fn submit_graph_projection_request(
    runtime: &GraphProjectionRuntime,
    request: ResolveGraphProjectionRequest,
) -> Result<(), CommandError> {
    runtime
        .submit(request)
        .map_err(graph_projection_runtime_error)
}

fn graph_projection_runtime_error(error: GraphProjectionRuntimeError) -> CommandError {
    match error {
        GraphProjectionRuntimeError::InvalidRequest => {
            CommandError::expected("graph_projection_request_stale")
        }
        GraphProjectionRuntimeError::QueueFull => {
            CommandError::expected("graph_projection_queue_full")
        }
        GraphProjectionRuntimeError::SubscriptionNotFound => {
            CommandError::expected("graph_projection_subscription_not_found")
        }
        GraphProjectionRuntimeError::SubscriberLimit => {
            CommandError::expected("graph_projection_subscriber_limit")
        }
        error @ (GraphProjectionRuntimeError::Unavailable
        | GraphProjectionRuntimeError::WorkerSpawn(_)) => {
            CommandError::diagnosed("graph_projection_runtime_unavailable", error)
        }
    }
}
