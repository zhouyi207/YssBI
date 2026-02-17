use crate::graph::{GraphId, PinId};
use crate::project::ProjectState;
use serde_json::Value;
use tauri::State;
use uuid::Uuid;

/// 断开指定 Pin 的所有连接（前端 Alt+Click 调用 disconnect_pin）
#[tauri::command]
pub fn disconnect_pin(
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

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    graph.disconnect_pin(pin);
    Ok(())
}

/// 连接两个 Pin（前端调用 connect_pins）
#[tauri::command]
pub fn connect_pins(
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

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    graph.connect(from_pin, to_pin)?;
    Ok(())
}

#[tauri::command]
pub fn create_connection(_from: String, _to: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_connection(_from: String, _to: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn get_connections() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn delete_connections_for_pin(_pin: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_connections_for_node(_node: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn update_canvas(_data: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn update_subgraph_io(_id: String, _data: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn rename_subgraph(_id: String, _name: String) -> Result<(), String> {
    Ok(())
}