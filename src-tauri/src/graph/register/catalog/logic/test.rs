#[cfg(test)]
mod tests {
    use crate::graph::{
        GraphData,
        pin::{DataRole, PinRole},
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
    fn test_equal_node_int32() {
        // 测试 Equal 节点比较 Int32 类型
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        // 创建 Equal 节点
        let equal_node = graph
            .create_node("logic.equal")
            .expect("Failed to create equal node");

        // 获取所有 Pin
        let pins = graph.get_node_pins(equal_node);

        // 找到 A 和 B 输入 Pin
        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");

        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        // 设置相等的值
        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Int32(42)))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Int32(42)))
            .expect("Failed to set pin B value");

        // 执行节点
        let definition = graph
            .get_node_definition(equal_node)
            .expect("Node definition not found");
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(result_value, DataValue::Boolean(true), "42 == 42 should be true");
    }

    #[test]
    fn test_equal_node_not_equal() {
        // 测试 Equal 节点比较不相等的值
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let equal_node = graph
            .create_node("logic.equal")
            .expect("Failed to create equal node");

        let pins = graph.get_node_pins(equal_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");

        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        // 设置不相等的值
        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Int32(10)))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Int32(20)))
            .expect("Failed to set pin B value");

        // 执行节点
        let definition = graph
            .get_node_definition(equal_node)
            .expect("Node definition not found");
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(result_value, DataValue::Boolean(false), "10 == 20 should be false");
    }

    #[test]
    fn test_equal_node_string() {
        // 测试 Equal 节点比较字符串
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let equal_node = graph
            .create_node("logic.equal")
            .expect("Failed to create equal node");

        let pins = graph.get_node_pins(equal_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");

        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        // 设置相等的字符串
        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::String("hello".to_string())))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::String("hello".to_string())))
            .expect("Failed to set pin B value");

        // 执行节点
        let definition = graph
            .get_node_definition(equal_node)
            .expect("Node definition not found");
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(
            result_value,
            DataValue::Boolean(true),
            "'hello' == 'hello' should be true"
        );
    }

    #[test]
    fn test_equal_node_float64() {
        // 测试 Equal 节点比较 Float64 类型
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let equal_node = graph
            .create_node("logic.equal")
            .expect("Failed to create equal node");

        let pins = graph.get_node_pins(equal_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");

        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        // 设置相等的浮点数
        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Float64(3.14)))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Float64(3.14)))
            .expect("Failed to set pin B value");

        // 执行节点
        let definition = graph
            .get_node_definition(equal_node)
            .expect("Node definition not found");
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(
            result_value,
            DataValue::Boolean(true),
            "3.14 == 3.14 should be true"
        );
    }

    #[test]
    fn test_equal_node_boolean() {
        // 测试 Equal 节点比较 Boolean 类型
        let registry = create_test_registry();
        let graph = Arc::new(GraphData::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let equal_node = graph
            .create_node("logic.equal")
            .expect("Failed to create equal node");

        let pins = graph.get_node_pins(equal_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");

        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        // 设置相等的布尔值
        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Boolean(true)))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Boolean(true)))
            .expect("Failed to set pin B value");

        // 执行节点
        let definition = graph
            .get_node_definition(equal_node)
            .expect("Node definition not found");
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(
            result_value,
            DataValue::Boolean(true),
            "true == true should be true"
        );
    }
}
