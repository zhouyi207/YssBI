use crate::event::event_node::InferredPinType;
use crate::event::{Event, EventConnection, EventNode, emit_project_event};
use crate::graph::{DataType, GraphId, NodeId, PinChangeSet, PinId};
use crate::log::log_app;
use crate::project::{GraphDocumentKind, ProjectState, read_project_index};
use crate::schema::PinInstanceDTO;
use crate::schema::pin::{data_type_to_container, data_type_to_pin_type};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use uuid::Uuid;

/// 将 PinChangeSet 转为事件并发送
pub fn emit_pin_change_events(
    app: &AppHandle,
    graph_id: GraphId,
    graph: &crate::graph::GraphInstance,
    change_sets: Vec<PinChangeSet>,
) {
    for cs in change_sets {
        let added_dtos: Vec<PinInstanceDTO> = cs
            .added_pins
            .iter()
            .map(|pin| {
                let resolved_type = graph.get_pin_data_type_by_pin_id(pin.id);
                PinInstanceDTO::from_pin_with_context(pin, resolved_type.as_ref())
            })
            .collect();

        let updated_dtos: Vec<PinInstanceDTO> = cs
            .updated_pins
            .iter()
            .map(|pin| {
                let resolved_type = graph.get_pin_data_type_by_pin_id(pin.id);
                PinInstanceDTO::from_pin_with_context(pin, resolved_type.as_ref())
            })
            .collect();

        let removed_pin_ids: Vec<PinId> = cs.removed_pin_ids;

        let pin_order = graph
            .get_node_instance(cs.node_id)
            .map(|node| node.pin_ids.clone());

        emit_project_event(
            app,
            Event::Node(EventNode::NodePinsUpdated {
                graph_id,
                node_id: cs.node_id,
                removed_pin_ids,
                added_pins: added_dtos,
                updated_pins: updated_dtos,
                removed_connections: cs.removed_connections,
                pin_order,
            }),
        );
    }
}

/// 将推断出的 pin 类型转为事件并发送
pub fn emit_inferred_types(app: &AppHandle, graph_id: GraphId, inferred: Vec<(PinId, DataType)>) {
    if inferred.is_empty() {
        return;
    }
    let pin_types: Vec<InferredPinType> = inferred
        .into_iter()
        .map(|(pin_id, dt)| InferredPinType {
            pin_id,
            pin_type: data_type_to_pin_type(&dt).to_string(),
            container_type: data_type_to_container(&dt).map(|s| s.to_string()),
            type_display: Some(dt.to_string()),
            data_type: Some(dt.clone()),
        })
        .collect();
    emit_project_event(
        app,
        Event::Node(EventNode::PinTypesInferred {
            graph_id,
            pin_types,
        }),
    );
}

// ==================== 结果 DTO ====================

/// ConnectPins 返回值，供前端 Command 系统使用
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectPinsResult {
    pub from_pin: String,
    pub to_pin: String,
    pub auto_disconnected_from: Option<String>,
    pub auto_disconnected_to: Option<String>,
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

// ==================== 核心连接命令 ====================

/// 连接两个 Pin（无序，后端自动验证方向和兼容性）
///
/// 返回 ConnectPinsResult，包含实际连接方向和被自动断开的旧连接。
/// 前端 Command 系统使用此结果构建 undo 上下文。
#[tauri::command]
pub fn connect_pins(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    pin_a: String,
    pin_b: String,
) -> Result<ConnectPinsResult, String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );
    let id_a = PinId::from(Uuid::parse_str(&pin_a).map_err(|e| format!("Invalid pin_a: {}", e))?);
    let id_b = PinId::from(Uuid::parse_str(&pin_b).map_err(|e| format!("Invalid pin_b: {}", e))?);

    log_app::info!(
        "[command.connect_pins] graph={}, a={}, b={}",
        subgraph_id,
        pin_a,
        pin_b
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (from_pin, to_pin, auto_disconnected_list, change_sets, inferred) =
        graph.connect(id_a, id_b)?;

    for (old_from, old_to) in &auto_disconnected_list {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionDeleted {
                graph_id,
                from_pin: *old_from,
                to_pin: *old_to,
            }),
        );
    }

    emit_project_event(
        &app,
        Event::Connection(EventConnection::ConnectionCreated {
            graph_id,
            from_pin,
            to_pin,
        }),
    );

    emit_pin_change_events(&app, graph_id, &graph, change_sets);
    emit_inferred_types(&app, graph_id, inferred);

    let (ad_from, ad_to) = auto_disconnected_list
        .first()
        .map(|(f, t)| (Some(f.to_string()), Some(t.to_string())))
        .unwrap_or((None, None));
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
        auto_disconnected_from: ad_from,
        auto_disconnected_to: ad_to,
        auto_disconnected,
    })
}

