use crate::event::{Event, EventConnection, emit_project_event};
use crate::execution::ResultSourceStore;
use crate::graph::{NodeId, PinId};
use crate::log::log_app;
use crate::project::{GraphResourcePath, ProjectState, emit_graph_pin_mutation_sync};
use crate::schema::GraphUndoPatch;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

fn parse_graph_path(graph_path: &str) -> Result<GraphResourcePath, String> {
    GraphResourcePath::new(graph_path).map_err(|e| e.to_string())
}

// ==================== 结果 DTO ====================

/// ConnectPins 返回值，供前端 Command 系统使用
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectPinsResult {
    pub from_pin: String,
    pub to_pin: String,
    pub auto_disconnected: Vec<AutoDisconnected>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDisconnected {
    pub from_pin: String,
    pub to_pin: String,
}

/// DisconnectPin 返回值
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedConnection {
    pub from_pin: String,
    pub to_pin: String,
}

/// DisconnectPin 命令返回值（含 undo 闭包快照）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectPinResult {
    pub removed_connections: Vec<RemovedConnection>,
    pub undo_patch: GraphUndoPatch,
}

// ==================== 核心连接命令 ====================

/// 连接两个 Pin（无序，后端自动验证方向和兼容性）
///
/// 返回 ConnectPinsResult，包含实际连接方向和被自动断开的旧连接。
/// 前端 Command 系统使用此结果构建 undo 上下文。
#[tauri::command]
pub fn connect_pins(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: String,
    pin_a: String,
    pin_b: String,
) -> Result<ConnectPinsResult, String> {
    let graph_path = parse_graph_path(&graph_path)?;
    let id_a = PinId::from(Uuid::parse_str(&pin_a).map_err(|e| format!("Invalid pin_a: {}", e))?);
    let id_b = PinId::from(Uuid::parse_str(&pin_b).map_err(|e| format!("Invalid pin_b: {}", e))?);

    log_app::info!(
        "[command.connect_pins] graph={}, a={}, b={}",
        graph_path,
        pin_a,
        pin_b
    );

    let graph = state
        .get_graph(&graph_path)
        .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

    let (from_pin, to_pin, auto_disconnected_list, change_sets, inferred) =
        graph.connect(id_a, id_b)?;

    for (old_from, old_to) in &auto_disconnected_list {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionDeleted {
                graph_path: graph_path.as_str().to_string(),
                from_pin: *old_from,
                to_pin: *old_to,
            }),
        );
    }

    emit_project_event(
        &app,
        Event::Connection(EventConnection::ConnectionCreated {
            graph_path: graph_path.as_str().to_string(),
            from_pin,
            to_pin,
        }),
    );

    emit_graph_pin_mutation_sync(
        &app,
        &source_store,
        &graph_path,
        &graph,
        &change_sets,
        inferred,
        &[],
    );

    let auto_disconnected = auto_disconnected_list
        .iter()
        .map(|(f, t)| AutoDisconnected {
            from_pin: f.to_string(),
            to_pin: t.to_string(),
        })
        .collect();
    Ok(ConnectPinsResult {
        from_pin: from_pin.to_string(),
        to_pin: to_pin.to_string(),
        auto_disconnected,
    })
}

/// 断开指定 Pin 的所有连接（前端 Alt+Click 调用 disconnect_pin）
///
/// 返回被断开的连接列表及 undo 闭包快照，供前端 Command 系统使用。
#[tauri::command]
pub fn disconnect_pin(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: String,
    pin_id: String,
) -> Result<DisconnectPinResult, String> {
    let graph_path = parse_graph_path(&graph_path)?;
    let pin = PinId::from(Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?);

    log_app::info!(
        "[command.disconnect_pin] graph={}, pin={}",
        graph_path,
        pin_id
    );

    let graph = state
        .get_graph(&graph_path)
        .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

    let (removed_connections, undo_patch, change_sets, inferred) = graph.disconnect_pin(pin);

    let removed = removed_connections
        .iter()
        .map(|(f, t)| RemovedConnection {
            from_pin: f.to_string(),
            to_pin: t.to_string(),
        })
        .collect();

    if !removed_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchDeleted {
                graph_path: graph_path.as_str().to_string(),
                removed_connections,
            }),
        );
    }

    emit_graph_pin_mutation_sync(
        &app,
        &source_store,
        &graph_path,
        &graph,
        &change_sets,
        inferred,
        &[],
    );
    Ok(DisconnectPinResult {
        removed_connections: removed,
        undo_patch,
    })
}

// ==================== 其他连接命令 ====================

