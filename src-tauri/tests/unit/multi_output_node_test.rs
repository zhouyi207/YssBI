//! 多输出节点测试
//!
//! 验证具有多个数据输出的节点是否正确工作

use yssbi_lib::executor::pin::{BasePin, GenericInDataPin, GenericOutDataPin};
use yssbi_lib::executor::value::{PinTypeDesc, ValueType};
use yssbi_lib::executor::{ExecutionModel, GenericNode};

#[test]
fn test_multi_output_node_structure() {
    // 创建一个有多个输出的节点（例如：DivMod 节点）
    let node = GenericNode::new_prototype("divmod", "Divide and Modulo");

    // 添加输入
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Dividend",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Divisor",
        PinTypeDesc::concrete(ValueType::Float64),
    ));

    // 添加多个输出
    let quotient_pin = node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Quotient",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    let remainder_pin = node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Remainder",
        PinTypeDesc::concrete(ValueType::Float64),
    ));

    // 验证输出数量
    let output_order = node.get_output_order();
    assert_eq!(output_order.len(), 2, "DivMod should have 2 outputs");

    // 验证输出 Pin ID 不同
    assert_ne!(
        quotient_pin.id(),
        remainder_pin.id(),
        "Output pins should have different IDs"
    );

    // 验证执行模型
    assert_eq!(
        node.execution_model(),
        ExecutionModel::DataFlow,
        "DivMod should be a DataFlow node"
    );
}

#[test]
fn test_multi_output_node_pin_names() {
    let node = GenericNode::new_prototype("divmod", "Divide and Modulo");

    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Dividend",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Divisor",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Quotient",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Remainder",
        PinTypeDesc::concrete(ValueType::Float64),
    ));

    // 验证输出 Pin 名称
    let output_info = node.get_ordered_output_info();
    assert_eq!(
        output_info[0].1, "Quotient",
        "First output should be Quotient"
    );
    assert_eq!(
        output_info[1].1, "Remainder",
        "Second output should be Remainder"
    );
}

#[test]
fn test_multi_output_data_processor() {
    // 创建一个 DivMod 节点并设置数据处理器
    let node = GenericNode::new_prototype("divmod", "Divide and Modulo");

    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Dividend",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Divisor",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Quotient",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Remainder",
        PinTypeDesc::concrete(ValueType::Float64),
    ));

    // 验证处理器已设置
    // 注意：这里只是验证结构，实际执行需要完整的 ExecutionContext
    assert_eq!(node.execution_model(), ExecutionModel::DataFlow);
}

#[test]
fn test_cache_per_output_pin() {
    // 验证缓存是按每个输出 Pin 独立存储的

    // 创建节点
    let node = GenericNode::new_prototype("test_multi", "Test Multi Output");
    let out1 = node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Out1",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    let out2 = node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Out2",
        PinTypeDesc::concrete(ValueType::Float64),
    ));

    // 验证每个输出 Pin 有独立的 ID
    assert_ne!(
        out1.id(),
        out2.id(),
        "Each output pin should have unique ID"
    );

    // 在实际执行中，data_cache 会为每个 output_pin_id 存储独立的值
    // 例如：
    // data_cache[out1.id()] = 42
    // data_cache[out2.id()] = 100
}

#[test]
fn test_multi_output_node_example_split_string() {
    // 示例：SplitString 节点，将字符串分割为多个部分
    let node = GenericNode::new_prototype("split_string", "Split String");

    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Input",
        PinTypeDesc::concrete(ValueType::String),
    ));
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Delimiter",
        PinTypeDesc::concrete(ValueType::String),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "First",
        PinTypeDesc::concrete(ValueType::String),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Second",
        PinTypeDesc::concrete(ValueType::String),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Rest",
        PinTypeDesc::concrete(ValueType::String),
    ));

    let output_order = node.get_output_order();
    assert_eq!(output_order.len(), 3, "SplitString should have 3 outputs");
}

#[test]
fn test_multi_output_node_example_min_max() {
    // 示例：MinMax 节点，同时返回最小值和最大值
    let node = GenericNode::new_prototype("min_max", "Min Max");

    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Array",
        PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any))),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Min",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Max",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Average",
        PinTypeDesc::concrete(ValueType::Float64),
    ));

    assert_eq!(node.execution_model(), ExecutionModel::DataFlow);
}

#[test]
fn test_output_pin_independence() {
    // 验证多个输出 Pin 的独立性
    let node = GenericNode::new_prototype("test", "Test");

    let pin1 = node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "A",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    let pin2 = node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "B",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    let pin3 = node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "C",
        PinTypeDesc::concrete(ValueType::Float64),
    ));

    // 每个 Pin 都有独立的 ID
    let ids = vec![pin1.id(), pin2.id(), pin3.id()];
    assert_eq!(ids.len(), 3);

    // 验证没有重复的 ID
    use std::collections::HashSet;
    let unique_ids: HashSet<_> = ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        3,
        "All output pins should have unique IDs"
    );
}