/// 断开指定 Pin 的所有连接（前端 Alt+Click 调用 disconnect_pin）
///
/// 返回被断开的连接列表，供前端 Command 系统使用。
#[tauri::command]
pub fn disconnect_pin(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    pin_id: String,
) -> Result<Vec<RemovedConnection>, String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid graph_id: {}", e))?,
    );
    let pin = PinId::from(Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?);

    log_app::info!(
        "[command.disconnect_pin] graph={}, pin={}",
        subgraph_id,
        pin_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (removed_connections, change_sets, inferred) = graph.disconnect_pin(pin);

    let result: Vec<RemovedConnection> = removed_connections
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
                graph_id,
                removed_connections,
            }),
        );
    }

    emit_pin_change_events(&app, graph_id, &graph, change_sets);
    emit_inferred_types(&app, graph_id, inferred);
    Ok(result)
}

// ==================== 其他连接命令 ====================

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
        subgraph_id,
        connection_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (change_sets, inferred) = graph.disconnect(from_pin, to_pin);

    emit_project_event(
        &app,
        Event::Connection(EventConnection::ConnectionDeleted {
            graph_id,
            from_pin,
            to_pin,
        }),
    );

    emit_pin_change_events(&app, graph_id, &graph, change_sets);
    emit_inferred_types(&app, graph_id, inferred);
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
    let pin = PinId::from(Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?);

    log_app::info!(
        "[command.delete_connections_for_pin] graph={}, pin={}",
        subgraph_id,
        pin_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (removed_connections, change_sets, inferred) = graph.disconnect_pin(pin);

    let removed_ids: Vec<String> = removed_connections
        .iter()
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
    emit_inferred_types(&app, graph_id, inferred);

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
    let nid =
        NodeId::from(Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?);

    log_app::info!(
        "[command.delete_connections_for_node] graph={}, node={}",
        subgraph_id,
        node_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let pin_instances = graph.get_pin_instances_by_node_id(nid);
    let mut all_removed_connections = Vec::new();
    let mut removed_ids = Vec::new();

    let mut all_inferred = Vec::new();
    for pin in &pin_instances {
        let (removed_connections, change_sets, inferred) = graph.disconnect_pin(pin.id);
        for (from, to) in &removed_connections {
            removed_ids.push(format!("{}->{}", from, to));
        }
        all_removed_connections.extend(removed_connections);
        all_inferred.extend(inferred);
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

    emit_inferred_types(&app, graph_id, all_inferred);

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

    log_app::debug!("[command.update_canvas] graph={}", subgraph_id);

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
    drop(data);

    Ok(())
}

/// 更新子图的输入输出定义（用于 Function 的参数定义）
#[tauri::command]
pub fn update_subgraph_io(
    _state: State<ProjectState>,
    _id: String,
    _data: Value,
) -> Result<(), String> {
    // TODO: Function 子图的 IO 定义尚未设计
    log_app::info!("[command.update_subgraph_io] Not yet implemented");
    Ok(())
}

/// 重命名子图
#[tauri::command]
pub fn rename_subgraph(state: State<ProjectState>, id: String, name: String) -> Result<(), String> {
    let graph_id =
        GraphId::from(Uuid::parse_str(&id).map_err(|e| format!("Invalid graph_id: {}", e))?);

    log_app::info!("[command.rename_subgraph] graph={}, new_name={}", id, name);

    if state.get_graph(&graph_id).is_none() {
        state.load_graph_from_current_project(&graph_id)?;
    }

    let graph_kind = state
        .project_data
        .read()
        .unwrap()
        .graphs
        .get(&graph_id)
        .map(|graph| graph.kind.clone())
        .ok_or_else(|| format!("Graph '{}' not found", id))?;

    let mut existing: Vec<String> = state
        .project_data
        .read()
        .unwrap()
        .graphs
        .values()
        .filter(|item| item.kind == graph_kind && item.id != graph_id)
        .map(|item| item.name.clone())
        .collect();
    if let Some(path) = state.get_path() {
        let expected_kind = GraphDocumentKind::from(&graph_kind);
        existing.extend(
            read_project_index(&path)
                .map_err(|e| e.to_string())?
                .graphs
                .into_iter()
                .filter(|item| item.graph_type == expected_kind && item.id != graph_id)
                .map(|item| item.name),
        );
    }
    existing.sort();
    existing.dedup();

    let mut data = state.project_data.write().unwrap();
    let graph = data
        .graphs
        .get_mut(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", id))?;
    graph.name = crate::project::unique_name::unique_name(&name, existing);
    drop(data);
    Ok(())
}
