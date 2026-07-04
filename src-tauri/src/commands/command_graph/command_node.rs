use crate::event::{Event, EventConnection, EventNode, emit_project_event};
use crate::execution::ResultSourceStore;
use crate::graph::{
    DataType, DataValue, GraphId, NodeId, NodeInstanceParams, PinChangeSet, PinDirection, PinId,
};
use crate::log::log_app;
use crate::project::{
    emit_inferred_types, emit_pin_change_events, emit_runtime_source_invalidation, ProjectState,
};
use crate::schema::{GraphUndoPatch, NodeInstanceDTO, PinInstanceDTO};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePositionUpdate {
    node_id: NodeId,
    x: f32,
    y: f32,
}

/// Return value from create_node — includes pin IDs for undo/redo context
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNodeResult {
    pub node_id: String,
    pub pin_ids: Vec<String>,
}

#[tauri::command]
pub fn create_node(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    node_type: &str,
    x: Option<f32>,
    y: Option<f32>,
    params: Option<NodeInstanceParams>,
) -> Result<CreateNodeResult, String> {
    log_app::info!(
        "create_node called: graph_id={}, node_type={}, x={:?}, y={:?}",
        graph_id,
        node_type,
        x,
        y
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
    let variable_symbols =
        ProjectState::variable_symbols_from_variables(&bounding.variables, &graph_id, &graph.kind);
    let dataframe_symbols = ProjectState::dataframe_symbols_from_databases(&bounding.databases);

    // 创建节点并设置位置
    let node_id =
        graph.create_node_with_position(node_type, x.unwrap_or(0.0), y.unwrap_or(0.0), params)?;
    graph.resolve_variable_nodes(&variable_symbols);
    graph.resolve_dataframe_nodes(&dataframe_symbols);

    // 获取创建的节点实例并转换为 DTO
    let node_instance = graph
        .get_node_instance(node_id)
        .ok_or_else(|| format!("Node '{}' not found after creation", node_id))?;

    let mut node_dto: NodeInstanceDTO = (&node_instance).into();

    // 填充 inputs 和 outputs，并构建 pins DTO 供前端直接使用
    let pin_instances = graph.get_pin_instances_by_node_id(node_id);
    let data_state = graph.data_state.read().unwrap();
    let mut pins_dto = Vec::with_capacity(pin_instances.len());
    for pin in &pin_instances {
        match pin.definition.direction {
            crate::graph::PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
            crate::graph::PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
        }
        let resolved_type = data_state.pin_types.get(&pin.id);
        pins_dto.push(PinInstanceDTO::from_pin_with_context(pin, resolved_type));
    }
    drop(data_state);
    drop(bounding);

    let pin_id_strings: Vec<String> = pin_instances.iter().map(|p| p.id.to_string()).collect();

    emit_project_event(
        &app,
        Event::Node(EventNode::NodeCreated {
            graph_id,
            node_id,
            data: node_dto,
            pins: pins_dto,
        }),
    );

    Ok(CreateNodeResult {
        node_id: node_id.to_string(),
        pin_ids: pin_id_strings,
    })
}

/// 批量创建节点请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateNodeRequest {
    node_type: String,
    x: Option<f32>,
    y: Option<f32>,
    params: Option<NodeInstanceParams>,
}

