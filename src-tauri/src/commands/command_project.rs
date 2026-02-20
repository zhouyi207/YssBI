use crate::database::{DatabaseInstance, DatabaseState};
use crate::event::EventProject;
use crate::graph::GraphId;
use crate::event::{emit_project_event, Event};
use crate::execution::Executor;
use crate::frontend::FrontendError;
use crate::graph::GraphKind;
use crate::log::LogLevel;
use crate::log_app;
use crate::project::{load_project_from_file, save_project_to_file, ProjectState};
use crate::schema::{
    ColumnInfoDTO, DatabaseDeclDTO, DatabasesVariablesDTO, GraphInstanceDTO, GraphsWithValidationDTO,
    InvalidReferenceDTO, ProjectDataDTO, VariableInstanceDTO,
};
use polars::prelude::*;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

use serde_json::{json, Value};

fn dtype_to_string(dt: &DataType) -> String {
    format!("{:?}", dt)
}

fn name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

/// 从 DatabaseInstance 提取 schema 信息（不 mutate，适用于 project_store 读锁下）
fn extract_database_schema(instance: &DatabaseInstance) -> Option<(String, Vec<ColumnInfoDTO>, usize, usize)> {
    let name = match &instance.decl.engine {
        crate::database::DatabaseEngine::Csv { path, .. } => name_from_path(path),
        crate::database::DatabaseEngine::Parquet { path, .. } => name_from_path(path),
        crate::database::DatabaseEngine::InMemory { name } => name.clone(),
        _ => instance.decl.id.clone(),
    };

    match &instance.state {
        DatabaseState::Loaded { dataframe } => {
            let schema = dataframe.schema();
            let columns: Vec<ColumnInfoDTO> = schema
                .iter_names()
                .filter_map(|n| schema.get(n).map(|dt| ColumnInfoDTO {
                    name: n.to_string(),
                    dtype: dtype_to_string(dt),
                }))
                .collect();
            let row_count = dataframe.height();
            let column_count = columns.len();
            Some((name, columns, row_count, column_count))
        }
        DatabaseState::Lazy { lazy_frame } => {
            let schema = lazy_frame.clone().collect_schema().ok()?;
            let columns: Vec<ColumnInfoDTO> = schema
                .iter_names()
                .filter_map(|n| schema.get(n).map(|dt| ColumnInfoDTO {
                    name: n.to_string(),
                    dtype: dtype_to_string(dt),
                }))
                .collect();
            let column_count = columns.len();
            let row_count = lazy_frame
                .clone()
                .select([len()])
                .collect()
                .ok()
                .and_then(|df| {
                    df.get_columns()
                        .first()
                        .and_then(|s| s.u32().ok())
                        .and_then(|ca| ca.get(0))
                        .map(|v| v as usize)
                })
                .unwrap_or(0);
            Some((name, columns, row_count, column_count))
        }
        DatabaseState::Failed { .. } => None,
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
            if let Some((name, columns, row_count, column_count)) = extract_database_schema(instance) {
                db_dto.name = Some(name);
                db_dto.columns = Some(columns);
                db_dto.row_count = Some(row_count);
                db_dto.column_count = Some(column_count);
            }
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
            if let Some((name, columns, row_count, column_count)) = extract_database_schema(instance) {
                db_dto.name = Some(name);
                db_dto.columns = Some(columns);
                db_dto.row_count = Some(row_count);
                db_dto.column_count = Some(column_count);
            }
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

    let valid_variable_ids: std::collections::HashSet<String> = data
        .variables
        .keys()
        .map(|k| k.to_string())
        .collect();
    let valid_dataframe_ids: std::collections::HashSet<String> =
        data.databases.keys().cloned().collect();
    let valid_graph_ids: std::collections::HashSet<GraphId> =
        data.graphs.keys().copied().collect();

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
                let parsed = uuid::Uuid::parse_str(sgid)
                    .ok()
                    .map(GraphId::from);
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

    state.set_data(project_data.clone());
    state.set_path(Some(path.clone()));
    emit_project_event(
        &app,
        Event::Project(EventProject::ProjectLoaded {
            data: project_data.clone(),
            path: Some(path),
        }),
    );
    Ok(())
}

#[tauri::command]
pub fn save_project(
    app: AppHandle,
    state: State<ProjectState>,
    path: String,
) -> Result<(), FrontendError> {
    log_app!(
        LogLevel::Info,
        "[command.save_project] Saving project to: {}",
        path
    );

    let mut project_data = state.get_data();
    project_data.update_metadata();

    save_project_to_file(&project_data, &path)?;
    state.set_path(Some(path.clone()));

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


/// 执行项目
///
/// 遍历所有 Event 图，从 event_begin 节点开始执行。
/// 若图中无 event_begin 节点则跳过该图。
#[tauri::command]
pub fn execute_project(state: State<ProjectState>) -> Result<Value, String> {
    let project_data = state.get_data();

    let mut all_logs = Vec::new();
    let mut executed_count = 0;

    for (_graph_id, graph) in project_data.graphs.iter() {
        if graph.kind != GraphKind::Event {
            continue;
        }

        let event_begin_nodes: Vec<_> = {
            let data_state = graph.data_state.read().map_err(|e| e.to_string())?;
            data_state
                .nodes
                .iter()
                .filter(|(_, n)| n.definition.node_type == "event_begin")
                .map(|(id, _)| *id)
                .collect()
        };

        if event_begin_nodes.is_empty() {
            log_app!(
                LogLevel::Info,
                "[execute_project] Graph '{}' has no event_begin node, skipping",
                graph.name
            );
            continue;
        }

        let entry_node = event_begin_nodes[0];
        log_app!(
            LogLevel::Info,
            "[execute_project] Starting graph '{}' from event_begin node {:?}",
            graph.name,
            entry_node
        );

        let runtime = crate::graph::GraphRuntime::new(
            Arc::new(graph.clone()),
            Arc::clone(&state.project_data),
            Arc::clone(&state.project_store),
        );

        let mut executor = Executor::new(Arc::new(Mutex::new(runtime)));
        executor.start(entry_node)?;

        for line in executor.logs() {
            all_logs.push(line.clone());
            log_app!(LogLevel::Info, "[Execute] {}", line);
        }
        executed_count += 1;
    }

    Ok(json!({
        "executedGraphs": executed_count,
        "logs": all_logs,
    }))
}
