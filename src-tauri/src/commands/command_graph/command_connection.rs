use crate::graph::{GraphId, NodeId, PinId, PinChangeSet};
use crate::event::{emit_project_event, Event, EventNode, EventConnection};
use crate::schema::PinInstanceDTO;
use crate::project::ProjectState;
use crate::log::log_app;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

/// 将 PinChangeSet 转为事件并发送
fn emit_pin_change_events(
    app: &AppHandle,
    graph_id: GraphId,
    graph: &crate::graph::GraphInstance,
    change_sets: Vec<PinChangeSet>,
) {
    for cs in change_sets {
        let added_dtos: Vec<PinInstanceDTO> = cs.added_pins.iter()
            .map(|pin| {
                let resolved_type = graph.get_pin_data_type_by_pin_id(pin.id);
                PinInstanceDTO::from_pin_with_context(pin, resolved_type.as_ref(), Vec::new())
            })
            .collect();

        let removed_pin_ids: Vec<PinId> = cs.removed_pin_ids;

        emit_project_event(
            app,
            Event::Node(EventNode::NodePinsUpdated {
                graph_id,
                node_id: cs.node_id,
                removed_pin_ids,
                added_pins: added_dtos,
                removed_connections: cs.removed_connections,
            }),
        );
    }
}

// ==================== 核心连接命令 ====================

/// 连接两个 Pin（前端 ConnectionService.connectPins）
#[tauri::command]
pub fn connect_pins(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    source_pin_id: String,
    target_pin_id: String,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );
    let from_pin = PinId::from(
        Uuid::parse_str(&source_pin_id).map_err(|e| format!("Invalid source_pin_id: {}", e))?,
    );
    let to_pin = PinId::from(
        Uuid::parse_str(&target_pin_id).map_err(|e| format!("Invalid target_pin_id: {}", e))?,
    );

    log_app::info!(
        "[command.connect_pins] graph={}, from={}, to={}",
        subgraph_id, source_pin_id, target_pin_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (auto_disconnected, change_sets) = graph.connect(from_pin, to_pin)?;

    // 先发送被自动断开的旧连接事件
    if let Some((old_from, old_to)) = auto_disconnected {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionDeleted {
                graph_id,
                from_pin: old_from,
                to_pin: old_to,
            }),
        );
    }

    // 发送新连接创建事件
    emit_project_event(
        &app,
        Event::Connection(EventConnection::ConnectionCreated {
            graph_id,
            from_pin: from_pin,
            to_pin: to_pin,
        }),
    );

    // 发送动态 pin 变更事件（如有）
    emit_pin_change_events(&app, graph_id, &graph, change_sets);
    Ok(())
}

/// 断开指定 Pin 的所有连接（前端 Alt+Click 调用 disconnect_pin）
#[tauri::command]
pub fn disconnect_pin(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    pin_id: String,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );
    let pin = PinId::from(
        Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?,
    );

    log_app::info!(
        "[command.disconnect_pin] graph={}, pin={}",
        subgraph_id, pin_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (removed_connections, change_sets) = graph.disconnect_pin(pin);

    // 发送批量断开事件
    if !removed_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchDeleted {
                graph_id,
                removed_connections,
            }),
        );
    }

    emit_pin_change_events(&app, graph_id, &graph, change_sets);
    Ok(())
}

// ==================== 兼容连接命令 ====================
// 前端 ConnectionService 的 createConnection/deleteConnection 等

/// 创建连接（与 connect_pins 功能相同，兼容旧接口）
///
/// 前端 ConnectionService.createConnection(subgraphId, sourcePinId, targetPinId)
#[tauri::command]
pub fn create_connection(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    source_pin_id: String,
    target_pin_id: String,
) -> Result<(), String> {
    connect_pins(app, state, subgraph_id, source_pin_id, target_pin_id)
}

/// 删除连接（按 from->to 格式的 connectionId 断开）
///
/// 前端 ConnectionService.deleteConnection(subgraphId, connectionId)
#[tauri::command]
pub fn delete_connection(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    connection_id: String,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );

    // connectionId 格式："{from_pin_uuid}->{to_pin_uuid}"
    let parts: Vec<&str> = connection_id.split("->").collect();
    if parts.len() != 2 {
        return Err(format!("Invalid connection_id format: '{}', expected 'from->to'", connection_id));
    }
    let from_pin = PinId::from(
        Uuid::parse_str(parts[0]).map_err(|e| format!("Invalid from_pin in connection_id: {}", e))?,
    );
    let to_pin = PinId::from(
        Uuid::parse_str(parts[1]).map_err(|e| format!("Invalid to_pin in connection_id: {}", e))?,
    );

    log_app::info!(
        "[command.delete_connection] graph={}, connection={}",
        subgraph_id, connection_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let change_sets = graph.disconnect(from_pin, to_pin);

    emit_project_event(
        &app,
        Event::Connection(EventConnection::ConnectionDeleted {
            graph_id,
            from_pin,
            to_pin,
        }),
    );

    emit_pin_change_events(&app, graph_id, &graph, change_sets);
    Ok(())
}

