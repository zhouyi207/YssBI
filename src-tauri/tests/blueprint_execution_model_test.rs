/// Blueprint 执行模型测试
/// 
/// 验证 Pure DataFlow 节点不能被直接执行，只能通过 Lazy Pull 求值

use yssbi_lib::executor::{ExecutionContext, ExecutionModel, GraphData, NodeData, PinData};

#[test]
fn test_pure_node_cannot_be_executed() {
    // 创建一个简单的图：Event -> Divide (错误连接)
    // Divide 是 Pure 节点，不应该有 exec 连接
    
    let graph = GraphData {
        version: "1.0.0".to_string(),
        nodes: vec![
            // Event 节点
            NodeData {
                id: "event1".to_string(),
                node_type: "event_on_run".to_string(),
                title: "On Run".to_string(),
                inputs: vec![],
                outputs: vec![
                    PinData {
                        id: "event1_exec_out".to_string(),
                        name: "Exec".to_string(),
                        pin_type: "exec".to_string(),
                        links: vec!["divide1_exec_in".to_string()], // 错误：连接到 Pure 节点
                        default_value: None,
                        is_array: false,
                    },
                ],
                variable_id: None,
                sub_graph_id: None,
            },
            // Divide 节点（Pure DataFlow）
            NodeData {
                id: "divide1".to_string(),
                node_type: "divide".to_string(),
                title: "Divide".to_string(),
                inputs: vec![
                    // 注意：Divide 不应该有 exec input
                    PinData {
                        id: "divide1_exec_in".to_string(),
                        name: "Exec".to_string(),
                        pin_type: "exec".to_string(),
                        links: vec![],
                        default_value: None,
                        is_array: false,
                    },
                    PinData {
                        id: "divide1_a".to_string(),
                        name: "A".to_string(),
                        pin_type: "number".to_string(),
                        links: vec![],
                        default_value: Some(serde_json::json!(10.0)),
                        is_array: false,
                    },
                    PinData {
                        id: "divide1_b".to_string(),
                        name: "B".to_string(),
                        pin_type: "number".to_string(),
                        links: vec![],
                        default_value: Some(serde_json::json!(2.0)),
                        is_array: false,
                    },
                ],
                outputs: vec![
                    PinData {
                        id: "divide1_result".to_string(),
                        name: "Result".to_string(),
                        pin_type: "number".to_string(),
                        links: vec![],
                        default_value: None,
                        is_array: false,
                    },
                ],
                variable_id: None,
                sub_graph_id: None,
            },
        ],
        variables: None,
    };

    let mut ctx = ExecutionContext::new(graph);
    
    // 执行应该失败，因为 Divide 是 Pure 节点
    let result = ctx.execute();
    
    assert!(result.is_err(), "Expected execution to fail for Pure node");
    
    let error_msg = result.unwrap_err();
    assert!(
        error_msg.contains("Pure DataFlow node") || error_msg.contains("cannot be executed"),
        "Error message should mention Pure DataFlow node. Got: {}",
        error_msg
    );
}

#[test]
fn test_execution_model_classification() {
    // 验证节点的 ExecutionModel 分类正确
    
    use yssbi_lib::executor::node::registry::get_registry;
    
    let registry = get_registry();
    
    // Event 节点
    if let Some(event_node) = registry.get_prototype("event_on_run") {
        assert_eq!(
            event_node.execution_model(),
            ExecutionModel::Event,
            "event_on_run should be Event model"
        );
    }
    
    // Pure DataFlow 节点
    if let Some(divide_node) = registry.get_prototype("divide") {
        assert_eq!(
            divide_node.execution_model(),
            ExecutionModel::DataFlow,
            "divide should be DataFlow model"
        );
    }
    
    if let Some(get_var_node) = registry.get_prototype("get_variable") {
        assert_eq!(
            get_var_node.execution_model(),
            ExecutionModel::DataFlow,
            "get_variable should be DataFlow model"
        );
    }
    
    // Hybrid 节点
    if let Some(print_node) = registry.get_prototype("print") {
        assert_eq!(
            print_node.execution_model(),
            ExecutionModel::Hybrid,
            "print should be Hybrid model"
        );
    }
    
    // ControlFlow 节点
    if let Some(sequence_node) = registry.get_prototype("sequence") {
        assert_eq!(
            sequence_node.execution_model(),
            ExecutionModel::ControlFlow,
            "sequence should be ControlFlow model"
        );
    }
}

