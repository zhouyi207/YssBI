use crate::graph::infer::{TypeConstraint, TypeVarDefinition, TypeVarKey};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use std::sync::Arc;

/// Convert 节点支持转换的标量类型（Input/Output 只能在此集合内）
const CONVERTIBLE_SCALAR_TYPES: &[DataType] = &[
    DataType::Boolean,
    DataType::Int32,
    DataType::Int64,
    DataType::Float32,
    DataType::Float64,
    DataType::String,
];

pub fn register(registry: &NodeRegistry) {
    register_convert(registry);
}

/// Convert 节点 - 类型转换
///
/// Input 与 Output 独立推断，不互相传播：
/// - Input 由上游连接确定，Output 保持 Any 但受约束：只能是 Input 可转换到的类型
/// - Output 由下游连接确定，Input 保持 Any 但受约束：只能是可转换到 Output 的类型
/// 约束：二者均为 OneOf(Boolean, Int32, Int64, Float32, Float64, String)
fn register_convert(registry: &NodeRegistry) {
    let convertible_constraint = TypeConstraint::OneOf(CONVERTIBLE_SCALAR_TYPES.to_vec());

    let input_type_var = TypeVarDefinition {
        id: TypeVarKey("T_Input".to_string()),
        constraints: vec![convertible_constraint.clone()],
        bound: None,
    };

    let output_type_var = TypeVarDefinition {
        id: TypeVarKey("T_Output".to_string()),
        constraints: vec![convertible_constraint],
        bound: None,
    };

    // 克隆 key 用于闭包
    let input_key = input_type_var.id.clone();
    let output_key = output_type_var.id.clone();

    let definition = NodeDefinition::new("Convert", vec!["Value".to_string(), "Conversion".to_string()])
        .with_ui_style("value")
        .with_description("Convert value from one type to another")
        .with_type_vars(vec![input_type_var, output_type_var])
        .with_pin_generator(Arc::new(move || {
            Ok(vec![
                PinDefinition::data_input(
                    "Input",
                    DataRole::Input,
                    PinDataTypeDefinition::type_var(input_key.clone()),
                ),
                PinDefinition::data_output(
                    "Output",
                    DataRole::Output,
                    PinDataTypeDefinition::type_var(output_key.clone()),
                ),
            ])
        }))
        // 数据求值器
        .with_data_evaluator(Arc::new(move |ctx| {
            // 获取输入值
            let input_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;

            // 获取输出 pin 的推断类型
            let output_type = ctx.get_pin_type_by_role(&PinRole::Data(DataRole::Output))?;

            // 记录转换信息
            ctx.log(format!(
                "Convert: {} -> {}",
                input_value.value_type().expect("None").to_string(),
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
/// 当 to_type 为 Any 时（Output 未连接下游），透传输入值
fn convert_to_type(from_value: DataValue, to_type: &DataType) -> Result<DataValue, String> {
    match to_type {
        DataType::Any => Ok(from_value),
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
        _ => Err(format!(
            "Cannot convert {:?} to Boolean",
            value.value_type()
        )),
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
        _ => Err(format!(
            "Cannot convert {:?} to Float32",
            value.value_type()
        )),
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
        _ => Err(format!(
            "Cannot convert {:?} to Float64",
            value.value_type()
        )),
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
        DataValue::DataSeries(v) => Ok(DataValue::String(format!("DataSeries({})", v.id))),
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

