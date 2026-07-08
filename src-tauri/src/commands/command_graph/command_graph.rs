use crate::event::emit_project_event;
use crate::event::{EventEvent, EventFunction, EventResource};
use crate::event::Event;
use crate::graph::register::event::EVENT_BEGIN_NODE_TYPE;
use crate::graph::register::function::{FUNCTION_ENTRY_NODE_TYPE, FUNCTION_RETURN_NODE_TYPE};
use crate::graph::{FunctionSignaturePin, GraphKind};
use crate::project::{
    GraphDocumentKind, GraphResourcePath, emit_pin_change_events, read_project_index,
};
use crate::schema::GraphInstanceDTO;
use crate::project::ProjectState;
use tauri::{AppHandle, State};

fn parse_graph_path(graph_path: &str) -> Result<GraphResourcePath, String> {
    GraphResourcePath::new(graph_path).map_err(|e| e.to_string())
}

fn existing_graph_names(
    state: &State<ProjectState>,
    graph_kind: GraphKind,
    excluded_path: Option<GraphResourcePath>,
) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = state
        .project_data
        .read()
        .unwrap()
        .graphs
        .values()
        .filter(|graph| {
            graph.kind == graph_kind && Some(graph.resource_path.clone()) != excluded_path
        })
        .map(|graph| graph.name.clone())
        .collect();

    if let Some(path) = state.get_path() {
        let index = read_project_index(&path).map_err(|e| e.to_string())?;
        let expected_kind = GraphDocumentKind::from(&graph_kind);
        names.extend(
            index
                .graphs
                .into_iter()
                .filter(|graph| {
                    graph.graph_type == expected_kind
                        && excluded_path
                            .as_ref()
                            .is_none_or(|path| graph.path != path.as_str())
                })
                .map(|graph| graph.name),
        );
    }

    names.sort();
    names.dedup();
    Ok(names)
}

#[tauri::command]
pub fn create_event(
    app: AppHandle,
    state: State<ProjectState>,
    graph_name: &str,
) -> Result<String, String> {
    let graph = state.add_draft_graph_with_existing_names(
        graph_name,
        GraphKind::Event,
        existing_graph_names(&state, GraphKind::Event, None)?,
    );
    // 每个事件图自动拥有一个系统托管的 Event Begin 壳节点（对齐 UE5 事件图）。
    graph.create_node_with_position(EVENT_BEGIN_NODE_TYPE, 120.0, 120.0, None)?;
    let graph_path = graph.resource_path.as_str().to_string();
    emit_project_event(
        &app,
        Event::Event(EventEvent::EventCreated {
            path: graph_path.clone(),
            data: (&graph).into(),
        }),
    );
    Ok(graph_path)
}

#[tauri::command]
pub fn create_function(
    app: AppHandle,
    state: State<ProjectState>,
    graph_name: &str,
) -> Result<String, String> {
    let graph = state.add_draft_graph_with_existing_names(
        graph_name,
        GraphKind::Function,
        existing_graph_names(&state, GraphKind::Function, None)?,
    );
    // 每个函数图自动拥有 Entry / Return 壳节点（对齐 UE5 函数图）。
    graph.create_node_with_position(FUNCTION_ENTRY_NODE_TYPE, 120.0, 160.0, None)?;
    graph.create_node_with_position(FUNCTION_RETURN_NODE_TYPE, 560.0, 160.0, None)?;
    // 默认签名含 exec 入/出参；投影到 Entry / Return 壳节点 pin。
    let _ = graph.sync_function_shell_pins();
    let graph_path = graph.resource_path.as_str().to_string();
    emit_project_event(
        &app,
        Event::Function(EventFunction::FunctionCreated {
            path: graph_path.clone(),
            data: (&graph).into(),
        }),
    );
    Ok(graph_path)
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: String,
) -> Result<(), String> {
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

/// `update_function_signature` 的返回：函数图 DTO + 已同步 Call pin 的调用方图 DTO + 副作用警告。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFunctionSignatureResult {
    pub graph: GraphInstanceDTO,
    pub caller_graphs: Vec<GraphInstanceDTO>,
    /// 签名无 exec 入参但函数体含副作用节点时前端据此提示。
    pub side_effect_warning: bool,
}

#[tauri::command]
pub fn update_function_signature(
    app: AppHandle,
    state: State<ProjectState>,
    function_path: String,
    inputs: Option<Vec<FunctionSignaturePin>>,
    outputs: Option<Vec<FunctionSignaturePin>>,
) -> Result<UpdateFunctionSignatureResult, String> {
    let function_path = parse_graph_path(&function_path)?;
    let (graph, change_sets) = state.update_function_signature(&function_path, inputs, outputs)?;
    let dto: GraphInstanceDTO = (&graph).into();
    emit_project_event(
        &app,
        Event::Function(EventFunction::FunctionUpdated {
            path: function_path.as_str().to_string(),
            data: dto.clone(),
        }),
    );
    // 签名变更后同步 Entry / Return 壳节点 pin（新增 / 移除 / 更新）到前端画布。
    emit_pin_change_events(&app, &function_path, &graph, &change_sets);
    // 同步所有引用该函数的 Call 节点 pin，并随 invoke 回包带回调用方图供前端即时刷新。
    let mut caller_graphs = Vec::new();
    for (caller_path, caller_graph, caller_sets) in
        state.sync_call_nodes_for_function(&function_path)
    {
        emit_pin_change_events(&app, &caller_path, &caller_graph, &caller_sets);
        caller_graphs.push((&caller_graph).into());
    }
    let side_effect_warning = !graph.signature_has_exec_input()
        && state.function_has_side_effect_nodes(&function_path);
    Ok(UpdateFunctionSignatureResult {
        graph: dto,
        caller_graphs,
        side_effect_warning,
    })
}

#[tauri::command]
pub fn get_graph(
    _app: AppHandle,
    state: State<ProjectState>,
    graph_path: String,
) -> Result<GraphInstanceDTO, String> {
    let graph_path = parse_graph_path(&graph_path)?;
    let graph = match state.get_graph(&graph_path) {
        Some(graph) => graph,
        None => state.load_graph_from_current_project(&graph_path)?.graph,
    };
    Ok((&graph).into())
}

#[tauri::command]
pub fn unload_project_graph(state: State<ProjectState>, graph_path: String) -> Result<(), String> {
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
) -> Result<SaveProjectGraphResult, String> {
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
pub fn duplicate_graph(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: String,
) -> Result<GraphInstanceDTO, String> {
    let graph_path = parse_graph_path(&graph_path)?;
    let project_path = state.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
    let document = crate::project::duplicate_project_graph_file(&project_path, &graph_path)
        .map_err(|e| e.to_string())?;
    let graph = state.insert_loaded_graph(document.graph.clone(), document.local_variables.clone());
    let event = match graph.kind {
        GraphKind::Event => Event::Event(EventEvent::EventCreated {
            path: graph.resource_path.as_str().to_string(),
            data: (&graph).into(),
        }),
        GraphKind::Function => Event::Function(EventFunction::FunctionCreated {
            path: graph.resource_path.as_str().to_string(),
            data: (&graph).into(),
        }),
    };
    emit_project_event(&app, event);
    Ok((&graph).into())
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
) -> Result<Vec<FunctionCallSiteDTO>, String> {
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
) -> Result<Vec<GraphInstanceDTO>, String> {
    let function_path = parse_graph_path(&function_path)?;
    let updated = state.purge_call_nodes_for_function(&function_path)?;
    Ok(updated.iter().map(|(_, g)| g.into()).collect())
}
