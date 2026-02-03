//! 控制流示例
//!
//! 演示 If-Else 节点的使用

use std::sync::Arc;
use yssbi_lib::executor::*;

fn main() {
    println!("=== 控制流示例 ===\n");

    // 创建注册中心
    let registry = Arc::new(NodeRegistry::new());
    node::catalog::register_builtin_nodes(&registry);

    // 创建 Graph
    let graph = Graph::new("control", "Control Flow", registry);

    // 创建节点
    let if_node = graph.create_node("if_else").expect("Failed to create if_else");
    let add_node = graph.create_node("add").expect("Failed to create add");
    let mul_node = graph.create_node("multiply").expect("Failed to create multiply");

    println!("创建了 3 个节点：If-Else, Add, Multiply\n");

    // 设置 If-Else 的条件为 true
    let if_pins = graph.get_node_pins(if_node);
    for pin in &if_pins {
        if pin.is_input() && pin.is_data() && pin.name == "Condition" {
            graph.set_pin_user_value(pin.id, Some(DataValue::Boolean(true)))
                .expect("Failed to set condition");
            println!("设置条件 = true");
        }
    }

    // 设置 Add 节点的输入
    let add_pins = graph.get_node_pins(add_node);
    for pin in &add_pins {
        if pin.is_input() && pin.is_data() {
            match pin.name.as_str() {
                "A" => graph.set_pin_user_value(pin.id, Some(DataValue::Float64(10.0))).unwrap(),
                "B" => graph.set_pin_user_value(pin.id, Some(DataValue::Float64(5.0))).unwrap(),
                _ => {}
            }
        }
    }

    // 设置 Multiply 节点的输入
    let mul_pins = graph.get_node_pins(mul_node);
    for pin in &mul_pins {
        if pin.is_input() && pin.is_data() {
            match pin.name.as_str() {
                "A" => graph.set_pin_user_value(pin.id, Some(DataValue::Float64(10.0))).unwrap(),
                "B" => graph.set_pin_user_value(pin.id, Some(DataValue::Float64(2.0))).unwrap(),
                _ => {}
            }
        }
    }

    println!("设置 Add: 10 + 5");
    println!("设置 Multiply: 10 * 2\n");

    // 注意：Add 和 Multiply 是纯数据节点，没有执行 Pin
    // If-Else 节点会根据条件选择执行分支
    println!("架构说明:");
    println!("  - If-Else 是控制流节点，有执行 Pin");
    println!("  - Add 和 Multiply 是数据节点，没有执行 Pin");
    println!("  - 当前示例展示数据节点的独立执行\n");

    // 执行
    let mut executor = GraphExecutor::new();
    match executor.execute(&graph) {
        Ok(_) => {
            println!("✓ 执行成功\n");
            
            println!("执行日志:");
            for log in executor.logs() {
                println!("  {}", log);
            }

            println!("\n结果:");
            let add_out = add_pins.iter().find(|p| p.is_output() && p.is_data()).unwrap();
            let mul_out = mul_pins.iter().find(|p| p.is_output() && p.is_data()).unwrap();

            if let Some(add_result) = graph.get_pin_value(add_out.id) {
                println!("  Add 结果: {:?}", add_result);
            }
            if let Some(mul_result) = graph.get_pin_value(mul_out.id) {
                println!("  Multiply 结果: {:?}", mul_result);
            }
        }
        Err(e) => println!("✗ 执行失败: {}", e),
    }

    println!("\n=== 示例完成 ===");
}
