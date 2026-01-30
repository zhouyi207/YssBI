//! 多输出数学节点
//! 
//! 包含返回多个计算结果的数学节点

use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin};
use crate::executor::value::{ValueType, PinTypeDesc};
use serde_json::json;

pub fn register(registry: &NodeRegistry) {
    let math_cat = vec!["Math".into(), "Multi-Output".into()];

    // ============================================================================
    // DivMod 节点 - 同时返回商和余数
    // ============================================================================
    {
        let node = GenericNode::new_prototype("divmod", "Divide and Modulo");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Dividend", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Divisor", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Quotient", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Remainder", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let dividend = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let divisor = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(1.0);
            
            // 防止除以零
            if divisor == 0.0 {
                return json!(null);
            }
            
            // 根据请求的输出 Pin 返回不同的值
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "Quotient" => json!((dividend / divisor).floor()),  // 商（向下取整）
                "Remainder" => json!(dividend % divisor),           // 余数
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            math_cat.clone(),
            "math".into(),
            Some("Returns both quotient and remainder of division".into())
        );
        registry.register("divmod".into(), Arc::new(node));
    }

    // ============================================================================
    // MinMax 节点 - 返回最小值、最大值和平均值
    // ============================================================================
    {
        let node = GenericNode::new_prototype("min_max", "Min Max Average");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Array", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Min", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Max", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Average", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let array = ctx.get_pin_value(&node.inputs[0].id);
            
            // 解析数组
            let numbers: Vec<f64> = if let Some(arr) = array.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_f64())
                    .collect()
            } else {
                vec![]
            };
            
            if numbers.is_empty() {
                return json!(null);
            }
            
            // 根据请求的输出 Pin 返回不同的值
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "Min" => {
                    let min = numbers.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                    json!(min)
                }
                "Max" => {
                    let max = numbers.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                    json!(max)
                }
                "Average" => {
                    let sum: f64 = numbers.iter().sum();
                    let avg = sum / numbers.len() as f64;
                    json!(avg)
                }
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            math_cat.clone(),
            "math".into(),
            Some("Returns minimum, maximum, and average of an array".into())
        );
        registry.register("min_max".into(), Arc::new(node));
    }

    // ============================================================================
    // SinCos 节点 - 同时返回正弦和余弦
    // ============================================================================
    {
        let node = GenericNode::new_prototype("sin_cos", "Sin and Cos");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Angle", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Sin", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Cos", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let angle = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "Sin" => json!(angle.sin()),
                "Cos" => json!(angle.cos()),
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            math_cat.clone(),
            "math".into(),
            Some("Returns both sine and cosine of an angle".into())
        );
        registry.register("sin_cos".into(), Arc::new(node));
    }

    // ============================================================================
    // PolarToCartesian 节点 - 极坐标转笛卡尔坐标
    // ============================================================================
    {
        let node = GenericNode::new_prototype("polar_to_cartesian", "Polar to Cartesian");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Radius", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Angle", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "X", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Y", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let radius = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let angle = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "X" => json!(radius * angle.cos()),
                "Y" => json!(radius * angle.sin()),
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            math_cat.clone(),
            "math".into(),
            Some("Converts polar coordinates (r, θ) to Cartesian (x, y)".into())
        );
        registry.register("polar_to_cartesian".into(), Arc::new(node));
    }

    // ============================================================================
    // CartesianToPolar 节点 - 笛卡尔坐标转极坐标
    // ============================================================================
    {
        let node = GenericNode::new_prototype("cartesian_to_polar", "Cartesian to Polar");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "X", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Y", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Radius", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Angle", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let x = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let y = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "Radius" => json!((x * x + y * y).sqrt()),
                "Angle" => json!(y.atan2(x)),
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            math_cat.clone(),
            "math".into(),
            Some("Converts Cartesian coordinates (x, y) to polar (r, θ)".into())
        );
        registry.register("cartesian_to_polar".into(), Arc::new(node));
    }

    // ============================================================================
    // Statistics 节点 - 返回多个统计值
    // ============================================================================
    {
        let node = GenericNode::new_prototype("statistics", "Statistics");
        node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Array", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Count", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Sum", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Mean", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Median", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "StdDev", PinTypeDesc::concrete(ValueType::Float64)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let array = ctx.get_pin_value(&node.inputs[0].id);
            
            let mut numbers: Vec<f64> = if let Some(arr) = array.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_f64())
                    .collect()
            } else {
                vec![]
            };
            
            if numbers.is_empty() {
                return json!(null);
            }
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "Count" => json!(numbers.len()),
                "Sum" => json!(numbers.iter().sum::<f64>()),
                "Mean" => {
                    let sum: f64 = numbers.iter().sum();
                    json!(sum / numbers.len() as f64)
                }
                "Median" => {
                    numbers.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let mid = numbers.len() / 2;
                    if numbers.len() % 2 == 0 {
                        json!((numbers[mid - 1] + numbers[mid]) / 2.0)
                    } else {
                        json!(numbers[mid])
                    }
                }
                "StdDev" => {
                    let mean: f64 = numbers.iter().sum::<f64>() / numbers.len() as f64;
                    let variance: f64 = numbers.iter()
                        .map(|&x| (x - mean).powi(2))
                        .sum::<f64>() / numbers.len() as f64;
                    json!(variance.sqrt())
                }
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            math_cat.clone(),
            "math".into(),
            Some("Returns multiple statistical values for an array".into())
        );
        registry.register("statistics".into(), Arc::new(node));
    }
}
