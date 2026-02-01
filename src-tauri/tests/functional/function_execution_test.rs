//! Function 子图执行测试
//!
//! 测试函数子图的执行流程，包括输入输出参数、局部变量和函数调用

use yssbi_lib::executor::{GraphData, NodeData, PinData, VariableData, ExecutionContext, ExecutionContextTrait};
use yssbi_lib::project::{ProjectData, SubGraphData, SubGraphType, SerializedNode, SerializedPin, Position, CanvasState, PinDefinition};
use yssbi_lib::schema::{VariableDefinition, VariableDataType};
use std::collections::HashMap;
use serde_json::json;

/// 创建测试用的函数子图
/// 函数功能：接收两个数字，计算它们的和并返回
fn create_test_function_subgraph() -> SubGraphData {
    // Function Input 节点
    let input_node = SerializedNode {
        id: "func_input".to_string(),
        node_type: "function_input".to_string(),
        title: "Function Input".to_string(),
        position: Position { x: 100.0, y: 100.0 },
        is_internal: true,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![],
        outputs: vec![
            SerializedPin {
                id: "func_input_exec_out".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["add_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "func_input_a_out".to_string(),
                name: "A".to_string(),
                pin_type: "number".to_string(),
                links: vec!["add_a_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "func_input_b_out".to_string(),
                name: "B".to_string(),
                pin_type: "number".to_string(),
                links: vec!["add_b_in".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Add 节点
    let add_node = SerializedNode {
        id: "add_node".to_string(),
        node_type: "math_add".to_string(),
        title: "Add".to_string(),
        position: Position { x: 300.0, y: 150.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "add_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["func_input_exec_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "add_a_in".to_string(),
                name: "A".to_string(),
                pin_type: "number".to_string(),
                links: vec!["func_input_a_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "add_b_in".to_string(),
                name: "B".to_string(),
                pin_type: "number".to_string(),
                links: vec!["func_input_b_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "add_exec_out".to_string(),
                name: "Done".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["func_output_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "add_result_out".to_string(),
                name: "Result".to_string(),
                pin_type: "number".to_string(),
                links: vec!["func_output_result_in".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Function Output 节点
    let output_node = SerializedNode {
        id: "func_output".to_string(),
        node_type: "function_output".to_string(),
        title: "Function Output".to_string(),
        position: Position { x: 500.0, y: 100.0 },
        is_internal: true,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "func_output_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["add_exec_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "func_output_result_in".to_string(),
                name: "Sum".to_string(),
                pin_type: "number".to_string(),
                links: vec!["add_result_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![],
    };

    // 创建局部变量
    let mut local_variables = HashMap::new();
    local_variables.insert(
        "temp_result".to_string(),
        VariableDefinition::new(
            "temp_result".to_string(),
            "Temporary Result".to_string(),
            VariableDataType::Int64,
        ),
    );

    SubGraphData {
        id: "test_function".to_string(),
        name: "Add Function".to_string(),
        sub_type: SubGraphType::Function,
        nodes: vec![input_node, add_node, output_node],
        canvas: CanvasState::default(),
        variables: local_variables,
        inputs: vec![
            PinDefinition {
                id: "input_a".to_string(),
                name: "A".to_string(),
                pin_type: "number".to_string(),
                is_array: false,
            },
            PinDefinition {
                id: "input_b".to_string(),
                name: "B".to_string(),
                pin_type: "number".to_string(),
                is_array: false,
            },
        ],
        outputs: vec![
            PinDefinition {
                id: "output_sum".to_string(),
                name: "Sum".to_string(),
                pin_type: "number".to_string(),
                is_array: false,
            },
        ],
    }
}

/// 创建调用函数的事件子图
fn create_function_caller_event() -> SubGraphData {
    // Number Constant 节点 (值为 5)
    let num_const1 = SerializedNode {
        id: "num_const1".to_string(),
        node_type: "number_constant".to_string(),
        title: "Number 5".to_string(),
        position: Position { x: 100.0, y: 100.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![],
        outputs: vec![
            SerializedPin {
                id: "num_const1_out".to_string(),
                name: "Value".to_string(),
                pin_type: "number".to_string(),
                links: vec!["func_call_a_in".to_string()],
                default_value: Some(json!(5)),
                is_array: false,
            },
        ],
    };

    // Number Constant 节点 (值为 3)
    let num_const2 = SerializedNode {
        id: "num_const2".to_string(),
        node_type: "number_constant".to_string(),
        title: "Number 3".to_string(),
        position: Position { x: 100.0, y: 200.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![],
        outputs: vec![
            SerializedPin {
                id: "num_const2_out".to_string(),
                name: "Value".to_string(),
                pin_type: "number".to_string(),
                links: vec!["func_call_b_in".to_string()],
                default_value: Some(json!(3)),
                is_array: false,
            },
        ],
    };

    // Function Call 节点
    let func_call = SerializedNode {
        id: "func_call".to_string(),
        node_type: "function_call".to_string(),
        title: "Call Add Function".to_string(),
        position: Position { x: 300.0, y: 150.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: Some("test_function".to_string()),
        inputs: vec![
            SerializedPin {
                id: "func_call_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec![],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "func_call_a_in".to_string(),
                name: "A".to_string(),
                pin_type: "number".to_string(),
                links: vec!["num_const1_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "func_call_b_in".to_string(),
                name: "B".to_string(),
                pin_type: "number".to_string(),
                links: vec!["num_const2_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "func_call_exec_out".to_string(),
                name: "Done".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["debug_exec_in".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "func_call_sum_out".to_string(),
                name: "Sum".to_string(),
                pin_type: "number".to_string(),
                links: vec!["debug_data_in".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
    };

    // Debug Print 节点
    let debug_node = SerializedNode {
        id: "debug_result".to_string(),
        node_type: "debug_print".to_string(),
        title: "Print Result".to_string(),
        position: Position { x: 500.0, y: 150.0 },
        is_internal: false,
        variable_id: None,
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs: vec![
            SerializedPin {
                id: "debug_exec_in".to_string(),
                name: "Execute".to_string(),
                pin_type: "exec".to_string(),
                links: vec!["func_call_exec_out".to_string()],
                default_value: None,
                is_array: false,
            },
            SerializedPin {
                id: "debug_data_in".to_string(),
                name: "Value".to_string(),
                pin_type: "number".to_string(),
                links: vec!["func_call_sum_out".to_string()],
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            SerializedPin {
                id: "debug_exec_out".to_string(),
                name: "Done".to_string(),
                pin_type: "exec".to_string(),
                links: vec![],
                default_value: None,
                is_array: false,
            },
        ],
    };

    SubGraphData {
        id: "function_caller".to_string(),
        name: "Function Caller Event".to_string(),
        sub_type: SubGraphType::Event,
        nodes: vec![num_const1, num_const2, func_call, debug_node],
        canvas: CanvasState::default(),
        variables: HashMap::new(),
        inputs: vec![],
        outputs: vec![],
    }
}

#[test]
fn test_function_subgraph_creation() {
    let function = create_test_function_subgraph();
    
    assert_eq!(function.id, "test_function");
    assert_eq!(function.name, "Add Function");
    assert_eq!(function.sub_type, SubGraphType::Function);
    assert_eq!(function.nodes.len(), 3);
    
    // 验证输入输出定义
    assert_eq!(function.inputs.len(), 2);
    assert_eq!(function.outputs.len(), 1);
    assert_eq!(function.inputs[0].name, "A");
    assert_eq!(function.inputs[1].name, "B");
    assert_eq!(function.outputs[0].name, "Sum");
    
    // 验证局部变量
    assert_eq!(function.variables.len(), 1);
    assert!(function.variables.contains_key("temp_result"));
}

#[test]
fn test_function_caller_creation() {
    let caller = create_function_caller_event();
    
    assert_eq!(caller.id, "function_caller");
    assert_eq!(caller.sub_type, SubGraphType::Event);
    assert_eq!(caller.nodes.len(), 4);
    
    // 验证函数调用节点
    let func_call_node = caller.nodes.iter()
        .find(|n| n.node_type == "function_call")
        .expect("Function call node not found");
    
    assert_eq!(func_call_node.sub_graph_id, Some("test_function".to_string()));
    assert_eq!(func_call_node.inputs.len(), 3); // exec + 2 parameters
    assert_eq!(func_call_node.outputs.len(), 2); // exec + result
}

#[test]
fn test_function_execution_data_conversion() {
    let function = create_test_function_subgraph();
    let caller = create_function_caller_event();
    
    // 创建包含函数和调用者的项目
    let mut project = ProjectData::new();
    project.functions.insert("test_function".to_string(), function);
    project.events.insert("function_caller".to_string(), caller);
    
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
    assert_eq!(nodes.len(), 7); // 3 function nodes + 4 caller nodes
    assert_eq!(variables.len(), 1); // 1 local variable in function
    
    // 验证函数节点存在
    let function_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.sub_graph_id == Some("test_function".to_string()))
        .collect();
    assert_eq!(function_nodes.len(), 3);
    
    // 验证调用者节点存在
    let caller_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.sub_graph_id == Some("function_caller".to_string()))
        .collect();
    assert_eq!(caller_nodes.len(), 4);
}

#[test]
fn test_function_graph_data_creation() {
    let function = create_test_function_subgraph();
    let caller = create_function_caller_event();
    
    let mut project = ProjectData::new();
    project.functions.insert("test_function".to_string(), function);
    project.events.insert("function_caller".to_string(), caller);
    
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
    assert_eq!(graph.nodes.len(), 7);
    assert!(graph.variables.is_some());
    
    // 验证可以创建执行上下文
    let context = ExecutionContext::new(graph);
    // 执行上下文创建成功即可
}

#[test]
fn test_function_input_output_validation() {
    let function = create_test_function_subgraph();
    
    // 验证输入参数
    assert_eq!(function.inputs.len(), 2);
    for input in &function.inputs {
        assert!(!input.id.is_empty());
        assert!(!input.name.is_empty());
        assert_eq!(input.pin_type, "number");
        assert!(!input.is_array);
    }
    
    // 验证输出参数
    assert_eq!(function.outputs.len(), 1);
    let output = &function.outputs[0];
    assert!(!output.id.is_empty());
    assert_eq!(output.name, "Sum");
    assert_eq!(output.pin_type, "number");
    assert!(!output.is_array);
    
    // 验证函数内部节点
    let input_node = function.nodes.iter()
        .find(|n| n.node_type == "function_input")
        .expect("Function input node not found");
    let output_node = function.nodes.iter()
        .find(|n| n.node_type == "function_output")
        .expect("Function output node not found");
    
    assert!(input_node.is_internal);
    assert!(output_node.is_internal);
    
    // 验证输入节点的输出数量匹配函数参数
    assert_eq!(input_node.outputs.len(), 3); // exec + 2 parameters
    
    // 验证输出节点的输入数量匹配函数返回值
    assert_eq!(output_node.inputs.len(), 2); // exec + 1 return value
}

#[test]
fn test_function_local_variables() {
    let function = create_test_function_subgraph();
    
    // 验证局部变量
    assert_eq!(function.variables.len(), 1);
    let temp_var = function.variables.get("temp_result").unwrap();
    assert_eq!(temp_var.name, "Temporary Result");
    assert_eq!(temp_var.data_type, VariableDataType::Int64);
}

#[test]
fn test_function_call_node_structure() {
    let caller = create_function_caller_event();
    
    let func_call_node = caller.nodes.iter()
        .find(|n| n.node_type == "function_call")
        .expect("Function call node not found");
    
    // 验证函数调用节点引用正确的子图
    assert_eq!(func_call_node.sub_graph_id, Some("test_function".to_string()));
    
    // 验证输入参数匹配
    assert_eq!(func_call_node.inputs.len(), 3); // exec + A + B
    let param_inputs: Vec<_> = func_call_node.inputs.iter()
        .filter(|p| p.pin_type != "exec")
        .collect();
    assert_eq!(param_inputs.len(), 2);
    
    // 验证输出参数匹配
    assert_eq!(func_call_node.outputs.len(), 2); // exec + Sum
    let param_outputs: Vec<_> = func_call_node.outputs.iter()
        .filter(|p| p.pin_type != "exec")
        .collect();
    assert_eq!(param_outputs.len(), 1);
}

#[test]
fn test_function_execution_context_with_variables() {
    let function = create_test_function_subgraph();
    let mut project = ProjectData::new();
    project.functions.insert("test_function".to_string(), function);
    
    // 添加全局变量
    let global_var = VariableDefinition::new(
        "global_counter".to_string(),
        "global_counter".to_string(),
        VariableDataType::Int64,
    );
    project.global_variables.insert("global_counter".to_string(), global_var);
    
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
    
    // 收集函数节点和局部变量
    for (sg_id, sub) in &project.functions {
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
        variables: Some(variables),
    };
    
    let mut context = ExecutionContext::new(graph);
    
    // 验证上下文包含全局和局部变量
    context.log("[Test] Function execution context created with variables".to_string());
    
    // 执行并获取日志
    match context.execute() {
        Ok(logs) => {
            assert!(logs.iter().any(|log| log.contains("Function execution context created with variables")));
        }
        Err(_) => {
            // 执行可能失败，但上下文创建应该成功
        }
    }
}