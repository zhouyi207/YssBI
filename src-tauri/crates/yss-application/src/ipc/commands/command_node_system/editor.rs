use super::common::parse_graph_path;
use crate::graph::open::{OpenGraphApplicationError, OpenGraphRequest};
use crate::ipc::error::CommandError;
use crate::ipc::graph_editor_sync::{Binding, GraphEditorSyncState};
use crate::ipc::schema::graph_clipboard::ClipboardSubgraphDto;
use crate::ipc::schema::graph_editing::{encode_graph_edit, encode_graph_session};
use crate::ipc::schema::graph_mutation::EditorGraphMutationDto;
use crate::session::{ApplicationState, SessionCaptureError};
use tauri::State;
use yss_graph_document::NodeId;
use yss_graph_editor::EditorGraphMutation;
use yss_ipc_contract::graph_editing::GraphEditorSyncResponseDto;
use yss_project_identity::ProjectInstanceId;

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "Tauri binds transport context and explicit graph command fields"
)]
pub async fn hydrate_editor_graph(
    window: tauri::WebviewWindow,
    sync: State<'_, GraphEditorSyncState>,
    cursor: Option<String>,
    state: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
) -> Result<GraphEditorSyncResponseDto, CommandError> {
    let binding = Binding {
        window: window.label().into(),
        project: project_instance_id.to_string(),
        graph: graph_path.clone(),
        locale: locale.clone(),
    };
    let state = state.inner().clone();
    let sync = sync.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let graph_path = parse_graph_path(graph_path)?;
        let receipt = state
            .open_graph(OpenGraphRequest::new(
                project_instance_id,
                graph_path,
                0,
                locale,
            ))
            .map_err(open_graph_command_error)?;
        let mut response = encode_graph_session(
            &sync,
            binding,
            cursor.as_deref(),
            crate::ipc::schema::graph_editing::graph_editor_session_to_transport(
                receipt.document(),
                receipt.projection(),
                receipt.editing(),
            ),
        )?;
        response.function_editor_projection = receipt.function_editor_projection().cloned();
        Ok(response)
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "Tauri binds transport context and explicit graph command fields"
)]
pub async fn resolve_editor_graph(
    window: tauri::WebviewWindow,
    sync: State<'_, GraphEditorSyncState>,
    cursor: Option<String>,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    version: yss_ipc_contract::graph_editing::GraphEditVersionDto,
    locale: String,
) -> Result<GraphEditorSyncResponseDto, CommandError> {
    let binding = Binding {
        window: window.label().into(),
        project: project_instance_id.to_string(),
        graph: graph_path.clone(),
        locale: locale.clone(),
    };
    let application = application.inner().clone();
    let sync = sync.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        application
            .resolve_editor_graph(
                project_instance_id,
                parse_graph_path(graph_path)?,
                crate::ipc::schema::graph_editing::graph_edit_version_from_transport(version)?,
                locale,
            )
            .map_err(map_editor_resource_error)
            .and_then(|result| encode_graph_edit(&sync, binding, cursor.as_deref(), &result))
    })
    .await
    .map_err(CommandError::internal)?
}

fn open_graph_command_error(error: OpenGraphApplicationError) -> CommandError {
    match error {
        OpenGraphApplicationError::EditingBusy => CommandError::expected("graph_edit_busy"),
        OpenGraphApplicationError::SessionCapture(error) => session_capture_command_error(error),
        OpenGraphApplicationError::SessionChanged => {
            CommandError::expected("stale_project_lifecycle")
        }
        OpenGraphApplicationError::Project(error) => {
            CommandError::diagnosed("graph_open_failed", error)
        }
        OpenGraphApplicationError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        OpenGraphApplicationError::Contract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        OpenGraphApplicationError::Materialization(error) => {
            CommandError::diagnosed("graph_materialization_failed", error)
        }
        OpenGraphApplicationError::Projection(error) => {
            CommandError::diagnosed("editor_projection_failed", error)
        }
    }
}

