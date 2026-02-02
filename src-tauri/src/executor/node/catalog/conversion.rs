use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericInDataPin, GenericOutDataPin};
use crate::executor::value::{ValueType, PinTypeDesc};

pub fn register(registry: &NodeRegistry) {
    // 1. ToString - 将任意类型转换为字符串
    {
        let node = GenericNode::new_prototype("to_string", "To String");
        // 输入使用类型推断，可以接受任意类型
        node.add_in_data_pin(GenericInDataPin::new(
            uuid::Uuid::nil(), 
            "Value", 
            PinTypeDesc::unknown()  // 使用类型推断
        ));
        // 输出固定为 String
        node.add_out_data_pin(GenericOutDataPin::new(
            uuid::Uuid::nil(), 
            "String", 
            PinTypeDesc::concrete(ValueType::String)
        ));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let value = ctx.get_pin_value(&node.inputs[0].id);
            
            // 将值转换为字符串
            let string_value = match value {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => "null".to_string(),
                serde_json::Value::Array(arr) => serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string()),
                serde_json::Value::Object(obj) => serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string()),
            };
            
            ctx.set_pin_value(pin_id, serde_json::Value::String(string_value.clone()));
            serde_json::Value::String(string_value)
        }));
        
        let mut node = node;
        node.set_metadata(
            vec!["Conversion".into()], 
            "default".into(), 
            Some("Convert any value to string".into())
        );
        registry.register("to_string".into(), Arc::new(node));
    }
    
    // 2. ToBool - 将任意类型转换为布尔值
    {
        let node = GenericNode::new_prototype("to_bool", "To Bool");
        node.add_in_data_pin(GenericInDataPin::new(
            uuid::Uuid::nil(), 
            "Value", 
            PinTypeDesc::unknown()
        ));
        node.add_out_data_pin(GenericOutDataPin::new(
            uuid::Uuid::nil(), 
            "Boolean", 
            PinTypeDesc::concrete(ValueType::Boolean)
        ));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let value = ctx.get_pin_value(&node.inputs[0].id);
            
            // 转换规则：
            // - 数字: 0 = false, 非0 = true
            // - 字符串: "" = false, 非空 = true
            // - 布尔: 保持原值
            // - 数组: [] = false, 非空 = true
            // - 对象: {} = false, 非空 = true
            // - null: false
            let bool_value = match value {
                serde_json::Value::Bool(b) => b,
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        i != 0
                    } else if let Some(f) = n.as_f64() {
                        f != 0.0
                    } else {
                        false
                    }
                },
                serde_json::Value::String(s) => !s.is_empty(),
                serde_json::Value::Array(arr) => !arr.is_empty(),
                serde_json::Value::Object(obj) => !obj.is_empty(),
                serde_json::Value::Null => false,
            };
            
            ctx.set_pin_value(pin_id, serde_json::Value::Bool(bool_value));
            serde_json::Value::Bool(bool_value)
        }));
        
        let mut node = node;
        node.set_metadata(
            vec!["Conversion".into()], 
            "default".into(), 
            Some("Convert any value to boolean".into())
        );
        registry.register("to_bool".into(), Arc::new(node));
    }
    
    // 3. ToInt - 将值转换为整数
    {
        let node = GenericNode::new_prototype("to_int", "To Int");
        node.add_in_data_pin(GenericInDataPin::new(
            uuid::Uuid::nil(), 
            "Value", 
            PinTypeDesc::unknown()
        ));
        node.add_out_data_pin(GenericOutDataPin::new(
            uuid::Uuid::nil(), 
            "Integer", 
            PinTypeDesc::concrete(ValueType::Int64)
        ));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let value = ctx.get_pin_value(&node.inputs[0].id);
            
            let int_value = match value {
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        i
                    } else if let Some(f) = n.as_f64() {
                        f as i64
                    } else {
                        0
                    }
                },
                serde_json::Value::String(s) => {
                    s.parse::<i64>().unwrap_or(0)
                },
                serde_json::Value::Bool(b) => if b { 1 } else { 0 },
                _ => 0,
            };
            
            ctx.set_pin_value(pin_id, serde_json::Value::Number(serde_json::Number::from(int_value)));
            serde_json::Value::Number(serde_json::Number::from(int_value))
        }));
        
        let mut node = node;
        node.set_metadata(
            vec!["Conversion".into()], 
            "default".into(), 
            Some("Convert value to integer".into())
        );
        registry.register("to_int".into(), Arc::new(node));
    }
    
    // 4. ToFloat - 将值转换为浮点数
    {
        let node = GenericNode::new_prototype("to_float", "To Float");
        node.add_in_data_pin(GenericInDataPin::new(
            uuid::Uuid::nil(), 
            "Value", 
            PinTypeDesc::unknown()
        ));
        node.add_out_data_pin(GenericOutDataPin::new(
            uuid::Uuid::nil(), 
            "Float", 
            PinTypeDesc::concrete(ValueType::Float64)
        ));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let value = ctx.get_pin_value(&node.inputs[0].id);
            
            let float_value = match value {
                serde_json::Value::Number(n) => {
                    n.as_f64().unwrap_or(0.0)
                },
                serde_json::Value::String(s) => {
                    s.parse::<f64>().unwrap_or(0.0)
                },
                serde_json::Value::Bool(b) => if b { 1.0 } else { 0.0 },
                _ => 0.0,
            };
            
            ctx.set_pin_value(pin_id, serde_json::json!(float_value));
            serde_json::json!(float_value)
        }));
        
        let mut node = node;
        node.set_metadata(
            vec!["Conversion".into()], 
            "default".into(), 
            Some("Convert value to float".into())
        );
        registry.register("to_float".into(), Arc::new(node));
    }
}
