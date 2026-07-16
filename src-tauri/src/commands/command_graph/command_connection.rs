use crate::error::AppError;
use crate::event::{Event, EventConnection, emit_project_event};
use crate::execution::ResultSourceStore;
use crate::graph::PinId;
use crate::log::log_app;
use crate::project::{GraphResourcePath, ProjectState, emit_graph_pin_mutation_sync};
use crate::schema::GraphUndoPatch;
use serde::Serialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

fn parse_graph_path(graph_path: &str) -> Result<GraphResourcePath, AppError> {
    GraphResourcePath::new(graph_path).map_err(AppError::from)
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
) -> Result<ConnectPinsResult, AppError> {
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
) -> Result<DisconnectPinResult, AppError> {
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
) -> Result<(), AppError> {
    let graph_path = parse_graph_path(&graph_path)?;

    // connectionId 格式："{from_pin_uuid}->{to_pin_uuid}"
    let parts: Vec<&str> = connection_id.split("->").collect();
    if parts.len() != 2 {
        return Err(AppError::new(
            "invalid_connection_id",
            format!(
                "Invalid connection_id format: '{}', expected 'from->to'",
                connection_id
            ),
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
