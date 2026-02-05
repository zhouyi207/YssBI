use crate::graph::infer::{TypeVarDefinition, TypeVarId};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDefinition, PinRole, PinTypeDesc};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataValue, DataType};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_convert(registry);
}

/// Convert 节点 - 类型转换
/// 
/// 输入和输出类型都是类型变量，根据连接推断
/// 在运行时根据输入和输出的实际类型执行转换
fn register_convert(registry: &NodeRegistry) {
    // 创建两个独立的类型变量
    let input_type_var = TypeVarId::new();
    let output_type_var = TypeVarId::new();

    let definition = NodeDefinition::new("value.convert", "Convert")
        .with_category(vec!["Value".to_string(), "Conversion".to_string()])
        .with_ui_style("value")
        .with_description("Convert value from one type to another")
        // 注册输入类型变量（无约束，接受任何类型）
        .add_type_var(TypeVarDefinition {
            id: input_type_var,
            constraints: vec![],
            bound: None,
        })
        // 注册输出类型变量（无约束，可转换为任何类型）
        .add_type_var(TypeVarDefinition {
            id: output_type_var,
            constraints: vec![],
            bound: None,
        })
        // 输入 pin
        .add_pin(PinDefinition::data_input(
            "Input",
            DataRole::Input,
            PinTypeDesc::type_var(input_type_var),
        ))
        // 输出 pin
        .add_pin(PinDefinition::data_output(
            "Output",
            DataRole::Output,
            PinTypeDesc::type_var(output_type_var),
        ))
        // 数据求值器
        .with_data_evaluator(Arc::new(move |ctx| {
            // 获取输入值
            let input_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            
            // 获取输出 pin 的推断类型
            let output_type = ctx.get_pin_type_by_role(&PinRole::Data(DataRole::Output))?;
            
            // 记录转换信息
            ctx.log(format!(
                "Convert: {} -> {}",
                input_value.value_type(),
                output_type
            ));
            
            // 执行类型转换
            let converted_value = convert_to_type(input_value, &output_type)?;
            
            // 输出转换后的值
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), converted_value)?;
            Ok(())
        }));

    registry.register(definition);
}

// ============================================================================
// 类型转换实现
// ============================================================================

/// 根据目标类型转换值
fn convert_to_type(from_value: DataValue, to_type: &DataType) -> Result<DataValue, String> {
    match to_type {
        DataType::Boolean => convert_to_boolean(from_value),
        DataType::Int32 => convert_to_int32(from_value),
        DataType::Int64 => convert_to_int64(from_value),
        DataType::Float32 => convert_to_float32(from_value),
        DataType::Float64 => convert_to_float64(from_value),
        DataType::String => convert_to_string_value(from_value),
        _ => Err(format!("Conversion to {:?} not supported", to_type)),
    }
}

/// Boolean <- 所有类型
fn convert_to_boolean(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Boolean(b)),
        DataValue::Int32(i) => Ok(DataValue::Boolean(i != 0)),
        DataValue::Int64(i) => Ok(DataValue::Boolean(i != 0)),
        DataValue::Float32(f) => Ok(DataValue::Boolean(f != 0.0)),
        DataValue::Float64(f) => Ok(DataValue::Boolean(f != 0.0)),
        DataValue::String(s) => parse_boolean(&s)
            .map(DataValue::Boolean)
            .ok_or_else(|| format!("Cannot convert string '{}' to Boolean", s)),
        DataValue::Null => Ok(DataValue::Boolean(false)),
        _ => Err(format!("Cannot convert {:?} to Boolean", value.value_type())),
    }
}

