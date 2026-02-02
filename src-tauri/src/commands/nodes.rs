//! 节点管理相关命令

use crate::project::{CanvasState, PinDefDto, SerializedNode, SubGraphData};
use crate::state::{emit_project_event, ProjectEvent, ProjectState};
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;

/// 获取子图的节点列表
#[tauri::command]
pub fn get_nodes(
    state: State<'_, ProjectState>,
    subgraph_id: String,
) -> Result<Vec<SerializedNode>, String> {
    state.get_nodes(&subgraph_id)
}

/// 设置子图的节点列表
#[tauri::command]
pub fn set_nodes(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    nodes: Vec<SerializedNode>,
) -> Result<(), String> {
    state.set_nodes(&subgraph_id, nodes.clone())?;
    emit_project_event(&app, ProjectEvent::NodesUpdated { subgraph_id, nodes });
    Ok(())
}

/// 创建单个节点
#[tauri::command]
pub fn create_node(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node: SerializedNode,
) -> Result<SerializedNode, String> {
    info!(
        "[create_node] subgraph_id={}, node_id={}, node_type={}",
        subgraph_id, node.id, node.node_type
    );
    let result = state.create_node(&subgraph_id, node)?;

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(result)
}

/// 删除单个节点
#[tauri::command]
pub fn delete_node(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
) -> Result<(), String> {
    info!(
        "[delete_node] subgraph_id={}, node_id={}",
        subgraph_id, node_id
    );
    state.delete_node(&subgraph_id, &node_id)?;

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(())
}

/// 批量创建节点
#[tauri::command]
pub fn create_nodes(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    nodes: Vec<SerializedNode>,
) -> Result<Vec<SerializedNode>, String> {
    info!(
        "[create_nodes] subgraph_id={}, count={}",
        subgraph_id,
        nodes.len()
    );
    let new_nodes = state.create_nodes(&subgraph_id, nodes)?;

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(new_nodes)
}

/// 连接两个 Pin
#[tauri::command]
pub fn connect_pins(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    source_pin_id: String,
    target_pin_id: String,
) -> Result<Vec<SerializedNode>, String> {
    info!(
        "[connect_pins] subgraph_id={}, source={}, target={}",
        subgraph_id, source_pin_id, target_pin_id
    );
    let nodes = state.connect_pins(&subgraph_id, &source_pin_id, &target_pin_id)?;

    // 发送节点更新事件
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id: subgraph_id.clone(),
            nodes: nodes.clone(),
        },
    );

    info!("[connect_pins] Connection successful");
    Ok(nodes)
}

/// 断开 Pin 的所有连接
#[tauri::command]
pub fn disconnect_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    pin_id: String,
) -> Result<Vec<SerializedNode>, String> {
    info!(
        "[disconnect_pin] subgraph_id={}, pin_id={}",
        subgraph_id, pin_id
    );
    let nodes = state.disconnect_pin(&subgraph_id, &pin_id)?;

    // 发送节点更新事件
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id: subgraph_id.clone(),
            nodes: nodes.clone(),
        },
    );

    info!("[disconnect_pin] Disconnection successful");
    Ok(nodes)
}

/// 更新子图的画布状态
#[tauri::command]
pub fn update_canvas(
    state: State<'_, ProjectState>,
    subgraph_id: String,
    canvas: CanvasState,
) -> Result<(), String> {
    state.update_canvas(&subgraph_id, canvas)
}