/// 批量创建节点（粘贴时使用），一次性发送所有节点信息，避免逐个创建的延迟
#[tauri::command]
pub fn batch_create_nodes(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    requests: Vec<BatchCreateNodeRequest>,
) -> Result<Vec<String>, String> {
    log_app::info!(
        "batch_create_nodes called: graph_id={}, count={}",
        graph_id,
        requests.len()
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
    let variable_symbols =
        ProjectState::variable_symbols_from_variables(&bounding.variables, &graph_id, &graph.kind);
    let dataframe_symbols = ProjectState::dataframe_symbols_from_databases(&bounding.databases);

    let mut results: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)> =
        Vec::with_capacity(requests.len());

    // 使用 create_node_raw 跳过逐个 infer_types
    let mut created_ids: Vec<NodeId> = Vec::with_capacity(requests.len());
    for req in &requests {
        let node_id = graph.create_node_raw(
            &req.node_type,
            req.x.unwrap_or(0.0),
            req.y.unwrap_or(0.0),
            req.params.clone(),
        )?;
        created_ids.push(node_id);
    }

    // 所有节点就位后统一推断一次类型
    graph.resolve_variable_nodes(&variable_symbols);
    graph.resolve_dataframe_nodes(&dataframe_symbols);
    let _ = graph.infer_types();

    // 构建 DTO
    for &node_id in &created_ids {
        let node_instance = graph
            .get_node_instance(node_id)
            .ok_or_else(|| format!("Node '{}' not found after creation", node_id))?;

        let mut node_dto: NodeInstanceDTO = (&node_instance).into();

        let pin_instances = graph.get_pin_instances_by_node_id(node_id);
        let data_state = graph.data_state.read().unwrap();
        let mut pins_dto = Vec::with_capacity(pin_instances.len());
        for pin in &pin_instances {
            match pin.definition.direction {
                crate::graph::PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
                crate::graph::PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
            }
            let resolved_type = data_state.pin_types.get(&pin.id);
            pins_dto.push(PinInstanceDTO::from_pin_with_context(pin, resolved_type));
        }
        drop(data_state);

        results.push((node_id, node_dto, pins_dto));
    }

    let node_ids: Vec<String> = results.iter().map(|(id, _, _)| id.to_string()).collect();

    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchCreated {
            graph_id,
            nodes: results,
        }),
    );
    drop(bounding);

    Ok(node_ids)
}

#[tauri::command]
pub fn delete_node(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_id: GraphId,
    node_id: NodeId,
) -> Result<(), String> {
    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    let deleted_pin_ids: Vec<PinId> = graph
        .get_pin_instances_by_node_id(node_id)
        .into_iter()
        .map(|pin| pin.id)
        .collect();

    graph.remove_node_raw(node_id)?;
    let _ = graph.infer_types();

    emit_project_event(
        &app,
        Event::Node(EventNode::NodeDeleted { graph_id, node_id }),
    );

    emit_runtime_source_invalidation(&app, &source_store, graph_id, &[], &deleted_pin_ids);

    // Do not resolve_dynamic_pins on neighbors: keeps dynamic pin IDs stable for undo.
    drop(bounding);

    Ok(())
}

/// 批量删除节点（单次 IPC + 单个事件）；返回删除前捕获的子图快照供 undo 使用。
#[tauri::command]
pub fn batch_delete_nodes(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_id: GraphId,
    node_ids: Vec<NodeId>,
) -> Result<GraphUndoPatch, String> {
    log_app::info!(
        "batch_delete_nodes called: graph_id={}, count={}",
        graph_id,
        node_ids.len()
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    let snapshot = graph.capture_subgraph_for_delete(&node_ids);

    let mut deleted_pin_ids = Vec::new();
    for &nid in &node_ids {
        deleted_pin_ids.extend(
            graph
                .get_pin_instances_by_node_id(nid)
                .into_iter()
                .map(|pin| pin.id),
        );
    }

    for &nid in &node_ids {
        graph.remove_node_raw(nid)?;
    }

    let _ = graph.infer_types();

    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchDeleted { graph_id, node_ids }),
    );

    emit_runtime_source_invalidation(&app, &source_store, graph_id, &[], &deleted_pin_ids);

    // Do not resolve_dynamic_pins on neighbors: keeps dynamic pin IDs stable for undo.
    drop(bounding);

    Ok(snapshot)
}

