use crate::graph::open::{OpenGraphApplicationError, OpenGraphRequest};
use crate::ipc::activity_panel_sync::{ActivityPanelSyncState, ActivityPanelUpdateDto};
use crate::ipc::error::CommandError;
use crate::ipc::graph_editor_sync::{Binding, GraphEditorSyncState};
use crate::ipc::schema::activity_panel::{
    ActivityPanelDocumentDto, ActivityPanelId, ActivityPanelRequest,
};
use crate::ipc::schema::{DatabaseDeclDTO, ProjectDatabasesDTO};
use crate::session::{ApplicationState, SessionCaptureError};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use tauri::{Manager, State, WebviewWindow};
use yss_ipc_contract::graph_editing::GraphEditorSyncResponseDto;
use yss_ipc_contract::project::ProjectActivationResultDto;
use yss_project::ProjectIndex;
use yss_project_registry::normalize_existing_path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRequiredDetails {
    recovery_required: bool,
}

/// 分阶段加载第一步：获取 databases（含 schema）
#[tauri::command]
pub fn get_project_databases(
    application: State<ApplicationState>,
) -> Result<ProjectDatabasesDTO, CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        log_domain = "data",
        log_event = "getProjectDataResources",
        "Loading project databases"
    );

    application
        .query_project_databases()
        .map_err(map_project_query_error)
        .and_then(project_databases_to_transport)
}

fn project_databases_to_transport(
    snapshot: crate::project::query::ProjectDatabasesSnapshot,
) -> Result<ProjectDatabasesDTO, CommandError> {
    let databases = snapshot
        .databases()
        .iter()
        .map(|database| {
            let mut dto = DatabaseDeclDTO::from(&database.declaration);
            let columns = crate::ipc::schema::column_info_from_schema(database.schema.columns());
            dto.column_count = Some(columns.len());
            dto.columns = Some(columns);
            (database.declaration.id.as_str().to_owned(), dto)
        })
        .collect();
    Ok(ProjectDatabasesDTO { databases })
}

/// 获取当前项目 activation，供项目加载后创建的独立 WebView 建立 lifecycle identity。
#[tauri::command]
pub fn get_current_project_activation(
    application: State<ApplicationState>,
) -> Result<ProjectActivationResultDto, CommandError> {
    application
        .query_current_project_activation()
        .map_err(map_project_query_error)
        .map(project_activation_to_transport)
}

fn project_activation_to_transport(
    activation: crate::project::query::ProjectActivation,
) -> ProjectActivationResultDto {
    ProjectActivationResultDto {
        path: activation.path,
        project_instance_id: activation.project_instance_id.to_string(),
        activation_revision: activation.activation_revision,
    }
}

