//! 节点注册中心
//!
//! 存放所有内置节点的定义。

use super::definition::NodeDefinition;
use super::types::PinDefinition;
use serde_json::Value;

/// 获取所有节点定义
pub fn get_all_node_definitions() -> Vec<NodeDefinition> {
    vec![
        // ========== 内部节点 ==========
        create_event_on_run(),
        create_function_entry(),
        create_function_return(),
        create_macro_inputs(),
        create_macro_outputs(),
        // ========== 函数调用节点 ==========
        create_call_function(),
        create_call_macro(),
        // ========== 功能节点 ==========
        create_print(),
        create_plot(),
        // ========== 数学节点 ==========
        create_add(),
        // ========== 分支节点 ==========
        create_if_else(),
        // ========== 变量节点 ==========
        create_get_variable(),
        create_set_variable(),
        // ========== 数据节点 ==========
        create_get_dataframe(),
        create_get_column(),
    ]
}

// ==================== 内部节点 ====================

fn create_event_on_run() -> NodeDefinition {
    NodeDefinition {
        node_type: "event_on_run".into(),
        category: "Internal".into(),
        title: "On Run".into(),
        ui_style: "event".into(),
        description: Some("Project or Event execution entry point".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "Exec".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("Exec".to_string())),
    }
}

fn create_function_entry() -> NodeDefinition {
    NodeDefinition {
        node_type: "function_entry".into(),
        category: "Internal".into(),
        title: "Entry".into(),
        ui_style: "event".into(),
        description: Some("Function execution entry point".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "Then".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("Then".to_string())),
    }
}

fn create_function_return() -> NodeDefinition {
    NodeDefinition {
        node_type: "function_return".into(),
        category: "Internal".into(),
        title: "Return".into(),
        ui_style: "event".into(),
        description: Some("Function execution exit point".into()),
        inputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        outputs: vec![],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("__RETURN__".to_string())),
    }
}

fn create_macro_inputs() -> NodeDefinition {
    NodeDefinition {
        node_type: "macro_inputs".into(),
        category: "Internal".into(),
        title: "Inputs".into(),
        ui_style: "event".into(),
        description: Some("Macro inputs container".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("In".to_string())),
    }
}

fn create_macro_outputs() -> NodeDefinition {
    NodeDefinition {
        node_type: "macro_outputs".into(),
        category: "Internal".into(),
        title: "Outputs".into(),
        ui_style: "event".into(),
        description: Some("Macro outputs container".into()),
        inputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        outputs: vec![],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("__RETURN__".to_string())),
    }
}

// ==================== 函数调用节点 ====================

fn create_call_function() -> NodeDefinition {
    NodeDefinition {
        node_type: "call_function".into(),
        category: "Function".into(),
        title: "Call Function".into(),
        ui_style: "default".into(),
        description: Some("Call a defined function".into()),
        inputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        outputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: Some(|_ctx, _node, _pin_id| Value::Null),
        flow_processor: Some(|ctx, node| {
            let sub_graph_id = node.sub_graph_id.as_ref().ok_or("Missing subGraphId")?;
            let sub_graph_id_clone = sub_graph_id.clone();
            let node_title = node.title.clone();
            let node_id = node.id.clone();

            // 找到入口节点
            let entry_node_id = ctx
                .find_node_by(&|n| {
                    n.node_type == "function_entry"
                        && (n.sub_graph_id.as_ref() == Some(&sub_graph_id_clone)
                            || n.title == node_title)
                })
                .ok_or(format!(
                    "Function entry not found for subGraphId {}",
                    sub_graph_id
                ))?;

            ctx.push_call_stack(node_id);
            ctx.log(format!("Calling function: {}", node.title));
            ctx.run_flow(&entry_node_id, "Then")?;
            ctx.pop_call_stack();
            ctx.log(format!("Returned from function: {}", node.title));

            Ok("Out".to_string())
        }),
    }
}

