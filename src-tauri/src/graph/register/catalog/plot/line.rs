//! Line 节点：接收两个 DataSeries（X、Y），X 可为数值或日期，Y 为数值，打开 Plot 窗口绘制折线图

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LinePlotData {
    data: Vec<LinePoint>,
    x_label: Option<String>,
    y_label: Option<String>,
    /// "date" = days since epoch, "datetime" = microseconds since epoch, "number" = 普通数值
    x_format: String,
    y_format: String,
}

#[derive(Serialize)]
struct LinePoint {
    x: f64,
    y: f64,
}

/// 将 Series 转为 Float64，支持数值类型和 Date/Datetime（转为可绘制的数值）
fn series_to_plot_f64(s: &polars::prelude::Series) -> Result<polars::prelude::Series, String> {
    use polars::prelude::DataType as P;
    let dt = s.dtype();
    let casted = if matches!(dt, P::Date) {
        s.cast(&P::Int32)
            .map_err(|e| format!("Date->Int32: {}", e))?
            .cast(&P::Float64)
            .map_err(|e| format!("Int32->Float64: {}", e))?
    } else if matches!(dt, P::Datetime(_, _)) {
        s.cast(&P::Int64)
            .map_err(|e| format!("Datetime->Int64: {}", e))?
            .cast(&P::Float64)
            .map_err(|e| format!("Int64->Float64: {}", e))?
    } else if matches!(dt, P::Time) {
        s.cast(&P::Int64)
            .map_err(|e| format!("Time->Int64: {}", e))?
            .cast(&P::Float64)
            .map_err(|e| format!("Int64->Float64: {}", e))?
    } else {
        s.cast(&P::Float64)
            .map_err(|e| format!("cast to Float64: {}", e))?
    };
    Ok(casted)
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Line", vec!["Plot".to_string()])
        .with_ui_style("plot")
        .with_localized_description("用两个 DataSeries 绘制折线图；X 可为数值或 Date，Y 须为数值", "Plot line chart from two DataSeries (X, Y). X can be numeric or Date, Y must be numeric.")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::data_input(
                "X",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
                    DataType::Float64,
                    DataType::Int64,
                    DataType::Int32,
                    DataType::Float32,
                    DataType::Date,
                ])))),
            )),
            PinSlot::fixed(PinDefinition::data_input(
                "Y",
                DataRole::Inputs(1),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
                    DataType::Float64,
                    DataType::Int64,
                    DataType::Int32,
                    DataType::Float32,
                    DataType::Date,
                ])))),
            )),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let x_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;
            let y_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(1)))?;

            let x_id = match &x_value {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("Line: X input must be a numeric DataSeries".to_string()),
            };
            let y_id = match &y_value {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("Line: Y input must be a numeric DataSeries".to_string()),
            };

            let x_series = ctx.get_series(&x_id)?;
            let y_series = ctx.get_series(&y_id)?;

            let x_cast = series_to_plot_f64(&x_series)
                .map_err(|e| format!("Line: X {}", e))?;
            let y_cast = series_to_plot_f64(&y_series)
                .map_err(|e| format!("Line: Y {}", e))?;

            let x_f64 = x_cast.f64().map_err(|e| format!("X is not plottable: {}", e))?;
            let y_f64 = y_cast.f64().map_err(|e| format!("Y is not plottable: {}", e))?;

            let data: Vec<LinePoint> = x_f64
                .into_iter()
                .zip(y_f64.into_iter())
                .filter_map(|(ox, oy)| match (ox, oy) {
                    (Some(x), Some(y)) => Some(LinePoint { x, y }),
                    _ => None,
                })
                .collect();

            if data.is_empty() {
                return Err("Line: no valid (x, y) pairs after filtering nulls".to_string());
            }

            let x_label = x_series.name().to_string();
            let y_label = y_series.name().to_string();

            let x_format = match x_series.dtype() {
                dt if matches!(dt, polars::prelude::DataType::Date) => "date",
                dt if matches!(dt, polars::prelude::DataType::Datetime(_, _)) => "datetime",
                _ => "number",
            };
            let y_format = match y_series.dtype() {
                dt if matches!(dt, polars::prelude::DataType::Date) => "date",
                dt if matches!(dt, polars::prelude::DataType::Datetime(_, _)) => "datetime",
                _ => "number",
            };

            let plot_data = LinePlotData {
                data,
                x_label: if x_label.is_empty() { None } else { Some(x_label) },
                y_label: if y_label.is_empty() { None } else { Some(y_label) },
                x_format: x_format.to_string(),
                y_format: y_format.to_string(),
            };

            let json = serde_json::to_string(&plot_data).map_err(|e| format!("Line: serialize failed: {}", e))?;
            ctx.open_window("line".to_string(), json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