/// 更新子图的输入输出定义
#[tauri::command]
pub fn update_subgraph_io(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    inputs: Option<Vec<PinDefDto>>,
    outputs: Option<Vec<PinDefDto>>,
) -> Result<SubGraphData, String> {
    state.update_subgraph_io(&subgraph_id, inputs, outputs)?;
    // 返回更新后的子图数据
    let updated = state
        .get_event(&subgraph_id)
        .or_else(|| state.get_function(&subgraph_id))
        .or_else(|| state.get_macro(&subgraph_id))
        .ok_or_else(|| format!("Subgraph '{}' not found after update", subgraph_id))?;

    // 发送相应的更新事件
    if state.get_event(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::EventUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else if state.get_function(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::FunctionUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else {
        emit_project_event(
            &app,
            ProjectEvent::MacroUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    }

    Ok(updated)
}

/// 重命名子图
#[tauri::command]
pub fn rename_subgraph(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    new_name: String,
) -> Result<SubGraphData, String> {
    state.rename_subgraph(&subgraph_id, new_name)?;
    // 返回更新后的子图数据
    let updated = state
        .get_event(&subgraph_id)
        .or_else(|| state.get_function(&subgraph_id))
        .or_else(|| state.get_macro(&subgraph_id))
        .ok_or_else(|| format!("Subgraph '{}' not found after rename", subgraph_id))?;

    // 发送相应的更新事件
    if state.get_event(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::EventUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else if state.get_function(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::FunctionUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else {
        emit_project_event(
            &app,
            ProjectEvent::MacroUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    }

    Ok(updated)
}

// ==================== 动态 Pin 命令 ====================

/// 获取节点的动态 Pin 约束
#[tauri::command]
pub fn get_node_dynamic_constraints(node_type: String) -> Result<serde_json::Value, String> {
    // 这里应该从节点注册表获取节点定义，然后检查其动态能力
    // 为了简化，我们返回一个示例约束
    match node_type.as_str() {
        "sequence_dynamic" => Ok(serde_json::json!({
            "canAddPins": true,
            "dynamicConfigs": [{
                "pinType": "Exec",
                "direction": "Output",
                "nameTemplate": "Then {}",
                "minCount": 2,
                "maxCount": 10,
                "canReorder": true
            }]
        })),
        _ => Ok(serde_json::json!({
            "canAddPins": false,
            "dynamicConfigs": []
        })),
    }
}

/// 为节点添加动态 Pin
#[tauri::command]
pub fn add_node_dynamic_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_config: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use crate::executor::node::implementation::{DynamicPinConfig, DynamicPinType, PinDirection};
    use crate::executor::value::PinTypeDesc;

    info!(
        "[add_node_dynamic_pin] subgraph_id={}, node_id={}",
        subgraph_id, node_id
    );

    // 解析 pin_config
    let pin_type = match pin_config.get("pinType").and_then(|v| v.as_str()) {
        Some("Exec") => DynamicPinType::Exec,
        Some("Data") => DynamicPinType::Data,
        _ => return Err("Invalid pin type".to_string()),
    };

    let direction = match pin_config.get("direction").and_then(|v| v.as_str()) {
        Some("Input") => PinDirection::Input,
        Some("Output") => PinDirection::Output,
        _ => return Err("Invalid pin direction".to_string()),
    };

    let name_template = pin_config
        .get("nameTemplate")
        .and_then(|v| v.as_str())
        .unwrap_or("Pin {}")
        .to_string();

    let config = DynamicPinConfig {
        pin_type,
        direction,
        name_template,
        data_type: PinTypeDesc::unknown(),
        min_count: 0,
        max_count: None,
        can_reorder: true,
    };

    // 这里应该找到对应的节点并添加 Pin
    // 为了简化，我们返回一个成功响应
    let pin_id = uuid::Uuid::new_v4();

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(serde_json::json!({
        "pinId": pin_id.to_string(),
        "name": config.name_template.replace("{}", "2"), // 示例名称
        "type": format!("{:?}", config.pin_type),
        "direction": format!("{:?}", config.direction)
    }))
}

/// 移除节点的动态 Pin
#[tauri::command]
pub fn remove_node_dynamic_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
) -> Result<(), String> {
    info!(
        "[remove_node_dynamic_pin] subgraph_id={}, node_id={}, pin_id={}",
        subgraph_id, node_id, pin_id
    );

    // 这里应该找到对应的节点并移除 Pin
    // 为了简化，我们直接返回成功

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(())
}

/// 验证 Pin 操作是否有效
#[tauri::command]
pub fn validate_pin_operation(
    node_type: String,
    operation: String,
    _pin_config: Option<serde_json::Value>,
) -> Result<bool, String> {
    info!(
        "[validate_pin_operation] node_type={}, operation={}",
        node_type, operation
    );

    match node_type.as_str() {
        "sequence_dynamic" => match operation.as_str() {
            "add" => {
                // 检查是否可以添加更多 Pin
                // 这里应该检查当前 Pin 数量和最大限制
                Ok(true) // 简化实现
            }
            "remove" => {
                // 检查是否可以移除 Pin
                // 这里应该检查当前 Pin 数量和最小限制
                Ok(true) // 简化实现
            }
            _ => Ok(false),
        },
        _ => Ok(false),
    }
}


// ==================== Pin 值管理命令 ====================

/// 更新 Pin 的用户值
#[tauri::command]
pub fn update_pin_user_value(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
    value: serde_json::Value,
) -> Result<(), String> {
    info!(
        "[update_pin_user_value] subgraph_id={}, node_id={}, pin_id={}, value={:?}",
        subgraph_id, node_id, pin_id, value
    );
    
    let mut project = state.data.write().unwrap();
    let subgraph = crate::get_subgraph_mut!(project, &subgraph_id)
        .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
    
    // 找到节点
    let node = subgraph.nodes.iter_mut()
        .find(|n| n.id == node_id)
        .ok_or_else(|| "Node not found".to_string())?;
    
    info!("[update_pin_user_value] Found node: {}", node.node_type);
    
    // 找到 Pin 并更新用户值
    let pin_found = if let Some(pin) = node.inputs.iter_mut().find(|p| p.id == pin_id) {
        info!("[update_pin_user_value] Found input pin: {}, old user_value: {:?}", pin.name, pin.user_value);
        pin.user_value = Some(value.clone());
        info!("[update_pin_user_value] Updated input pin user_value to: {:?}", pin.user_value);
        true
    } else if let Some(pin) = node.outputs.iter_mut().find(|p| p.id == pin_id) {
        info!("[update_pin_user_value] Found output pin: {}, old user_value: {:?}", pin.name, pin.user_value);
        pin.user_value = Some(value.clone());
        info!("[update_pin_user_value] Updated output pin user_value to: {:?}", pin.user_value);
        true
    } else {
        false
    };
    
    if !pin_found {
        info!("[update_pin_user_value] Pin not found! Available pins:");
        for pin in &node.inputs {
            info!("  Input: {} (id: {})", pin.name, pin.id);
        }
        for pin in &node.outputs {
            info!("  Output: {} (id: {})", pin.name, pin.id);
        }
        return Err("Pin not found".to_string());
    }
    
    info!("[update_pin_user_value] Successfully updated pin value");
    
    // 发送节点更新事件
    drop(project); // 释放锁
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );
    
    Ok(())
}

/// 清除 Pin 的用户值（恢复默认值）
#[tauri::command]
pub fn clear_pin_user_value(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
) -> Result<(), String> {
    info!(
        "[clear_pin_user_value] subgraph_id={}, node_id={}, pin_id={}",
        subgraph_id, node_id, pin_id
    );
    
    let mut project = state.data.write().unwrap();
    let subgraph = crate::get_subgraph_mut!(project, &subgraph_id)
        .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
    
    let node = subgraph.nodes.iter_mut()
        .find(|n| n.id == node_id)
        .ok_or_else(|| "Node not found".to_string())?;
    
    let pin_found = if let Some(pin) = node.inputs.iter_mut().find(|p| p.id == pin_id) {
        pin.user_value = None;
        true
    } else if let Some(pin) = node.outputs.iter_mut().find(|p| p.id == pin_id) {
        pin.user_value = None;
        true
    } else {
        false
    };
    
    if !pin_found {
        return Err("Pin not found".to_string());
    }
    
    // 发送节点更新事件
    drop(project);
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );
    
    Ok(())
}

// ==================== 动态 Pin 管理命令 ====================

/// 为节点添加动态 Pin（完整实现）
#[tauri::command]
pub fn add_dynamic_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_type: String,      // "data" 或 "exec"
    direction: String,     // "input" 或 "output"
) -> Result<serde_json::Value, String> {
    use crate::executor::node::implementation::PinDirection;
    use crate::project::{SerializedPin, DynamicPinMetadata};
    
    info!(
        "[add_dynamic_pin] subgraph_id={}, node_id={}, pin_type={}, direction={}",
        subgraph_id, node_id, pin_type, direction
    );
    
    let mut project = state.data.write().unwrap();
    let subgraph = crate::get_subgraph_mut!(project, &subgraph_id)
        .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
    
    // 找到节点
    let node = subgraph.nodes.iter_mut()
        .find(|n| n.id == node_id)
        .ok_or_else(|| "Node not found".to_string())?;
    
    // 从注册表获取原型
    let registry = crate::executor::node::registry::get_registry();
    let prototype = registry.get_node(&node.node_type)
        .ok_or_else(|| format!("Node type '{}' not found in registry", node.node_type))?;
    
    // 解析参数
    let pin_direction = match direction.as_str() {
        "input" => PinDirection::Input,
        "output" => PinDirection::Output,
        _ => return Err("Invalid direction, must be 'input' or 'output'".to_string()),
    };
    
    // 获取动态配置
    let config = prototype.get_dynamic_constraints(&pin_type, &pin_direction)
        .ok_or("Node does not support dynamic pins for this type/direction")?;
    
    // 验证是否可以添加
    let current_count = match (&pin_direction, pin_type.as_str()) {
        (PinDirection::Input, "data") => node.inputs.iter().filter(|p| p.pin_type != "exec").count(),
        (PinDirection::Output, "data") => node.outputs.iter().filter(|p| p.pin_type != "exec").count(),
        (PinDirection::Input, "exec") => node.inputs.iter().filter(|p| p.pin_type == "exec").count(),
        (PinDirection::Output, "exec") => node.outputs.iter().filter(|p| p.pin_type == "exec").count(),
        _ => 0,
    };
    
    if let Some(max) = config.max_count {
        if current_count >= max {
            return Err(format!("Cannot add more pins: current={}, max={}", current_count, max));
        }
    }
    
    // 生成新 Pin
    let pin_id = uuid::Uuid::new_v4();
    let pin_name = config.name_template.replace("{}", &(current_count + 1).to_string());
    
    let new_pin = SerializedPin {
        id: pin_id.to_string(),
        name: pin_name.clone(),
        pin_type: config.data_type.type_string(),
        links: vec![],
        default_value: None,
        user_value: None,
        is_array: false,
    };
    
    // 添加到节点
    match pin_direction {
        PinDirection::Input => node.inputs.push(new_pin),
        PinDirection::Output => node.outputs.push(new_pin),
    }
    
    // 记录动态 Pin 元数据
    let metadata = DynamicPinMetadata {
        pin_id: pin_id.to_string(),
        pin_type: format!("{:?}", config.pin_type),
        direction: format!("{:?}", pin_direction),
        name: pin_name.clone(),
        data_type: config.data_type.type_string(),
        is_dynamic: true,
    };
    
    if node.dynamic_pins.is_none() {
        node.dynamic_pins = Some(vec![]);
    }
    node.dynamic_pins.as_mut().unwrap().push(metadata);
    
    // 发送节点更新事件
    drop(project);
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );
    
    // 返回结果
    Ok(serde_json::json!({
        "pinId": pin_id.to_string(),
        "name": pin_name,
        "type": format!("{:?}", config.pin_type),
        "direction": format!("{:?}", pin_direction)
    }))
}

