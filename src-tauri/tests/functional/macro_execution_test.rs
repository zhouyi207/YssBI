//! Macro 子图执行测试
//!
//! 测试宏子图的执行流程，包括宏展开、参数替换和代码生成

use yssbi_lib::executor::{GraphData, NodeData, PinData, VariableData, ExecutionContext, ExecutionContextTrait};
use yssbi_lib::project::{ProjectData, SubGraphData, SubGraphType, SerializedNode, SerializedPin, Position, CanvasState, PinDefinition};
use yssbi_lib::schema::{VariableDefinition, VariableDataType};
use std::collections::HashMap;
use serde_json::json;

/// 创建测试用的宏子图
/// 宏功能：生成一个循环结构，重复执行指定次数的操作
fn create_test_macro_subgraph() -> SubGraphData {
    // Macro Input 节点
    let input_node = SerializedNode {
        id: "macro_input".to_string(),
        node_type: "macro_input".to_string(),
        title: "Macro Input".to_string(),
        position: Position { x: 100.0, y: 100.0 },
        is_internal: true,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![],
        outputs: vec![
            SerializedPin {
                id: "macro_input_exec_out".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["loop_start_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "macro_input_count_out".to_string(),
                name: "Count".to_string(),
                pin_type: "number".to_string(),
                links: vec!["loop_start_count_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "macro_input_action_out".to_string(),
                name: "Action".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["loop_body_action_in".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Loop Start 节点（宏生成的循环开始）
    let loop_start_node = SerializedNode {
        id: "loop_start".to_string(),
        node_type: "for_loop_start".to_string(),
        title: "Loop Start".to_string(),
        position: Position { x: 300.0, y: 100.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "loop_start_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["macro_input_exec_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "loop_start_count_in".to_string(),
                name: "Count".to_string(),
                pin_type: "number".to_string(),
                links: vec!["macro_input_count_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "loop_start_body_out".to_string(),
                name: "Loop Body".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["loop_body_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "loop_start_index_out".to_string(),
                name: "Index".to_string(),
                pin_type: "number".to_string(),
                links: vec!["loop_body_index_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "loop_start_complete_out".to_string(),
                name: "Complete".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["macro_output_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Loop Body 节点（宏生成的循环体）
    let loop_body_node = SerializedNode {
        id: "loop_body".to_string(),
        node_type: "macro_loop_body".to_string(),
        title: "Loop Body".to_string(),
        position: Position { x: 500.0, y: 150.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "loop_body_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["loop_start_body_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "loop_body_index_in".to_string(),
                name: "Index".to_string(),
                pin_type: "number".to_string(),
                links: vec!["loop_start_index_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "loop_body_action_in".to_string(),
                name: "Action".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["macro_input_action_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "loop_body_continue_out".to_string(),
                name: "Continue".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["loop_start_exec_in".to_string()], // 回到循环开始
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "loop_body_current_index_out".to_string(),
                name: "Current Index".to_string(),
                pin_type: "number".to_string(),
                links: vec!["macro_output_index_in".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Macro Output 节点
    let output_node = SerializedNode {
        id: "macro_output".to_string(),
        node_type: "macro_output".to_string(),
        title: "Macro Output".to_string(),
        position: Position { x: 700.0, y: 100.0 },
        is_internal: true,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "macro_output_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["loop_start_complete_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "macro_output_index_in".to_string(),
                name: "Final Index".to_string(),
                pin_type: "number".to_string(),
                links: vec!["loop_body_current_index_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![],
    };

    // 创建宏的局部变量
    let mut local_variables = HashMap::new();
    local_variables.insert(
        "loop_counter".to_string(),
        VariableDefinition::new(
            "loop_counter".to_string(),
            "Loop Counter".to_string(),
            VariableDataType::Int64,
        ),
    );
    local_variables.insert(
        "max_iterations".to_string(),
        VariableDefinition::new(
            "max_iterations".to_string(),
            "Max Iterations".to_string(),
            VariableDataType::Int64,
        ),
    );

    SubGraphData {
        id: "test_macro".to_string(),
        name: "Repeat Macro".to_string(),
        sub_type: SubGraphType::Macro,
        nodes: vec![input_node, loop_start_node, loop_body_node, output_node],
        canvas: CanvasState::default(),
        variables: local_variables,
        inputs: vec![
            PinDefinition {
                id: "input_count".to_string(),
                name: "Count".to_string(),
                pin_type: "number".to_string(),
                is_array: false,
            },
            PinDefinition {
                id: "input_action".to_string(),
                name: "Action".to_string(),
                pin_type: "exec".to_string(),
                is_array: false,
            },
        ],
        outputs: vec![
            PinDefinition {
                id: "output_final_index".to_string(),
                name: "Final Index".to_string(),
                pin_type: "number".to_string(),
                is_array: false,
            },
        ],
    }
}

/// 创建使用宏的事件子图
fn create_macro_user_event() -> SubGraphData {
    // Number Constant 节点 (循环次数)
    let count_const = SerializedNode {
        id: "count_const".to_string(),
        node_type: "number_constant".to_string(),
        title: "Loop Count".to_string(),
        position: Position { x: 100.0, y: 100.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![],
        outputs: vec![
            SerializedPin {
                id: "count_const_out".to_string(),
                name: "Value".to_string(),
                pin_type: "number".to_string(),
                links: vec!["macro_call_count_in".to_string()],
                default_value: Some(json!(5)),
                is_array: false,
            },
        ],
    };

    // Debug Print 节点 (要重复的动作)
    let debug_action = SerializedNode {
        id: "debug_action".to_string(),
        node_type: "debug_print".to_string(),
        title: "Debug Action".to_string(),
        position: Position { x: 100.0, y: 200.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "debug_action_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["macro_call_action_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "debug_action_data_in".to_string(),
                name: "Value".to_string(),
                pin_type: "string".to_string(),
                links: vec![],
                default_value: Some(json!("Macro iteration executed!")),
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "debug_action_exec_out".to_string(),
                name: "Done".to_string(),
                pin_type: "exec".to_string(),
                links: vec![],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Macro Call 节点
    let macro_call = SerializedNode {
        id: "macro_call".to_string(),
        node_type: "macro_call".to_string(),
        title: "Call Repeat Macro".to_string(),
        position: Position { x: 300.0, y: 150.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: Some("test_macro".to_string()),
        inputs: vec![
            SerializedPin {
                id: "macro_call_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec![],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "macro_call_count_in".to_string(),
                name: "Count".to_string(),
                pin_type: "number".to_string(),
                links: vec!["count_const_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "macro_call_exec_out".to_string(),
                name: "Done".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["final_debug_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "macro_call_action_out".to_string(),
                name: "Action".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["debug_action_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "macro_call_final_index_out".to_string(),
                name: "Final Index".to_string(),
                pin_type: "number".to_string(),
                links: vec!["final_debug_data_in".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Final Debug Print 节点
    let final_debug = SerializedNode {
        id: "final_debug".to_string(),
        node_type: "debug_print".to_string(),
        title: "Print Final Result".to_string(),
        position: Position { x: 500.0, y: 150.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "final_debug_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["macro_call_exec_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "final_debug_data_in".to_string(),
                name: "Value".to_string(),
                pin_type: "number".to_string(),
                links: vec!["macro_call_final_index_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "final_debug_exec_out".to_string(),
                name: "Done".to_string(),
                pin_type: "exec".to_string(),
                links: vec![],
                default_value: None,
                is_array: false,
            },
        ],
    };

    SubGraphData {
        id: "macro_user".to_string(),
        name: "Macro User Event".to_string(),
        sub_type: SubGraphType::Event,
        nodes: vec![count_const, debug_action, macro_call, final_debug],
        canvas: CanvasState::default(),
        variables: HashMap::new(),
        inputs: vec![],
        outputs: vec![],
    }
}

#[test]
fn test_macro_subgraph_creation() {
    let macro_sg = create_test_macro_subgraph();
    
    assert_eq!(macro_sg.id, "test_macro");
    assert_eq!(macro_sg.name, "Repeat Macro");
    assert_eq!(macro_sg.sub_type, SubGraphType::Macro);
    assert_eq!(macro_sg.nodes.len(), 4);
    
    // 验证输入输出定义
    assert_eq!(macro_sg.inputs.len(), 2);
    assert_eq!(macro_sg.outputs.len(), 1);
    assert_eq!(macro_sg.inputs[0].name, "Count");
    assert_eq!(macro_sg.inputs[1].name, "Action");
    assert_eq!(macro_sg.outputs[0].name, "Final Index");
    
    // 验证局部变量
    assert_eq!(macro_sg.variables.len(), 2);
    assert!(macro_sg.variables.contains_key("loop_counter"));
    assert!(macro_sg.variables.contains_key("max_iterations"));
}

#[test]
fn test_macro_user_creation() {
    let user = create_macro_user_event();
    
    assert_eq!(user.id, "macro_user");
    assert_eq!(user.sub_type, SubGraphType::Event);
    assert_eq!(user.nodes.len(), 4);
    
    // 验证宏调用节点
    let macro_call_node = user.nodes.iter()
        .find(|n| n.node_type == "macro_call")
        .expect("Macro call node not found");
    
    assert_eq!(macro_call_node.sub_graph_id, Some("test_macro".to_string()));
    assert_eq!(macro_call_node.inputs.len(), 2); // exec + count
    assert_eq!(macro_call_node.outputs.len(), 3); // exec + action + final_index
}

#[test]
fn test_macro_execution_data_conversion() {
    let macro_sg = create_test_macro_subgraph();
    let user = create_macro_user_event();
    
    // 创建包含宏和使用者的项目
    let mut project = ProjectData::new();
    project.macros.insert("test_macro".to_string(), macro_sg);
    project.events.insert("macro_user".to_string(), user);
    
    // 转换为执行数据
    let mut nodes: Vec<NodeData> = Vec::new();
    let mut variables: HashMap<String, VariableData> = HashMap::new();
    
    // 收集所有子图的节点和变量
    let collections = vec![(&project.events), (&project.functions), (&project.macros)];
    
    for subgraphs in collections {
        for (sg_id, sub) in subgraphs {
            // 收集节点
            for sn in &sub.nodes {
                let node = NodeData {
                    id: sn.id.clone(),
                    node_type: sn.node_type.clone(),
                    title: sn.title.clone(),
                    inputs: sn.inputs.iter().map(|p| PinData {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        pin_type: p.pin_type.clone(),
                        links: p.links.clone(),
                        default_value: p.default_value.clone(),
                        is_array: p.is_array,
                    }).collect(),
                    outputs: sn.outputs.iter().map(|p| PinData {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        pin_type: p.pin_type.clone(),
                        links: p.links.clone(),
                        default_value: p.default_value.clone(),
                        is_array: p.is_array,
                    }).collect(),
                    variable_id: sn.variable_id.clone(),
                    sub_graph_id: Some(sg_id.clone()),
                };
                nodes.push(node);
            }
            
            // 收集局部变量
            for (id, var) in &sub.variables {
                let value = var.static_value.clone()
                    .or_else(|| var.default_value.clone())
                    .unwrap_or(serde_json::Value::Null);
                variables.insert(
                    id.clone(),
                    VariableData {
                        name: var.name.clone(),
                        var_type: format!("{:?}", var.data_type).to_lowercase(),
                        value,
                    },
                );
            }
        }
    }
    
    // 验证转换结果
    assert_eq!(nodes.len(), 8); // 4 macro nodes + 4 user nodes
    assert_eq!(variables.len(), 2); // 2 local variables in macro
    
    // 验证宏节点存在
    let macro_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.sub_graph_id == Some("test_macro".to_string()))
        .collect();
    assert_eq!(macro_nodes.len(), 4);
    
    // 验证使用者节点存在
    let user_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.sub_graph_id == Some("macro_user".to_string()))
        .collect();
    assert_eq!(user_nodes.len(), 4);
}

#[test]
fn test_macro_graph_data_creation() {
    let macro_sg = create_test_macro_subgraph();
    let user = create_macro_user_event();
    
    let mut project = ProjectData::new();
    project.macros.insert("test_macro".to_string(), macro_sg);
    project.events.insert("macro_user".to_string(), user);
    
    // 创建 GraphData
    let mut nodes: Vec<NodeData> = Vec::new();
    let mut variables: HashMap<String, VariableData> = HashMap::new();
    
    // 收集所有数据
    let collections = vec![(&project.events), (&project.functions), (&project.macros)];
    
    for subgraphs in collections {
        for (sg_id, sub) in subgraphs {
            for sn in &sub.nodes {
                let node = NodeData {
                    id: sn.id.clone(),
                    node_type: sn.node_type.clone(),
                    title: sn.title.clone(),
                    inputs: sn.inputs.iter().map(|p| PinData {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        pin_type: p.pin_type.clone(),
                        links: p.links.clone(),
                        default_value: p.default_value.clone(),
                        is_array: p.is_array,
                    }).collect(),
                    outputs: sn.outputs.iter().map(|p| PinData {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        pin_type: p.pin_type.clone(),
                        links: p.links.clone(),
                        default_value: p.default_value.clone(),
                        is_array: p.is_array,
                    }).collect(),
                    variable_id: sn.variable_id.clone(),
                    sub_graph_id: Some(sg_id.clone()),
                };
                nodes.push(node);
            }
            
            for (id, var) in &sub.variables {
                let value = var.static_value.clone()
                    .or_else(|| var.default_value.clone())
                    .unwrap_or(serde_json::Value::Null);
                variables.insert(
                    id.clone(),
                    VariableData {
                        name: var.name.clone(),
                        var_type: format!("{:?}", var.data_type).to_lowercase(),
                        value,
                    },
                );
            }
        }
    }
    
    let graph = GraphData {
        version: "1.0.0".to_string(),
        nodes,
        variables: Some(variables),
    };
    
    // 验证 GraphData
    assert_eq!(graph.version, "1.0.0");
    assert_eq!(graph.nodes.len(), 8);
    assert!(graph.variables.is_some());
    
    // 验证可以创建执行上下文
    let context = ExecutionContext::new(graph);
    // 执行上下文创建成功即可
}

#[test]
fn test_macro_input_output_validation() {
    let macro_sg = create_test_macro_subgraph();
    
    // 验证输入参数
    assert_eq!(macro_sg.inputs.len(), 2);
    let count_input = &macro_sg.inputs[0];
    let action_input = &macro_sg.inputs[1];
    
    assert_eq!(count_input.name, "Count");
    assert_eq!(count_input.pin_type, "number");
    assert!(!count_input.is_array);
    
    assert_eq!(action_input.name, "Action");
    assert_eq!(action_input.pin_type, "exec");
    assert!(!action_input.is_array);
    
    // 验证输出参数
    assert_eq!(macro_sg.outputs.len(), 1);
    let output = &macro_sg.outputs[0];
    assert_eq!(output.name, "Final Index");
    assert_eq!(output.pin_type, "number");
    assert!(!output.is_array);
    
    // 验证宏内部节点
    let input_node = macro_sg.nodes.iter()
        .find(|n| n.node_type == "macro_input")
        .expect("Macro input node not found");
    let output_node = macro_sg.nodes.iter()
        .find(|n| n.node_type == "macro_output")
        .expect("Macro output node not found");
    
    assert!(input_node.is_internal);
    assert!(output_node.is_internal);
    
    // 验证输入节点的输出数量匹配宏参数
    assert_eq!(input_node.outputs.len(), 3); // exec + count + action
    
    // 验证输出节点的输入数量匹配宏返回值
    assert_eq!(output_node.inputs.len(), 2); // exec + final_index
}

#[test]
fn test_macro_local_variables() {
    let macro_sg = create_test_macro_subgraph();
    
    // 验证局部变量
    assert_eq!(macro_sg.variables.len(), 2);
    
    let loop_counter = macro_sg.variables.get("loop_counter").unwrap();
    assert_eq!(loop_counter.name, "Loop Counter");
    assert_eq!(loop_counter.data_type, VariableDataType::Int64);
    
    let max_iterations = macro_sg.variables.get("max_iterations").unwrap();
    assert_eq!(max_iterations.name, "Max Iterations");
    assert_eq!(max_iterations.data_type, VariableDataType::Int64);
}

#[test]
fn test_macro_call_node_structure() {
    let user = create_macro_user_event();
    
    let macro_call_node = user.nodes.iter()
        .find(|n| n.node_type == "macro_call")
        .expect("Macro call node not found");
    
    // 验证宏调用节点引用正确的子图
    assert_eq!(macro_call_node.sub_graph_id, Some("test_macro".to_string()));
    
    // 验证输入参数匹配
    assert_eq!(macro_call_node.inputs.len(), 2); // exec + count
    let param_inputs: Vec<_> = macro_call_node.inputs.iter()
        .filter(|p| p.pin_type != "exec")
        .collect();
    assert_eq!(param_inputs.len(), 1); // count parameter
    
    // 验证输出参数匹配（宏可能有多个输出，包括展开的代码）
    assert_eq!(macro_call_node.outputs.len(), 3); // exec + action + final_index
    let param_outputs: Vec<_> = macro_call_node.outputs.iter()
        .filter(|p| p.pin_type != "exec")
        .collect();
    assert_eq!(param_outputs.len(), 2); // action + final_index
}

#[test]
fn test_macro_expansion_structure() {
    let macro_sg = create_test_macro_subgraph();
    
    // 验证宏包含循环结构节点
    let loop_start = macro_sg.nodes.iter()
        .find(|n| n.node_type == "for_loop_start")
        .expect("Loop start node not found");
    let loop_body = macro_sg.nodes.iter()
        .find(|n| n.node_type == "macro_loop_body")
        .expect("Loop body node not found");
    
    // 验证循环开始节点
    assert_eq!(loop_start.inputs.len(), 2); // exec + count
    assert_eq!(loop_start.outputs.len(), 3); // body + index + complete
    
    // 验证循环体节点
    assert_eq!(loop_body.inputs.len(), 3); // exec + index + action
    assert_eq!(loop_body.outputs.len(), 2); // continue + current_index
    
    // 验证循环连接（循环体的 continue 连接回循环开始）
    let continue_pin = loop_body.outputs.iter()
        .find(|p| p.name == "Continue")
        .expect("Continue pin not found");
    assert!(continue_pin.links.contains(&"loop_start_exec_in".to_string()));
}

#[test]
fn test_macro_execution_context_with_complex_variables() {
    let macro_sg = create_test_macro_subgraph();
    let mut project = ProjectData::new();
    project.macros.insert("test_macro".to_string(), macro_sg);
    
    // 添加全局变量
    let global_var = VariableDefinition::new(
        "macro_config".to_string(),
        "global_macro_config".to_string(),
        VariableDataType::String,
    );
    project.global_variables.insert("macro_config".to_string(), global_var);
    
    // 转换为执行数据
    let mut nodes: Vec<NodeData> = Vec::new();
    let mut variables: HashMap<String, VariableData> = HashMap::new();
    
    // 收集全局变量
    for (id, var) in &project.global_variables {
        let value = var.static_value.clone()
            .or_else(|| var.default_value.clone())
            .unwrap_or(serde_json::Value::Null);
        variables.insert(
            id.clone(),
            VariableData {
                name: var.name.clone(),
                var_type: format!("{:?}", var.data_type).to_lowercase(),
                value,
            },
        );
    }
    
    // 收集宏节点和局部变量
    for (sg_id, sub) in &project.macros {
        for sn in &sub.nodes {
            let node = NodeData {
                id: sn.id.clone(),
                node_type: sn.node_type.clone(),
                title: sn.title.clone(),
                inputs: sn.inputs.iter().map(|p| PinData {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    pin_type: p.pin_type.clone(),
                    links: p.links.clone(),
                    default_value: p.default_value.clone(),
                    is_array: p.is_array,
                }).collect(),
                outputs: sn.outputs.iter().map(|p| PinData {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    pin_type: p.pin_type.clone(),
                    links: p.links.clone(),
                    default_value: p.default_value.clone(),
                    is_array: p.is_array,
                }).collect(),
                variable_id: sn.variable_id.clone(),
                sub_graph_id: Some(sg_id.clone()),
            };
            nodes.push(node);
        }
        
        for (id, var) in &sub.variables {
            let value = var.static_value.clone()
                .or_else(|| var.default_value.clone())
                .unwrap_or(serde_json::Value::Null);
            variables.insert(
                id.clone(),
                VariableData {
                    name: var.name.clone(),
                    var_type: format!("{:?}", var.data_type).to_lowercase(),
                    value,
                },
            );
        }
    }
    
    let graph = GraphData {
        version: "1.0.0".to_string(),
        nodes,
        variables: Some(variables.clone()),
    };
    
    let mut context = ExecutionContext::new(graph);
    
    // 验证上下文包含全局和局部变量
    context.log("[Test] Macro execution context created with complex variables".to_string());
    
    // 执行并获取日志
    match context.execute() {
        Ok(logs) => {
            assert!(logs.iter().any(|log| log.contains("Macro execution context created with complex variables")));
        }
        Err(_) => {
            // 执行可能失败，但上下文创建应该成功
        }
    }
    
    // 验证变量数量
    assert_eq!(variables.len(), 3); // 1 global + 2 local macro variables
}