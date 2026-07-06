//! Histogram 节点：接收一个数值 DataSeries，计算直方图分箱，打开 Plot 窗口绘制直方图

use crate::execution::{ExecutionEffect, PlotChart};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataType, DataValue};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct HistogramPlotData {
    data: Vec<HistogramBin>,
    x_label: Option<String>,
    y_label: Option<String>,
}

#[derive(Serialize)]
struct HistogramBin {
    label: String,
    count: u32,
}

/// Sturges 规则估算分箱数: k = ceil(log2(n) + 1)
fn sturges_bins(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let k = (n as f64).log2().ceil() as usize + 1;
    k.max(1).min(100)
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Histogram", vec!["Plot".to_string()])
        .with_ui_style("plot")
                .with_documentation(docs::plot::HISTOGRAM_ZH, docs::plot::HISTOGRAM_EN)
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::data_input(
                "Values",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                    vec![
                        DataType::Float64,
                        DataType::Int64,
                    ],
                )))),
            )),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;

            let id = match &value {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("Histogram: input must be a numeric DataSeries".to_string()),
            };

            let series = ctx.get_data_series(&id)?;
            let cast = series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("Histogram: cannot cast to Float64: {}", e))?;
            let f64_chunk = cast
                .f64()
                .map_err(|e| format!("Histogram: not numeric: {}", e))?;

            let values: Vec<f64> = f64_chunk
                .into_iter()
                .filter_map(|v| v)
                .filter(|v| v.is_finite())
                .collect();

            if values.is_empty() {
                return Err("Histogram: need at least 1 valid value".to_string());
            }

            let n = values.len();
            let num_bins = sturges_bins(n);
            let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let range = max_val - min_val;

            // 避免除零，若所有值相同则用一个小范围
            let bin_width = if range > 0.0 {
                range / num_bins as f64
            } else {
                1.0
            };

            let mut counts: Vec<u32> = vec![0; num_bins];

            for &v in &values {
                let idx = if v <= min_val {
                    0
                } else if v >= max_val {
                    num_bins - 1
                } else {
                    let i = ((v - min_val) / bin_width).floor() as usize;
                    i.min(num_bins - 1)
                };
                counts[idx] += 1;
            }

            let data: Vec<HistogramBin> = (0..num_bins)
                .map(|i| {
                    let lo = min_val + i as f64 * bin_width;
                    let hi = min_val + (i + 1) as f64 * bin_width;
                    let label = format!("[{:.2}, {:.2})", lo, hi);
                    HistogramBin {
                        label,
                        count: counts[i],
                    }
                })
                .collect();

            let x_label = series.name().to_string();

            let plot_data = HistogramPlotData {
                data,
                x_label: if x_label.is_empty() {
                    None
                } else {
                    Some(x_label)
                },
                y_label: Some("Frequency".to_string()),
            };

            let json = serde_json::to_string(&plot_data)
                .map_err(|e| format!("Histogram: serialize failed: {}", e))?;
            ctx.publish_plot(PlotChart::Histogram, json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
