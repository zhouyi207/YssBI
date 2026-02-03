//! 集成测试

use std::sync::Arc;
use yssbi_lib::executor::*;

#[test]
fn test_basic_math_graph() {
    // 创建注册中心
    let registry = Arc::new(NodeRegistry::new());
    node::catalog::register_builtin_nodes(&registry);

    // 创建 Graph
    let graph = Graph::new("test", "Test Graph", registry);

    // 创建 Add 节点
    let add_node = graph.create_node("add").unwrap();
    let add_pins = graph.get_node_pins(add_node);

    // 设置输入
    for pin in &add_pins {
        if pin.is_input() && pin.name == "A" {
            graph.set_pin_user_value(pin.id, Some(DataValue::Float64(10.0))).unwrap();
        }
        if pin.is_input() && pin.name == "B" {
            graph.set_pin_user_value(pin.id, Some(DataValue::Float64(20.0))).unwrap();
        }
    }

    // 执行
    let mut executor = GraphExecutor::new();
    executor.execute(&graph).unwrap();

    // 验证结果
    let output = add_pins.iter().find(|p| p.is_output()).unwrap();
    let result = graph.get_pin_value(output.id).unwrap();
    
    assert_eq!(result.as_f64(), Some(30.0));
}

#[test]
fn test_connection_cycle_detection() {
    let registry = Arc::new(NodeRegistry::new());
    node::catalog::register_builtin_nodes(&registry);

    let graph = Graph::new("test", "Test", registry);

    // 创建两个节点
    let node1 = graph.create_node("add").unwrap();
    let node2 = graph.create_node("multiply").unwrap();

    let pins1 = graph.get_node_pins(node1);
    let pins2 = graph.get_node_pins(node2);

    let out1 = pins1.iter().find(|p| p.is_output()).unwrap();
    let in2 = pins2.iter().find(|p| p.is_input() && p.name == "A").unwrap();
    let out2 = pins2.iter().find(|p| p.is_output()).unwrap();
    let in1 = pins1.iter().find(|p| p.is_input() && p.name == "A").unwrap();

    // 连接 node1 -> node2
    graph.connect(out1.id, in2.id).unwrap();

    // 尝试连接 node2 -> node1（会形成循环）
    let result = graph.connect(out2.id, in1.id);
    assert!(result.is_err());
}
