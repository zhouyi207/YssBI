use crate::application::database::name_from_path;
use crate::database::{DatabaseInstance, DatabaseState};
use crate::event::{emit_project_event, Event, EventProject};
use crate::execution::ExecutionEvent;
use crate::frontend::FrontendError;
use crate::graph::GraphId;
use crate::log::LogLevel;
use crate::log_app;
use crate::project::{
    default_project_parent_directory as default_project_parent_directory_impl,
    execute_project_data, load_project_from_file, normalize_project_name, save_project_as_to_directory,
    save_project_to_file,
    validate_new_project_path as validate_new_project_path_impl, LegacyProjectRecord, ProjectData,
    ProjectIndex, ProjectPathValidation, ProjectRecord, ProjectRegistry, ProjectState,
    PROJECT_METADATA_FILE,
};
use crate::schema::{
    ColumnInfoDTO, DatabaseDeclDTO, DatabasesVariablesDTO, GraphInstanceDTO,
    GraphsWithValidationDTO, InvalidReferenceDTO, ProjectDataDTO, VariableInstanceDTO,
};
use polars::prelude::DataType;
use tauri::{ipc::Channel, AppHandle, State};

use serde_json::Value;
use std::collections::HashMap;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedProjectGraphDTO {
    pub graph: GraphInstanceDTO,
    pub variables: HashMap<String, VariableInstanceDTO>,
}

fn dtype_to_string(dt: &DataType) -> String {
    format!("{:?}", dt)
}

/// 从 `DatabaseInstance` 抽取出来给前端的 schema 视图。
enum SchemaInfo {
    Ready {
        name: String,
        columns: Vec<ColumnInfoDTO>,
        row_count: usize,
        column_count: usize,
    },
    /// 上一次 IO 失败。
    Failed {
        name: String,
        error: String,
    },
}

fn database_display_name(instance: &DatabaseInstance) -> String {
    instance
        .decl
        .name
        .clone()
        .unwrap_or_else(|| match &instance.decl.engine {
            crate::database::DatabaseEngine::Csv { path, .. } => name_from_path(path),
            crate::database::DatabaseEngine::Parquet { path, .. } => name_from_path(path),
            crate::database::DatabaseEngine::Sql { table, .. } => table.clone(),
            crate::database::DatabaseEngine::Excel { sheet, .. } => sheet.clone(),
            crate::database::DatabaseEngine::DuckDb { .. } => instance.decl.id.clone(),
            crate::database::DatabaseEngine::InMemory { name } => name.clone(),
        })
}

/// 从 DatabaseInstance 提取 schema 信息（不 mutate，适用于 project_store 读锁下）
/// 优先使用 decl.name（后端 unique name），否则从 path 推导
fn extract_database_schema(instance: &DatabaseInstance) -> SchemaInfo {
    let name = database_display_name(instance);

    match &instance.state {
        DatabaseState::Loaded { dataframe, .. } => {
            let schema = dataframe.schema();
            let columns: Vec<ColumnInfoDTO> = schema
                .iter_names()
                .filter_map(|n| {
                    schema.get(n).map(|dt| ColumnInfoDTO {
                        name: n.to_string(),
                        dtype: dtype_to_string(dt),
                    })
                })
                .collect();
            let row_count = dataframe.height();
            let column_count = columns.len();
            SchemaInfo::Ready {
                name,
                columns,
                row_count,
                column_count,
            }
        }
        DatabaseState::DuckDb {
            row_count,
            columns,
            ..
        } => {
            let columns: Vec<ColumnInfoDTO> = columns
                .iter()
                .map(|col| ColumnInfoDTO {
                    name: col.name.clone(),
                    dtype: col.dtype.clone(),
                })
                .collect();
            let column_count = columns.len();
            SchemaInfo::Ready {
                name,
                columns,
                row_count: *row_count,
                column_count,
            }
        }
        DatabaseState::Failed { error } => SchemaInfo::Failed {
            name,
            error: error.clone(),
        },
    }
}

