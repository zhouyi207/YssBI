//! 动态 Add 节点实现
//!
//! 支持动态添加输入的加法节点

use std::sync::Arc;
use crate::executor::node::implementation::{
    GenericNode, DynamicPinConfig, DynamicPinType, PinDirection, 
    NodeDynamicCapability
};
use crate::executor::node::registry::NodeRegistry;
use crate::executor::pin::{GenericInDataPin, GenericOutDataPin};
use crate::executor::value::{PinTypeDesc, TypeConstraint};
use serde_json::Value;

/// 创建动态 Add 节点
pub fn create_dynamic_add_node() -> GenericNode {
    let node = GenericNode::new_prototype("dynamic_add", "Add (Dynamic)");
    
    // 添加初始的两个输入 Pin（使用 Numeric 约束）
    let numeric_type = PinTypeDesc::unknown()
        .with_constraints(vec![TypeConstraint::Numeric]);
    
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Input 1",
        numeric_type.clone()
    ));
    
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Input 2",
        numeric_type.clone()
    ));
    
    // 添加输出 Pin（也是 Numeric）
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Sum",
        numeric_type.clone()
    ));
    
    // 配置动态能力
    let dynamic_config = DynamicPinConfig {
        pin_type: DynamicPinType::Data,
        direction: PinDirection::Input,
        name_template: "Input {}".to_string(),
        data_type: numeric_type,
        min_count: 2,
        max_count: Some(10),
        can_reorder: true,
    };
    
    // 创建数据处理器（不是流程处理器）
    node.set_data_processor(Box::new(|ctx, node_dto, _pin_id| {
        let mut sum = 0.0;
        
        // 遍历所有输入，累加值
        for input in &node_dto.inputs {
            let value = ctx.get_pin_value(&input.id);
            sum += value.as_f64().unwrap_or(0.0);
        }
        
        Value::from(sum)
    }));
    
    // 设置动态能力（不需要 processor_generator，因为数据处理器不依赖 Pin 数量）
    let capability = NodeDynamicCapability {
        can_add_pins: true,
        dynamic_configs: vec![dynamic_config],
        processor_generator: None,  // 数据节点不需要重新生成处理器
    };
    
    node.set_dynamic_capability(capability);
    
    // 设置元数据
    let mut node = node;
    node.set_metadata(
        vec!["Math".into(), "Dynamic".into()],
        "math".into(),
        Some("Add multiple numbers together (2-10 inputs)".into())
    );
    
    node
}

/// 注册动态数学节点
pub fn register(registry: &NodeRegistry) {
    let dynamic_add = create_dynamic_add_node();
    registry.register("dynamic_add".into(), Arc::new(dynamic_add));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::node::implementation::PinDirection;
    
    #[test]
    fn test_dynamic_add_creation() {
        let node = create_dynamic_add_node();
        
        // 验证初始状态
        assert_eq!(node.input_names().len(), 2);
        assert_eq!(node.output_names().len(), 1);
        assert!(node.supports_dynamic_pins());
    }
    
    #[test]
    fn test_add_dynamic_pin() {
        let node = create_dynamic_add_node();
        
        // 获取动态配置
        let config = node.get_dynamic_constraints("data", &PinDirection::Input)
            .expect("Should have dynamic config");
        
        // 添加第 3 个输入
        let pin_id = node.add_dynamic_pin(&config)
            .expect("Should add pin successfully");
        
        // 验证
        assert_eq!(node.input_names().len(), 3);
        assert!(pin_id != uuid::Uuid::nil());
    }
    
    #[test]
    fn test_max_pins_limit() {
        let node = create_dynamic_add_node();
        
        let config = node.get_dynamic_constraints("data", &PinDirection::Input)
            .expect("Should have dynamic config");
        
        // 添加到最大数量（10 个）
        for _ in 2..10 {
            node.add_dynamic_pin(&config).expect("Should add pin");
        }
        
        // 尝试添加第 11 个应该失败
        let result = node.add_dynamic_pin(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot add more pins"));
    }
    
    #[test]
    fn test_remove_dynamic_pin() {
        let node = create_dynamic_add_node();
        
        let config = node.get_dynamic_constraints("data", &PinDirection::Input)
            .expect("Should have dynamic config");
        
        // 添加第 3 个输入
        let pin_id = node.add_dynamic_pin(&config).unwrap();
        
        // 移除第 3 个输入应该成功
        assert!(node.remove_dynamic_pin(pin_id).is_ok());
        assert_eq!(node.input_names().len(), 2);
    }
}
