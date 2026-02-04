//! 类型推断系统集成测试

use std::sync::Arc;
use yssbi_lib::executor::{
    graph::Graph,
    infer::{TypeConstraint, TypeVarId},
    node::NodeDefinition,
    pin::{DataRole, PinDefinition, PinTypeDesc},
    register::NodeRegistry,
    value::ValueType,
};

#[test]
fn test_type_inference_in_graph() {
    // 创建节点注册表
    let registry = Arc::new(NodeRegistry::new());

    // 注册一个 Number 常量节点
    let number_node = NodeDefinition::new("constant.number", "Number")
        .add_pin(PinDefinition::data_output(
            "Value",
            DataRole::Result,
            PinTypeDesc::concrete(ValueType::Float64),
        ));
    registry.register(number_node);

    // 注册一个 Print 节点（接受任意类型）
    let print_node = NodeDefinition::new("debug.print", "Print")
        .add_pin(PinDefinition::data_input(
            "Value",
            DataRole::Input(1),
            PinTypeDesc::unknown(), // Unknown 类型，可以接受任意类型
        ));
    registry.register(print_node);

    // 创建图
    let graph = Graph::new("test-graph", "Test Graph", registry.clone());

    // 创建节点
    let number_node_id = graph.create_node("constant.number").unwrap();
    let print_node_id = graph.create_node("debug.print").unwrap();

    // 获取 Pin
    let number_pins = graph.get_node_pins(number_node_id);
    let print_pins = graph.get_node_pins(print_node_id);

    let number_output = number_pins
        .iter()
        .find(|p| p.definition.name == "Value")
        .unwrap();
    let print_input = print_pins
        .iter()
        .find(|p| p.definition.name == "Value")
        .unwrap();

    // 连接节点（类型推断会自动进行）
    let result = graph.connect(number_output.id, print_input.id);
    assert!(result.is_ok(), "Connection should succeed with type inference");

    // 验证类型推断结果
    let inferred_type = graph.get_inferred_type(print_input.id);
    assert_eq!(
        inferred_type,
        Some(ValueType::Float64),
        "Print input should be inferred as Float64"
    );
}

#[test]
fn test_type_variable_inference() {
    // 创建节点注册表
    let registry = Arc::new(NodeRegistry::new());

    // 创建类型变量
    let type_var = TypeVarId::new();

    // 注册 Add 节点（使用类型变量）
    let add_node = NodeDefinition::new("math.add", "Add")
        .add_pin(PinDefinition::data_input(
            "A",
            DataRole::Operands(1),
            PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
        ))
        .add_pin(PinDefinition::data_input(
            "B",
            DataRole::Operands(2),
            PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
        ))
        .add_pin(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinTypeDesc::type_var(type_var),
        ));
    registry.register(add_node);

    // 注册 Number 节点
    let number_node = NodeDefinition::new("constant.number", "Number")
        .add_pin(PinDefinition::data_output(
            "Value",
            DataRole::Result,
            PinTypeDesc::concrete(ValueType::Float64),
        ));
    registry.register(number_node);

    // 创建图
    let graph = Graph::new("test-graph", "Test Graph", registry.clone());

    // 创建节点
    let number_node_id = graph.create_node("constant.number").unwrap();
    let add_node_id = graph.create_node("math.add").unwrap();

    // 获取 Pin
    let number_pins = graph.get_node_pins(number_node_id);
    let add_pins = graph.get_node_pins(add_node_id);

    let number_output = number_pins
        .iter()
        .find(|p| p.definition.name == "Value")
        .unwrap();
    let add_input_a = add_pins.iter().find(|p| p.definition.name == "A").unwrap();
    let add_input_b = add_pins.iter().find(|p| p.definition.name == "B").unwrap();
    let add_output = add_pins
        .iter()
        .find(|p| p.definition.name == "Result")
        .unwrap();

    // 连接 Number -> Add.A
    let result = graph.connect(number_output.id, add_input_a.id);
    assert!(
        result.is_ok(),
        "Connection should succeed with type variable inference"
    );

    // 验证类型推断：所有共享类型变量的 Pin 都应该被推断为 Float64
    let inferred_a = graph.get_inferred_type(add_input_a.id);
    let inferred_b = graph.get_inferred_type(add_input_b.id);
    let inferred_result = graph.get_inferred_type(add_output.id);

    assert_eq!(
        inferred_a,
        Some(ValueType::Float64),
        "Add.A should be inferred as Float64"
    );
    assert_eq!(
        inferred_b,
        Some(ValueType::Float64),
        "Add.B should be inferred as Float64 (same type variable)"
    );
    assert_eq!(
        inferred_result,
        Some(ValueType::Float64),
        "Add.Result should be inferred as Float64 (same type variable)"
    );
}

