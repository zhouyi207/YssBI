#[cfg(test)]
mod tests {
    use crate::graph::{
        GraphInstance, GraphRuntime,
        pin::{PinRole, DataRole},
        register::NodeRegistry,
        value::DataValue,
    };
    use std::sync::{Arc, Mutex};

    /// 创建测试用的注册表
    fn create_test_registry() -> Arc<NodeRegistry> {
        let registry = Arc::new(NodeRegistry::new());
        super::super::register(&registry);
        registry
    }

    #[test]
    fn test_add_node_float64() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new("Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let add_node = graph.create_node("Math:Operators:Add (+)").expect("Failed to create add node");
        let pins = graph.get_pin_instances_by_node_id(add_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value_by_pin_id(pin_a.id, DataValue::Float64(3.14))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value_by_pin_id(pin_b.id, DataValue::Float64(2.86))
            .expect("Failed to set pin B value");

        // 使用 Executor 执行节点
        let graph_runtime = Arc::new(Mutex::new(GraphRuntime::new(graph.clone())));
        let mut ctx = crate::execution::NodeExecutionContext::new(graph_runtime.clone(), add_node);
        
        // 获取节点定义并执行 data_evaluator
        let definition = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_node_definition_by_node_id(add_node)
        };
        
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        // 从 runtime 获取计算结果
        let result_value = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_pin_data_value_by_pin_id(result_pin.id)
        };

        if let DataValue::Float64(val) = result_value {
            assert!((val - 6.0).abs() < 0.0001, "Expected 6.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }

    #[test]
    fn test_subtract_node() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new("Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let subtract_node = graph.create_node("Math:Operators:Subtract (-)").expect("Failed to create subtract node");
        let pins = graph.get_pin_instances_by_node_id(subtract_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value_by_pin_id(pin_a.id, DataValue::Float64(10.0))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value_by_pin_id(pin_b.id, DataValue::Float64(3.0))
            .expect("Failed to set pin B value");

        // 使用 Executor 执行节点
        let graph_runtime = Arc::new(Mutex::new(GraphRuntime::new(graph.clone())));
        let mut ctx = crate::execution::NodeExecutionContext::new(graph_runtime.clone(), subtract_node);
        
        // 获取节点定义并执行 data_evaluator
        let definition = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_node_definition_by_node_id(subtract_node)
        };
        
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        // 从 runtime 获取计算结果
        let result_value = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_pin_data_value_by_pin_id(result_pin.id)
        };

        if let DataValue::Float64(val) = result_value {
            assert!((val - 7.0).abs() < 0.0001, "Expected 7.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }

    #[test]
    fn test_multiply_node() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new("Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let multiply_node = graph.create_node("Math:Operators:Multiply (*)").expect("Failed to create multiply node");
        let pins = graph.get_pin_instances_by_node_id(multiply_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value_by_pin_id(pin_a.id, DataValue::Float64(4.0))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value_by_pin_id(pin_b.id, DataValue::Float64(2.5))
            .expect("Failed to set pin B value");

        // 使用 Executor 执行节点
        let graph_runtime = Arc::new(Mutex::new(GraphRuntime::new(graph.clone())));
        let mut ctx = crate::execution::NodeExecutionContext::new(graph_runtime.clone(), multiply_node);
        
        // 获取节点定义并执行 data_evaluator
        let definition = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_node_definition_by_node_id(multiply_node)
        };
        
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        // 从 runtime 获取计算结果
        let result_value = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_pin_data_value_by_pin_id(result_pin.id)
        };

        if let DataValue::Float64(val) = result_value {
            assert!((val - 10.0).abs() < 0.0001, "Expected 10.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }

    #[test]
    fn test_divide_node() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new("Test Graph", crate::graph::GraphKind::Event,  registry.clone()));

        let divide_node = graph.create_node("Math:Operators:Divide (/)").expect("Failed to create divide node");
        let pins = graph.get_pin_instances_by_node_id(divide_node);

        let pin_a = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
            .expect("Pin A not found");
        
        let pin_b = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
            .expect("Pin B not found");

        graph
            .set_pin_user_value_by_pin_id(pin_a.id, DataValue::Float64(20.0))
            .expect("Failed to set pin A value");
        
        graph
            .set_pin_user_value_by_pin_id(pin_b.id, DataValue::Float64(4.0))
            .expect("Failed to set pin B value");

        // 使用 Executor 执行节点
        let graph_runtime = Arc::new(Mutex::new(GraphRuntime::new(graph.clone())));
        let mut ctx = crate::execution::NodeExecutionContext::new(graph_runtime.clone(), divide_node);
        
        // 获取节点定义并执行 data_evaluator
        let definition = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_node_definition_by_node_id(divide_node)
        };
        
        let evaluator = definition.data_evaluator.as_ref().expect("Data evaluator not found");
        let result = evaluator(&mut ctx);

        assert!(result.is_ok(), "Data evaluator failed: {:?}", result.err());

        let result_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
            .expect("Result pin not found");

        // 从 runtime 获取计算结果
        let result_value = {
            let runtime = graph_runtime.lock().unwrap();
            runtime.get_pin_data_value_by_pin_id(result_pin.id)
        };

        if let DataValue::Float64(val) = result_value {
            assert!((val - 5.0).abs() < 0.0001, "Expected 5.0, got {}", val);
        } else {
            panic!("Expected Float64, got {:?}", result_value);
        }
    }
}
