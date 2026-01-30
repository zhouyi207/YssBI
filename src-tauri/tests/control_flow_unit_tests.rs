//! 控制流节点单元测试
//! 
//! 简化版测试，专注于节点注册和基本属性验证

use yssbi_lib::executor::{ExecutionModel, GenericNode};
use yssbi_lib::executor::node::registry::get_registry;
use yssbi_lib::executor::pin::{GenericInDataPin, GenericInExecPin, GenericOutExecPin};

// ============================================================================
// 节点注册测试
// ============================================================================

#[test]
fn test_all_control_nodes_registered() {
    let registry = get_registry();
    
    // 验证所有控制流节点都已注册
    assert!(registry.get_prototype("if_else").is_some(), "IfElse should be registered");
    assert!(registry.get_prototype("sequence").is_some(), "Sequence should be registered");
    assert!(registry.get_prototype("sequence5").is_some(), "Sequence5 should be registered");
    assert!(registry.get_prototype("while_loop").is_some(), "WhileLoop should be registered");
    assert!(registry.get_prototype("for_loop").is_some(), "ForLoop should be registered");
}

// ============================================================================
// IfElse 节点测试
// ============================================================================

#[test]
fn test_if_else_execution_model() {
    let registry = get_registry();
    let if_else = registry.get_prototype("if_else").expect("IfElse should be registered");
    
    // IfElse 应该是 Hybrid 节点（有 ExecPin 和 DataPin）
    assert_eq!(
        if_else.execution_model(),
        ExecutionModel::Hybrid,
        "IfElse should be a Hybrid node"
    );
}

#[test]
fn test_if_else_pin_structure() {
    // 创建 IfElse 节点原型
    let node = GenericNode::new_prototype("if_else", "If Else");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", ValueType::Boolean));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "True"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "False"));
    
    // 验证 Pin 数量
    let input_order = node.get_input_order();
    let output_order = node.get_output_order();
    
    assert_eq!(input_order.len(), 2, "IfElse should have 2 inputs (1 exec + 1 data)");
    assert_eq!(output_order.len(), 2, "IfElse should have 2 outputs (True + False)");
    
    // 验证执行模型
    assert_eq!(node.execution_model(), ExecutionModel::Hybrid);
}

#[test]
fn test_if_else_pin_names() {
    let node = GenericNode::new_prototype("if_else", "If Else");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", ValueType::Boolean));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "True"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "False"));
    
    // 验证输入 Pin 名称
    let input_info = node.get_ordered_input_info();
    assert_eq!(input_info[0].1, "In", "First input should be 'In'");
    assert_eq!(input_info[1].1, "Condition", "Second input should be 'Condition'");
    
    // 验证输出 Pin 名称
    let output_info = node.get_ordered_output_info();
    assert_eq!(output_info[0].1, "True", "First output should be 'True'");
    assert_eq!(output_info[1].1, "False", "Second output should be 'False'");
}

// ============================================================================
// Sequence 节点测试
// ============================================================================

#[test]
fn test_sequence_execution_model() {
    let registry = get_registry();
    let sequence = registry.get_prototype("sequence").expect("Sequence should be registered");
    
    // Sequence 应该是 ControlFlow 节点（只有 ExecPin）
    assert_eq!(
        sequence.execution_model(),
        ExecutionModel::ControlFlow,
        "Sequence should be a ControlFlow node"
    );
}

#[test]
fn test_sequence_pin_structure() {
    let node = GenericNode::new_prototype("sequence", "Sequence");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));
    
    // 验证 Pin 数量
    let input_order = node.get_input_order();
    let output_order = node.get_output_order();
    
    assert_eq!(input_order.len(), 1, "Sequence should have 1 input");
    assert_eq!(output_order.len(), 2, "Sequence should have 2 outputs");
    
    // 验证执行模型
    assert_eq!(node.execution_model(), ExecutionModel::ControlFlow);
}

#[test]
fn test_sequence_pin_names() {
    let node = GenericNode::new_prototype("sequence", "Sequence");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));
    
    let output_info = node.get_ordered_output_info();
    assert_eq!(output_info[0].1, "Then 0");
    assert_eq!(output_info[1].1, "Then 1");
}

// ============================================================================
// Sequence5 节点测试
// ============================================================================

#[test]
fn test_sequence5_execution_model() {
    let registry = get_registry();
    let sequence5 = registry.get_prototype("sequence5").expect("Sequence5 should be registered");
    
    assert_eq!(
        sequence5.execution_model(),
        ExecutionModel::ControlFlow,
        "Sequence5 should be a ControlFlow node"
    );
}

#[test]
fn test_sequence5_pin_structure() {
    let node = GenericNode::new_prototype("sequence5", "Sequence 5");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 2"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 3"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 4"));
    
    let output_order = node.get_output_order();
    assert_eq!(output_order.len(), 5, "Sequence5 should have 5 outputs");
}

#[test]
fn test_sequence5_pin_names() {
    let node = GenericNode::new_prototype("sequence5", "Sequence 5");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    
    for i in 0..5 {
        node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), &format!("Then {}", i)));
    }
    
    let output_info = node.get_ordered_output_info();
    for i in 0..5 {
        assert_eq!(output_info[i].1, format!("Then {}", i));
    }
}

