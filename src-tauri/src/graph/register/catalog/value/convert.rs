use crate::graph::infer::{TypeConstraint, TypeVarDefinition, TypeVarKey};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use num_traits::{One, Zero};
use polars::prelude::{DataType as PDataType, Series};
use std::sync::Arc;
use yss_sci::api::database::dtype_from_string;

pub fn register(registry: &NodeRegistry) {
    register_convert(registry);
    register_series_string_to_categorical(registry);
    register_series_string_to_float64(registry);
    register_series_string_to_int64(registry);
    register_series_int64_to_string(registry);
    register_series_float64_to_string(registry);
    register_series_int64_to_float64(registry);
    register_series_float64_to_int64(registry);
    register_series_int64_to_bool(registry);
    register_series_float64_to_bool(registry);
    register_series_categorical_to_string(registry);
    register_series_int64_to_categorical(registry);
    register_series_categorical_to_int64(registry);
    register_series_float64_to_categorical(registry);
    register_series_categorical_to_float64(registry);
}

fn series_input_id(ctx: &mut dyn crate::execution::NodeExecutionContextTrait) -> Result<String, String> {
    let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
    match &series_value {
        DataValue::DataSeries(v) => Ok(v.id.clone()),
        _ => Err("Input is not a DataSeries".to_string()),
    }
}

fn emit_series_output(
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
    series: Series,
    element_type: DataType,
) -> Result<(), String> {
    let result_id = ctx.put_series(series)?;
    ctx.emit_output_by_role(
        &PinRole::Data(DataRole::Output),
        DataValue::DataSeries(DataSeriesValue::with_element_type(result_id, element_type)),
    )?;
    Ok(())
}

fn ensure_categorical_or_enum(s: &Series, node: &str) -> Result<(), String> {
    match s.dtype() {
        PDataType::Categorical(_, _) | PDataType::Enum(_, _) => Ok(()),
        _ => Err(format!(
            "{}: expected Categorical or Enum series, got {:?}",
            node,
            s.dtype()
        )),
    }
}

fn register_convert(registry: &NodeRegistry) {
    let input_type_var = TypeVarDefinition {
        id: TypeVarKey("T_Input".to_string()),
        constraints: vec![TypeConstraint::ConvertibleTo(TypeVarKey("T_Output".to_string()))],
        bound: None,
    };

    let output_type_var = TypeVarDefinition {
        id: TypeVarKey("T_Output".to_string()),
        constraints: vec![TypeConstraint::ConvertibleFrom(TypeVarKey("T_Input".to_string()))],
        bound: None,
    };

    let definition = NodeDefinition::new("Convert", vec!["Value".to_string(), "Conversion".to_string()])
        .with_ui_style("value")
        .with_description("Convert value from one type to another")
        .with_type_vars(vec![input_type_var, output_type_var])
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "Input", DataRole::Input, PinDataTypeDefinition::type_var(TypeVarKey("T_Input".to_string())),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Output", DataRole::Output, PinDataTypeDefinition::type_var(TypeVarKey("T_Output".to_string())),
            )),
        ])
        .with_data_evaluator(Arc::new(move |ctx| {
            let input_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            let output_type = ctx.get_pin_type_by_role(&PinRole::Data(DataRole::Output))?;
            ctx.log(format!(
                "Convert: {} -> {}",
                input_value.value_type().expect("None").to_string(),
                output_type
            ));
            let converted_value = convert_to_type(input_value, &output_type)?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), converted_value)?;
            Ok(())
        }));

    registry.register(definition);
}

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

fn convert_to_boolean(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Boolean(b)),
        DataValue::Int32(i) => Ok(DataValue::Boolean(i != 0)),
        DataValue::Int64(i) => Ok(DataValue::Boolean(i != 0)),
        DataValue::Float32(f) => Ok(DataValue::Boolean(!f.is_zero())),
        DataValue::Float64(f) => Ok(DataValue::Boolean(!f.is_zero())),
        DataValue::String(s) => parse_boolean(&s)
            .map(DataValue::Boolean)
            .ok_or_else(|| format!("Cannot convert string '{}' to Boolean", s)),
        DataValue::Null => Ok(DataValue::Boolean(false)),
        _ => Err(format!("Cannot convert {:?} to Boolean", value.value_type())),
    }
}