/// Int32 <- 所有类型
fn convert_to_int32(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Int32(if b { 1 } else { 0 })),
        DataValue::Int32(i) => Ok(DataValue::Int32(i)),
        DataValue::Int64(i) => {
            if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                Ok(DataValue::Int32(i as i32))
            } else {
                Err(format!("Int64 value {} out of Int32 range", i))
            }
        }
        DataValue::Float32(f) => Ok(DataValue::Int32(f as i32)),
        DataValue::Float64(f) => Ok(DataValue::Int32(f as i32)),
        DataValue::String(s) => s
            .parse::<i32>()
            .map(DataValue::Int32)
            .map_err(|_| format!("Cannot parse string '{}' as Int32", s)),
        DataValue::Null => Ok(DataValue::Int32(0)),
        _ => Err(format!("Cannot convert {:?} to Int32", value.value_type())),
    }
}

/// Int64 <- 所有类型
fn convert_to_int64(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Int64(if b { 1 } else { 0 })),
        DataValue::Int32(i) => Ok(DataValue::Int64(i as i64)),
        DataValue::Int64(i) => Ok(DataValue::Int64(i)),
        DataValue::Float32(f) => Ok(DataValue::Int64(f as i64)),
        DataValue::Float64(f) => Ok(DataValue::Int64(f as i64)),
        DataValue::String(s) => s
            .parse::<i64>()
            .map(DataValue::Int64)
            .map_err(|_| format!("Cannot parse string '{}' as Int64", s)),
        DataValue::Null => Ok(DataValue::Int64(0)),
        _ => Err(format!("Cannot convert {:?} to Int64", value.value_type())),
    }
}

/// Float32 <- 所有类型
fn convert_to_float32(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Float32(if b { 1.0 } else { 0.0 })),
        DataValue::Int32(i) => Ok(DataValue::Float32(i as f32)),
        DataValue::Int64(i) => Ok(DataValue::Float32(i as f32)),
        DataValue::Float32(f) => Ok(DataValue::Float32(f)),
        DataValue::Float64(f) => Ok(DataValue::Float32(f as f32)),
        DataValue::String(s) => s
            .parse::<f32>()
            .map(DataValue::Float32)
            .map_err(|_| format!("Cannot parse string '{}' as Float32", s)),
        DataValue::Null => Ok(DataValue::Float32(0.0)),
        _ => Err(format!("Cannot convert {:?} to Float32", value.value_type())),
    }
}

/// Float64 <- 所有类型
fn convert_to_float64(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Float64(if b { 1.0 } else { 0.0 })),
        DataValue::Int32(i) => Ok(DataValue::Float64(i as f64)),
        DataValue::Int64(i) => Ok(DataValue::Float64(i as f64)),
        DataValue::Float32(f) => Ok(DataValue::Float64(f as f64)),
        DataValue::Float64(f) => Ok(DataValue::Float64(f)),
        DataValue::String(s) => s
            .parse::<f64>()
            .map(DataValue::Float64)
            .map_err(|_| format!("Cannot parse string '{}' as Float64", s)),
        DataValue::Null => Ok(DataValue::Float64(0.0)),
        _ => Err(format!("Cannot convert {:?} to Float64", value.value_type())),
    }
}

/// String <- 所有类型
fn convert_to_string_value(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::String(b.to_string())),
        DataValue::Int32(i) => Ok(DataValue::String(i.to_string())),
        DataValue::Int64(i) => Ok(DataValue::String(i.to_string())),
        DataValue::Float32(f) => Ok(DataValue::String(f.to_string())),
        DataValue::Float64(f) => Ok(DataValue::String(f.to_string())),
        DataValue::String(s) => Ok(DataValue::String(s)),
        DataValue::Null => Ok(DataValue::String(String::from("null"))),
        DataValue::Array(_) => Ok(DataValue::String(format!("{:?}", value))),
        DataValue::Object(_) => Ok(DataValue::String(format!("{:?}", value))),
        DataValue::DataFrame(id) => Ok(DataValue::String(format!("DataFrame({})", id))),
    }
}

/// 辅助函数：解析字符串为 Boolean
fn parse_boolean(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" | "" => Some(false),
        _ => None,
    }
}
