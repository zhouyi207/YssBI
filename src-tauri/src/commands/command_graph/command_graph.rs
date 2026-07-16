use crate::error::AppError;
use crate::event::Event;
use crate::event::emit_project_event;
use crate::event::{EventEvent, EventFunction, EventResource};
use crate::graph::register::event::EVENT_BEGIN_NODE_TYPE;
use crate::graph::register::function::{FUNCTION_ENTRY_NODE_TYPE, FUNCTION_RETURN_NODE_TYPE};
use crate::graph::{FunctionSignaturePin, GraphKind};
use crate::project::ProjectState;
use crate::project::{GraphDocumentKind, GraphResourcePath};
use crate::schema::GraphInstanceDTO;
use tauri::{AppHandle, State};

fn parse_graph_path(graph_path: &str) -> Result<GraphResourcePath, AppError> {
    GraphResourcePath::new(graph_path).map_err(AppError::from)
}

#[tauri::command]
pub fn create_event(state: State<ProjectState>, graph_name: &str) -> Result<String, AppError> {
    let graph = state.create_graph(graph_name, GraphKind::Event)?;
    // 每个事件图自动拥有一个系统托管的 Event Begin 壳节点（对齐 UE5 事件图）。
    graph.create_node_with_position(EVENT_BEGIN_NODE_TYPE, 120.0, 120.0, None)?;
    let graph_path = graph.resource_path.clone();
    state.commit_persisted_graph_and_unload(&graph_path)?;
    Ok(graph_path.as_str().to_string())
}

#[tauri::command]
pub fn create_function(state: State<ProjectState>, graph_name: &str) -> Result<String, AppError> {
    let graph = state.create_graph(graph_name, GraphKind::Function)?;
    // 每个函数图自动拥有 Entry / Return 壳节点（对齐 UE5 函数图）。
    graph.create_node_with_position(FUNCTION_ENTRY_NODE_TYPE, 120.0, 160.0, None)?;
    graph.create_node_with_position(FUNCTION_RETURN_NODE_TYPE, 560.0, 160.0, None)?;
    // 默认签名含 exec 入/出参；投影到 Entry / Return 壳节点 pin。
    let _ = graph.sync_function_shell_pins();
    let graph_path = graph.resource_path.clone();
    state.commit_persisted_graph_and_unload(&graph_path)?;
    Ok(graph_path.as_str().to_string())
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: String,
) -> Result<(), AppError> {
    let graph_path = parse_graph_path(&graph_path)?;
    let loaded_graph = state.remove_graph(&graph_path);
    let removed_kind = if let Some(path) = state.get_path() {
        crate::project::remove_project_graph_from_file(&path, &graph_path)
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let graph_kind = loaded_graph
        .as_ref()
        .map(|graph| graph.kind.clone())
        .or_else(|| removed_kind.map(GraphKind::from))
        .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

    let event = match graph_kind {
        GraphKind::Event => Event::Event(EventEvent::EventDeleted {
            path: graph_path.as_str().to_string(),
        }),
        GraphKind::Function => Event::Function(EventFunction::FunctionDeleted {
            path: graph_path.as_str().to_string(),
        }),
    };
    emit_project_event(&app, event);
    Ok(())
}

/// `update_function_signature` 的返回：函数图 DTO + 已同步 Call pin 的调用方图 DTO。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFunctionSignatureResult {
    pub graph: GraphInstanceDTO,
    pub caller_graphs: Vec<GraphInstanceDTO>,
}

