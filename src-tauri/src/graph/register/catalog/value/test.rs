#[cfg(test)]
mod tests {
    use crate::graph::{
        GraphInstance, GraphKind,
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
    fn test_boolean_constant() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            GraphKind::Event,
            registry.clone(),
        ));

        // 创建 Boolean 常量节点
        let const_node = graph
            .create_node("Value:Constants:Boolean")
            .expect("Failed to create boolean constant node");

        // 获取输出 Pin
        let pins = graph.get_pin_instances_by_node_id(const_node);
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        // 设置值
        graph
            .set_pin_user_value_by_pin_id(result_pin.id, DataValue::Boolean(true))
            .expect("Failed to set value");

        // 验证值
        let value = graph
            .get_pin_user_value_by_pin_id(result_pin.id)
            .expect("Failed to get value");

        assert_eq!(value, DataValue::Boolean(true));
    }

    #[test]
    fn test_int32_constant() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            GraphKind::Event,
            registry.clone(),
        ));

        let const_node = graph
            .create_node("Value:Constants:Int32")
            .expect("Failed to create int32 constant node");

        let pins = graph.get_pin_instances_by_node_id(const_node);
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        graph
            .set_pin_user_value_by_pin_id(result_pin.id, DataValue::Int32(42))
            .expect("Failed to set value");

        let value = graph
            .get_pin_user_value_by_pin_id(result_pin.id)
            .expect("Failed to get value");

        assert_eq!(value, DataValue::Int32(42));
    }

    #[test]
    fn test_float64_constant() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            GraphKind::Event,
            registry.clone(),
        ));

        let const_node = graph
            .create_node("Value:Constants:Float64")
            .expect("Failed to create float64 constant node");

        let pins = graph.get_pin_instances_by_node_id(const_node);
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        graph
            .set_pin_user_value_by_pin_id(result_pin.id, DataValue::Float64(3.14))
            .expect("Failed to set value");

        let value = graph
            .get_pin_user_value_by_pin_id(result_pin.id)
            .expect("Failed to get value");

        if let DataValue::Float64(val) = value {
            const TOL: f64 = 1e-4;
            assert!((val - 3.14).abs() < TOL);
        } else {
            panic!("Expected Float64");
        }
    }

    #[test]
    fn test_string_constant() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            GraphKind::Event,
            registry.clone(),
        ));

        let const_node = graph
            .create_node("Value:Constants:String")
            .expect("Failed to create string constant node");

        let pins = graph.get_pin_instances_by_node_id(const_node);
        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        graph
            .set_pin_user_value_by_pin_id(result_pin.id, DataValue::String("Hello".to_string()))
            .expect("Failed to set value");

        let value = graph
            .get_pin_user_value_by_pin_id(result_pin.id)
            .expect("Failed to get value");

        assert_eq!(value, DataValue::String("Hello".to_string()));
    }

    #[test]
    fn test_convert_node() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            GraphKind::Event,
            registry.clone(),
        ));

        // 创建 Convert 节点
        let convert_node = graph
            .create_node("Value:Conversion:Convert")
            .expect("Failed to create convert node");

        // 获取 Pin
        let pins = graph.get_pin_instances_by_node_id(convert_node);
        let input_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .expect("Input pin not found");

        let _output_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
            .expect("Output pin not found");

        // 设置输入值（Int32）
        graph
            .set_pin_user_value_by_pin_id(input_pin.id, DataValue::Int32(42))
            .expect("Failed to set input value");

        // 执行转换（需要设置输出类型）
        // 注意：这个测试可能需要类型推断系统的支持
        // 暂时跳过实际的转换测试
    }
}
