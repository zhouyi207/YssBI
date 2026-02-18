use crate::graph::{GraphId, PinId};
use crate::graph::value::DataValue;
use crate::project::ProjectState;
use crate::log::log_app;
use tauri::State;
use uuid::Uuid;

/// 更新 Pin 的用户输入值
///
/// 前端 PinService.updatePinUserValue(subgraphId, nodeId, pinId, value)
#[tauri::command]
pub fn update_pin_user_value(
    state: State<ProjectState>,
    subgraph_id: String,
    pin_id: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid subgraph_id: {}", e))?,
    );
    let pin = PinId::from(
        Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?,
    );

    let data_value: DataValue = serde_json::from_value(value.clone())
        .map_err(|e| format!("Invalid value: {}", e))?;

    log_app::info!(
        "[command.update_pin_user_value] graph={}, pin={}, value={:?}",
        subgraph_id, pin_id, data_value
    );

    let bounding = state.project_data.read().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    graph.set_pin_user_value_by_pin_id(pin, data_value)?;
    Ok(())
}

/// 清除 Pin 的用户输入值（恢复为 None，使用默认值或连接值）
///
/// 前端 PinService.clearPinUserValue(subgraphId, nodeId, pinId)
#[tauri::command]
pub fn clear_pin_user_value(
    state: State<ProjectState>,
    subgraph_id: String,
    pin_id: String,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid subgraph_id: {}", e))?,
    );
    let pin = PinId::from(
        Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?,
    );

    log_app::info!(
        "[command.clear_pin_user_value] graph={}, pin={}",
        subgraph_id, pin_id
    );

    let bounding = state.project_data.read().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    graph.clear_pin_user_value_by_pin_id(pin)?;
    Ok(())
}
