//! GenericNode Pin 顺序追踪集成测试

// 对于集成测试，我们需要通过 lib crate 名称导入
// 在 Cargo.toml 中，lib name 是 "yssbi_lib"
use yssbi_lib::executor::value::{PinTypeDesc, ValueType};
use yssbi_lib::executor::pin::{
    BasePin, GenericInDataPin, GenericInExecPin, GenericOutDataPin, GenericOutExecPin,
};
use yssbi_lib::executor::GenericNode;

#[test]
fn test_pin_ordering() {
    let node = GenericNode::new_prototype("test_node", "Test Node");

    // 添加一些 Pin（注意添加顺序）
    let exec_in = GenericInExecPin::new(uuid::Uuid::new_v4(), "Execute");
    let data_in1 = GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "Input1",
        PinTypeDesc::concrete(ValueType::String),
    );
    let data_in2 = GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "Input2",
        PinTypeDesc::concrete(ValueType::Float64),
    );

    let exec_out = GenericOutExecPin::new(uuid::Uuid::new_v4(), "Done");
    let data_out1 = GenericOutDataPin::new(
        uuid::Uuid::new_v4(),
        "Output1",
        PinTypeDesc::concrete(ValueType::String),
    );
    let data_out2 = GenericOutDataPin::new(
        uuid::Uuid::new_v4(),
        "Output2",
        PinTypeDesc::concrete(ValueType::Float64),
    );

    // 按特定顺序添加 Pin
    node.add_in_exec_pin(exec_in);
    node.add_input(data_in1);
    node.add_input(data_in2);

    node.add_out_exec_pin(exec_out);
    node.add_output(data_out1);
    node.add_output(data_out2);

    // 验证顺序
    let input_info = node.get_ordered_input_info();
    let output_info = node.get_ordered_output_info();

    println!("Input order:");
    for (pin_id, name, pin_type) in &input_info {
        println!("  {:?}: {} ({})", pin_id, name, pin_type);
    }

    println!("Output order:");
    for (pin_id, name, pin_type) in &output_info {
        println!("  {:?}: {} ({})", pin_id, name, pin_type);
    }

    // 验证顺序是否正确
    assert_eq!(input_info.len(), 3);
    assert_eq!(input_info[0].1, "Execute");
    assert_eq!(input_info[1].1, "Input1");
    assert_eq!(input_info[2].1, "Input2");

    assert_eq!(output_info.len(), 3);
    assert_eq!(output_info[0].1, "Done");
    assert_eq!(output_info[1].1, "Output1");
    assert_eq!(output_info[2].1, "Output2");
}

#[test]
fn test_pin_reordering() {
    let node = GenericNode::new_prototype("test_node", "Test Node");

    // 添加一些输入 Pin
    let pin1 = node.add_input(GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "First",
        PinTypeDesc::concrete(ValueType::String),
    ));
    let pin2 = node.add_input(GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "Second",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    let pin3 = node.add_input(GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "Third",
        PinTypeDesc::concrete(ValueType::Boolean),
    ));

    let pin1_id = pin1.id();
    let pin2_id = pin2.id();
    let pin3_id = pin3.id();

    // 验证初始顺序
    let initial_order = node.get_input_order();
    assert_eq!(initial_order, vec![pin1_id, pin2_id, pin3_id]);

    // 重新排序
    let new_order = vec![pin3_id, pin1_id, pin2_id];
    assert!(node.reorder_inputs(new_order.clone()).is_ok());

    // 验证新顺序
    let reordered = node.get_input_order();
    assert_eq!(reordered, new_order);

    // 验证序列化时也按新顺序
    let input_info = node.get_ordered_input_info();
    assert_eq!(input_info[0].1, "Third");
    assert_eq!(input_info[1].1, "First");
    assert_eq!(input_info[2].1, "Second");
}

#[test]
fn test_pin_removal_updates_order() {
    let node = GenericNode::new_prototype("test_node", "Test Node");

    // 添加一些输入 Pin
    let pin1 = node.add_input(GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "First",
        PinTypeDesc::concrete(ValueType::String),
    ));
    let pin2 = node.add_input(GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "Second",
        PinTypeDesc::concrete(ValueType::Float64),
    ));
    let pin3 = node.add_input(GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "Third",
        PinTypeDesc::concrete(ValueType::Boolean),
    ));

    let pin1_id = pin1.id();
    let pin2_id = pin2.id();
    let pin3_id = pin3.id();

    // 验证初始顺序
    let initial_order = node.get_input_order();
    assert_eq!(initial_order, vec![pin1_id, pin2_id, pin3_id]);

    // 移除中间的 Pin
    assert!(node.remove_input(pin2_id));

    // 验证顺序已更新
    let updated_order = node.get_input_order();
    assert_eq!(updated_order, vec![pin1_id, pin3_id]);

    // 验证信息也正确更新
    let input_info = node.get_ordered_input_info();
    assert_eq!(input_info.len(), 2);
    assert_eq!(input_info[0].1, "First");
    assert_eq!(input_info[1].1, "Third");
}

#[test]
fn test_serialization_order() {
    let node = GenericNode::new_prototype("test_node", "Test Node");

    // 添加 Pin 的特定顺序
    let exec_in = GenericInExecPin::new(uuid::Uuid::new_v4(), "Execute");
    let data_in = GenericInDataPin::new(
        uuid::Uuid::new_v4(),
        "Data",
        PinTypeDesc::concrete(ValueType::String),
    );
    let exec_out = GenericOutExecPin::new(uuid::Uuid::new_v4(), "Done");
    let data_out = GenericOutDataPin::new(
        uuid::Uuid::new_v4(),
        "Result",
        PinTypeDesc::concrete(ValueType::String),
    );

    node.add_in_exec_pin(exec_in);
    node.add_input(data_in);
    node.add_out_exec_pin(exec_out);
    node.add_output(data_out);

    // 序列化节点
    let serialized = serde_json::to_string(&node).expect("Failed to serialize node");
    println!("Serialized node: {}", serialized);

    // 解析序列化结果来验证顺序
    let parsed: serde_json::Value =
        serde_json::from_str(&serialized).expect("Failed to parse serialized node");

    let inputs = parsed["inputs"]
        .as_array()
        .expect("inputs should be an array");
    let outputs = parsed["outputs"]
        .as_array()
        .expect("outputs should be an array");

    // 验证输入顺序
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0]["name"].as_str().unwrap(), "Execute");
    assert_eq!(inputs[0]["type"].as_str().unwrap(), "exec");
    assert_eq!(inputs[1]["name"].as_str().unwrap(), "Data");
    assert_eq!(inputs[1]["type"].as_str().unwrap(), "string");

    // 验证输出顺序
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0]["name"].as_str().unwrap(), "Done");
    assert_eq!(outputs[0]["type"].as_str().unwrap(), "exec");
    assert_eq!(outputs[1]["name"].as_str().unwrap(), "Result");
    assert_eq!(outputs[1]["type"].as_str().unwrap(), "string");
}
