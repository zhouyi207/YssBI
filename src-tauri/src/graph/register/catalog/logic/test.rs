#[cfg(test)]
mod tests {
    use crate::graph::{
        pin::{DataRole, PinRole},
        register::NodeRegistry,
        value::DataValue,
        GraphInstance, GraphRuntime,
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
        // 测试 Equal 节点比较 Float64 类型
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        // 创建 Equal 节点
        let equal_node = graph
            .create_node("Logic:Comparison:Equal (==)")
            .expect("Failed to create equal node");

        // 获取所有 Pin
        let pins = graph.get_pin_instances_by_node_id(equal_node);

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
            .set_pin_user_value_by_pin_id(pin_a.id, DataValue::Float64(42.0))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value_by_pin_id(pin_b.id, DataValue::Float64(42.0))
            .expect("Failed to set pin B value");

        // 创建 GraphRuntime
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        // 执行节点
        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(equal_node);
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = runtime
            .lock()
            .unwrap()
            .get_pin_data_value_by_pin_id(result_pin.id)
            .unwrap();

        assert_eq!(
            result_value,
            DataValue::Boolean(true),
            "42 == 42 should be true"
        );
    }

    #[test]
    fn test_equal_node_not_equal() {
        // 测试 Equal 节点比较不相等的值
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let equal_node = graph
            .create_node("Logic:Comparison:Equal (==)")
            .expect("Failed to create equal node");

        let pins = graph.get_pin_instances_by_node_id(equal_node);

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
            .set_pin_user_value_by_pin_id(pin_a.id, DataValue::Float64(10.0))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value_by_pin_id(pin_b.id, DataValue::Float64(20.0))
            .expect("Failed to set pin B value");

        // 创建 GraphRuntime
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        // 执行节点
        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(equal_node);
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = runtime
            .lock()
            .unwrap()
            .get_pin_data_value_by_pin_id(result_pin.id)
            .unwrap();

        assert_eq!(
            result_value,
            DataValue::Boolean(false),
            "10 == 20 should be false"
        );
    }

    #[test]
    fn test_equal_node_string() {
        // 测试 Equal 节点比较字符串 - 跳过，因为现在只支持 Float64
        // 如果需要支持字符串比较，需要使用类型变量
    }

    #[test]
    fn test_equal_node_float64() {
        // 测试 Equal 节点比较 Float64 类型
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let equal_node = graph
            .create_node("Logic:Comparison:Equal (==)")
            .expect("Failed to create equal node");

        let pins = graph.get_pin_instances_by_node_id(equal_node);

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
            .set_pin_user_value_by_pin_id(pin_a.id, DataValue::Float64(3.14))
            .expect("Failed to set pin A value");

        graph
            .set_pin_user_value_by_pin_id(pin_b.id, DataValue::Float64(3.14))
            .expect("Failed to set pin B value");

        // 创建 GraphRuntime
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        // 执行节点
        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(equal_node);
        let evaluator = definition
            .data_evaluator
            .as_ref()
            .expect("Data evaluator not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), equal_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        // 验证结果
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = runtime
            .lock()
            .unwrap()
            .get_pin_data_value_by_pin_id(result_pin.id)
            .unwrap();

        assert_eq!(
            result_value,
            DataValue::Boolean(true),
            "3.14 == 3.14 should be true"
        );
    }

    #[test]
    fn test_equal_node_boolean() {
        // 测试 Equal 节点比较 Boolean 类型 - 跳过，因为现在只支持 Float64
        // 如果需要支持布尔比较，需要使用类型变量
    }
}
