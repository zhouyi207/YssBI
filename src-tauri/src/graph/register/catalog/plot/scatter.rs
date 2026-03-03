//! Scatter 节点：接收两个数值 DataSeries（X、Y），打开 Plot 窗口绘制散点图

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct ScatterPlotData {
    data: Vec<ScatterPoint>,
    x_label: Option<String>,
    y_label: Option<String>,
}

#[derive(Serialize)]
struct ScatterPoint {
    x: f64,
    y: f64,
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Scatter", vec!["Plot".to_string()])
        .with_ui_style("plot")
        .with_description("Plot scatter chart from two numeric DataSeries (X, Y)")
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
                ])))),
            )),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let x_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;
            let y_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(1)))?;

            let x_id = match &x_value {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("Scatter: X input must be a numeric DataSeries".to_string()),
            };
            let y_id = match &y_value {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("Scatter: Y input must be a numeric DataSeries".to_string()),
            };

            let x_series = ctx.get_series(&x_id)?;
            let y_series = ctx.get_series(&y_id)?;

            let x_cast = x_series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("Scatter: X cannot cast to Float64: {}", e))?;
            let y_cast = y_series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("Scatter: Y cannot cast to Float64: {}", e))?;

            let x_f64 = x_cast.f64().map_err(|e| format!("Scatter: X is not numeric: {}", e))?;
            let y_f64 = y_cast.f64().map_err(|e| format!("Scatter: Y is not numeric: {}", e))?;

            let data: Vec<ScatterPoint> = x_f64
                .into_iter()
                .zip(y_f64.into_iter())
                .filter_map(|(ox, oy)| match (ox, oy) {
                    (Some(x), Some(y)) => Some(ScatterPoint { x, y }),
                    _ => None,
                })
                .collect();

            if data.is_empty() {
                return Err("Scatter: no valid (x, y) pairs after filtering nulls".to_string());
            }

            let x_label = x_series.name().to_string();
            let y_label = y_series.name().to_string();

            let plot_data = ScatterPlotData {
                data,
                x_label: if x_label.is_empty() { None } else { Some(x_label) },
                y_label: if y_label.is_empty() { None } else { Some(y_label) },
            };

            let json = serde_json::to_string(&plot_data).map_err(|e| format!("Scatter: serialize failed: {}", e))?;
            ctx.open_window("scatter".to_string(), json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