// ============================================================================
// WhileLoop 节点测试
// ============================================================================

#[test]
fn test_while_loop_execution_model() {
    let registry = get_registry();
    let while_loop = registry.get_prototype("while_loop").expect("WhileLoop should be registered");
    
    // WhileLoop 应该是 Hybrid 节点
    assert_eq!(
        while_loop.execution_model(),
        ExecutionModel::Hybrid,
        "WhileLoop should be a Hybrid node"
    );
}

#[test]
fn test_while_loop_pin_structure() {
    let node = GenericNode::new_prototype("while_loop", "While Loop");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", ValueType::Boolean));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "MaxIterations", ValueType::Float64));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Loop Body"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Completed"));
    
    let input_order = node.get_input_order();
    let output_order = node.get_output_order();
    
    assert_eq!(input_order.len(), 3, "WhileLoop should have 3 inputs");
    assert_eq!(output_order.len(), 2, "WhileLoop should have 2 outputs");
}

#[test]
fn test_while_loop_pin_names() {
    let node = GenericNode::new_prototype("while_loop", "While Loop");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", ValueType::Boolean));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "MaxIterations", ValueType::Float64));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Loop Body"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Completed"));
    
    let input_info = node.get_ordered_input_info();
    assert_eq!(input_info[0].1, "In");
    assert_eq!(input_info[1].1, "Condition");
    assert_eq!(input_info[2].1, "MaxIterations");
    
    let output_info = node.get_ordered_output_info();
    assert_eq!(output_info[0].1, "Loop Body");
    assert_eq!(output_info[1].1, "Completed");
}

// ============================================================================
// ForLoop 节点测试
// ============================================================================

#[test]
fn test_for_loop_execution_model() {
    let registry = get_registry();
    let for_loop = registry.get_prototype("for_loop").expect("ForLoop should be registered");
    
    assert_eq!(
        for_loop.execution_model(),
        ExecutionModel::Hybrid,
        "ForLoop should be a Hybrid node"
    );
}

#[test]
fn test_for_loop_pin_structure() {
    let node = GenericNode::new_prototype("for_loop", "For Loop");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Start", ValueType::Float64));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "End", ValueType::Float64));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Step", ValueType::Float64));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Loop Body"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Completed"));
    
    let input_order = node.get_input_order();
    let output_order = node.get_output_order();
    
    assert_eq!(input_order.len(), 4, "ForLoop should have 4 inputs");
    assert_eq!(output_order.len(), 2, "ForLoop should have 2 outputs");
}

#[test]
fn test_for_loop_pin_names() {
    let node = GenericNode::new_prototype("for_loop", "For Loop");
    node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Start", ValueType::Float64));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "End", ValueType::Float64));
    node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Step", ValueType::Float64));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Loop Body"));
    node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Completed"));
    
    let input_info = node.get_ordered_input_info();
    assert_eq!(input_info[0].1, "In");
    assert_eq!(input_info[1].1, "Start");
    assert_eq!(input_info[2].1, "End");
    assert_eq!(input_info[3].1, "Step");
    
    let output_info = node.get_ordered_output_info();
    assert_eq!(output_info[0].1, "Loop Body");
    assert_eq!(output_info[1].1, "Completed");
}

// ============================================================================
// 执行模型汇总测试
// ============================================================================

#[test]
fn test_execution_models_summary() {
    let registry = get_registry();
    
    // ControlFlow 节点
    let control_flow_nodes = vec!["sequence", "sequence5"];
    for node_type in control_flow_nodes {
        let proto = registry.get_prototype(node_type).expect(&format!("{} should be registered", node_type));
        assert_eq!(
            proto.execution_model(),
            ExecutionModel::ControlFlow,
            "{} should be ControlFlow",
            node_type
        );
    }
    
    // Hybrid 节点
    let hybrid_nodes = vec!["if_else", "while_loop", "for_loop"];
    for node_type in hybrid_nodes {
        let proto = registry.get_prototype(node_type).expect(&format!("{} should be registered", node_type));
        assert_eq!(
            proto.execution_model(),
            ExecutionModel::Hybrid,
            "{} should be Hybrid",
            node_type
        );
    }
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
fn test_node_lookup_performance() {
    use std::time::Instant;
    
    let registry = get_registry();
    let start = Instant::now();
    
    // 查询 1000 次
    for _ in 0..1000 {
        let _ = registry.get_prototype("if_else");
        let _ = registry.get_prototype("sequence");
        let _ = registry.get_prototype("for_loop");
    }
    
    let duration = start.elapsed();
    
    // 3000 次查询应该在 10ms 内完成
    assert!(
        duration.as_millis() < 10,
        "Node lookup should be fast, took {:?}",
        duration
    );
}

#[test]
fn test_node_creation_performance() {
    use std::time::Instant;
use crate::executor::value::ValueType;
    
    let start = Instant::now();
    
    // 创建 100 个节点
    for _ in 0..100 {
        let _ = GenericNode::new_prototype("test", "Test");
    }
    
    let duration = start.elapsed();
    
    // 100 个节点创建应该在 10ms 内完成
    assert!(
        duration.as_millis() < 10,
        "Node creation should be fast, took {:?}",
        duration
    );
}