fn session_capture_command_error(error: SessionCaptureError) -> CommandError {
    match error {
        SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
        SessionCaptureError::Replacing => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
            .with_details(super::common::RecoveryRequiredDetails {
                recovery_required: true,
            }),
    }
}

#[tauri::command]
pub fn export_graph_subgraph(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    version: yss_ipc_contract::graph_editing::GraphEditVersionDto,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, CommandError> {
    let graph_path = parse_graph_path(graph_path)?;
    let document = application
        .current_graph_document(
            &project_instance_id,
            &graph_path,
            crate::ipc::schema::graph_editing::graph_edit_version_from_transport(version)?,
        )
        .map_err(map_editor_resource_error)?;
    application
        .export_graph_subgraph(
            project_instance_id,
            graph_path,
            (*document).clone(),
            node_ids,
        )
        .map(ClipboardSubgraphDto::from)
        .map_err(map_editor_resource_error)
}

pub(super) fn parse_editor_mutation(
    mutation: serde_json::Value,
) -> Result<EditorGraphMutation, CommandError> {
    let mutation =
        serde_json::from_value::<EditorGraphMutationDto>(mutation.clone()).map_err(|_| {
            let code = if is_create_node_descriptor_shape_error(&mutation) {
                "catalog_descriptor_invalid"
            } else {
                "invalid_editor_mutation"
            };
            CommandError::expected(code)
        })?;
    mutation
        .try_into()
        .map_err(|_| CommandError::expected("invalid_editor_mutation"))
}

fn is_create_node_descriptor_shape_error(request: &serde_json::Value) -> bool {
    let Some(mutation) = request.as_object() else {
        return false;
    };
    if mutation.get("type").and_then(serde_json::Value::as_str) != Some("createNode") {
        return false;
    }
    let Some(create) = mutation
        .get("payload")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    if create.contains_key("parameters") {
        return true;
    }
    create.get("descriptor").is_none_or(|descriptor| {
        serde_json::from_value::<crate::ipc::schema::catalog::NodeCreationDescriptorDto>(
            descriptor.clone(),
        )
        .is_err()
    })
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "Tauri binds transport context and explicit graph command fields"
)]
pub async fn edit_graph(
    window: tauri::WebviewWindow,
    sync: State<'_, GraphEditorSyncState>,
    cursor: Option<String>,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    locale: String,
    version: yss_ipc_contract::graph_editing::GraphEditVersionDto,
    operation_id: yss_project_identity::OperationId,
    mutation: serde_json::Value,
) -> Result<GraphEditorSyncResponseDto, CommandError> {
    let binding = Binding {
        window: window.label().into(),
        project: project_instance_id.to_string(),
        graph: graph_path.clone(),
        locale: locale.clone(),
    };
    let application = application.inner().clone();
    let sync = sync.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mutation = parse_editor_mutation(mutation)?;
        let graph_path = parse_graph_path(graph_path)?;
        let result = application
            .edit_graph(
                crate::graph::editing::GraphEditRequest {
                    project_instance_id,
                    graph_path,
                    locale,
                    operation_id,
                    version: crate::ipc::schema::graph_editing::graph_edit_version_from_transport(
                        version,
                    )?,
                },
                mutation,
            )
            .map_err(map_editor_resource_error)?;
        encode_graph_edit(&sync, binding, cursor.as_deref(), &result)
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "Tauri binds transport context and explicit graph command fields"
)]
pub async fn change_graph_history(
    window: tauri::WebviewWindow,
    sync: State<'_, GraphEditorSyncState>,
    cursor: Option<String>,
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    version: yss_ipc_contract::graph_editing::GraphEditVersionDto,
    operation_id: yss_project_identity::OperationId,
    locale: String,
    redo: bool,
) -> Result<GraphEditorSyncResponseDto, CommandError> {
    let binding = Binding {
        window: window.label().into(),
        project: project_instance_id.to_string(),
        graph: graph_path.clone(),
        locale: locale.clone(),
    };
    let application = application.inner().clone();
    let sync = sync.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        application
            .change_graph_history(
                crate::graph::editing::GraphEditRequest {
                    project_instance_id,
                    graph_path: parse_graph_path(graph_path)?,
                    locale,
                    operation_id,
                    version: crate::ipc::schema::graph_editing::graph_edit_version_from_transport(
                        version,
                    )?,
                },
                redo,
            )
            .map_err(map_editor_resource_error)
            .and_then(|result| encode_graph_edit(&sync, binding, cursor.as_deref(), &result))
    })
    .await
    .map_err(CommandError::internal)?
}