/// 把 `SchemaInfo` 写入 `DatabaseDeclDTO`，以便 `get_project_*` 命令复用。
fn apply_schema_info(dto: &mut DatabaseDeclDTO, info: SchemaInfo) {
    match info {
        SchemaInfo::Ready {
            name,
            columns,
            row_count,
            column_count,
        } => {
            dto.name = Some(name);
            dto.columns = Some(columns);
            dto.row_count = Some(row_count);
            dto.column_count = Some(column_count);
        }
        SchemaInfo::Failed { name, error } => {
            dto.name = Some(name);
            dto.load_error = Some(error);
        }
    }
}

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

    // 从 project_store 补充 database schema 信息
    let store = state.project_store.read().unwrap();
    let mut enriched = std::collections::HashMap::new();
    for (id, decl) in data.databases.iter() {
        let mut db_dto = DatabaseDeclDTO::from(decl);
        if let Some(instance) = store.databases.get(id) {
            apply_schema_info(&mut db_dto, extract_database_schema(instance));
        }
        enriched.insert(id.clone(), db_dto);
    }
    dto.databases = enriched;

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

    let mut databases = std::collections::HashMap::new();
    let store = state.project_store.read().unwrap();
    for (id, decl) in data.databases.iter() {
        let mut db_dto = DatabaseDeclDTO::from(decl);
        if let Some(instance) = store.databases.get(id) {
            apply_schema_info(&mut db_dto, extract_database_schema(instance));
        }
        databases.insert(id.clone(), db_dto);
    }

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

    let valid_variable_ids: std::collections::HashSet<String> =
        data.variables.keys().map(|k| k.to_string()).collect();
    let valid_dataframe_ids: std::collections::HashSet<String> =
        data.databases.keys().cloned().collect();
    let valid_graph_ids: std::collections::HashSet<GraphId> = data.graphs.keys().copied().collect();

    let mut graphs = std::collections::HashMap::new();
    let mut invalid_references = std::collections::HashMap::new();

    for (graph_id, graph) in data.graphs.iter() {
        let dto = GraphInstanceDTO::from(graph);
        graphs.insert(*graph_id, dto);

        let data_state = graph.data_state.read().unwrap();
        let mut refs = Vec::new();
        for node in data_state.nodes.values() {
            let mut inv = InvalidReferenceDTO {
                node_id: node.id.to_string(),
                variable_id: None,
                dataframe_id: None,
                sub_graph_id: None,
            };
            let mut has_invalid = false;

            if let Some(vid) = node.instance_params.variable_id() {
                if !valid_variable_ids.contains(vid) {
                    inv.variable_id = Some(vid.to_string());
                    has_invalid = true;
                }
            }
            if let Some(dfid) = node.instance_params.dataframe_id() {
                if !valid_dataframe_ids.contains(dfid) {
                    inv.dataframe_id = Some(dfid.to_string());
                    has_invalid = true;
                }
            }
            if let Some(sgid) = node.instance_params.sub_graph_id() {
                let parsed = uuid::Uuid::parse_str(sgid).ok().map(GraphId::from);
                if let Some(gid) = parsed {
                    if !valid_graph_ids.contains(&gid) {
                        inv.sub_graph_id = Some(sgid.to_string());
                        has_invalid = true;
                    }
                } else {
                    inv.sub_graph_id = Some(sgid.to_string());
                    has_invalid = true;
                }
            }

            if has_invalid {
                refs.push(inv);
            }
        }
        if !refs.is_empty() {
            invalid_references.insert(*graph_id, refs);
        }
    }

    GraphsWithValidationDTO {
        graphs,
        invalid_references,
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

    path
}

