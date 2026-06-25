//! DataSeries 比较节点：> < >= <= == !=
//!
//! 支持 Series vs 标量 或 Series vs Series（逐元素比较）

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::Series;
use std::sync::Arc;

/// 执行比较：series vs 标量（使用具体类型，Polars 不支持 AnyValue 直接比较）
fn compare_series_scalar(series: &Series, scalar: &DataValue, op: &str) -> Result<Series, String> {
    use polars::prelude::*;
    let to_series = |ca: BooleanChunked| Ok(ca.into_series());
    match scalar {
        DataValue::Float64(v) => {
            let r = match op {
                "gt" => series.gt(*v),
                "lt" => series.lt(*v),
                "gte" => series.gt_eq(*v),
                "lte" => series.lt_eq(*v),
                "eq" => series.equal(*v),
                "neq" => series.not_equal(*v),
                _ => return Err(format!("Compare: unknown op {}", op)),
            };
            r.map_err(|e| format!("Compare {}: {}", op, e))
                .and_then(to_series)
        }
        DataValue::Int64(v) => {
            let r = match op {
                "gt" => series.gt(*v),
                "lt" => series.lt(*v),
                "gte" => series.gt_eq(*v),
                "lte" => series.lt_eq(*v),
                "eq" => series.equal(*v),
                "neq" => series.not_equal(*v),
                _ => return Err(format!("Compare: unknown op {}", op)),
            };
            r.map_err(|e| format!("Compare {}: {}", op, e))
                .and_then(to_series)
        }
        DataValue::Int32(v) => {
            let r = match op {
                "gt" => series.gt(*v),
                "lt" => series.lt(*v),
                "gte" => series.gt_eq(*v),
                "lte" => series.lt_eq(*v),
                "eq" => series.equal(*v),
                "neq" => series.not_equal(*v),
                _ => return Err(format!("Compare: unknown op {}", op)),
            };
            r.map_err(|e| format!("Compare {}: {}", op, e))
                .and_then(to_series)
        }
        DataValue::Boolean(v) => {
            if op != "eq" && op != "neq" {
                return Err(format!(
                    "Compare: Boolean only supports == and !=, not {}",
                    op
                ));
            }
            let scalar_series = Series::from_iter(std::iter::repeat(*v).take(series.len()));
            let r = if op == "eq" {
                series.equal(&scalar_series)
            } else {
                series.not_equal(&scalar_series)
            };
            r.map_err(|e| format!("Compare {}: {}", op, e))
                .and_then(to_series)
        }
        DataValue::String(s) => {
            if op != "eq" && op != "neq" {
                return Err(format!(
                    "Compare: String only supports == and !=, not {}",
                    op
                ));
            }
            let scalar_series = Series::from_iter(std::iter::repeat(s.as_str()).take(series.len()));
            let r = if op == "eq" {
                series.equal(&scalar_series)
            } else {
                series.not_equal(&scalar_series)
            };
            r.map_err(|e| format!("Compare {}: {}", op, e))
                .and_then(to_series)
        }
        DataValue::Float32(v) => {
            let r = match op {
                "gt" => series.gt(*v),
                "lt" => series.lt(*v),
                "gte" => series.gt_eq(*v),
                "lte" => series.lt_eq(*v),
                "eq" => series.equal(*v),
                "neq" => series.not_equal(*v),
                _ => return Err(format!("Compare: unknown op {}", op)),
            };
            r.map_err(|e| format!("Compare {}: {}", op, e))
                .and_then(to_series)
        }
        _ => Err(
            "Compare: Value must be scalar (Boolean, Int, Float, String) or DataSeries".to_string(),
        ),
    }
}

