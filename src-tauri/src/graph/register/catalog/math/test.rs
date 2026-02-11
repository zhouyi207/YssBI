#[cfg(test)]
mod tests {
    use crate::graph::{
        GraphInstance,
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
    fn test_add_node_float64() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let add_node = graph.create_node("Math:Operators:Add (+)").expect("Failed to create add node");
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

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), add_node);
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
    fn test_subtract_node() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let sub_node = graph.create_node("Math:Operators:Subtract (-)").expect("Failed to create subtract node");
        let pins = graph.get_node_pins(sub_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Float64(10.0)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Float64(3.0)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(sub_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), sub_node);
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
            assert!((val - 7.0).abs() < 0.0001, "Expected 7.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }

    #[test]
    fn test_multiply_node() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let mul_node = graph.create_node("Math:Operators:Multiply (*)").expect("Failed to create multiply node");
        let pins = graph.get_node_pins(mul_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Float64(4.0)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Float64(5.0)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(mul_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), mul_node);
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
            assert!((val - 20.0).abs() < 0.0001, "Expected 20.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }

    #[test]
    fn test_divide_node() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(crate::graph::GraphId::new(), "Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let div_node = graph.create_node("Math:Operators:Divide (/)").expect("Failed to create divide node");
        let pins = graph.get_node_pins(div_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value(pin_a.id, Some(DataValue::Float64(20.0)))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value(pin_b.id, Some(DataValue::Float64(4.0)))
            .expect("Failed to set pin B value");

        let definition = graph.get_node_definition(div_node).expect("Node definition not found");
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");

        let mut ctx = crate::execution::GraphExecutionContext::new(graph.clone(), div_node);
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
            assert!((val - 5.0).abs() < 0.0001, "Expected 5.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }
}
