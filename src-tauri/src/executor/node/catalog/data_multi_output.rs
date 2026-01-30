//! 多输出数据节点
//! 
//! 包含返回多个结果的数据处理节点

use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin};
use serde_json::{json, Value};
use crate::executor::value::{ValueType, PinTypeDesc};

pub fn register(registry: &NodeRegistry) {
    let data_cat = vec!["Data".into(), "Multi-Output".into()];

    // ============================================================================
    // GetObjectProperties 节点 - 提取对象的多个属性
    // ============================================================================
    {
        let node = GenericNode::new_prototype("get_object_props", "Get Object Properties");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Object", PinTypeDesc::concrete(ValueType::Struct(vec![]))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Keys", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Values", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Count", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let obj = ctx.get_pin_value(&node.inputs[0].id);
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            if let Some(map) = obj.as_object() {
                match output_name {
                    "Keys" => {
                        let keys: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                        json!(keys)
                    }
                    "Values" => {
                        let values: Vec<&Value> = map.values().collect();
                        json!(values)
                    }
                    "Count" => json!(map.len()),
                    _ => json!(null)
                }
            } else {
                json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            data_cat.clone(),
            "data".into(),
            Some("Extracts keys, values, and count from an object".into())
        );
        registry.register("get_object_props".into(), Arc::new(node));
    }

    // ============================================================================
    // ArrayInfo 节点 - 返回数组的多个属性
    // ============================================================================
    {
        let node = GenericNode::new_prototype("array_info", "Array Info");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Array", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Length", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "First", PinTypeDesc::concrete(ValueType::Any)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Last", PinTypeDesc::concrete(ValueType::Any)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "IsEmpty", PinTypeDesc::concrete(ValueType::Boolean)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let array = ctx.get_pin_value(&node.inputs[0].id);
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            if let Some(arr) = array.as_array() {
                match output_name {
                    "Length" => json!(arr.len()),
                    "First" => arr.first().cloned().unwrap_or(json!(null)),
                    "Last" => arr.last().cloned().unwrap_or(json!(null)),
                    "IsEmpty" => json!(arr.is_empty()),
                    _ => json!(null)
                }
            } else {
                json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            data_cat.clone(),
            "data".into(),
            Some("Returns multiple properties of an array".into())
        );
        registry.register("array_info".into(), Arc::new(node));
    }

    // ============================================================================
    // PartitionArray 节点 - 将数组分为两部分
    // ============================================================================
    {
        let node = GenericNode::new_prototype("partition_array", "Partition Array");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Array", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Index", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Before", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "After", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "AtIndex", PinTypeDesc::concrete(ValueType::Any)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let array = ctx.get_pin_value(&node.inputs[0].id);
            let index = ctx.get_pin_value(&node.inputs[1].id)
                .as_f64()
                .unwrap_or(0.0) as usize;
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            if let Some(arr) = array.as_array() {
                match output_name {
                    "Before" => {
                        let before: Vec<&Value> = arr.iter().take(index).collect();
                        json!(before)
                    }
                    "After" => {
                        let after: Vec<&Value> = arr.iter().skip(index + 1).collect();
                        json!(after)
                    }
                    "AtIndex" => arr.get(index).cloned().unwrap_or(json!(null)),
                    _ => json!(null)
                }
            } else {
                json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            data_cat.clone(),
            "data".into(),
            Some("Splits an array at a given index".into())
        );
        registry.register("partition_array".into(), Arc::new(node));
    }

    // ============================================================================
    // FilterArray 节点 - 过滤数组并返回多个结果
    // ============================================================================
    {
        let node = GenericNode::new_prototype("filter_array_multi", "Filter Array (Multi)");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Array", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Threshold", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Above", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Below", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Equal", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let array = ctx.get_pin_value(&node.inputs[0].id);
            let threshold = ctx.get_pin_value(&node.inputs[1].id)
                .as_f64()
                .unwrap_or(0.0);
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            if let Some(arr) = array.as_array() {
                let numbers: Vec<f64> = arr.iter()
                    .filter_map(|v| v.as_f64())
                    .collect();
                
                match output_name {
                    "Above" => {
                        let above: Vec<f64> = numbers.iter()
                            .filter(|&&x| x > threshold)
                            .copied()
                            .collect();
                        json!(above)
                    }
                    "Below" => {
                        let below: Vec<f64> = numbers.iter()
                            .filter(|&&x| x < threshold)
                            .copied()
                            .collect();
                        json!(below)
                    }
                    "Equal" => {
                        let equal: Vec<f64> = numbers.iter()
                            .filter(|&&x| (x - threshold).abs() < f64::EPSILON)
                            .copied()
                            .collect();
                        json!(equal)
                    }
                    _ => json!(null)
                }
            } else {
                json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            data_cat.clone(),
            "data".into(),
            Some("Filters an array into three groups based on a threshold".into())
        );
        registry.register("filter_array_multi".into(), Arc::new(node));
    }

    // ============================================================================
    // DateTimeParts 节点 - 解析日期时间为多个部分
    // ============================================================================
    {
        let node = GenericNode::new_prototype("datetime_parts", "DateTime Parts");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Timestamp", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Year", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Month", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Day", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Hour", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Minute", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Second", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let timestamp = ctx.get_pin_value(&node.inputs[0].id)
                .as_f64()
                .unwrap_or(0.0) as i64;
            
            // 简化的日期时间解析（实际应该使用 chrono crate）
            // 这里只是示例，返回占位值
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            // 简化实现：假设 timestamp 是秒数
            let seconds_per_minute = 60;
            let seconds_per_hour = 3600;
            let seconds_per_day = 86400;
            
            match output_name {
                "Year" => json!(1970 + timestamp / (seconds_per_day * 365)),
                "Month" => json!((timestamp / (seconds_per_day * 30)) % 12 + 1),
                "Day" => json!((timestamp / seconds_per_day) % 30 + 1),
                "Hour" => json!((timestamp / seconds_per_hour) % 24),
                "Minute" => json!((timestamp / seconds_per_minute) % 60),
                "Second" => json!(timestamp % 60),
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            data_cat.clone(),
            "data".into(),
            Some("Extracts year, month, day, hour, minute, second from a timestamp".into())
        );
        registry.register("datetime_parts".into(), Arc::new(node));
    }
}