/// 执行比较：series vs series（逐元素）
fn compare_series_series(a: &Series, b: &Series, op: &str) -> Result<Series, String> {
    use polars::prelude::*;
    if a.len() != b.len() {
        return Err(format!(
            "Compare: Series lengths differ ({} vs {}). Use same-length series or scalar.",
            a.len(),
            b.len()
        ));
    }
    let result = match op {
        "gt" => a.gt(b),
        "lt" => a.lt(b),
        "gte" => a.gt_eq(b),
        "lte" => a.lt_eq(b),
        "eq" => a.equal(b),
        "neq" => a.not_equal(b),
        _ => return Err(format!("Compare: unknown op {}", op)),
    };
    result
        .map_err(|e| format!("Compare {}: {}", op, e))
        .map(|ca: BooleanChunked| ca.into_series())
}

fn register_compare_node(
    registry: &NodeRegistry,
    name: &str,
    op: &str,
    desc_zh: &str,
    desc_en: &str,
) {
    let name = name.to_string();
    let op = op.to_string();
    let definition = NodeDefinition::new(
        name.clone(),
        vec![
            "Data".to_string(),
            "Series".to_string(),
            "Comparison".to_string(),
        ],
    )
    .with_ui_style("dataframe")
    .with_localized_description(desc_zh, desc_en)
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Any))),
        )),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Value",
                DataRole::Custom("value".to_string()),
                PinDataTypeDefinition::concrete(DataType::one_of(vec![
                    DataType::Float64,
                    DataType::Int64,
                    DataType::String,
                    DataType::Boolean,
                    DataType::DataSeries(Box::new(DataType::Any)),
                ])),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
        )),
    ])
    .with_data_evaluator(Arc::new(move |ctx| {
        let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let series_id = match &series_value {
            DataValue::DataSeries(v) => v.id.clone(),
            DataValue::Null => {
                return Err(format!("{}: Series input is not connected", name));
            }
            other => {
                return Err(format!(
                    "{}: Series input must be a DataSeries (got {:?})",
                    name,
                    other.value_type().unwrap_or(DataType::Any)
                ));
            }
        };

        let value_input =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("value".to_string())))?;

        let series = ctx.get_series(&series_id)?;

        let result_series = match &value_input {
            DataValue::DataSeries(v) => {
                let other = ctx.get_series(&v.id)?;
                compare_series_series(&series, &other, &op)?
            }
            DataValue::Null => {
                return Err(format!(
                    "{}: Value is not connected. Connect a scalar or another DataSeries.",
                    name
                ));
            }
            scalar => compare_series_scalar(&series, scalar, &op)?,
        };

        let result_id = ctx.put_series(result_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataSeries(DataSeriesValue::with_element_type(
                result_id,
                DataType::Boolean,
            )),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

pub fn register(registry: &NodeRegistry) {
    register_compare_node(
        registry,
        "Series Greater Than (>)",
        "gt",
        "逐元素大于：Series > Value（标量或 DataSeries）",
        "Element-wise greater than: Series > Value (scalar or DataSeries)",
    );
    register_compare_node(
        registry,
        "Series Less Than (<)",
        "lt",
        "逐元素小于：Series < Value（标量或 DataSeries）",
        "Element-wise less than: Series < Value (scalar or DataSeries)",
    );
    register_compare_node(
        registry,
        "Series Greater Equal (>=)",
        "gte",
        "逐元素大于等于：Series >= Value（标量或 DataSeries）",
        "Element-wise greater or equal: Series >= Value (scalar or DataSeries)",
    );
    register_compare_node(
        registry,
        "Series Less Equal (<=)",
        "lte",
        "逐元素小于等于：Series <= Value（标量或 DataSeries）",
        "Element-wise less or equal: Series <= Value (scalar or DataSeries)",
    );
    register_compare_node(
        registry,
        "Series Equal (==)",
        "eq",
        "逐元素相等：Series == Value（标量或 DataSeries）",
        "Element-wise equality: Series == Value (scalar or DataSeries)",
    );
    register_compare_node(
        registry,
        "Series Not Equal (!=)",
        "neq",
        "逐元素不等：Series != Value（标量或 DataSeries）",
        "Element-wise not equal: Series != Value (scalar or DataSeries)",
    );
}
