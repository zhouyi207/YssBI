//! 基础 Graph 示例
//!
//! 演示如何使用新架构创建和执行一个简单的数学计算图

use std::sync::Arc;
use yssbi_lib::executor::{
    node::{NodeRegistry, catalog::register_builtin_nodes},
    graph::{Graph, GraphExecutor},
    value::DataValue,
};

fn main() {
    println!("=== 基础 Graph 示例 ===\n");

    // 1. 创建节点注册中心并注册内置节点
    let registry = Arc::new(NodeRegistry::new());
    register_builtin_nodes(&registry);
    println!("✓ 注册了 {} 个节点类型", registry.node_types().len());

    // 2. 创建 Graph
    let graph = Graph::new("main", "Main Graph", registry.clone());
    println!("✓ 创建了 Graph: {}", graph.name);

    // 3. 创建节点：计算 (5 + 3) * 2
    println!("\n创建节点...");
    
    // Add 节点：5 + 3
    let add_node = graph.create_node("add").expect("Failed to create add node");
    println!("  - Add 节点: {:?}", add_node);
    
    // Multiply 节点：result * 2
    let mul_node = graph.create_node("multiply").expect("Failed to create multiply node");
    println!("  - Multiply 节点: {:?}", mul_node);

    // 4. 设置输入值
    println!("\n设置输入值...");
    let add_pins = graph.get_node_pins(add_node);
    
    // 找到 Add 节点的输入 Pin（通过 Role）
    for pin in &add_pins {
        if pin.is_input() && pin.is_data() {
            match pin.name.as_str() {
                "A" => {
                    graph.set_pin_user_value(pin.id, Some(DataValue::Float64(5.0)))
                        .expect("Failed to set value");
                    println!("  - 设置 Add.A = 5.0");
                }
                "B" => {
                    graph.set_pin_user_value(pin.id, Some(DataValue::Float64(3.0)))
                        .expect("Failed to set value");
                    println!("  - 设置 Add.B = 3.0");
                }
                _ => {}
            }
        }
    }

    let mul_pins = graph.get_node_pins(mul_node);
    for pin in &mul_pins {
        if pin.is_input() && pin.is_data() && pin.name == "B" {
            graph.set_pin_user_value(pin.id, Some(DataValue::Float64(2.0)))
                .expect("Failed to set value");
            println!("  - 设置 Multiply.B = 2.0");
        }
    }

    // 5. 连接节点
    println!("\n连接节点...");
    
    // 找到 Add 的输出和 Multiply 的第一个输入
    let add_output = add_pins.iter()
        .find(|p| p.is_output() && p.is_data())
        .expect("Add output not found");
    
    let mul_input_a = mul_pins.iter()
        .find(|p| p.is_input() && p.is_data() && p.name == "A")
        .expect("Multiply input A not found");

    graph.connect(add_output.id, mul_input_a.id)
        .expect("Failed to connect");
    println!("  - 连接 Add.Result -> Multiply.A");

    // 6. 执行 Graph
    println!("\n执行 Graph...");
    let mut executor = GraphExecutor::new();
    
    match executor.execute(&graph) {
        Ok(_) => {
            println!("✓ 执行成功！");
            
            // 7. 查看结果
            println!("\n执行日志:");
            for log in executor.logs() {
                println!("  {}", log);
            }

            println!("\n结果:");
            let mul_output = mul_pins.iter()
                .find(|p| p.is_output() && p.is_data())
                .expect("Multiply output not found");
            
            if let Some(result) = graph.get_pin_value(mul_output.id) {
                println!("  (5 + 3) * 2 = {:?}", result);
            }
        }
        Err(e) => {
            println!("✗ 执行失败: {}", e);
        }
    }

    println!("\n=== 示例完成 ===");
}
