use crate::graph::infer::{TypeConstraint, TypeVarDefinition, TypeVarKey};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use num_traits::{One, Zero};
use polars::prelude::{DataType as PDataType, Series};
use std::sync::Arc;
use yss_sci::api::database::dtype_from_string;

pub fn register(registry: &NodeRegistry) {
    register_convert(registry);
    register_data_series_string_to_categorical(registry);
    register_data_series_string_to_float64(registry);
    register_data_series_string_to_int64(registry);
    register_data_series_int64_to_string(registry);
    register_data_series_float64_to_string(registry);
    register_data_series_int64_to_float64(registry);
    register_data_series_float64_to_int64(registry);
    register_data_series_int64_to_bool(registry);
    register_data_series_float64_to_bool(registry);
    register_data_series_categorical_to_string(registry);
    register_data_series_int64_to_categorical(registry);
    register_data_series_categorical_to_int64(registry);
    register_data_series_float64_to_categorical(registry);
    register_data_series_categorical_to_float64(registry);
}

fn data_series_input_id(
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
) -> Result<String, String> {
    let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
    match &series_value {
        DataValue::DataSeries(v) => Ok(v.id.clone()),
        _ => Err("Input is not a DataSeries".to_string()),
    }
}

fn emit_data_series_output(
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
    series: Series,
    element_type: DataType,
) -> Result<(), String> {
    let result_id = ctx.put_data_series(series)?;
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
        constraints: vec![TypeConstraint::ConvertibleTo(TypeVarKey(
            "T_Output".to_string(),
        ))],
        bound: None,
    };

    let output_type_var = TypeVarDefinition {
        id: TypeVarKey("T_Output".to_string()),
        constraints: vec![TypeConstraint::ConvertibleFrom(TypeVarKey(
            "T_Input".to_string(),
        ))],
        bound: None,
    };

    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Convert",
            vec!["Value".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("value")
        .with_type_vars(vec![input_type_var, output_type_var])
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "Input",
                DataRole::Input,
                PinDataTypeDefinition::type_var(TypeVarKey("T_Input".to_string())),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Output",
                DataRole::Output,
                PinDataTypeDefinition::type_var(TypeVarKey("T_Output".to_string())),
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
        })),
        "Convert",
    );
    registry.register(definition);
}

fn convert_to_type(from_value: DataValue, to_type: &DataType) -> Result<DataValue, String> {
    match to_type {
        DataType::Any => Ok(from_value),
        DataType::Boolean => convert_to_boolean(from_value),
        DataType::Int64 => convert_to_int64(from_value),
        DataType::Float64 => convert_to_float64(from_value),
        DataType::String => convert_to_string_value(from_value),
        _ => Err(format!("Conversion to {:?} not supported", to_type)),
    }
}

fn convert_to_boolean(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Boolean(b)),
        DataValue::Int64(i) => Ok(DataValue::Boolean(i != 0)),
        DataValue::Float64(f) => Ok(DataValue::Boolean(!f.is_zero())),
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

fn convert_to_int64(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Int64(if b { i64::one() } else { i64::zero() })),
        DataValue::Int64(i) => Ok(DataValue::Int64(i)),
        DataValue::Float64(f) => Ok(DataValue::Int64(f as i64)),
        DataValue::String(s) => s
            .parse::<i64>()
            .map(DataValue::Int64)
            .map_err(|_| format!("Cannot parse string '{}' as Int64", s)),
        DataValue::Null => Ok(DataValue::Int64(i64::zero())),
        _ => Err(format!("Cannot convert {:?} to Int64", value.value_type())),
    }
}

fn convert_to_float64(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::Float64(if b { f64::one() } else { f64::zero() })),
        DataValue::Int64(i) => Ok(DataValue::Float64(i as f64)),
        DataValue::Float64(f) => Ok(DataValue::Float64(f)),
        DataValue::String(s) => s
            .parse::<f64>()
            .map(DataValue::Float64)
            .map_err(|_| format!("Cannot parse string '{}' as Float64", s)),
        DataValue::Null => Ok(DataValue::Float64(f64::zero())),
        _ => Err(format!(
            "Cannot convert {:?} to Float64",
            value.value_type()
        )),
    }
}