/// 连接 DTO（用于序列化返回给前端）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDTO {
    id: String,
    from: String,
    to: String,
}

/// 获取子图的所有连接
///
/// 前端 ConnectionService.getConnections(subgraphId)
#[tauri::command]
pub fn get_connections(
    state: State<ProjectState>,
    subgraph_id: String,
) -> Result<Vec<ConnectionDTO>, String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let connections: Vec<ConnectionDTO> = graph.all_connections()
        .into_iter()
        .map(|c| {
            let from_str = c.from_pin.to_string();
            let to_str = c.to_pin.to_string();
            ConnectionDTO {
                id: format!("{}->{}", from_str, to_str),
                from: from_str,
                to: to_str,
            }
        })
        .collect();

    Ok(connections)
}

/// 断开 Pin 的所有连接（与 disconnect_pin 功能相同，兼容旧接口）
///
/// 前端 ConnectionService.deleteConnectionsForPin(subgraphId, pinId)
#[tauri::command]
pub fn delete_connections_for_pin(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    pin_id: String,
) -> Result<Vec<String>, String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );
    let pin = PinId::from(
        Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?,
    );

    log_app::info!(
        "[command.delete_connections_for_pin] graph={}, pin={}",
        subgraph_id, pin_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (removed_connections, change_sets) = graph.disconnect_pin(pin);

    let removed_ids: Vec<String> = removed_connections.iter()
        .map(|(from, to)| format!("{}->{}", from, to))
        .collect();

    if !removed_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchDeleted {
                graph_id,
                removed_connections,
            }),
        );
    }

    emit_pin_change_events(&app, graph_id, &graph, change_sets);

    Ok(removed_ids)
}

/// 删除节点的所有连接
///
/// 前端 ConnectionService.deleteConnectionsForNode(subgraphId, nodeId)
#[tauri::command]
pub fn delete_connections_for_node(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    node_id: String,
) -> Result<Vec<String>, String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );
    let nid = NodeId::from(
        Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?,
    );

    log_app::info!(
        "[command.delete_connections_for_node] graph={}, node={}",
        subgraph_id, node_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let pin_instances = graph.get_pin_instances_by_node_id(nid);
    let mut all_removed_connections = Vec::new();
    let mut removed_ids = Vec::new();

    for pin in &pin_instances {
        let (removed_connections, change_sets) = graph.disconnect_pin(pin.id);
        for (from, to) in &removed_connections {
            removed_ids.push(format!("{}->{}", from, to));
        }
        all_removed_connections.extend(removed_connections);
        emit_pin_change_events(&app, graph_id, &graph, change_sets);
    }

    if !all_removed_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchDeleted {
                graph_id,
                removed_connections: all_removed_connections,
            }),
        );
    }

    Ok(removed_ids)
}

// ==================== 子图管理命令 ====================

/// 更新画布视图状态（位置、缩放等）
#[tauri::command]
pub fn update_canvas(
    state: State<ProjectState>,
    subgraph_id: String,
    canvas: Value,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );

    log_app::info!(
        "[command.update_canvas] graph={}",
        subgraph_id
    );

    let mut data = state.project_data.write().unwrap();
    let graph = data
        .graphs
        .get_mut(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    if let Some(x) = canvas.get("x").and_then(|v| v.as_f64()) {
        graph.position.x = x;
    }
    if let Some(y) = canvas.get("y").and_then(|v| v.as_f64()) {
        graph.position.y = y;
    }
    if let Some(scale) = canvas.get("scale").and_then(|v| v.as_f64()) {
        graph.position.scale = scale;
    }

    Ok(())
}

/// 更新子图的输入输出定义（用于 Function/Macro 的参数定义）
#[tauri::command]
pub fn update_subgraph_io(
    _state: State<ProjectState>,
    _id: String,
    _data: Value,
) -> Result<(), String> {
    // TODO: Function/Macro 子图的 IO 定义尚未设计
    log_app::info!("[command.update_subgraph_io] Not yet implemented");
    Ok(())
}

/// 重命名子图
#[tauri::command]
pub fn rename_subgraph(
    state: State<ProjectState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );

    log_app::info!(
        "[command.rename_subgraph] graph={}, new_name={}",
        id, name
    );

    let mut data = state.project_data.write().unwrap();
    let graph = data
        .graphs
        .get_mut(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", id))?;

    graph.name = name;
    Ok(())
}
