//! KDE 节点：接收一个数值 DataSeries，计算核密度估计，打开 Plot 窗口绘制 KDE 曲线

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

/// 高斯核 K(u) = (1/sqrt(2*pi)) * exp(-u^2/2)
#[inline]
fn gaussian_kernel(u: f64) -> f64 {
    const INV_SQRT_2PI: f64 = 0.3989422804014327;
    INV_SQRT_2PI * (-0.5 * u * u).exp()
}

/// Silverman 带宽: h = 1.06 * sigma * n^(-1/5)
fn silverman_bandwidth(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    if n < 2.0 {
        return 1.0;
    }
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let sigma = variance.sqrt();
    if sigma <= 0.0 || !sigma.is_finite() {
        return 1.0;
    }
    1.06 * sigma * n.powf(-0.2)
}

/// 在 x 处计算 KDE: f(x) = (1/(n*h)) * sum K((x - xi)/h)
fn kde_at(x: f64, values: &[f64], h: f64) -> f64 {
    if values.is_empty() || h <= 0.0 {
        return 0.0;
    }
    let n = values.len() as f64;
    let sum: f64 = values.iter().map(|&xi| gaussian_kernel((x - xi) / h)).sum();
    sum / (n * h)
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("KDE", vec!["Plot".to_string()])
        .with_ui_style("plot")
        .with_localized_description(
            "对数值 DataSeries 绘制核密度估计图",
            "Plot kernel density estimation from a numeric DataSeries",
        )
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::data_input(
                "Values",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                    vec![
                        DataType::Float64,
                        DataType::Int64,
                        DataType::Int32,
                        DataType::Float32,
                    ],
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

            let h = silverman_bandwidth(&values);
            let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let range = max_val - min_val;
            let pad = (range * 0.15).max(h * 2.0).max(0.1);
            let x_min = min_val - pad;
            let x_max = max_val + pad;

            const GRID_POINTS: usize = 256;
            let data: Vec<KdePoint> = (0..=GRID_POINTS)
                .map(|i| {
                    let t = i as f64 / GRID_POINTS as f64;
                    let x = x_min + t * (x_max - x_min);
                    let y = kde_at(x, &values, h);
                    KdePoint { x, y }
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
            ctx.open_window("kde".to_string(), json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