#[test]
fn test_type_constraint_validation() {
    // 创建节点注册表
    let registry = Arc::new(NodeRegistry::new());

    // 创建类型变量
    let type_var = TypeVarId::new();

    // 注册 Add 节点（只接受数字类型）
    let add_node = NodeDefinition::new("math.add", "Add")
        .add_pin(PinDefinition::data_input(
            "A",
            DataRole::Operands(1),
            PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
        ));
    registry.register(add_node);

    // 注册 String 节点
    let string_node = NodeDefinition::new("constant.string", "String")
        .add_pin(PinDefinition::data_output(
            "Value",
            DataRole::Result,
            PinTypeDesc::concrete(ValueType::String),
        ));
    registry.register(string_node);

    // 创建图
    let graph = Graph::new("test-graph", "Test Graph", registry.clone());

    // 创建节点
    let string_node_id = graph.create_node("constant.string").unwrap();
    let add_node_id = graph.create_node("math.add").unwrap();

    // 获取 Pin
    let string_pins = graph.get_node_pins(string_node_id);
    let add_pins = graph.get_node_pins(add_node_id);

    let string_output = string_pins
        .iter()
        .find(|p| p.definition.name == "Value")
        .unwrap();
    let add_input = add_pins.iter().find(|p| p.definition.name == "A").unwrap();

    // 尝试连接 String -> Add.A（应该失败，因为 String 不满足 Numeric 约束）
    let result = graph.connect(string_output.id, add_input.id);
    assert!(
        result.is_err(),
        "Connection should fail: String does not satisfy Numeric constraint"
    );
}

#[test]
fn test_pin_compatibility_check() {
    // 创建节点注册表
    let registry = Arc::new(NodeRegistry::new());

    // 注册节点
    let float_node = NodeDefinition::new("constant.float", "Float")
        .add_pin(PinDefinition::data_output(
            "Value",
            DataRole::Result,
            PinTypeDesc::concrete(ValueType::Float64),
        ));
    registry.register(float_node);

    let int_node = NodeDefinition::new("constant.int", "Int")
        .add_pin(PinDefinition::data_input(
            "Value",
            DataRole::PrimaryInput,
            PinTypeDesc::concrete(ValueType::Int64),
        ));
    registry.register(int_node);

    // 创建图
    let graph = Graph::new("test-graph", "Test Graph", registry.clone());

    // 创建节点
    let float_node_id = graph.create_node("constant.float").unwrap();
    let int_node_id = graph.create_node("constant.int").unwrap();

    // 获取 Pin
    let float_pins = graph.get_node_pins(float_node_id);
    let int_pins = graph.get_node_pins(int_node_id);

    let float_output = float_pins
        .iter()
        .find(|p| p.definition.name == "Value")
        .unwrap();
    let int_input = int_pins
        .iter()
        .find(|p| p.definition.name == "Value")
        .unwrap();

    // 检查兼容性（Float64 和 Int64 在类型推断系统中是兼容的）
    let compatible = graph.are_pins_compatible(float_output.id, int_input.id);
    assert!(
        compatible,
        "Float64 and Int64 should be compatible (numeric types)"
    );
}
