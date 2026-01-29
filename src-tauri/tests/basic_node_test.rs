//! 基本节点功能测试

use yssbi_lib::executor::{BasePin, GenericNode, GenericInDataPin};

#[test]
fn test_basic_node_creation() {
    let node = GenericNode::new_prototype("test_node", "Test Node");
    
    // 测试基本属性
    assert_eq!(node.node_type(), "test_node");
    
    // 测试初始状态
    let input_order = node.get_input_order();
    let output_order = node.get_output_order();
    
    assert!(input_order.is_empty());
    assert!(output_order.is_empty());
}

#[test]
fn test_simple_pin_addition() {
    let node = GenericNode::new_prototype("test_node", "Test Node");
    
    // 添加一个简单的输入 Pin
    let pin = GenericInDataPin::new(uuid::Uuid::new_v4(), "TestInput", "string");
    let pin_id = pin.id();
    
    node.add_input(pin);
    
    // 验证 Pin 被正确添加
    let input_order = node.get_input_order();
    assert_eq!(input_order.len(), 1);
    assert_eq!(input_order[0], pin_id);
    
    // 验证调试信息
    let input_info = node.get_ordered_input_info();
    assert_eq!(input_info.len(), 1);
    assert_eq!(input_info[0].1, "TestInput");
    assert_eq!(input_info[0].2, "string");
}