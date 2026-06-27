use crate::event::{Event, EventNode, EventVariable, InferredPinType, emit_project_event};
use crate::graph::pin::PinKind;
use crate::graph::value::{DataType, DataValue};
use crate::project::ProjectState;
use crate::schema::{VariableInstanceDTO, data_type_to_container, data_type_to_pin_type};
use crate::variable::{VariableId, VariableScope};
use tauri::{AppHandle, State};

/// 创建变量（统一接口，支持全局和局部变量）
#[tauri::command]
pub fn create_variable(
    state: State<ProjectState>,
    app: AppHandle,
    name: &str,
    data_type: DataType,
    data_value: DataValue,
    description: &str,
    scope: VariableScope,
    tags: Vec<String>,
) -> Result<String, String> {
    let variable = state.add_variable(name, data_type, data_value, description, scope, tags);
    let variable_id = variable.id.to_string();
    if matches!(variable.scope, VariableScope::Global) {
        state.persist_current_project()?;
    }
    emit_project_event(
        &app,
        Event::Variable(EventVariable::VariableCreated {
            variable_id: variable.id,
            variable_scope: variable.scope.clone(),
            data: (&variable).into(),
        }),
    );
    Ok(variable_id)
}

/// 获取变量（统一接口）
#[tauri::command]
pub fn get_variable(
    _app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
) -> Result<VariableInstanceDTO, String> {
    let variable = state.get_variable(&variable_id).unwrap();
    Ok((&variable).into())
}

/// 更新变量（统一接口，部分更新）
#[tauri::command]
pub fn update_variable(
    app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
    name: Option<String>,
    data_type: Option<DataType>,
    data_value: Option<DataValue>,
    description: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<(), String> {
    let type_changed = data_type.is_some();
    let name_changed = name.is_some();

    let updated = state
        .update_variable(&variable_id, name, data_type, data_value, description, tags)
        .ok_or_else(|| format!("Variable '{}' not found", variable_id))?;
    let persist_global = matches!(updated.scope, VariableScope::Global);

    emit_project_event(
        &app,
        Event::Variable(EventVariable::VariableUpdated {
            variable_id: updated.id,
            variable_scope: updated.scope.clone(),
            data: (&updated).into(),
        }),
    );

    // 当变量类型或名称改变时，更新所有引用该变量的节点
    if type_changed || name_changed {
        let var_id_str = variable_id.to_string();
        let new_data_type = &updated.data_type;
        let new_name = &updated.name;
        let project_data = state.project_data.read().unwrap();

        for (graph_id, graph) in project_data.graphs.iter() {
            let data_state = graph.data_state.read().unwrap();
            let mut inferred_pins = Vec::new();

            for node in data_state.nodes.values() {
                let refs_this_var = node.instance_params.variable_id() == Some(&var_id_str);
                if !refs_this_var {
                    continue;
                }

                // 更新 instance_params 中的 type 和 name
                // (需要 drop read lock 先, 故收集后再写)
                if type_changed {
                    for &pin_id in &node.pin_ids {
                        if let Some(pin) = data_state.pins.get(&pin_id) {
                            if pin.definition.kind == PinKind::Data {
                                inferred_pins.push(InferredPinType {
                                    pin_id,
                                    pin_type: data_type_to_pin_type(new_data_type).to_string(),
                                    container_type: data_type_to_container(new_data_type)
                                        .map(|s| s.to_string()),
                                    type_display: Some(new_data_type.to_string()),
                                    data_type: Some(new_data_type.clone()),
                                });
                            }
                        }
                    }
                }
            }

            // 收集需要更新 instance_params 的节点 ID
            let nodes_to_update: Vec<_> = data_state
                .nodes
                .values()
                .filter(|n| n.instance_params.variable_id() == Some(&var_id_str))
                .map(|n| n.id)
                .collect();

            drop(data_state);

            // 写回 pin_types 和 instance_params
            {
                let mut data_state = graph.data_state.write().unwrap();
                for ipt in &inferred_pins {
                    let dt = new_data_type.clone();
                    data_state.pin_types.insert(ipt.pin_id, dt);
                }
                for nid in &nodes_to_update {
                    if let Some(node) = data_state.nodes.get_mut(nid) {
                        if let crate::graph::NodeInstanceParams::Variable {
                            ref mut variable_type,
                            ref mut variable_name,
                            ..
                        } = node.instance_params
                        {
                            if type_changed {
                                *variable_type = Some(new_data_type.to_string());
                            }
                            if name_changed {
                                *variable_name = Some(new_name.clone());
                            }
                        }
                    }
                }
            }

            if !inferred_pins.is_empty() {
                emit_project_event(
                    &app,
                    Event::Node(EventNode::PinTypesInferred {
                        graph_id: *graph_id,
                        pin_types: inferred_pins,
                    }),
                );
            }
        }
    }

    if persist_global {
        state.persist_current_project()?;
    }
    Ok(())
}

/// 删除变量（统一接口）
#[tauri::command]
pub fn delete_variable(
    app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
) -> Result<(), String> {
    let variable = state.remove_variable(&variable_id).unwrap();
    if matches!(variable.scope, VariableScope::Global) {
        state.persist_current_project()?;
    }
    emit_project_event(
        &app,
        Event::Variable(EventVariable::VariableDeleted {
            variable_id: variable.id,
            variable_scope: variable.scope,
        }),
    );
    Ok(())
}