/// Apply a previously captured undo patch (DeleteNodes undo / DisconnectPin undo / Composite redo).
#[tauri::command]
pub fn apply_graph_patch(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    patch: GraphUndoPatch,
) -> Result<(), String> {
    log_app::info!(
        "[apply_graph_patch] graph={}, nodes={}, neighbors={}, connections={}",
        graph_id,
        patch.nodes.len(),
        patch.neighbor_nodes.len(),
        patch.connections.len()
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
    let variable_symbols =
        ProjectState::variable_symbols_from_variables(&bounding.variables, &graph_id, &graph.kind);
    let dataframe_symbols = ProjectState::dataframe_symbols_from_databases(&bounding.databases);

    let result = graph.apply_graph_patch(patch, &variable_symbols, &dataframe_symbols)?;

    if !result.node_batches.is_empty() {
        emit_project_event(
            &app,
            Event::Node(EventNode::NodesBatchCreated {
                graph_id,
                nodes: result.node_batches,
            }),
        );
    }

    // Pins before connections so the frontend store has pin entries before batchConnect.
    emit_pin_change_events(&app, graph_id, &graph, &result.change_sets);

    if !result.established_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchCreated {
                graph_id,
                connections: result.established_connections,
            }),
        );
    }
    emit_inferred_types(&app, graph_id, result.inferred);
    drop(bounding);

    Ok(())
}

/// 批量更新节点位置（拖拽结束时调用，CQRS 模式）
#[tauri::command]
pub fn update_node_positions(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    updates: Vec<NodePositionUpdate>,
) -> Result<(), String> {
    let updates_tuple: Vec<(NodeId, f32, f32)> =
        updates.iter().map(|u| (u.node_id, u.x, u.y)).collect();

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    graph.set_node_positions(&updates_tuple)?;

    emit_project_event(
        &app,
        Event::Node(EventNode::NodePositionsUpdated {
            graph_id,
            updates: updates_tuple,
        }),
    );
    drop(bounding);

    Ok(())
}

// ==================== Undo/Redo 支持命令 ====================