fn map_editor_resource_error(
    error: crate::graph::resources::ResourceMutationApplicationError,
) -> CommandError {
    super::common::resource_mutation_to_command_error(error, "graph_edit_changed")
}

#[tauri::command]
pub async fn get_graph_edit_receipt(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    version: yss_ipc_contract::graph_editing::GraphEditVersionDto,
    operation_id: yss_project_identity::OperationId,
) -> Result<Option<yss_ipc_contract::graph_editing::GraphEditCommandReceiptDto>, CommandError> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = parse_graph_path(graph_path)?;
        let version =
            crate::ipc::schema::graph_editing::graph_edit_version_from_transport(version)?;
        application
            .graph_edit_receipt(&project_instance_id, &path, version, operation_id)
            .map_err(map_editor_resource_error)
            .map(|receipt| {
                receipt.map(|receipt| {
                    crate::ipc::schema::graph_editing::graph_edit_receipt_to_transport(
                        &path, &receipt,
                    )
                })
            })
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
pub fn subscribe_graph_activity(
    window: tauri::WebviewWindow,
    application: State<'_, ApplicationState>,
    channels: State<'_, crate::ipc::channel::graph_activity::GraphActivityChannels>,
    project_instance_id: ProjectInstanceId,
    on_event: tauri::ipc::Channel<yss_ipc_contract::graph_editing::GraphActivityDto>,
) -> Result<String, CommandError> {
    let (sender, receiver) = tokio::sync::broadcast::channel(256);
    let subscription = application
        .subscribe_graph_activity(
            &project_instance_id,
            std::sync::Arc::new(move |activity| {
                let _ = sender.send(activity);
            }),
        )
        .map_err(map_editor_resource_error)?;
    channels.bind_window(&window);
    channels.subscribe(
        window.label().into(),
        project_instance_id.to_string(),
        receiver,
        subscription,
        on_event,
    )
}

#[tauri::command]
pub fn unsubscribe_graph_activity(
    window: tauri::WebviewWindow,
    channels: State<'_, crate::ipc::channel::graph_activity::GraphActivityChannels>,
    subscription_id: String,
) {
    channels.unsubscribe(window.label(), &subscription_id);
}

#[tauri::command]
pub fn get_execution_run_state(
    application: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    execution_session_id: String,
    run_id: String,
) -> Result<Option<&'static str>, CommandError> {
    let id = run_id
        .parse::<u64>()
        .map_err(|_| CommandError::expected("invalid_run_id"))?;
    if id == 0 || id.to_string() != run_id {
        return Err(CommandError::expected("invalid_run_id"));
    }
    application
        .execution_run_state(&project_instance_id, &execution_session_id, id)
        .map(|state| {
            state.map(|state| {
                use yss_graph_execution::run_registry::RunState;
                match state {
                    RunState::Admitted => "admitted",
                    RunState::Running => "running",
                    RunState::Finalizing => "finalizing",
                    RunState::Succeeded => "succeeded",
                    RunState::Failed => "failed",
                    RunState::Cancelled => "cancelled",
                }
            })
        })
        .map_err(map_editor_resource_error)
}