/// 移除节点的动态 Pin（完整实现）
#[tauri::command]
pub fn remove_dynamic_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
) -> Result<(), String> {
    
    info!(
        "[remove_dynamic_pin] subgraph_id={}, node_id={}, pin_id={}",
        subgraph_id, node_id, pin_id
    );
    
    let mut project = state.data.write().unwrap();
    let subgraph = crate::get_subgraph_mut!(project, &subgraph_id)
        .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
    
    let node = subgraph.nodes.iter_mut()
        .find(|n| n.id == node_id)
        .ok_or_else(|| "Node not found".to_string())?;
    
    // 检查是否是动态 Pin
    let is_dynamic = node.dynamic_pins.as_ref()
        .and_then(|pins| pins.iter().find(|p| p.pin_id == pin_id))
        .is_some();
    
    if !is_dynamic {
        return Err("Cannot remove static pin, only dynamic pins can be removed".to_string());
    }
    
    // 从注册表获取原型以验证约束
    let registry = crate::executor::node::registry::get_registry();
    if let Some(prototype) = registry.get_node(&node.node_type) {
        // 获取 Pin 的元数据
        if let Some(dynamic_pins) = &node.dynamic_pins {
            if let Some(pin_meta) = dynamic_pins.iter().find(|p| p.pin_id == pin_id) {
                // 验证最小数量约束
                let pin_type_str = match pin_meta.pin_type.as_str() {
                    "Exec" => "exec",
                    "Data" => "data",
                    _ => "data",
                };
                
                let pin_direction = match pin_meta.direction.as_str() {
                    "Input" => crate::executor::node::implementation::PinDirection::Input,
                    "Output" => crate::executor::node::implementation::PinDirection::Output,
                    _ => crate::executor::node::implementation::PinDirection::Input,
                };
                
                if let Some(config) = prototype.get_dynamic_constraints(pin_type_str, &pin_direction) {
                    let current_count = match (&pin_direction, pin_type_str) {
                        (crate::executor::node::implementation::PinDirection::Input, "data") => 
                            node.inputs.iter().filter(|p| p.pin_type != "exec").count(),
                        (crate::executor::node::implementation::PinDirection::Output, "data") => 
                            node.outputs.iter().filter(|p| p.pin_type != "exec").count(),
                        (crate::executor::node::implementation::PinDirection::Input, "exec") => 
                            node.inputs.iter().filter(|p| p.pin_type == "exec").count(),
                        (crate::executor::node::implementation::PinDirection::Output, "exec") => 
                            node.outputs.iter().filter(|p| p.pin_type == "exec").count(),
                        _ => 0,
                    };
                    
                    if current_count <= config.min_count {
                        return Err(format!(
                            "Cannot remove pin: current={}, min={}",
                            current_count, config.min_count
                        ));
                    }
                }
            }
        }
    }
    
    // 移除 Pin
    let removed_from_inputs = node.inputs.iter().position(|p| p.id == pin_id)
        .map(|idx| node.inputs.remove(idx))
        .is_some();
    
    let removed_from_outputs = if !removed_from_inputs {
        node.outputs.iter().position(|p| p.id == pin_id)
            .map(|idx| node.outputs.remove(idx))
            .is_some()
    } else {
        false
    };
    
    if !removed_from_inputs && !removed_from_outputs {
        return Err("Pin not found in node".to_string());
    }
    
    // 从动态 Pin 元数据中移除
    if let Some(dynamic_pins) = &mut node.dynamic_pins {
        dynamic_pins.retain(|p| p.pin_id != pin_id);
    }
    
    // 发送节点更新事件
    drop(project);
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );
    
    Ok(())
}