fn convert_to_string_value(value: DataValue) -> Result<DataValue, String> {
    match value {
        DataValue::Boolean(b) => Ok(DataValue::String(b.to_string())),
        DataValue::Int64(i) => Ok(DataValue::String(i.to_string())),
        DataValue::Float64(f) => Ok(DataValue::String(f.to_string())),
        DataValue::String(s) => Ok(DataValue::String(s)),
        DataValue::Null => Ok(DataValue::String(String::from("null"))),
        DataValue::Array(_) => Ok(DataValue::String(format!("{:?}", value))),
        DataValue::Object(_) => Ok(DataValue::String(format!("{:?}", value))),
        DataValue::DataFrame(id) => Ok(DataValue::String(format!("DataFrame({})", id))),
        DataValue::DataSeries(v) => Ok(DataValue::String(format!("DataSeries({})", v.id))),
        DataValue::Struct {
            type_key,
            handle_id,
        } => Ok(DataValue::String(format!(
            "Struct<{}>({})",
            type_key, handle_id
        ))),
    }
}

fn parse_boolean(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" | "" => Some(false),
        _ => None,
    }
}

fn register_data_series_string_to_categorical(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "String to Categorical",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                    DataType::Categorical,
                ))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("String to Categorical: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let target_dtype = dtype_from_string("categorical");
            let casted = series
                .cast(&target_dtype)
                .map_err(|e| format!("String to Categorical: cast failed: {}", e))?;
            emit_data_series_output(ctx, casted, DataType::Categorical)
                .map_err(|e| format!("String to Categorical: {}", e))?;
            Ok(())
        })),
        "String to Categorical",
    );
    registry.register(definition);
}

fn register_data_series_string_to_float64(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "String to Float64",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("String to Float64: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::Float64)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("String to Float64: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Float64)
                .map_err(|e| format!("String to Float64: {}", e))?;
            Ok(())
        })),
        "String to Float64",
    );
    registry.register(definition);
}

fn register_data_series_string_to_int64(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "String to Int64",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("String to Int64: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::Int64)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("String to Int64: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Int64)
                .map_err(|e| format!("String to Int64: {}", e))?;
            Ok(())
        })),
        "String to Int64",
    );
    registry.register(definition);
}

fn register_data_series_int64_to_string(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Int64 to String",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Int64 to String: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::String)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Int64 to String: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::String)
                .map_err(|e| format!("Int64 to String: {}", e))?;
            Ok(())
        })),
        "Int64 to String",
    );
    registry.register(definition);
}

fn register_data_series_float64_to_string(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Float64 to String",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Float64 to String: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::String)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Float64 to String: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::String)
                .map_err(|e| format!("Float64 to String: {}", e))?;
            Ok(())
        })),
        "Float64 to String",
    );
    registry.register(definition);
}

fn register_data_series_int64_to_float64(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Int64 to Float64",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Int64 to Float64: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::Float64)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Int64 to Float64: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Float64)
                .map_err(|e| format!("Int64 to Float64: {}", e))?;
            Ok(())
        })),
        "Int64 to Float64",
    );
    registry.register(definition);
}

fn register_data_series_float64_to_int64(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Float64 to Int64",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Float64 to Int64: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::Int64)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Float64 to Int64: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Int64)
                .map_err(|e| format!("Float64 to Int64: {}", e))?;
            Ok(())
        })),
        "Float64 to Int64",
    );
    registry.register(definition);
}

fn register_data_series_int64_to_bool(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Int64 to Boolean",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Int64 to Boolean: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::Boolean)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Int64 to Boolean: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Boolean)
                .map_err(|e| format!("Int64 to Boolean: {}", e))?;
            Ok(())
        })),
        "Int64 to Boolean",
    );
    registry.register(definition);
}

