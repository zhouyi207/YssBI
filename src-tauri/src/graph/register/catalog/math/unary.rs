//! 一元数学函数节点：ln, log2, log10, exp, sqrt, square
//!
//! 支持 scalar (Float64) 与 DataSeries<数值类型> 输入：
//! - scalar → scalar
//! - DataSeries → DataSeries（逐元素）

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::IntoSeries;
use std::sync::Arc;

fn numeric_input_type() -> DataType {
    DataType::one_of(vec![DataType::number(), DataType::number_series()])
}

fn numeric_output_type() -> DataType {
    DataType::one_of(vec![
        DataType::Float64,
        DataType::DataSeries(Box::new(DataType::Float64)),
    ])
}

fn scalar_to_f64(v: &DataValue) -> Option<f64> {
    v.as_f64()
}

fn register_unary_fn(
    registry: &NodeRegistry,
    name: &str,
    f: fn(f64) -> f64,
) {
    let name_owned = name.to_string();
    let definition = docs::math::apply_docs(
        NodeDefinition::new(name, vec!["Math".to_string(), "Functions".to_string()])
        .with_ui_style("math")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "X",
                DataRole::Input,
                PinDataTypeDefinition::concrete(numeric_input_type()),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Result",
                DataRole::Output,
                PinDataTypeDefinition::concrete(numeric_output_type()),
            )),
        ])
        .with_data_evaluator(Arc::new(move |ctx| {
            let input = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            let result = match &input {
                DataValue::DataSeries(dsv) => {
                    let series = ctx.get_data_series(&dsv.id)?;
                    let cast = series
                        .cast(&polars::prelude::DataType::Float64)
                        .map_err(|e| format!("{}: cannot cast to Float64: {}", name_owned, e))?;
                    let ca = cast.f64().map_err(|e| format!("{}: {}", name_owned, e))?;
                    let result_series: polars::prelude::Float64Chunked =
                        ca.into_iter().map(|opt| opt.map(f)).collect();
                    let id = ctx.put_data_series(result_series.into_series())?;
                    DataValue::DataSeries(DataSeriesValue::with_element_type(id, DataType::Float64))
                }
                _ => {
                    let x = scalar_to_f64(&input)
                        .ok_or_else(|| format!("{}: input is not numeric", name_owned))?;
                    DataValue::Float64(f(x))
                }
            };
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), result)?;
            Ok(())
        })),
        name,
    );
    registry.register(definition);
}

pub fn register(registry: &NodeRegistry) {
    register_unary_fn(registry, "Ln", f64::ln);
    register_unary_fn(registry, "Log2", f64::log2);
    register_unary_fn(registry, "Log10", f64::log10);
    register_unary_fn(registry, "Exp", f64::exp);
    register_unary_fn(registry, "Sqrt", f64::sqrt);
    register_unary_fn(registry, "Square", |x| x * x);
}