fn convert_to_int32(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Int32(if b { i32::one() } else { i32::zero() })),
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
        DataValue::String(s) => s.parse::<i32>().map(DataValue::Int32).map_err(|_| format!("Cannot parse string '{}' as Int32", s)),
        DataValue::Null => Ok(DataValue::Int32(i32::zero())),
        _ => Err(format!("Cannot convert {:?} to Int32", value.value_type())),
    }
}

fn convert_to_int64(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Int64(if b { i64::one() } else { i64::zero() })),
        DataValue::Int32(i) => Ok(DataValue::Int64(i as i64)),
        DataValue::Int64(i) => Ok(DataValue::Int64(i)),
        DataValue::Float32(f) => Ok(DataValue::Int64(f as i64)),
        DataValue::Float64(f) => Ok(DataValue::Int64(f as i64)),
        DataValue::String(s) => s.parse::<i64>().map(DataValue::Int64).map_err(|_| format!("Cannot parse string '{}' as Int64", s)),
        DataValue::Null => Ok(DataValue::Int64(i64::zero())),
        _ => Err(format!("Cannot convert {:?} to Int64", value.value_type())),
    }
}

fn convert_to_float32(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Float32(if b { f32::one() } else { f32::zero() })),
        DataValue::Int32(i) => Ok(DataValue::Float32(i as f32)),
        DataValue::Int64(i) => Ok(DataValue::Float32(i as f32)),
        DataValue::Float32(f) => Ok(DataValue::Float32(f)),
        DataValue::Float64(f) => Ok(DataValue::Float32(f as f32)),
        DataValue::String(s) => s.parse::<f32>().map(DataValue::Float32).map_err(|_| format!("Cannot parse string '{}' as Float32", s)),
        DataValue::Null => Ok(DataValue::Float32(f32::zero())),
        _ => Err(format!("Cannot convert {:?} to Float32", value.value_type())),
    }
}

fn convert_to_float64(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Float64(if b { f64::one() } else { f64::zero() })),
        DataValue::Int32(i) => Ok(DataValue::Float64(i as f64)),
        DataValue::Int64(i) => Ok(DataValue::Float64(i as f64)),
        DataValue::Float32(f) => Ok(DataValue::Float64(f as f64)),
        DataValue::Float64(f) => Ok(DataValue::Float64(f)),
        DataValue::String(s) => s.parse::<f64>().map(DataValue::Float64).map_err(|_| format!("Cannot parse string '{}' as Float64", s)),
        DataValue::Null => Ok(DataValue::Float64(f64::zero())),
        _ => Err(format!("Cannot convert {:?} to Float64", value.value_type())),
    }
}

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
        DataValue::Struct { type_key, handle_id } => Ok(DataValue::String(format!("Struct<{}>({})", type_key, handle_id))),
    }
}

fn parse_boolean(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" | "" => Some(false),
        _ => None,
    }
}