#[tauri::command]
pub fn get_project_index(state: State<ProjectState>) -> Result<ProjectIndex, String> {
    let path = state.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
    crate::project::read_project_index(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn default_project_parent_directory() -> Result<String, String> {
    default_project_parent_directory_impl()
}

#[tauri::command]
pub fn validate_new_project_path(path: String) -> ProjectPathValidation {
    validate_new_project_path_impl(&path)
}

#[tauri::command]
pub async fn list_registered_projects(
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<ProjectRecord>, String> {
    registry.list_projects().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn register_project(
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
) -> Result<ProjectRecord, String> {
    registry.register_project(&name, &path).await
}

#[tauri::command]
pub async fn remove_registered_project(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<(), String> {
    registry.remove_project(&id).await
}

#[tauri::command]
pub async fn toggle_registered_project_favorite(
    registry: State<'_, ProjectRegistry>,
    id: String,
) -> Result<bool, String> {
    registry.toggle_favorite(&id).await
}

#[tauri::command]
pub async fn migrate_legacy_registered_projects(
    registry: State<'_, ProjectRegistry>,
    projects: Vec<LegacyProjectRecord>,
) -> Result<(), String> {
    registry.migrate_legacy_projects(projects).await
}

#[tauri::command]
pub fn get_project_registry_path(registry: State<ProjectRegistry>) -> String {
    registry.path().to_string_lossy().into_owned()
}

/// 加载项目（从状态管理层）
#[tauri::command]
pub fn load_project(
    app: AppHandle,
    state: State<ProjectState>,
    path: String,
) -> Result<(), FrontendError> {
    log_app!(
        LogLevel::Info,
        "[command.load_project] Loading project from: {}",
        path
    );

    let project_data = load_project_from_file(&path)?;

    log_app!(
        LogLevel::Info,
        "[command.load_project] Project loaded: {}",
        project_data.info()
    );

    // Defensively reset the previous project's in-memory runtime (loaded
    // graphs, schema providers, project_store) before applying the new
    // manifest. `set_data` would overwrite the maps anyway, but doing an
    // explicit `clear()` keeps the contract simple: every project switch
    // starts from a clean state.
    state.clear();
    state.set_path(Some(path.clone()));
    state.set_data(project_data.clone());
    emit_project_event(
        &app,
        Event::Project(EventProject::ProjectLoaded {
            data: project_data.clone(),
            path: Some(path),
        }),
    );
    Ok(())
}

/// 将当前项目另存为新目录（完整复制 events/functions/database 等）。
#[tauri::command]
pub async fn save_project_as(
    app: AppHandle,
    state: State<'_, ProjectState>,
    registry: State<'_, ProjectRegistry>,
    path: String,
) -> Result<ProjectRecord, String> {
    log_app!(
        LogLevel::Info,
        "[command.save_project_as] Saving project copy to: {}",
        path
    );

    let new_metadata_path =
        save_project_as_to_directory(&state, &path).map_err(|e| e.to_string())?;

    let project_data = load_project_from_file(&new_metadata_path).map_err(|e| e.to_string())?;

    state.clear();
    state.set_path(Some(new_metadata_path.clone()));
    state.set_data(project_data.clone());

    let record = registry
        .register_project(
            &project_data.metadata.project_name,
            &new_metadata_path,
        )
        .await?;

    emit_project_event(
        &app,
        Event::Project(EventProject::ProjectLoaded {
            data: project_data,
            path: Some(new_metadata_path),
        }),
    );

    Ok(record)
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    state: State<'_, ProjectState>,
    registry: State<'_, ProjectRegistry>,
    name: String,
    path: String,
) -> Result<ProjectRecord, String> {
    let validation = validate_new_project_path_impl(&path);
    if !validation.ok {
        return Err(validation.message.unwrap_or_else(|| "项目路径无效".into()));
    }

    let project_root = std::path::PathBuf::from(path.trim());
    std::fs::create_dir_all(&project_root).map_err(|e| format!("无法创建项目文件夹: {e}"))?;
    crate::project::ensure_project_database_dir(&project_root)
        .map_err(|e| format!("无法创建 database 目录: {e}"))?;
    let project_file_path = project_root.join(PROJECT_METADATA_FILE);

    let mut project_data = ProjectData::new();
    let normalized_name = normalize_project_name(&name);
    project_data.metadata.project_name = normalized_name.clone();
    project_data.update_metadata();
    save_project_to_file(&project_data, project_root.to_string_lossy().as_ref())
        .map_err(|e| e.to_string())?;

    let record = registry
        .register_project(
            &normalized_name,
            project_file_path.to_string_lossy().as_ref(),
        )
        .await?;

    state.set_path(Some(record.path.clone()));
    state.set_data(project_data.clone());
    emit_project_event(
        &app,
        Event::Project(EventProject::ProjectLoaded {
            data: project_data,
            path: Some(record.path.clone()),
        }),
    );

    Ok(record)
}

#[tauri::command]
pub fn flush_project(
    app: AppHandle,
    state: State<ProjectState>,
) -> Result<(), String> {
    let path = state.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
    log_app!(LogLevel::Info, "[command.flush_project] Flushing project");

    state.persist_current_project()?;

    emit_project_event(&app, Event::Project(EventProject::ProjectSaved { path }));
    Ok(())
}

/// 新建项目（清空当前状态）
#[tauri::command]
pub fn new_project(app: AppHandle, state: State<ProjectState>) -> Result<(), String> {
    log_app!(LogLevel::Info, "[command.new_project] Creating new project");

    state.clear();
    emit_project_event(&app, Event::Project(EventProject::ProjectCleared));
    Ok(())
}

/// 执行指定的 Event 图。
/// 若传入 graph_id 则只执行该图，否则执行所有 Event 图。
#[tauri::command]
pub async fn execute_project(
    state: State<'_, ProjectState>,
    window_store: State<'_, crate::execution::WindowDataStore>,
    on_event: Channel<ExecutionEvent>,
    graph_id: Option<String>,
) -> Result<Value, String> {
    let project_data = state.get_data();
    let window_store = window_store.inner().clone();
    let project_data_state = state.project_data.clone();
    let project_store = state.project_store.clone();

    let target_graph_id: Option<GraphId> = graph_id
        .as_deref()
        .map(|s| {
            uuid::Uuid::parse_str(s)
                .map(GraphId::from)
                .map_err(|e| format!("Invalid graph_id '{}': {}", s, e))
        })
        .transpose()?;

    tauri::async_runtime::spawn_blocking(move || {
        execute_project_data(
            project_data,
            project_data_state,
            project_store,
            window_store,
            on_event,
            target_graph_id,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 新窗口通过 key 拉取数据（非破坏性读取，兼容 React Strict Mode）
#[tauri::command]
pub fn get_window_data(
    state: State<crate::execution::WindowDataStore>,
    key: String,
) -> Result<Option<String>, String> {
    Ok(state.get(&key))
}

#[tauri::command]
pub fn load_project_graph(
    state: State<ProjectState>,
    graph_id: GraphId,
) -> Result<LoadedProjectGraphDTO, String> {
    let document = state.load_graph_from_current_project(&graph_id)?;
    Ok(LoadedProjectGraphDTO {
        graph: GraphInstanceDTO::from(&document.graph),
        variables: document
            .local_variables
            .iter()
            .map(|(id, variable)| (id.to_string(), VariableInstanceDTO::from(variable)))
            .collect(),
    })
}