/// 获取当前项目路径
#[tauri::command]
pub fn get_project_path(
    application: State<ApplicationState>,
) -> Result<Option<String>, CommandError> {
    let path = application
        .query_project_path()
        .map_err(map_project_query_error)?;

    tracing::info!(
        target: "yssbi::commands::project",
        log_domain = "application",
        log_event = "getProjectPath",
        path = ?path,
        "Read project path"
    );

    Ok(path.map(|path| normalize_existing_path(&path).unwrap_or(path)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndexSnapshotDto {
    index: ProjectIndex,
    activity_panels: BTreeMap<&'static str, ActivityPanelUpdateDto>,
}

#[tauri::command]
pub async fn get_project_index(
    window: WebviewWindow,
    application: State<'_, ApplicationState>,
    project_instance_id: String,
    locale: String,
    activity_panels: Vec<ActivityPanelRequest>,
) -> Result<ProjectIndexSnapshotDto, CommandError> {
    if activity_panels.len() > 2 {
        return Err(CommandError::expected("activity_panel_scope_invalid"));
    }
    let mut ids = BTreeSet::new();
    for request in &activity_panels {
        super::super::command_activity_panel::validate_activity_query(
            &locale,
            request.cursor.as_deref(),
        )?;
        if !request.panel_id.is_project_scoped() || !ids.insert(request.panel_id) {
            return Err(CommandError::expected("activity_panel_scope_invalid"));
        }
    }
    super::super::command_activity_panel::validate_activity_query(&locale, None)?;
    let application = application.inner().clone();
    let sync = window.state::<ActivityPanelSyncState>().inner().clone();
    let window_label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = application
            .query_project_index(
                yss_project_identity::ProjectInstanceId::from_existing(project_instance_id),
                &locale,
                ids.contains(&ActivityPanelId::Nodes),
            )
            .map_err(map_project_query_error)?;
        let mut updates = BTreeMap::new();
        for document in snapshot.activity_panels {
            let Some(request) = activity_panels
                .iter()
                .find(|request| request.panel_id.as_str() == document.panel_id)
            else {
                continue;
            };
            updates.insert(
                document.panel_id,
                sync.publish(
                    &window_label,
                    &locale,
                    request.cursor.as_deref(),
                    ActivityPanelDocumentDto::try_from(document)?,
                )?,
            );
        }
        Ok(ProjectIndexSnapshotDto {
            index: snapshot.index,
            activity_panels: updates,
        })
    })
    .await
    .map_err(CommandError::internal)?
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "Tauri binds graph load identity and transport context"
)]
pub async fn load_project_graph(
    window: tauri::WebviewWindow,
    sync: State<'_, GraphEditorSyncState>,
    cursor: Option<String>,
    state: State<'_, ApplicationState>,
    project_instance_id: String,
    graph_path: String,
    locale: Option<String>,
    lifecycle_token: u64,
) -> Result<GraphEditorSyncResponseDto, CommandError> {
    let binding = Binding {
        window: window.label().into(),
        project: project_instance_id.clone(),
        graph: graph_path.clone(),
        locale: locale.clone().unwrap_or_else(|| "en-US".into()),
    };
    let state = state.inner().clone();
    let sync = sync.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project_instance_id =
            yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
        let graph_path = yss_graph_document::GraphResourcePath::new(graph_path)
            .map_err(|_| CommandError::expected("invalid_project_format"))?;
        let receipt = state
            .open_graph(OpenGraphRequest::new(
                project_instance_id,
                graph_path,
                lifecycle_token,
                locale.as_deref().unwrap_or("en-US"),
            ))
            .map_err(open_graph_command_error)?;
        let mut response = crate::ipc::schema::graph_editing::encode_graph_session(
            &sync,
            binding,
            cursor.as_deref(),
            crate::ipc::schema::graph_editing::graph_editor_session_to_transport(
                receipt.document(),
                receipt.projection(),
                receipt.editing(),
                receipt.result_state(),
            )?,
        )?;
        response.function_editor_projection = receipt.function_editor_projection().cloned();
        Ok(response)
    })
    .await
    .map_err(CommandError::internal)?
}

fn open_graph_command_error(error: OpenGraphApplicationError) -> CommandError {
    match error {
        OpenGraphApplicationError::EditingBusy => CommandError::expected("graph_edit_busy"),
        OpenGraphApplicationError::SessionCapture(error) => match error {
            SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
            SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
                .with_details(RecoveryRequiredDetails {
                    recovery_required: true,
                }),
        },
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

/// Resolve the on-disk path for a project resource (graph / database / chart).
#[tauri::command]
pub fn get_project_resource_path(
    application: State<ApplicationState>,
    kind: String,
    resource_id: String,
) -> Result<String, CommandError> {
    application
        .reveal_project_resource(kind, resource_id)
        .map_err(map_project_query_error)
}

pub(crate) fn map_project_query_error(
    error: crate::project::query::ProjectQueryApplicationError,
) -> CommandError {
    use crate::project::query::ProjectQueryApplicationError;
    match error {
        ProjectQueryApplicationError::SessionCapture(error) => match error {
            SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
            SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
                .with_details(RecoveryRequiredDetails {
                    recovery_required: true,
                }),
        },
        ProjectQueryApplicationError::ProjectIdentityMismatch { .. } => {
            CommandError::expected("stale_project_lifecycle")
        }
        ProjectQueryApplicationError::Project(error) => {
            crate::ipc::commands::project_failure::application_project_command_error(error)
        }
        ProjectQueryApplicationError::Catalog(error) => {
            super::super::command_node_system::catalog_query_command_error(error)
        }
        ProjectQueryApplicationError::ProjectRead(error) => {
            CommandError::diagnosed("project_query_failed", error)
        }
        ProjectQueryApplicationError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        ProjectQueryApplicationError::InvalidResourceReference => {
            CommandError::expected("invalid_resource_reference")
        }
        ProjectQueryApplicationError::ResourceNotFound => {
            CommandError::expected("resource_not_found")
        }
        ProjectQueryApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("project_query_session_changed", error)
        }
    }
}
