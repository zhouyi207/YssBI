use crate::graph::pin::PinDataTypeDefinition;
use crate::graph::value::{DataValue, DataType};
use crate::graph::{GraphId, PinId};
use crate::log::log_app;
use crate::project::ProjectState;
use tauri::State;
use uuid::Uuid;

/// 检查 DataValue 的类型是否与 Pin 期望的 DataType 兼容
fn is_type_compatible(value_type: Option<DataType>, expected: &DataType) -> bool {
    if matches!(expected, DataType::Any) {
        return true;
    }
    let Some(vt) = value_type else {
        return false;
    };
    match expected {
        DataType::Array(inner) => {
            if let DataType::Array(v_inner) = vt {
                is_type_compatible(Some(*v_inner), inner)
            } else {
                false
            }
        }
        DataType::DataSeries(inner) => {
            if let DataType::DataSeries(v_inner) = vt {
                is_type_compatible(Some(*v_inner), inner)
            } else {
                false
            }
        }
        _ => vt == *expected,
    }
}

/// 更新 Pin 的用户输入值
///
/// 前端必须以 DataValue DTO 格式传入（如 {"String": "你好"}），
/// 后端仅反序列化并校验类型是否与 Pin 的 DataType 匹配。
#[tauri::command]
pub fn update_pin_user_value(
    state: State<ProjectState>,
    subgraph_id: String,
    pin_id: String,
    value: DataValue,
) -> Result<(), String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid subgraph_id: {}", e))?,
    );
    let pin_uuid = PinId::from(
        Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?,
    );

    let bounding = state.project_data.read().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let pin = graph
        .get_pin_instance_by_pin_id(pin_uuid)
        .ok_or_else(|| format!("Pin '{}' not found", pin_id))?;

    let expected_type = pin
        .definition
        .data_type
        .as_ref()
        .and_then(|dt| match dt {
            PinDataTypeDefinition::Concrete(t) => Some(t.clone()),
            _ => None,
        });

    if let Some(ref expected) = expected_type {
        let value_type = value.value_type();
        if !is_type_compatible(value_type.clone(), expected) {
            return Err(format!(
                "Type mismatch: pin expects {:?}, got {:?}",
                expected,
                value_type
            ));
        }
    }

    log_app::info!(
        "[command.update_pin_user_value] graph={}, pin={}, value={:?}",
        subgraph_id, pin_id, value
    );

    graph.set_pin_user_value_by_pin_id(pin_uuid, value)?;
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
