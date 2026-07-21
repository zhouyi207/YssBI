//! KDE 节点：接收一个数值 DataSeries，计算核密度估计，打开 Plot 窗口绘制 KDE 曲线

use crate::execution::{ExecutionEffect, PlotChart};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataType, DataValue};
use crate::sci::kde::gaussian_kde_grid;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct KdePlotData {
    data: Vec<KdePoint>,
    x_label: Option<String>,
    y_label: Option<String>,
}

#[derive(Serialize)]
struct KdePoint {
    x: f64,
    y: f64,
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("KDE", vec!["Plot".to_string()])
        .with_ui_style("plot")
        .with_documentation(docs::plot::KDE_ZH, docs::plot::KDE_EN)
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::data_input(
                "Values",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                    vec![DataType::Float64, DataType::Int64],
                )))),
            )),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;

            let id = match &value {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("KDE: input must be a numeric DataSeries".to_string()),
            };

            let series = ctx.get_data_series(&id)?;
            let cast = series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("KDE: cannot cast to Float64: {}", e))?;
            let f64_chunk = cast.f64().map_err(|e| format!("KDE: not numeric: {}", e))?;

            let values: Vec<f64> = f64_chunk
                .into_iter()
                .filter_map(|v| v)
                .filter(|v| v.is_finite())
                .collect();

            if values.len() < 2 {
                return Err("KDE: need at least 2 valid values".to_string());
            }

            const GRID_POINTS: usize = 256;
            let data = gaussian_kde_grid(&values, GRID_POINTS)
                .into_iter()
                .map(|point| KdePoint {
                    x: point.x,
                    y: point.density,
                })
                .collect();

            let x_label = series.name().to_string();

            let plot_data = KdePlotData {
                data,
                x_label: if x_label.is_empty() {
                    None
                } else {
                    Some(x_label)
                },
                y_label: Some("Density".to_string()),
            };

            let json = serde_json::to_string(&plot_data)
                .map_err(|e| format!("KDE: serialize failed: {}", e))?;
            ctx.publish_plot(PlotChart::Kde, json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
