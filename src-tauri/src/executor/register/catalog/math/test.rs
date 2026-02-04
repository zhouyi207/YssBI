#[cfg(test)]
mod tests {
    use crate::executor::{
        graph::Graph,
        pin::{PinRole, DataRole},
        register::NodeRegistry,
        value::DataValue,
    };
    use std::sync::Arc;

    /// 创建测试用的注册表
    fn create_test_registry() -> Arc<NodeRegistry> {
        let registry = Arc::new(NodeRegistry::new());
        super::super::register(&registry);
        registry
    }

    #[test]
    fn test_add_node_int32() {
        // 创建注册表和图
        let registry = create_test_registry();
        let mut graph = Graph::new("test_graph", "Test Graph", registry.clone());

        // 创建 Add 节点
        let add_node = graph.create_node("math.add").expect("Failed to create add node");

        // 设置输入值
        let pins = graph.get_node_pins(add_node);
        
        // 找到 A 和 B 输入 Pin（索引从 0 开始）
        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        // 设置用户值
        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Int32(10)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Int32(20)))
            .expect("Failed to set pin B value");

        // 执行节点（使用 data_evaluator）
        let definition = graph.get_node_definition(add_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add_node);
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

        assert_eq!(result_value, DataValue::Int32(30));
    }

    #[test]
    fn test_add_node_float64() {
        let registry = create_test_registry();
        let mut graph = Graph::new("test_graph", "Test Graph", registry.clone());

        let add_node = graph.create_node("math.add").expect("Failed to create add node");
        let pins = graph.get_node_pins(add_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Float64(3.14)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Float64(2.86)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(add_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        if let DataValue::Float64(val) = result_value {
            assert!((val - 6.0).abs() < 0.0001, "Expected 6.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }

    #[test]
    fn test_add_node_mixed_types() {
        let registry = create_test_registry();
        let mut graph = Graph::new("test_graph", "Test Graph", registry.clone());

        let add_node = graph.create_node("math.add").expect("Failed to create add node");
        let pins = graph.get_node_pins(add_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        // Int32 + Float64 应该提升为 Float64
        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Int32(10)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Float64(5.5)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(add_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        if let DataValue::Float64(val) = result_value {
            assert!((val - 15.5).abs() < 0.0001, "Expected 15.5, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }

    #[test]
    fn test_add_node_with_connections() {
        let registry = create_test_registry();
        let mut graph = Graph::new("test_graph", "Test Graph", registry.clone());

        // 创建两个常量节点和一个 Add 节点
        // 注意：这里假设有常量节点，如果没有，这个测试需要调整
        let add_node = graph.create_node("math.add").expect("Failed to create add node");
        
        // 直接设置值进行测试
        let pins = graph.get_node_pins(add_node);
        
        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Int64(100)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Int64(200)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(add_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(result_value, DataValue::Int64(300));
    }

    #[test]
    fn test_add_node_zero_values() {
        let registry = create_test_registry();
        let mut graph = Graph::new("test_graph", "Test Graph", registry.clone());

        let add_node = graph.create_node("math.add").expect("Failed to create add node");
        let pins = graph.get_node_pins(add_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Int32(0)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Int32(0)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(add_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(result_value, DataValue::Int32(0));
    }

    #[test]
    fn test_add_node_negative_values() {
        let registry = create_test_registry();
        let mut graph = Graph::new("test_graph", "Test Graph", registry.clone());

        let add_node = graph.create_node("math.add").expect("Failed to create add node");
        let pins = graph.get_node_pins(add_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Int32(-10)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Int32(-20)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(add_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add_node);
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        let result_value = graph
            .resolve_pin_value(result_pin.id)
            .expect("Result value not found");

        assert_eq!(result_value, DataValue::Int32(-30));
    }

    #[test]
    fn test_add_node_chained_calculation() {
        // 测试三个 Add 节点的级联计算
        // add1: 10 + 20 = 30
        // add2: 5 + 15 = 20
        // add3: add1.result + add2.result = 30 + 20 = 50
        
        let registry = create_test_registry();
        let mut graph = Graph::new("test_graph", "Test Graph", registry.clone());

        // 创建三个 Add 节点
        let add1_node = graph.create_node("math.add").expect("Failed to create add1 node");
        let add2_node = graph.create_node("math.add").expect("Failed to create add2 node");
        let add3_node = graph.create_node("math.add").expect("Failed to create add3 node");

        // === 设置 add1 的输入值 ===
        let add1_pins = graph.get_node_pins(add1_node);
        let add1_pin_a = add1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Add1 Pin A not found");
        
        let add1_pin_b = add1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Add1 Pin B not found");

        let add1_result_pin = add1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Add1 Result pin not found");

        graph
            .set_pin_user_value(add1_pin_a.id, Some(DataValue::Int32(10)))
            .expect("Failed to set add1 pin A value");
        
        graph
            .set_pin_user_value(add1_pin_b.id, Some(DataValue::Int32(20)))
            .expect("Failed to set add1 pin B value");

        // === 设置 add2 的输入值 ===
        let add2_pins = graph.get_node_pins(add2_node);
        let add2_pin_a = add2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Add2 Pin A not found");
        
        let add2_pin_b = add2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Add2 Pin B not found");

        let add2_result_pin = add2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Add2 Result pin not found");

        graph
            .set_pin_user_value(add2_pin_a.id, Some(DataValue::Int32(5)))
            .expect("Failed to set add2 pin A value");
        
        graph
            .set_pin_user_value(add2_pin_b.id, Some(DataValue::Int32(15)))
            .expect("Failed to set add2 pin B value");

        // === 获取 add3 的输入 Pin ===
        let add3_pins = graph.get_node_pins(add3_node);
        let add3_pin_a = add3_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Add3 Pin A not found");
        
        let add3_pin_b = add3_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Add3 Pin B not found");

        let add3_result_pin = add3_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Add3 Result pin not found");

        // === 连接 add1.result -> add3.A 和 add2.result -> add3.B ===
        graph
            .connect(add1_result_pin.id, add3_pin_a.id)
            .expect("Failed to connect add1 result to add3 A");
        
        graph
            .connect(add2_result_pin.id, add3_pin_b.id)
            .expect("Failed to connect add2 result to add3 B");

        // === 执行 add1 节点 ===
        let add1_definition = graph.get_node_definition(add1_node).expect("Add1 definition not found");
        let add1_evaluator = add1_definition.data_evaluator.as_ref().expect("Add1 data evaluator not found");

        let mut add1_ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add1_node);
        let add1_result = add1_evaluator(&mut add1_ctx);
        assert!(add1_result.is_ok(), "Add1 data evaluator failed: {:?}", add1_result.err());

        // 验证 add1 的结果
        let add1_output = graph
            .resolve_pin_value(add1_result_pin.id)
            .expect("Add1 result value not found");
        assert_eq!(add1_output, DataValue::Int32(30), "Add1 result should be 30");

        // === 执行 add2 节点 ===
        let add2_definition = graph.get_node_definition(add2_node).expect("Add2 definition not found");
        let add2_evaluator = add2_definition.data_evaluator.as_ref().expect("Add2 data evaluator not found");

        let mut add2_ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add2_node);
        let add2_result = add2_evaluator(&mut add2_ctx);
        assert!(add2_result.is_ok(), "Add2 data evaluator failed: {:?}", add2_result.err());

        // 验证 add2 的结果
        let add2_output = graph
            .resolve_pin_value(add2_result_pin.id)
            .expect("Add2 result value not found");
        assert_eq!(add2_output, DataValue::Int32(20), "Add2 result should be 20");

        // === 执行 add3 节点 ===
        let add3_definition = graph.get_node_definition(add3_node).expect("Add3 definition not found");
        let add3_evaluator = add3_definition.data_evaluator.as_ref().expect("Add3 data evaluator not found");

        let mut add3_ctx = crate::executor::graph::GraphExecutionContext::new(&mut graph, add3_node);
        let add3_result = add3_evaluator(&mut add3_ctx);
        assert!(add3_result.is_ok(), "Add3 data evaluator failed: {:?}", add3_result.err());

        // === 验证 add3 的最终结果 ===
        let add3_output = graph
            .resolve_pin_value(add3_result_pin.id)
            .expect("Add3 result value not found");
        
        assert_eq!(add3_output, DataValue::Int32(50), "Add3 result should be 50 (30 + 20)");

        // 额外验证：确保 add3 的输入确实来自上游连接
        let add3_input_a = graph
            .resolve_pin_value(add3_pin_a.id)
            .expect("Add3 input A not found");
        let add3_input_b = graph
            .resolve_pin_value(add3_pin_b.id)
            .expect("Add3 input B not found");
        
        assert_eq!(add3_input_a, DataValue::Int32(30), "Add3 input A should be 30 from add1");
        assert_eq!(add3_input_b, DataValue::Int32(20), "Add3 input B should be 20 from add2");
    }
}