#[tauri::command]
pub fn update_function_signature(
    app: AppHandle,
    state: State<ProjectState>,
    function_path: String,
    inputs: Option<Vec<FunctionSignaturePin>>,
    outputs: Option<Vec<FunctionSignaturePin>>,
) -> Result<UpdateFunctionSignatureResult, AppError> {
    let function_path = parse_graph_path(&function_path)?;
    let (graph, _change_sets) = state.update_function_signature(&function_path, inputs, outputs)?;
    let dto: GraphInstanceDTO = (&graph).into();
    // FunctionUpdated：供未发起 invoke 的监听方刷新；发起方以 invoke 回包为准（前端 echo guard 忽略）。
    emit_project_event(
        &app,
        Event::Function(EventFunction::FunctionUpdated {
            path: function_path.as_str().to_string(),
            data: dto.clone(),
        }),
    );
    // invoke 回包已含完整 Graph DTO（含 Entry/Return 与 Call pin），不在此 emit NodePinsUpdated
    //（对齐 resolve_graph_dynamic_pins：避免与 addGraphFromData 竞态）。
    let mut caller_graphs = Vec::new();
    for (_caller_path, caller_graph, _caller_sets) in
        state.sync_call_nodes_for_function(&function_path)
    {
        caller_graphs.push((&caller_graph).into());
    }
    Ok(UpdateFunctionSignatureResult {
        graph: dto,
        caller_graphs,
    })
}

#[tauri::command]
pub fn get_graph(
    _app: AppHandle,
    state: State<ProjectState>,
    graph_path: String,
) -> Result<GraphInstanceDTO, AppError> {
    let graph_path = parse_graph_path(&graph_path)?;
    let graph = match state.get_graph(&graph_path) {
        Some(graph) => graph,
        None => state.load_graph_from_current_project(&graph_path)?.graph,
    };
    Ok((&graph).into())
}

#[tauri::command]
pub fn unload_project_graph(
    state: State<ProjectState>,
    graph_path: String,
) -> Result<(), AppError> {
    let graph_path = parse_graph_path(&graph_path)?;
    state.unload_graph(&graph_path);
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectGraphResult {
    pub path: String,
}

#[tauri::command]
pub fn save_project_graph(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: String,
) -> Result<SaveProjectGraphResult, AppError> {
    let graph_path = parse_graph_path(&graph_path)?;
    let kind = graph_path.kind().map_err(|e| e.to_string())?;
    let moved_to = state
        .persist_loaded_graph(&graph_path)
        .map_err(|e| e.to_string())?;
    if let Some(to) = moved_to {
        let kind_str = match kind {
            GraphDocumentKind::Event => "event",
            GraphDocumentKind::Function => "function",
        };
        emit_project_event(
            &app,
            Event::Resource(EventResource::GraphResourceMoved {
                from: graph_path.as_str().to_string(),
                to: to.as_str().to_string(),
                kind: kind_str.to_string(),
            }),
        );
        Ok(SaveProjectGraphResult {
            path: to.as_str().to_string(),
        })
    } else {
        Ok(SaveProjectGraphResult {
            path: graph_path.as_str().to_string(),
        })
    }
}

#[tauri::command]
pub fn duplicate_graph(state: State<ProjectState>, graph_path: String) -> Result<String, AppError> {
    let graph_path = parse_graph_path(&graph_path)?;
    Ok(state
        .duplicate_persisted_graph(&graph_path)?
        .as_str()
        .to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCallSiteDTO {
    pub caller_graph_path: String,
    pub node_ids: Vec<crate::graph::NodeId>,
}

/// 查询引用目标函数的所有 Call Function 节点（按 caller 图分组）。
#[tauri::command]
pub fn get_function_call_sites(
    state: State<ProjectState>,
    function_path: String,
) -> Result<Vec<FunctionCallSiteDTO>, AppError> {
    let function_path = parse_graph_path(&function_path)?;
    let sites = state.get_function_call_sites(&function_path);
    Ok(sites
        .into_iter()
        .map(|(caller, node_ids)| FunctionCallSiteDTO {
            caller_graph_path: caller.as_str().to_string(),
            node_ids,
        })
        .collect())
}

/// 删除函数前移除所有 caller 图中的 Call Function 节点。
#[tauri::command]
pub fn purge_function_call_sites(
    state: State<ProjectState>,
    function_path: String,
) -> Result<Vec<GraphInstanceDTO>, AppError> {
    let function_path = parse_graph_path(&function_path)?;
    let updated = state.purge_call_nodes_for_function(&function_path)?;
    Ok(updated.iter().map(|(_, g)| g.into()).collect())
}
