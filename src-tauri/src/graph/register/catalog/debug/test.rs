#[cfg(test)]
mod tests {
    use crate::execution::Executor;
    use crate::graph::{
        GraphData,
        pin::{DataRole, ExecRole, PinRole},
        register::NodeRegistry,
        value::DataValue,
    };
    use std::sync::Arc;

    /// 创建测试用的注册表
    fn create_test_registry() -> Arc<NodeRegistry> {
        let registry = Arc::new(NodeRegistry::new());
        // 注册所有节点
        crate::graph::register::catalog::register_builtin_nodes(&registry);
        registry
    }

    #[test]
    fn test_print_node_basic() {
        // 测试 Print 节点的基本功能
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        // 创建 Print 节点
        let print_node = graph
            .create_node("debug.print")
            .expect("Failed to create print node");

        // 获取所有 Pin
        let pins = graph.get_node_pins(print_node);

        // 找到 Message 输入 Pin
        let message_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
            .expect("Message pin not found");

        // 设置消息
        graph
            .set_pin_user_value(message_pin.id, Some(DataValue::String("Hello from test!".to_string())))
            .expect("Failed to set message value");

        // 使用 Executor 执行
        let mut executor = Executor::new(graph.clone());
        let result = executor.start(print_node);

        assert!(
            result.is_ok(),
            "Executor failed: {:?}",
            result.err()
        );

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        println!("\n=== Test Passed ===");
        println!("Print node executed successfully with message: 'Hello from test!'");
    }

    #[test]
    fn test_print_node_default_message() {
        // 测试 Print 节点使用默认消息
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        // 创建 Print 节点（不设置消息，使用默认值）
        let print_node = graph
            .create_node("debug.print")
            .expect("Failed to create print node");

        // 使用 Executor 执行
        let mut executor = Executor::new(graph.clone());
        let result = executor.start(print_node);

        assert!(
            result.is_ok(),
            "Executor failed: {:?}",
            result.err()
        );

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        println!("\n=== Test Passed ===");
        println!("Print node executed successfully with default message: 'Hello, World!'");
    }

    #[test]
    fn test_print_node_chain() {
        // 测试多个 Print 节点的链式执行
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        // 创建三个 Print 节点
        let print1_node = graph
            .create_node("debug.print")
            .expect("Failed to create print1 node");

        let print2_node = graph
            .create_node("debug.print")
            .expect("Failed to create print2 node");

        let print3_node = graph
            .create_node("debug.print")
            .expect("Failed to create print3 node");

        // 设置消息
        let print1_pins = graph.get_node_pins(print1_node);
        let print1_message = print1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
            .expect("Print1 message pin not found");

        graph
            .set_pin_user_value(print1_message.id, Some(DataValue::String("First".to_string())))
            .expect("Failed to set print1 message");

        let print2_pins = graph.get_node_pins(print2_node);
        let print2_message = print2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
            .expect("Print2 message pin not found");

        graph
            .set_pin_user_value(print2_message.id, Some(DataValue::String("Second".to_string())))
            .expect("Failed to set print2 message");

        let print3_pins = graph.get_node_pins(print3_node);
        let print3_message = print3_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
            .expect("Print3 message pin not found");

        graph
            .set_pin_user_value(print3_message.id, Some(DataValue::String("Third".to_string())))
            .expect("Failed to set print3 message");

        // 连接：print1.out -> print2.in -> print3.in
        let print1_out = print1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecOut))
            .expect("Print1 out pin not found");

        let print2_in = print2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Print2 in pin not found");

        graph
            .connect(print1_out.id, print2_in.id)
            .expect("Failed to connect print1 to print2");

        let print2_out = print2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecOut))
            .expect("Print2 out pin not found");

        let print3_in = print3_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Print3 in pin not found");

        graph
            .connect(print2_out.id, print3_in.id)
            .expect("Failed to connect print2 to print3");

        // 使用 Executor 执行
        let mut executor = Executor::new(graph.clone());
        let result = executor.start(print1_node);

        assert!(
            result.is_ok(),
            "Executor failed: {:?}",
            result.err()
        );

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        println!("\n=== Test Passed ===");
        println!("Print chain executed successfully: First -> Second -> Third");
    }
}
