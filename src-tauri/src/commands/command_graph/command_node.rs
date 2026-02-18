use crate::graph::{GraphId, NodeId, NodeInstanceParams};
use crate::project::ProjectState;
use crate::event::{emit_project_event, Event, EventNode};
use crate::schema::{NodeInstanceDTO, PinInstanceDTO};
use crate::log::log_app;
use serde::Deserialize;
use tauri::{AppHandle, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePositionUpdate {
    node_id: NodeId,
    x: f32,
    y: f32,
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
) -> Result<String, String> {
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
    let mut pins_dto = Vec::with_capacity(pin_instances.len());
    for pin in &pin_instances {
        match pin.definition.direction {
            crate::graph::PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
            crate::graph::PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
        }
        pins_dto.push(PinInstanceDTO::from(pin));
    }

    // 发送节点创建事件（含 pins，便于前端 hydrate）
    emit_project_event(
        &app,
        Event::Node(EventNode::NodeCreated {
            graph_id,
            node_id,
            data: node_dto,
            pins: pins_dto,
        }),
    );
    
    // 返回节点 ID
    Ok(node_id.to_string())
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