/// 删除连接（按 from->to 格式的 connectionId 断开）
///
/// 前端 ConnectionService.deleteConnection(graphPath, connectionId)
#[tauri::command]
pub fn delete_connection(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: String,
    connection_id: String,
) -> Result<(), String> {
    let graph_path = parse_graph_path(&graph_path)?;

    // connectionId 格式："{from_pin_uuid}->{to_pin_uuid}"
    let parts: Vec<&str> = connection_id.split("->").collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid connection_id format: '{}', expected 'from->to'",
            connection_id
        ));
    }
    let from_pin = PinId::from(
        Uuid::parse_str(parts[0])
            .map_err(|e| format!("Invalid from_pin in connection_id: {}", e))?,
    );
    let to_pin = PinId::from(
        Uuid::parse_str(parts[1]).map_err(|e| format!("Invalid to_pin in connection_id: {}", e))?,
    );

    log_app::info!(
        "[command.delete_connection] graph={}, connection={}",
        graph_path,
        connection_id
    );

    let graph = state
        .get_graph(&graph_path)
        .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

    let (change_sets, inferred) = graph.disconnect(from_pin, to_pin);

    emit_project_event(
        &app,
        Event::Connection(EventConnection::ConnectionDeleted {
            graph_path: graph_path.as_str().to_string(),
            from_pin,
            to_pin,
        }),
    );

    emit_graph_pin_mutation_sync(
        &app,
        &source_store,
        &graph_path,
        &graph,
        &change_sets,
        inferred,
        &[],
    );
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
/// 前端 ConnectionService.getConnections(graphPath)
#[tauri::command]
pub fn get_connections(
    state: State<ProjectState>,
    graph_path: String,
) -> Result<Vec<ConnectionDTO>, String> {
    let graph_path = parse_graph_path(&graph_path)?;

    let graph = state
        .get_graph(&graph_path)
        .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

    let connections: Vec<ConnectionDTO> = graph
        .all_connections()
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
/// 前端 ConnectionService.deleteConnectionsForPin(graphPath, pinId)
#[tauri::command]
pub fn delete_connections_for_pin(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: String,
    pin_id: String,
) -> Result<Vec<String>, String> {
    let graph_path = parse_graph_path(&graph_path)?;
    let pin = PinId::from(Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?);

    log_app::info!(
        "[command.delete_connections_for_pin] graph={}, pin={}",
        graph_path,
        pin_id
    );

    let graph = state
        .get_graph(&graph_path)
        .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

    let (removed_connections, _, change_sets, inferred) = graph.disconnect_pin(pin);

    let removed_ids: Vec<String> = removed_connections
        .iter()
        .map(|(from, to)| format!("{}->{}", from, to))
        .collect();

    if !removed_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchDeleted {
                graph_path: graph_path.as_str().to_string(),
                removed_connections,
            }),
        );
    }

    emit_graph_pin_mutation_sync(
        &app,
        &source_store,
        &graph_path,
        &graph,
        &change_sets,
        inferred,
        &[],
    );

    Ok(removed_ids)
}

/// 删除节点的所有连接
///
/// 前端 ConnectionService.deleteConnectionsForNode(graphPath, nodeId)
#[tauri::command]
pub fn delete_connections_for_node(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: String,
    node_id: String,
) -> Result<Vec<String>, String> {
    let graph_path = parse_graph_path(&graph_path)?;
    let nid =
        NodeId::from(Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?);

    log_app::info!(
        "[command.delete_connections_for_node] graph={}, node={}",
        graph_path,
        node_id
    );

    let graph = state
        .get_graph(&graph_path)
        .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

    let pin_instances = graph.get_pin_instances_by_node_id(nid);
    let mut all_removed_connections = Vec::new();
    let mut removed_ids = Vec::new();

    let mut all_inferred = Vec::new();
    let mut all_change_sets = Vec::new();
    for pin in &pin_instances {
        let (removed_connections, _, change_sets, inferred) = graph.disconnect_pin(pin.id);
        for (from, to) in &removed_connections {
            removed_ids.push(format!("{}->{}", from, to));
        }
        all_removed_connections.extend(removed_connections);
        all_inferred.extend(inferred);
        all_change_sets.extend(change_sets);
    }

    if !all_removed_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchDeleted {
                graph_path: graph_path.as_str().to_string(),
                removed_connections: all_removed_connections,
            }),
        );
    }

    emit_graph_pin_mutation_sync(
        &app,
        &source_store,
        &graph_path,
        &graph,
        &all_change_sets,
        all_inferred,
        &[],
    );

    Ok(removed_ids)
}

// ==================== 子图管理命令 ====================

/// 更新画布视图状态（位置、缩放等）
#[tauri::command]
pub fn update_canvas(
    state: State<ProjectState>,
    graph_path: String,
    canvas: Value,
) -> Result<(), String> {
    let graph_path = parse_graph_path(&graph_path)?;

    log_app::debug!("[command.update_canvas] graph={}", graph_path);

    state.with_graph_mut(&graph_path, |mut ctx| {
        if let Some(x) = canvas.get("x").and_then(|v| v.as_f64()) {
            ctx.graph().position.x = x;
        }
        if let Some(y) = canvas.get("y").and_then(|v| v.as_f64()) {
            ctx.graph().position.y = y;
        }
        if let Some(scale) = canvas.get("scale").and_then(|v| v.as_f64()) {
            ctx.graph().position.scale = scale;
        }
        Ok(())
    })
}