fn register_series_string_to_categorical(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "String to Categorical",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of String type to Categorical")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("String to Categorical: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let target_dtype = dtype_from_string("categorical");
        let casted = series
            .cast(&target_dtype)
            .map_err(|e| format!("String to Categorical: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::Categorical)
            .map_err(|e| format!("String to Categorical: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_string_to_float64(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "String to Float64",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of String type to Float64")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("String to Float64: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::Float64)
            .map_err(|e: polars::error::PolarsError| format!("String to Float64: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::Float64).map_err(|e| format!("String to Float64: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_string_to_int64(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "String to Int64",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of String type to Int64")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("String to Int64: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::Int64)
            .map_err(|e: polars::error::PolarsError| format!("String to Int64: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::Int64).map_err(|e| format!("String to Int64: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_int64_to_string(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Int64 to String",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of Int64 type to String")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Int64 to String: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::String)
            .map_err(|e: polars::error::PolarsError| format!("Int64 to String: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::String).map_err(|e| format!("Int64 to String: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_float64_to_string(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Float64 to String",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of Float64 type to String")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Float64 to String: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::String)
            .map_err(|e: polars::error::PolarsError| format!("Float64 to String: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::String).map_err(|e| format!("Float64 to String: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_int64_to_float64(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Int64 to Float64",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of Int64 type to Float64")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Int64 to Float64: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::Float64)
            .map_err(|e: polars::error::PolarsError| format!("Int64 to Float64: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::Float64).map_err(|e| format!("Int64 to Float64: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_float64_to_int64(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Float64 to Int64",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of Float64 type to Int64 (truncates toward zero; out of range / non-finite → null per Polars)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Float64 to Int64: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::Int64)
            .map_err(|e: polars::error::PolarsError| format!("Float64 to Int64: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::Int64).map_err(|e| format!("Float64 to Int64: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_int64_to_bool(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Int64 to Boolean",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of Int64 to Boolean (0 → false, non-zero → true; null stays null)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Int64 to Boolean: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::Boolean)
            .map_err(|e: polars::error::PolarsError| format!("Int64 to Boolean: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::Boolean).map_err(|e| format!("Int64 to Boolean: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_float64_to_bool(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Float64 to Boolean",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description(
        "Convert a DataSeries of Float64 to Boolean (0 → false, non-zero → true; null / non-finite per Polars)",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Float64 to Boolean: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let casted = series
            .cast(&PDataType::Boolean)
            .map_err(|e: polars::error::PolarsError| format!("Float64 to Boolean: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::Boolean).map_err(|e| format!("Float64 to Boolean: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_categorical_to_string(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Categorical to String",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert a DataSeries of Categorical (or Enum) type to String")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Categorical to String: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        ensure_categorical_or_enum(&series, "Categorical to String")?;
        let casted = series
            .cast(&PDataType::String)
            .map_err(|e: polars::error::PolarsError| format!("Categorical to String: cast failed: {}", e))?;
        emit_series_output(ctx, casted, DataType::String).map_err(|e| format!("Categorical to String: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_int64_to_categorical(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Int64 to Categorical",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert Int64 to Categorical (via String encoding, same category pool as other cat casts)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Int64 to Categorical: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let as_str = series
            .cast(&PDataType::String)
            .map_err(|e: polars::error::PolarsError| format!("Int64 to Categorical: to string: {}", e))?;
        let cat_dtype = dtype_from_string("categorical");
        let casted = as_str
            .cast(&cat_dtype)
            .map_err(|e| format!("Int64 to Categorical: to categorical: {}", e))?;
        emit_series_output(ctx, casted, DataType::Categorical).map_err(|e| format!("Int64 to Categorical: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_categorical_to_int64(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Categorical to Int64",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert Categorical to Int64 (category labels must parse as integers; invalid → null)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Categorical to Int64: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        ensure_categorical_or_enum(&series, "Categorical to Int64")?;
        let as_str = series
            .cast(&PDataType::String)
            .map_err(|e: polars::error::PolarsError| format!("Categorical to Int64: to string: {}", e))?;
        let casted = as_str
            .cast(&PDataType::Int64)
            .map_err(|e: polars::error::PolarsError| format!("Categorical to Int64: to int64: {}", e))?;
        emit_series_output(ctx, casted, DataType::Int64).map_err(|e| format!("Categorical to Int64: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_float64_to_categorical(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Float64 to Categorical",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert Float64 to Categorical (via String representation)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Float64 to Categorical: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        let as_str = series
            .cast(&PDataType::String)
            .map_err(|e: polars::error::PolarsError| format!("Float64 to Categorical: to string: {}", e))?;
        let cat_dtype = dtype_from_string("categorical");
        let casted = as_str
            .cast(&cat_dtype)
            .map_err(|e| format!("Float64 to Categorical: to categorical: {}", e))?;
        emit_series_output(ctx, casted, DataType::Categorical).map_err(|e| format!("Float64 to Categorical: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_series_categorical_to_float64(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Categorical to Float64",
        vec!["Data".to_string(), "Conversion".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Convert Categorical to Float64 (category labels must parse as floats; invalid → null)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_id = series_input_id(ctx).map_err(|e| format!("Categorical to Float64: {}", e))?;
        let series = ctx.get_series(&series_id)?;
        ensure_categorical_or_enum(&series, "Categorical to Float64")?;
        let as_str = series
            .cast(&PDataType::String)
            .map_err(|e: polars::error::PolarsError| format!("Categorical to Float64: to string: {}", e))?;
        let casted = as_str
            .cast(&PDataType::Float64)
            .map_err(|e: polars::error::PolarsError| format!("Categorical to Float64: to float64: {}", e))?;
        emit_series_output(ctx, casted, DataType::Float64).map_err(|e| format!("Categorical to Float64: {}", e))?;
        Ok(())
    }));
    registry.register(definition);
}