fn create_call_macro() -> NodeDefinition {
    NodeDefinition {
        node_type: "call_macro".into(),
        category: "Macro".into(),
        title: "Call Macro".into(),
        ui_style: "default".into(),
        description: Some("Call a defined macro".into()),
        inputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        outputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|ctx, node| {
            let sub_graph_id = node.sub_graph_id.as_ref().ok_or("Missing subGraphId")?;
            let sub_graph_id_clone = sub_graph_id.clone();
            let node_title = node.title.clone();
            let node_id = node.id.clone();

            let entry_node_id = ctx
                .find_node_by(&|n| {
                    n.node_type == "macro_inputs"
                        && (n.sub_graph_id.as_ref() == Some(&sub_graph_id_clone)
                            || n.title == node_title)
                })
                .ok_or("Macro entry not found")?;

            ctx.push_call_stack(node_id);
            ctx.run_flow(&entry_node_id, "In")?;
            ctx.pop_call_stack();

            Ok("Out".to_string())
        }),
    }
}

// ==================== 功能节点 ====================

fn create_print() -> NodeDefinition {
    NodeDefinition {
        node_type: "print".into(),
        category: "Debug".into(),
        title: "Print".into(),
        ui_style: "default".into(),
        description: Some("Print a value to the log".into()),
        inputs: vec![
            PinDefinition {
                name: "In".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
            PinDefinition {
                name: "Value".into(),
                pin_type: "string".into(),
                default_value: Some(Value::String("".into())),
            },
        ],
        outputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|ctx, node| {
            let data_pin = node
                .inputs
                .iter()
                .find(|p| p.name == "Value")
                .ok_or("Print node missing 'Value' input")?;
            let val = ctx.get_pin_value(&data_pin.id);
            let output = if let Value::String(s) = &val {
                s.clone()
            } else {
                val.to_string()
            };
            ctx.log(format!("[NODE PRINT]: {}", output));
            println!("[NODE PRINT]: {}", output);
            Ok("Out".to_string())
        }),
    }
}

// ==================== 数学节点 ====================

fn create_add() -> NodeDefinition {
    NodeDefinition {
        node_type: "add".into(),
        category: "Math".into(),
        title: "Add".into(),
        ui_style: "math".into(),
        description: Some("Add two numbers".into()),
        inputs: vec![
            PinDefinition {
                name: "A".into(),
                pin_type: "float".into(),
                default_value: Some(Value::from(0.0)),
            },
            PinDefinition {
                name: "B".into(),
                pin_type: "float".into(),
                default_value: Some(Value::from(0.0)),
            },
        ],
        outputs: vec![PinDefinition {
            name: "Sum".into(),
            pin_type: "float".into(),
            default_value: None,
        }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx
                .get_pin_value(&node.inputs[0].id)
                .as_f64()
                .unwrap_or(0.0);
            let b = ctx
                .get_pin_value(&node.inputs[1].id)
                .as_f64()
                .unwrap_or(0.0);
            Value::from(a + b)
        }),
        flow_processor: None,
    }
}

// ==================== 分支节点 ====================

