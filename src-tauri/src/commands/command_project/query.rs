use crate::application::database_schema::enriched_database_dtos;
use crate::error::AppError;
use crate::log::LogLevel;
use crate::log_app;
use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::project::{
    ProjectIndex, ProjectState, RevealProjectResourceRequest, format_path_for_user_path,
    normalize_existing_path, resolve_reveal_path,
};
use crate::schema::{DatabasesVariablesDTO, VariableInstanceDTO};
use tauri::State;

/// 分阶段加载第一步：获取 databases + variables（含 schema）
#[tauri::command]
pub fn get_project_databases_variables(state: State<ProjectState>) -> DatabasesVariablesDTO {
    let data = state.get_data();

    log_app!(
        LogLevel::Info,
        "[command.get_project_databases_variables] Loading databases + variables"
    );

    let store = state.project_store.read().unwrap();
    let databases = enriched_database_dtos(&data.databases, &store);
    let variables = data
        .variables
        .iter()
        .map(|(k, v)| (k.to_string(), VariableInstanceDTO::from(v)))
        .collect();

    DatabasesVariablesDTO {
        databases,
        variables,
    }
}

/// 获取当前项目路径
#[tauri::command]
pub fn get_project_path(state: State<ProjectState>) -> Option<String> {
    let path = state.get_path();

    log_app!(
        LogLevel::Info,
        "[command.get_project_path] Path: {:?}",
        path
    );

    path.map(|path| normalize_existing_path(&path).unwrap_or(path))
}

#[tauri::command]
pub fn get_project_index(state: State<ProjectState>) -> Result<ProjectIndex, AppError> {
    let path = state.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
    state.apply_global_variables_from_disk(&path)?;
    Ok(crate::project::read_project_index(&path).map_err(AppError::from)?)
}

#[tauri::command]
pub fn load_project_graph(
    state: State<ProjectState>,
    graph_path: String,
    locale: Option<String>,
) -> Result<EditorGraphProjectionDto, AppError> {
    let graph_path =
        crate::project::GraphResourcePath::new(graph_path).map_err(|e| e.to_string())?;
    state.load_graph_from_current_project(&graph_path)?;
    state
        .graph_projection(&graph_path, locale.as_deref().unwrap_or("en-US"))
        .map_err(AppError::internal)
}

/// Resolve the on-disk path for a project resource (graph / database / worksheet).
#[tauri::command]
pub fn get_project_resource_path(
    state: State<ProjectState>,
    kind: String,
    resource_id: String,
) -> Result<String, AppError> {
    let request = RevealProjectResourceRequest::from_parts(&kind, resource_id)?;
    let path = resolve_reveal_path(&state, request).map_err(|e| e.to_string())?;
    if !path.exists() {
        return Err(AppError::new(
            "resource_not_found",
            format!("File not found: {}", path.display()),
        ));
    }
    Ok(format_path_for_user_path(&path))
}
