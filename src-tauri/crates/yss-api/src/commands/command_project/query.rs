use crate::error::CommandError;
use crate::schema::application_event::ProjectActivationResultDto;
use crate::schema::graph_draft::GraphEditorSessionDto;
use crate::schema::{DatabaseDeclDTO, DatabasesVariablesDTO, VariableInstanceDTO};
use serde::Serialize;
use tauri::State;
use yss_application::execution::{ApplicationState, SessionCaptureError};
use yss_application::graph_open::{OpenGraphApplicationError, OpenGraphRequest};
use yss_project::ProjectIndex;
use yss_project_registry::normalize_existing_path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRequiredDetails {
    recovery_required: bool,
}

/// 分阶段加载第一步：获取 databases + variables（含 schema）
#[tauri::command]
pub fn get_project_databases_variables(
    application: State<ApplicationState>,
) -> Result<DatabasesVariablesDTO, CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "data",
        diagnostic_event = "getProjectDataResources",
        "Loading project databases and variables"
    );

    application
        .query_project_databases_variables()
        .map_err(map_project_query_error)
        .and_then(project_databases_variables_to_transport)
}

fn project_databases_variables_to_transport(
    snapshot: yss_application::project_query::ProjectDatabasesVariablesSnapshot,
) -> Result<DatabasesVariablesDTO, CommandError> {
    let databases = snapshot
        .databases()
        .iter()
        .map(|database| {
            let mut dto = DatabaseDeclDTO::from(&database.declaration);
            let columns = crate::schema::column_info_from_schema(database.schema.columns());
            dto.column_count = Some(columns.len());
            dto.columns = Some(columns);
            (database.declaration.id.as_str().to_owned(), dto)
        })
        .collect();
    let variables = snapshot
        .variables()
        .iter()
        .map(|variable| {
            VariableInstanceDTO::try_from(variable)
                .map(|dto| (variable.id.to_string(), dto))
                .map_err(|error| CommandError::diagnosed("project_variable_mapping_failed", error))
        })
        .collect::<Result<_, _>>()?;
    Ok(DatabasesVariablesDTO {
        databases,
        variables,
    })
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
    activation: yss_application::project_query::ProjectActivation,
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
        diagnostic_domain = "application",
        diagnostic_event = "getProjectPath",
        path = ?path,
        "Read project path"
    );

    Ok(path.map(|path| normalize_existing_path(&path).unwrap_or(path)))
}

#[tauri::command]
pub fn get_project_index(
    application: State<ApplicationState>,
    project_instance_id: String,
) -> Result<ProjectIndex, CommandError> {
    let project_instance_id =
        yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
    application
        .query_project_index(project_instance_id)
        .map_err(map_project_query_error)
}

#[tauri::command]
pub fn load_project_graph(
    state: State<'_, ApplicationState>,
    project_instance_id: String,
    graph_path: String,
    locale: Option<String>,
    lifecycle_token: u64,
) -> Result<GraphEditorSessionDto, CommandError> {
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
    Ok(
        crate::schema::graph_draft::graph_editor_session_to_transport(
            receipt.document(),
            receipt.projection(),
        ),
    )
}

fn open_graph_command_error(error: OpenGraphApplicationError) -> CommandError {
    match error {
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

fn map_project_query_error(
    error: yss_application::project_query::ProjectQueryApplicationError,
) -> CommandError {
    use yss_application::project_query::ProjectQueryApplicationError;
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
            crate::commands::project_failure::application_project_command_error(error)
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
