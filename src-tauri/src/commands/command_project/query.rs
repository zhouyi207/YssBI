use crate::application::database_schema::enriched_database_dtos;
use crate::log::LogLevel;
use crate::log_app;
use crate::project::{
    ProjectIndex, ProjectState, RevealProjectResourceRequest, collect_invalid_graph_references,
    format_path_for_user_path, normalize_existing_path, resolve_reveal_path,
};
use crate::schema::{
    DatabasesVariablesDTO, GraphInstanceDTO, GraphsWithValidationDTO, ProjectDataDTO,
    VariableInstanceDTO,
};
use tauri::State;

use super::types::LoadedProjectGraphDTO;

/// 获取当前项目数据（含 database schema，从 project_store 补充）
#[tauri::command]
pub fn get_project_data(state: State<ProjectState>) -> ProjectDataDTO {
    let data = state.get_data();

    log_app!(
        LogLevel::Info,
        "[command.get_project_data] ProjectData: {}",
        data.info()
    );

    let mut dto = ProjectDataDTO::from(&data);
    let store = state.project_store.read().unwrap();
    dto.databases = enriched_database_dtos(&data.databases, &store);
    dto
}

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

/// 分阶段加载第二步：获取 graphs，并根据已加载的 databases/variables 校验引用
#[tauri::command]
pub fn get_project_graphs(state: State<ProjectState>) -> GraphsWithValidationDTO {
    let data = state.get_data();

    log_app!(
        LogLevel::Info,
        "[command.get_project_graphs] Loading graphs with reference validation"
    );

    let graphs = data
        .graphs
        .iter()
        .map(|(graph_path, graph)| (graph_path.as_str().to_string(), GraphInstanceDTO::from(graph)))
        .collect();

    GraphsWithValidationDTO {
        graphs,
        invalid_references: collect_invalid_graph_references(&data),
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
pub fn get_project_index(state: State<ProjectState>) -> Result<ProjectIndex, String> {
    let path = state.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
    state.apply_global_variables_from_disk(&path)?;
    crate::project::read_project_index(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_project_graph(
    state: State<ProjectState>,
    graph_path: String,
) -> Result<LoadedProjectGraphDTO, String> {
    let graph_path = crate::project::GraphResourcePath::new(graph_path).map_err(|e| e.to_string())?;
    let document = state.load_graph_from_current_project(&graph_path)?;
    Ok(LoadedProjectGraphDTO {
        graph: GraphInstanceDTO::from(&document.graph),
        variables: document
            .local_variables
            .iter()
            .map(|(id, variable)| (id.to_string(), VariableInstanceDTO::from(variable)))
            .collect(),
    })
}

/// Resolve the on-disk path for a project resource (graph / database / worksheet).
#[tauri::command]
pub fn get_project_resource_path(
    state: State<ProjectState>,
    kind: String,
    resource_id: String,
) -> Result<String, String> {
    let request = RevealProjectResourceRequest::from_parts(&kind, resource_id)?;
    let path = resolve_reveal_path(&state, request).map_err(|e| e.to_string())?;
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }
    Ok(format_path_for_user_path(&path))
}
