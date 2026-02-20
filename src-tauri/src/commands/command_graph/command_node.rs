use crate::graph::{GraphId, NodeId, PinId, NodeInstanceParams, DataValue};
use crate::project::ProjectState;
use crate::event::{emit_project_event, Event, EventNode};
use crate::schema::{NodeInstanceDTO, PinInstanceDTO};
use crate::log::log_app;
use serde::{Deserialize, Serialize};
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
    log_app::info!("create_node called: graph_id={}, node_type={}, x={:?}, y={:?}", graph_id, node_type, x, y);
    
    let bounding = state.project_data.write().unwrap();
    let graph = bounding.graphs.get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
    
    // 创建节点并设置位置
    let node_id = graph.create_node_with_position(
        node_type,
        x.unwrap_or(0.0),
        y.unwrap_or(0.0),
        params,
    )?;
    
    // 获取创建的节点实例并转换为 DTO
    let node_instance = graph.get_node_instance(node_id)
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
        pins_dto.push(PinInstanceDTO::from_pin_with_context(pin, resolved_type, Vec::new()));
    }
    drop(data_state);

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
    log_app::info!("batch_create_nodes called: graph_id={}, count={}", graph_id, requests.len());

    let bounding = state.project_data.write().unwrap();
    let graph = bounding.graphs.get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    let mut results: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)> = Vec::with_capacity(requests.len());

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
    let _ = graph.infer_types();

    // 构建 DTO
    for &node_id in &created_ids {
        let node_instance = graph.get_node_instance(node_id)
            .ok_or_else(|| format!("Node '{}' not found after creation", node_id))?;

        let mut node_dto: NodeInstanceDTO = (&node_instance).into();

        let pin_instances = graph.get_pin_instances_by_node_id(node_id);
        let mut pins_dto = Vec::with_capacity(pin_instances.len());
        for pin in &pin_instances {
            match pin.definition.direction {
                crate::graph::PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
                crate::graph::PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
            }
            pins_dto.push(PinInstanceDTO::from(pin));
        }

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

    Ok(node_ids)
}

#[tauri::command]
pub fn delete_node(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    node_id: NodeId,
) -> Result<(), String> {
    let bounding = state.project_data.write().unwrap();
    let graph = bounding.graphs.get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
    
    graph.remove_node(node_id)?;
    
    // 发送节点删除事件
    emit_project_event(
        &app,
        Event::Node(EventNode::NodeDeleted {
            graph_id,
            node_id,
        }),
    );
    
    Ok(())
}

/// 批量删除节点（单次 IPC + 单个事件）
#[tauri::command]
pub fn batch_delete_nodes(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    node_ids: Vec<NodeId>,
) -> Result<(), String> {
    log_app::info!("batch_delete_nodes called: graph_id={}, count={}", graph_id, node_ids.len());

    let bounding = state.project_data.write().unwrap();
    let graph = bounding.graphs.get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    for &nid in &node_ids {
        graph.remove_node_raw(nid)?;
    }
    let _ = graph.infer_types();

    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchDeleted {
            graph_id,
            node_ids,
        }),
    );

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
    let updates_tuple: Vec<(NodeId, f32, f32)> = updates
        .iter()
        .map(|u| (u.node_id, u.x, u.y))
        .collect();

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
    let nid = NodeId::from(
        Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?,
    );
    let pids: Vec<PinId> = pin_ids
        .iter()
        .map(|s| Uuid::parse_str(s).map(PinId::from).map_err(|e| format!("Invalid pin_id '{}': {}", s, e)))
        .collect::<Result<Vec<_>, _>>()?;

    log_app::info!(
        "[create_node_with_id] graph={}, node={}, type={}, pins={}",
        graph_id, node_id, node_type, pids.len()
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    graph.create_node_raw_with_ids(
        &node_type,
        nid,
        &pids,
        x.unwrap_or(0.0),
        y.unwrap_or(0.0),
        params,
    )?;
    let _ = graph.infer_types();

    let node_instance = graph.get_node_instance(nid)
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
        pins_dto.push(PinInstanceDTO::from_pin_with_context(pin, resolved_type, Vec::new()));
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

    Ok(())
}

/// DTO for a node snapshot (used by restore_nodes).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreNodeDTO {
    pub node_id: String,
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    pub params: Option<NodeInstanceParams>,
    pub pins: Vec<RestorePinDTO>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePinDTO {
    pub pin_id: String,
    pub user_value: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreConnectionDTO {
    pub from_pin: String,
    pub to_pin: String,
}

/// Incrementally restore nodes/pins/connections that were previously deleted.
/// Used by the DeleteNodes command's undo.
#[tauri::command]
pub fn restore_nodes(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    nodes: Vec<RestoreNodeDTO>,
    connections: Vec<RestoreConnectionDTO>,
) -> Result<(), String> {
    log_app::info!(
        "[restore_nodes] graph={}, nodes={}, connections={}",
        graph_id, nodes.len(), connections.len()
    );

    let bounding = state.project_data.write().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    let mut all_results: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)> = Vec::new();

    for node_snap in &nodes {
        let nid = NodeId::from(
            Uuid::parse_str(&node_snap.node_id)
                .map_err(|e| format!("Invalid node_id '{}': {}", node_snap.node_id, e))?,
        );
        let pids: Vec<PinId> = node_snap
            .pins
            .iter()
            .map(|p| {
                Uuid::parse_str(&p.pin_id)
                    .map(PinId::from)
                    .map_err(|e| format!("Invalid pin_id '{}': {}", p.pin_id, e))
            })
            .collect::<Result<Vec<_>, _>>()?;

        graph.create_node_raw_with_ids(
            &node_snap.node_type,
            nid,
            &pids,
            node_snap.x,
            node_snap.y,
            node_snap.params.clone(),
        )?;

        // Restore pin user values
        for pin_snap in &node_snap.pins {
            if let Some(ref raw_val) = pin_snap.user_value {
                let pid = PinId::from(Uuid::parse_str(&pin_snap.pin_id).unwrap());
                if let Ok(dv) = serde_json::from_value::<DataValue>(raw_val.clone()) {
                    let _ = graph.set_pin_user_value_by_pin_id(pid, dv);
                }
            }
        }

        // Build DTOs for event
        let node_instance = graph.get_node_instance(nid)
            .ok_or_else(|| format!("Restored node '{}' not found", node_snap.node_id))?;
        let mut node_dto: NodeInstanceDTO = (&node_instance).into();
        let pin_instances = graph.get_pin_instances_by_node_id(nid);
        let mut pins_dto = Vec::with_capacity(pin_instances.len());
        for pin in &pin_instances {
            match pin.definition.direction {
                crate::graph::PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
                crate::graph::PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
            }
            pins_dto.push(PinInstanceDTO::from(pin));
        }
        all_results.push((nid, node_dto, pins_dto));
    }

    // Restore connections
    for conn in &connections {
        let from_pid = PinId::from(
            Uuid::parse_str(&conn.from_pin)
                .map_err(|e| format!("Invalid from_pin '{}': {}", conn.from_pin, e))?,
        );
        let to_pid = PinId::from(
            Uuid::parse_str(&conn.to_pin)
                .map_err(|e| format!("Invalid to_pin '{}': {}", conn.to_pin, e))?,
        );
        let _ = graph.connect(from_pid, to_pid);
    }

    let _ = graph.infer_types();

    // Emit batch created event so frontend adds nodes to store
    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchCreated {
            graph_id,
            nodes: all_results,
        }),
    );

    Ok(())
}
