use crate::commands::command_graph::command_connection::emit_pin_change_events;
use crate::graph::pin::PinDataTypeDefinition;
use crate::graph::value::{DataType, DataValue};
use crate::graph::{GraphId, NodeId, PinId};
use crate::log::log_app;
use crate::project::ProjectState;
use crate::schema::{GraphInstanceDTO, PinInstanceDTO};
use serde::Serialize;
use tauri::{AppHandle, State};
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
    let pin_uuid =
        PinId::from(Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?);

    let bounding = state.project_data.read().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let pin = graph
        .get_pin_instance_by_pin_id(pin_uuid)
        .ok_or_else(|| format!("Pin '{}' not found", pin_id))?;

    let expected_type = pin.definition.data_type.as_ref().and_then(|dt| match dt {
        PinDataTypeDefinition::Concrete(t) => Some(t.clone()),
        _ => None,
    });

    if let Some(ref expected) = expected_type {
        let value_type = value.value_type();
        if !is_type_compatible(value_type.clone(), expected) {
            return Err(format!(
                "Type mismatch: pin expects {:?}, got {:?}",
                expected, value_type
            ));
        }
    }

    log_app::info!(
        "[command.update_pin_user_value] graph={}, pin={}, value={:?}",
        subgraph_id,
        pin_id,
        value
    );

    graph.set_pin_user_value_by_pin_id(pin_uuid, value)?;
    drop(bounding);
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
    let pin = PinId::from(Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?);

    log_app::info!(
        "[command.clear_pin_user_value] graph={}, pin={}",
        subgraph_id,
        pin_id
    );

    let bounding = state.project_data.read().unwrap();
    let graph = bounding
        .graphs
        .get(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    graph.clear_pin_user_value_by_pin_id(pin)?;
    drop(bounding);
    Ok(())
}

// ==================== Repeatable Pin 管理 ====================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRepeatablePinResult {
    pub pin_id: String,
    pub pin: PinInstanceDTO,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRepeatablePinResult {
    pub removed_pin_id: String,
    pub slot_index: usize,
    pub pin_index: usize,
    pub removed_connections: Vec<(String, String)>,
}

/// 向节点的 Repeatable 槽位追加一个新 pin
///
/// 前端调用 `PinService.addRepeatablePin(subgraphId, nodeId, slotIndex)`
#[tauri::command]
pub fn add_repeatable_pin(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    node_id: String,
    slot_index: usize,
) -> Result<AddRepeatablePinResult, String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid subgraph_id: {}", e))?,
    );
    let nid =
        NodeId::from(Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?);

    log_app::info!(
        "[command.add_repeatable_pin] graph={}, node={}, slot={}",
        subgraph_id,
        node_id,
        slot_index
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (change_set, resolve_sets) = graph.add_repeatable_pin(nid, slot_index)?;

    let added_pin = change_set
        .added_pins
        .first()
        .ok_or_else(|| "No pin was created".to_string())?;

    let resolved_type = graph.get_pin_data_type_by_pin_id(added_pin.id);
    let pin_dto =
        PinInstanceDTO::from_pin_with_context(added_pin, resolved_type.as_ref(), Vec::new());
    let pin_id_str = added_pin.id.to_string();

    let mut all_sets = vec![change_set];
    all_sets.extend(resolve_sets);
    emit_pin_change_events(&app, graph_id, &graph, all_sets);

    Ok(AddRepeatablePinResult {
        pin_id: pin_id_str,
        pin: pin_dto,
    })
}

/// 从节点移除一个 Repeatable 槽位的 pin
///
/// 前端调用 `PinService.removeRepeatablePin(subgraphId, nodeId, pinId)`
#[tauri::command]
pub fn remove_repeatable_pin(
    app: AppHandle,
    state: State<ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
) -> Result<RemoveRepeatablePinResult, String> {
    let graph_id = GraphId::from(
        Uuid::parse_str(&subgraph_id).map_err(|e| format!("Invalid subgraph_id: {}", e))?,
    );
    let nid =
        NodeId::from(Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?);
    let pid = PinId::from(Uuid::parse_str(&pin_id).map_err(|e| format!("Invalid pin_id: {}", e))?);

    log_app::info!(
        "[command.remove_repeatable_pin] graph={}, node={}, pin={}",
        subgraph_id,
        node_id,
        pin_id
    );

    let graph = state
        .get_graph(&graph_id)
        .ok_or_else(|| format!("Graph '{}' not found", subgraph_id))?;

    let (change_set, pin_index, resolve_sets) = graph.remove_repeatable_pin(nid, pid)?;

    let slot_index = {
        let node = graph
            .get_node_instance(nid)
            .ok_or_else(|| "Node not found after remove".to_string())?;
        node.definition
            .pin_slots
            .iter()
            .position(|s| s.repeatable_template_role().is_some())
            .unwrap_or(0)
    };

    let removed_conns: Vec<(String, String)> = change_set
        .removed_connections
        .iter()
        .map(|(f, t)| (f.to_string(), t.to_string()))
        .collect();

    let mut all_sets = vec![change_set];
    all_sets.extend(resolve_sets);
    emit_pin_change_events(&app, graph_id, &graph, all_sets);

    Ok(RemoveRepeatablePinResult {
        removed_pin_id: pin_id,
        slot_index,
        pin_index,
        removed_connections: removed_conns,
    })
}

/// 打开图 Tab 时物化 schema 派生 pin（DESIGN_RULE §3.7）
///
/// 返回完整 Graph DTO；前端以 DTO 为准灌入 store，不在此 emit pin 事件（避免与 addGraphFromData 竞态）。
#[tauri::command]
pub fn resolve_graph_dynamic_pins(
    _app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
) -> Result<GraphInstanceDTO, String> {
    if state.get_graph(&graph_id).is_none() {
        state.load_graph_from_current_project(&graph_id)?;
    }

    log_app::info!("[command.resolve_graph_dynamic_pins] graph={}", graph_id);

    let (graph, _change_sets, _inferred) = state.resolve_graph_dynamic_pins(&graph_id)?;

    Ok((&graph).into())
}
