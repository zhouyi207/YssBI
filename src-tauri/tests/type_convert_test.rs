use std::sync::Arc;
use yssbi_lib::execution::Executor;
use yssbi_lib::graph::{
    core::GraphInstance,
    pin::{DataRole, PinRole},
    register::NodeRegistry,
    value::DataValue,
};

/// 创建测试用的注册表
fn create_test_registry() -> Arc<NodeRegistry> {
    let registry = Arc::new(NodeRegistry::new());
    // 注册所有内置节点
    yssbi_lib::graph::register::catalog::register_builtin_nodes(&registry);
    registry
}

#[test]
fn test_multiple_type_conversions() {
    // 测试多种类型转换
    //
    // 图结构：
    // 1. Boolean(true) -> Convert -> Print (应输出 "true")
    // 2. Int64(100) -> Convert -> Print (应输出 "100")
    // 3. Float64(3.14) -> Convert -> Print (应输出 "3.14")

    let registry = create_test_registry();
    let graph = Arc::new(GraphInstance::new(
        "Multiple Type Conversion Test",
        yssbi_lib::graph::GraphKind::Event,
        registry.clone(),
    ));

    println!("\n=== Creating Nodes ===");

    // 创建常量节点
    let bool_const = graph
        .create_node("Value:Constants:Boolean")
        .expect("Failed to create boolean constant");
    let int64_const = graph
        .create_node("Value:Constants:Int64")
        .expect("Failed to create int64 constant");
    let float64_const = graph
        .create_node("Value:Constants:Float64")
        .expect("Failed to create float64 constant");

    // 创建 Convert 节点
    let convert_bool = graph
        .create_node("Value:Conversion:Convert")
        .expect("Failed to create convert node");
    let convert_int64 = graph
        .create_node("Value:Conversion:Convert")
        .expect("Failed to create convert node");
    let convert_float64 = graph
        .create_node("Value:Conversion:Convert")
        .expect("Failed to create convert node");

    // 创建 Print 节点
    let print_bool = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_int64 = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_float64 = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");

    println!("Created all nodes");

    println!("\n=== Setting Values ===");

    // 设置 Boolean 常量
    let bool_pins = graph.get_pin_instances_by_node_id(bool_const);
    let bool_output = bool_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
        .expect("Bool output not found");
    graph
        .set_pin_user_value_by_pin_id(bool_output.id, DataValue::Boolean(true))
        .expect("Failed to set bool value");
    println!("Set boolean value: true");

    // 设置 Int64 常量
    let int64_pins = graph.get_pin_instances_by_node_id(int64_const);
    let int64_output = int64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
        .expect("Int64 output not found");
    graph
        .set_pin_user_value_by_pin_id(int64_output.id, DataValue::Int64(100))
        .expect("Failed to set int64 value");
    println!("Set int64 value: 100");

    // 设置 Float64 常量
    let float64_pins = graph.get_pin_instances_by_node_id(float64_const);
    let float64_output = float64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
        .expect("Float64 output not found");
    graph
        .set_pin_user_value_by_pin_id(float64_output.id, DataValue::Float64(3.14))
        .expect("Failed to set float64 value");
    println!("Set float64 value: 3.14");

    println!("\n=== Connecting Nodes ===");

    // 连接 Boolean 路径
    let convert_bool_pins = graph.get_pin_instances_by_node_id(convert_bool);
    let convert_bool_input = convert_bool_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
        .unwrap();
    let convert_bool_output = convert_bool_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
        .unwrap();

    graph
        .connect(bool_output.id, convert_bool_input.id)
        .unwrap();

    let print_bool_pins = graph.get_pin_instances_by_node_id(print_bool);
    let print_bool_message = print_bool_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
        .unwrap();

    graph
        .connect(convert_bool_output.id, print_bool_message.id)
        .unwrap();
    println!("Connected Boolean path");

    // 连接 Int64 路径
    let convert_int64_pins = graph.get_pin_instances_by_node_id(convert_int64);
    let convert_int64_input = convert_int64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
        .unwrap();
    let convert_int64_output = convert_int64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
        .unwrap();

    graph
        .connect(int64_output.id, convert_int64_input.id)
        .unwrap();

    let print_int64_pins = graph.get_pin_instances_by_node_id(print_int64);
    let print_int64_message = print_int64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
        .unwrap();

    graph
        .connect(convert_int64_output.id, print_int64_message.id)
        .unwrap();
    println!("Connected Int64 path");

    // 连接 Float64 路径
    let convert_float64_pins = graph.get_pin_instances_by_node_id(convert_float64);
    let convert_float64_input = convert_float64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
        .unwrap();
    let convert_float64_output = convert_float64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
        .unwrap();

    graph
        .connect(float64_output.id, convert_float64_input.id)
        .unwrap();

    let print_float64_pins = graph.get_pin_instances_by_node_id(print_float64);
    let print_float64_message = print_float64_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
        .unwrap();

    graph
        .connect(convert_float64_output.id, print_float64_message.id)
        .unwrap();
    println!("Connected Float64 path");

    println!("\n=== Executing Graph ===");

    // 执行所有 print 节点
    use yssbi_lib::graph::core::GraphRuntime;
    let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new(graph.clone())));
    let mut executor = Executor::new(runtime);

    println!("\n--- Executing Boolean conversion ---");
    let result = executor.start(print_bool);
    assert!(result.is_ok(), "Boolean conversion failed");

    println!("\n--- Executing Int64 conversion ---");
    let result = executor.start(print_int64);
    assert!(result.is_ok(), "Int64 conversion failed");

    println!("\n--- Executing Float64 conversion ---");
    let result = executor.start(print_float64);
    assert!(result.is_ok(), "Float64 conversion failed");

    println!("\n=== Execution Logs ===");
    let logs = executor.logs();
    for log in logs {
        println!("{}", log);
    }

    // 验证输出
    println!("\n=== Verifying Outputs ===");

    assert!(
        logs.iter().any(|log| log.contains("Print: true")),
        "Expected 'true' output not found"
    );
    println!("✓ Boolean conversion: true");

    assert!(
        logs.iter().any(|log| log.contains("Print: 100")),
        "Expected '100' output not found"
    );
    println!("✓ Int64 conversion: 100");

    assert!(
        logs.iter().any(|log| log.contains("Print: 3.14")),
        "Expected '3.14' output not found"
    );
    println!("✓ Float64 conversion: 3.14");

    println!("\n=== Test Completed Successfully ===");
}
