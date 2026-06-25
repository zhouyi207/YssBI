//! 一元数学函数节点：ln, log2, log10, exp, sqrt, square
//!
//! 支持 scalar (Float64) 与 DataSeries<数值类型> 输入：
//! - scalar → scalar
//! - DataSeries → DataSeries（逐元素）

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::IntoSeries;
use std::sync::Arc;

fn numeric_input_type() -> DataType {
    DataType::one_of(vec![
        DataType::Float64,
        DataType::Float32,
        DataType::Int64,
        DataType::Int32,
        DataType::DataSeries(Box::new(DataType::one_of(vec![
            DataType::Float64,
            DataType::Float32,
            DataType::Int64,
            DataType::Int32,
        ]))),
    ])
}

fn numeric_output_type() -> DataType {
    DataType::one_of(vec![
        DataType::Float64,
        DataType::DataSeries(Box::new(DataType::Float64)),
    ])
}

fn scalar_to_f64(v: &DataValue) -> Option<f64> {
    match v {
        DataValue::Float64(x) => Some(*x),
        DataValue::Float32(x) => Some(*x as f64),
        DataValue::Int64(x) => Some(*x as f64),
        DataValue::Int32(x) => Some(*x as f64),
        _ => None,
    }
}

fn register_unary_fn(
    registry: &NodeRegistry,
    name: &str,
    desc_zh: &str,
    desc_en: &str,
    f: fn(f64) -> f64,
) {
    let name_owned = name.to_string();
    let definition = NodeDefinition::new(name, vec!["Math".to_string(), "Functions".to_string()])
        .with_ui_style("math")
        .with_localized_description(desc_zh, desc_en)
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
                    let series = ctx.get_series(&dsv.id)?;
                    let cast = series
                        .cast(&polars::prelude::DataType::Float64)
                        .map_err(|e| format!("{}: cannot cast to Float64: {}", name_owned, e))?;
                    let ca = cast.f64().map_err(|e| format!("{}: {}", name_owned, e))?;
                    let result_series: polars::prelude::Float64Chunked =
                        ca.into_iter().map(|opt| opt.map(f)).collect();
                    let id = ctx.put_series(result_series.into_series())?;
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
        }));
    registry.register(definition);
}

pub fn register(registry: &NodeRegistry) {
    register_unary_fn(
        registry,
        "Ln",
        "自然对数（以 e 为底），逐元素计算",
        "Natural logarithm (base e) of each element",
        f64::ln,
    );
    register_unary_fn(
        registry,
        "Log2",
        "以 2 为底的对数，逐元素计算",
        "Base-2 logarithm of each element",
        f64::log2,
    );
    register_unary_fn(
        registry,
        "Log10",
        "以 10 为底的对数，逐元素计算",
        "Base-10 logarithm of each element",
        f64::log10,
    );
    register_unary_fn(
        registry,
        "Exp",
        "指数函数 e^x，逐元素计算",
        "Exponential (e^x) of each element",
        f64::exp,
    );
    register_unary_fn(
        registry,
        "Sqrt",
        "平方根，逐元素计算",
        "Square root of each element",
        f64::sqrt,
    );
    register_unary_fn(
        registry,
        "Square",
        "平方 x²，逐元素计算",
        "Square (x²) of each element",
        |x| x * x,
    );
}