fn create_if_else() -> NodeDefinition {
    NodeDefinition {
        node_type: "if_else".into(),
        category: "Branch".into(),
        title: "Branch".into(),
        ui_style: "default".into(),
        description: Some("Branch execution based on condition".into()),
        inputs: vec![
            PinDefinition {
                name: "In".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
            PinDefinition {
                name: "Cond".into(),
                pin_type: "bool".into(),
                default_value: Some(Value::Bool(false)),
            },
        ],
        outputs: vec![
            PinDefinition {
                name: "True".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
            PinDefinition {
                name: "False".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
        ],
        data_processor: None,
        flow_processor: Some(|ctx, node| {
            let data_pin = node
                .inputs
                .iter()
                .find(|p| p.pin_type != "exec")
                .ok_or("Branch node missing data input")?;
            let val = ctx.get_pin_value(&data_pin.id);
            let condition = val
                .as_bool()
                .unwrap_or_else(|| val.as_f64().unwrap_or(0.0) != 0.0);
            let next = if condition { "True" } else { "False" };
            ctx.log(format!(
                "  Branch condition is {}, moving to '{}'",
                condition, next
            ));
            Ok(next.to_string())
        }),
    }
}

// ==================== 变量节点 ====================

fn create_get_variable() -> NodeDefinition {
    NodeDefinition {
        node_type: "get_variable".into(),
        category: "Variable".into(),
        title: "Get Variable".into(),
        ui_style: "default".into(),
        description: Some("Get variable value".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "Value".into(),
            pin_type: "object".into(),
            default_value: None,
        }],
        data_processor: Some(|ctx, node, _pin_id| {
            if let Some(var_id) = &node.variable_id {
                match ctx.get_variable(var_id) {
                    Some(val) => val.clone(),
                    None => {
                        ctx.log(format!(
                            "[Error] Variable ID '{}' not found in context.",
                            var_id
                        ));
                        Value::Null
                    }
                }
            } else {
                ctx.log(format!(
                    "[Error] Get Variable node '{}' has no variable assigned.",
                    node.id
                ));
                Value::Null
            }
        }),
        flow_processor: None,
    }
}

fn create_set_variable() -> NodeDefinition {
    NodeDefinition {
        node_type: "set_variable".into(),
        category: "Variable".into(),
        title: "Set Variable".into(),
        ui_style: "default".into(),
        description: Some("Set variable value".into()),
        inputs: vec![
            PinDefinition {
                name: "In".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
            PinDefinition {
                name: "Value".into(),
                pin_type: "object".into(),
                default_value: None,
            },
        ],
        outputs: vec![
            PinDefinition {
                name: "Out".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
            PinDefinition {
                name: "Value".into(),
                pin_type: "object".into(),
                default_value: None,
            },
        ],
        data_processor: Some(|ctx, node, _pin_id| {
            if let Some(var_id) = &node.variable_id {
                ctx.get_variable(var_id).cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }),
        flow_processor: Some(|ctx, node| {
            let var_id = node.variable_id.as_ref().ok_or_else(|| {
                format!(
                    "[Error] Set Variable node '{}' has no variable assigned.",
                    node.id
                )
            })?;

            let data_pin = node
                .inputs
                .iter()
                .find(|p| p.name == "Value")
                .ok_or("Set Variable missing 'Value' input")?;
            let val = ctx.get_pin_value(&data_pin.id);

            if ctx.set_variable(var_id, val) {
                Ok("Out".to_string())
            } else {
                Err(format!(
                    "[Error] Cannot set unknown variable ID '{}'.",
                    var_id
                ))
            }
        }),
    }
}

// ==================== 数据节点 ====================

fn create_get_dataframe() -> NodeDefinition {
    NodeDefinition {
        node_type: "get_dataframe".into(),
        category: "Data".into(),
        title: "Get DataFrame".into(),
        ui_style: "default".into(),
        description: Some("Get a loaded DataFrame".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "DataFrame".into(),
            pin_type: "dataframe".into(),
            default_value: None,
        }],
        data_processor: Some(|ctx, node, _pin_id| {
            if let Some(df_id) = &node.variable_id {
                // 后端执行时需要从 state 获取实际数据，目前先返回 Null
                // 实际实现应该在 context 中支持 get_dataframe
                Value::String(df_id.clone())
            } else {
                Value::Null
            }
        }),
        flow_processor: None,
    }
}

fn create_get_column() -> NodeDefinition {
    NodeDefinition {
        node_type: "get_column".into(),
        category: "Data".into(),
        title: "Get Column".into(),
        ui_style: "default".into(),
        description: Some("Get a column from a DataFrame".into()),
        inputs: vec![PinDefinition {
            name: "DataFrame".into(),
            pin_type: "dataframe".into(),
            default_value: None,
        }],
        outputs: vec![PinDefinition {
            name: "Column".into(),
            pin_type: "array".into(),
            default_value: None,
        }],
        data_processor: Some(|_ctx, _node, _pin_id| Value::Null),
        flow_processor: None,
    }
}

// ==================== 可视化节点 ====================

fn create_plot() -> NodeDefinition {
    NodeDefinition {
        node_type: "plot".into(),
        category: "Visualization".into(),
        title: "Plot".into(),
        ui_style: "default".into(),
        description: Some("Open a new plot window for data visualization".into()),
        inputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        outputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|ctx, node| {
            // 生成唯一的窗口标签
            let label = format!("plot-{}", node.id);
            let title = format!("Plot - {}", node.title);
            
            // 打开新窗口显示 plot 页面
            // 这里使用 index.html，你可以创建一个专门的 plot.html
            let url = "index.html#/plot".to_string();
            
            match ctx.open_window(label.clone(), title, url) {
                Ok(_) => {
                    ctx.log(format!("[Plot] Window '{}' opened successfully", label));
                    Ok("Out".to_string())
                }
                Err(e) => {
                    ctx.log(format!("[Error] Failed to open plot window: {}", e));
                    Err(e)
                }
            }
        }),
    }
}