#[test]
fn test_correct_lazy_evaluation() {
    // 测试正确的 Lazy Pull 求值流程
    // Event -> Print -> (value) -> Divide -> GetVariable
    
    let graph = GraphData {
        version: "1.0.0".to_string(),
        nodes: vec![
            // Event 节点
            NodeData {
                id: "event1".to_string(),
                node_type: "event_on_run".to_string(),
                title: "On Run".to_string(),
                inputs: vec![],
                outputs: vec![
                    PinData {
                        id: "event1_exec_out".to_string(),
                        name: "Exec".to_string(),
                        pin_type: "exec".to_string(),
                        links: vec!["print1_exec_in".to_string()],
                        default_value: None,
                        is_array: false,
                    },
                ],
                variable_id: None,
                sub_graph_id: None,
            },
            // Print 节点（Hybrid）
            NodeData {
                id: "print1".to_string(),
                node_type: "print".to_string(),
                title: "Print".to_string(),
                inputs: vec![
                    PinData {
                        id: "print1_exec_in".to_string(),
                        name: "Exec".to_string(),
                        pin_type: "exec".to_string(),
                        links: vec![],
                        default_value: None,
                        is_array: false,
                    },
                    PinData {
                        id: "print1_value".to_string(),
                        name: "Value".to_string(),
                        pin_type: "any".to_string(),
                        links: vec!["divide1_result".to_string()], // 数据连接
                        default_value: None,
                        is_array: false,
                    },
                ],
                outputs: vec![
                    PinData {
                        id: "print1_exec_out".to_string(),
                        name: "Exec".to_string(),
                        pin_type: "exec".to_string(),
                        links: vec![],
                        default_value: None,
                        is_array: false,
                    },
                ],
                variable_id: None,
                sub_graph_id: None,
            },
            // Divide 节点（Pure DataFlow）
            NodeData {
                id: "divide1".to_string(),
                node_type: "divide".to_string(),
                title: "Divide".to_string(),
                inputs: vec![
                    PinData {
                        id: "divide1_a".to_string(),
                        name: "A".to_string(),
                        pin_type: "number".to_string(),
                        links: vec![],
                        default_value: Some(serde_json::json!(10.0)),
                        is_array: false,
                    },
                    PinData {
                        id: "divide1_b".to_string(),
                        name: "B".to_string(),
                        pin_type: "number".to_string(),
                        links: vec![],
                        default_value: Some(serde_json::json!(2.0)),
                        is_array: false,
                    },
                ],
                outputs: vec![
                    PinData {
                        id: "divide1_result".to_string(),
                        name: "Result".to_string(),
                        pin_type: "number".to_string(),
                        links: vec![],
                        default_value: None,
                        is_array: false,
                    },
                ],
                variable_id: None,
                sub_graph_id: None,
            },
        ],
        variables: None,
    };

    let mut ctx = ExecutionContext::new(graph);
    
    // 执行应该成功
    let result = ctx.execute();
    
    assert!(result.is_ok(), "Execution should succeed with correct data flow");
    
    let logs = result.unwrap();
    
    // 验证执行日志
    let log_str = logs.join("\n");
    
    // 应该执行 Event 和 Print，但不应该执行 Divide
    assert!(log_str.contains("Executing Node") && log_str.contains("On Run"), 
            "Should execute Event node");
    assert!(log_str.contains("Executing Node") && log_str.contains("Print"), 
            "Should execute Print node");
    
    // Divide 不应该出现在 "Executing Node" 日志中
    // 它应该通过 Lazy Pull 求值
    let executing_lines: Vec<&str> = logs.iter()
        .filter(|line| line.contains(">>> Executing Node"))
        .map(|s| s.as_str())
        .collect();
    
    let divide_executed = executing_lines.iter()
        .any(|line| line.contains("Divide"));
    
    assert!(!divide_executed, 
            "Divide should NOT be in execution flow, it should be lazily evaluated");
}

#[test]
fn test_cyclic_dependency_detection() {
    // 测试循环依赖检测
    // 这个测试需要构造一个循环的数据依赖图
    // 例如: A -> B -> C -> A
    
    // 注意：实际构造循环依赖比较复杂，这里只是框架
    // 真实测试需要根据具体节点类型来构造
    
    // TODO: 实现循环依赖测试
}