fn register_data_series_float64_to_bool(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Float64 to Boolean",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Float64 to Boolean: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let casted =
                series
                    .cast(&PDataType::Boolean)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Float64 to Boolean: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Boolean)
                .map_err(|e| format!("Float64 to Boolean: {}", e))?;
            Ok(())
        })),
        "Float64 to Boolean",
    );
    registry.register(definition);
}

fn register_data_series_categorical_to_string(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Categorical to String",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                    DataType::Categorical,
                ))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::String))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Categorical to String: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            ensure_categorical_or_enum(&series, "Categorical to String")?;
            let casted =
                series
                    .cast(&PDataType::String)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Categorical to String: cast failed: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::String)
                .map_err(|e| format!("Categorical to String: {}", e))?;
            Ok(())
        })),
        "Categorical to String",
    );
    registry.register(definition);
}

fn register_data_series_int64_to_categorical(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Int64 to Categorical",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                    DataType::Categorical,
                ))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Int64 to Categorical: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let as_str =
                series
                    .cast(&PDataType::String)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Int64 to Categorical: to string: {}", e)
                    })?;
            let cat_dtype = dtype_from_string("categorical");
            let casted = as_str
                .cast(&cat_dtype)
                .map_err(|e| format!("Int64 to Categorical: to categorical: {}", e))?;
            emit_data_series_output(ctx, casted, DataType::Categorical)
                .map_err(|e| format!("Int64 to Categorical: {}", e))?;
            Ok(())
        })),
        "Int64 to Categorical",
    );
    registry.register(definition);
}

fn register_data_series_categorical_to_int64(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Categorical to Int64",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                    DataType::Categorical,
                ))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Categorical to Int64: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            ensure_categorical_or_enum(&series, "Categorical to Int64")?;
            let as_str =
                series
                    .cast(&PDataType::String)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Categorical to Int64: to string: {}", e)
                    })?;
            let casted =
                as_str
                    .cast(&PDataType::Int64)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Categorical to Int64: to int64: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Int64)
                .map_err(|e| format!("Categorical to Int64: {}", e))?;
            Ok(())
        })),
        "Categorical to Int64",
    );
    registry.register(definition);
}

fn register_data_series_float64_to_categorical(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Float64 to Categorical",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                    DataType::Categorical,
                ))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Float64 to Categorical: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            let as_str =
                series
                    .cast(&PDataType::String)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Float64 to Categorical: to string: {}", e)
                    })?;
            let cat_dtype = dtype_from_string("categorical");
            let casted = as_str
                .cast(&cat_dtype)
                .map_err(|e| format!("Float64 to Categorical: to categorical: {}", e))?;
            emit_data_series_output(ctx, casted, DataType::Categorical)
                .map_err(|e| format!("Float64 to Categorical: {}", e))?;
            Ok(())
        })),
        "Float64 to Categorical",
    );
    registry.register(definition);
}

fn register_data_series_categorical_to_float64(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(
            "Categorical to Float64",
            vec!["Data".to_string(), "Conversion".to_string()],
        )
        .with_ui_style("dataframe")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                    DataType::Categorical,
                ))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataSeries",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_id =
                data_series_input_id(ctx).map_err(|e| format!("Categorical to Float64: {}", e))?;
            let series = ctx.get_data_series(&series_id)?;
            ensure_categorical_or_enum(&series, "Categorical to Float64")?;
            let as_str =
                series
                    .cast(&PDataType::String)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Categorical to Float64: to string: {}", e)
                    })?;
            let casted =
                as_str
                    .cast(&PDataType::Float64)
                    .map_err(|e: polars::error::PolarsError| {
                        format!("Categorical to Float64: to float64: {}", e)
                    })?;
            emit_data_series_output(ctx, casted, DataType::Float64)
                .map_err(|e| format!("Categorical to Float64: {}", e))?;
            Ok(())
        })),
        "Categorical to Float64",
    );
    registry.register(definition);
}