/// Create a node with specific IDs (for redo — preserves node/pin identity).
#[tauri::command]
pub fn create_node_with_id(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    node_id: String,
    pin_ids: Vec<String>,
    node_type: String,
    x: Option<f32>,
    y: Option<f32>,
    params: Option<NodeInstanceParams>,
) -> Result<(), String> {
    let nid =
        NodeId::from(Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?);
    let pids: Vec<PinId> = pin_ids
        .iter()
        .map(|s| {
            Uuid::parse_str(s)
                .map(PinId::from)
                .map_err(|e| format!("Invalid pin_id '{}': {}", s, e))
        })
        .collect::<Result<Vec<_>, _>>()?;

    log_app::info!(
        "[create_node_with_id] graph={}, node={}, type={}, pins={}",
        graph_id,
        node_id,
        node_type,
        pids.len()
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
    let variable_symbols =
        ProjectState::variable_symbols_from_variables(&bounding.variables, &graph_id, &graph.kind);
    let dataframe_symbols = ProjectState::dataframe_symbols_from_databases(&bounding.databases);

    graph.create_node_raw_with_ids(
        &node_type,
        nid,
        &pids,
        x.unwrap_or(0.0),
        y.unwrap_or(0.0),
        params,
    )?;
    graph.resolve_variable_nodes(&variable_symbols);
    graph.resolve_dataframe_nodes(&dataframe_symbols);
    // 与 `create_node_with_position` 一致：id 指定的新建节点同样没有任何连接，
    // 不会改变已有 pin 类型，故跳过全图类型推断，保持 O(1)。

    let node_instance = graph
        .get_node_instance(nid)
        .ok_or_else(|| format!("Node '{}' not found after creation", node_id))?;

    let mut node_dto: NodeInstanceDTO = (&node_instance).into();
    let pin_instances = graph.get_pin_instances_by_node_id(nid);
    let data_state_r = graph.data_state.read().unwrap();
    let mut pins_dto = Vec::with_capacity(pin_instances.len());
    for pin in &pin_instances {
        match pin.definition.direction {
            crate::graph::PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
            crate::graph::PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
        }
        let resolved_type = data_state_r.pin_types.get(&pin.id);
        pins_dto.push(PinInstanceDTO::from_pin_with_context(pin, resolved_type));
    }
    drop(data_state_r);

    emit_project_event(
        &app,
        Event::Node(EventNode::NodeCreated {
            graph_id,
            node_id: nid,
            data: node_dto,
            pins: pins_dto,
        }),
    );
    drop(bounding);

    Ok(())
}

// ==================== Batch Create with Connections ====================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPinEntry {
    pub pin_id: String,
    pub name: String,
    pub direction: PinDirection,
    pub user_value: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchNodeEntry {
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    pub params: Option<NodeInstanceParams>,
    pub pins: Vec<BatchPinEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConnectionEntry {
    pub from_pin: String,
    pub to_pin: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateWithConnectionsResult {
    pub node_ids: Vec<String>,
    pub pin_mapping: HashMap<String, String>,
    pub undo_patch: GraphUndoPatch,
}

/// Batch-create nodes with pin remapping and connection restoration.
/// Used by paste, template import, and similar bulk-creation scenarios.
#[tauri::command]
pub fn batch_create_with_connections(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    entries: Vec<BatchNodeEntry>,
    connections: Vec<BatchConnectionEntry>,
) -> Result<BatchCreateWithConnectionsResult, String> {
    log_app::info!(
        "[batch_create_with_connections] graph={}, entries={}, connections={}",
        graph_id,
        entries.len(),
        connections.len()
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
    let variable_symbols =
        ProjectState::variable_symbols_from_variables(&bounding.variables, &graph_id, &graph.kind);
    let dataframe_symbols = ProjectState::dataframe_symbols_from_databases(&bounding.databases);

    let mut all_results: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)> = Vec::new();
    let mut created_node_ids: Vec<NodeId> = Vec::new();
    let mut pin_mapping: HashMap<String, String> = HashMap::new();
    let mut used_new_pins: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in &entries {
        let node_id =
            graph.create_node_raw(&entry.node_type, entry.x, entry.y, entry.params.clone())?;
        created_node_ids.push(node_id);

        let new_pins = graph.get_pin_instances_by_node_id(node_id);

        for old_pin in &entry.pins {
            if let Some(new_pin) = new_pins.iter().find(|np| {
                np.definition.name == old_pin.name
                    && np.definition.direction == old_pin.direction
                    && !used_new_pins.contains(&np.id.to_string())
            }) {
                let new_id = new_pin.id.to_string();
                pin_mapping.insert(old_pin.pin_id.clone(), new_id.clone());
                used_new_pins.insert(new_id);

                if let Some(ref raw_val) = old_pin.user_value {
                    if let Ok(dv) = serde_json::from_value::<DataValue>(raw_val.clone()) {
                        let _ = graph.set_pin_user_value_by_pin_id(new_pin.id, dv);
                    }
                }
            }
        }
        graph.resolve_variable_nodes(&variable_symbols);
        graph.resolve_dataframe_nodes(&dataframe_symbols);

        let node_instance = graph
            .get_node_instance(node_id)
            .ok_or_else(|| format!("Node '{}' not found after creation", node_id))?;
        let mut node_dto: NodeInstanceDTO = (&node_instance).into();
        let pin_instances = graph.get_pin_instances_by_node_id(node_id);
        let data_state = graph.data_state.read().unwrap();
        let mut pins_dto = Vec::with_capacity(pin_instances.len());
        for pin in &pin_instances {
            match pin.definition.direction {
                PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
                PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
            }
            let resolved_type = data_state.pin_types.get(&pin.id);
            pins_dto.push(PinInstanceDTO::from_pin_with_context(pin, resolved_type));
        }
        drop(data_state);
        all_results.push((node_id, node_dto, pins_dto));
    }

    let node_id_strings: Vec<String> = created_node_ids.iter().map(|id| id.to_string()).collect();

    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchCreated {
            graph_id,
            nodes: all_results,
        }),
    );

    // Multi-pass connection restoration with one shared side-effect flush per pass.
    //
    // Each connection only mutates topology (`connect_topology`); schema propagation
    // and type inference run once per pass via `finish_graph_effects`. This both
    // materializes any dynamic pins later connections depend on and keeps the cost
    // O(passes) instead of O(connections). All connection/pin/type events are emitted
    // once at the end so the frontend renders the pasted subgraph atomically rather
    // than wiring nodes "one by one".
    //
    // Dynamic pins are created lazily by the per-pass flush; since the connection
    // array order is not guaranteed to respect that dependency, connections targeting
    // not-yet-materialized pins are retried until no more progress is made.
    let mut pending: Vec<usize> = (0..connections.len()).collect();
    let mut established: Vec<(PinId, PinId)> = Vec::new();
    let mut all_change_sets: Vec<PinChangeSet> = Vec::new();
    let mut last_inferred: Vec<(PinId, DataType)> = Vec::new();

    loop {
        let mut next_pending = Vec::new();
        let mut pass_seeds: Vec<NodeId> = Vec::new();
        let mut made_progress = false;

        for &idx in &pending {
            let conn = &connections[idx];
            let new_from = pin_mapping.get(&conn.from_pin).cloned();
            let new_to = pin_mapping.get(&conn.to_pin).cloned();

            let topo = match (new_from, new_to) {
                (Some(from_str), Some(to_str)) => {
                    match (Uuid::parse_str(&from_str), Uuid::parse_str(&to_str)) {
                        (Ok(from_uuid), Ok(to_uuid)) => graph
                            .connect_topology(PinId::from(from_uuid), PinId::from(to_uuid))
                            .ok(),
                        _ => None,
                    }
                }
                _ => None,
            };

            if let Some(topo) = topo {
                made_progress = true;
                established.push((topo.from_pin, topo.to_pin));
                pass_seeds.extend(topo.seed_nodes);
            } else {
                next_pending.push(idx);
            }
        }

        if made_progress {
            let (change_sets, inferred) = graph.finish_graph_effects(&pass_seeds);
            all_change_sets.extend(change_sets);
            last_inferred = inferred;

            // Re-scan created nodes for dynamic pins materialized by this pass's flush,
            // so the next pass can map connections targeting them.
            for (entry_idx, entry) in entries.iter().enumerate() {
                let nid = created_node_ids[entry_idx];
                let current_pins = graph.get_pin_instances_by_node_id(nid);
                for old_pin in &entry.pins {
                    if pin_mapping.contains_key(&old_pin.pin_id) {
                        continue;
                    }
                    if let Some(new_pin) = current_pins.iter().find(|np| {
                        np.definition.name == old_pin.name
                            && np.definition.direction == old_pin.direction
                            && !used_new_pins.contains(&np.id.to_string())
                    }) {
                        let new_id = new_pin.id.to_string();
                        pin_mapping.insert(old_pin.pin_id.clone(), new_id.clone());
                        used_new_pins.insert(new_id);
                        if let Some(ref raw_val) = old_pin.user_value {
                            if let Ok(dv) = serde_json::from_value::<DataValue>(raw_val.clone()) {
                                let _ = graph.set_pin_user_value_by_pin_id(new_pin.id, dv);
                            }
                        }
                    }
                }
            }
        }

        if !made_progress || next_pending.is_empty() {
            break;
        }
        pending = next_pending;
    }

    if !established.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchCreated {
                graph_id,
                connections: established,
            }),
        );
    }
    emit_pin_change_events(&app, graph_id, &graph, &all_change_sets);
    emit_inferred_types(&app, graph_id, last_inferred);

    let undo_patch = graph.capture_subgraph(&created_node_ids);
    drop(bounding);

    Ok(BatchCreateWithConnectionsResult {
        node_ids: node_id_strings,
        pin_mapping,
        undo_patch,
    })
}
