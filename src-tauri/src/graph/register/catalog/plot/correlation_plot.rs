//! Correlation Plot 节点：接收多个数值 DataSeries，计算相关系数矩阵和 p 值，打开 Plot 窗口绘制热力图

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use serde::Serialize;
use statrs::distribution::{ContinuousCDF, StudentsT};
use std::sync::Arc;

#[derive(Serialize)]
struct CorrelationPlotData {
    labels: Vec<String>,
    matrix: Vec<Vec<f64>>,
    p_matrix: Vec<Vec<f64>>,
}

/// 计算 Pearson 相关系数
fn pearson_corr_manual(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return f64::NAN;
    }
    let mean_a: f64 = a.iter().take(n).sum::<f64>() / n as f64;
    let mean_b: f64 = b.iter().take(n).sum::<f64>() / n as f64;
    let mut var_a = 0.0_f64;
    let mut var_b = 0.0_f64;
    let mut cov = 0.0_f64;
    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }
    let std_a = var_a.sqrt();
    let std_b = var_b.sqrt();
    if std_a <= 0.0 || std_b <= 0.0 {
        return f64::NAN;
    }
    cov / (std_a * std_b)
}

/// 计算 Pearson 相关系数的双侧 p 值
fn pearson_p_value(r: f64, n: usize) -> f64 {
    if n < 3 {
        return f64::NAN;
    }
    let r_abs = r.abs();
    if r_abs >= 1.0 || !r_abs.is_finite() {
        return if r_abs >= 1.0 { 0.0 } else { f64::NAN };
    }
    let df = (n - 2) as f64;
    let t = r * (df / (1.0 - r * r)).sqrt();
    let t_abs = t.abs();
    if !t_abs.is_finite() {
        return f64::NAN;
    }
    let dist = match StudentsT::new(0.0, 1.0, df) {
        Ok(d) => d,
        Err(_) => return f64::NAN,
    };
    let p = 2.0 * (1.0 - dist.cdf(t_abs));
    if p.is_finite() { p } else { f64::NAN }
}

fn numeric_dataseries_type() -> PinDataTypeDefinition {
    PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Float64,
        DataType::Int64,
        DataType::Int32,
        DataType::Float32,
    ]))))
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Correlation Plot", vec!["Plot".to_string()])
        .with_ui_style("plot")
        .with_localized_description("由数值 DataSeries 绘制相关热力图（可用 + 添加更多序列）", "Plot correlation heatmap from numeric DataSeries (use + to add more)")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::repeatable(
                PinDefinition::data_input("", DataRole::Inputs(0), numeric_dataseries_type()),
                "DataSeries",
                2,
                None,
            ),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let all_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
            let series_values: Vec<_> = all_values
                .into_iter()
                .filter_map(|v| match &v {
                    DataValue::DataSeries(s) => Some(s.id.clone()),
                    _ => None,
                })
                .collect();

            if series_values.len() < 2 {
                return Err("Correlation Plot: at least 2 connected DataSeries required (use + to add more)".to_string());
            }

            let mut labels: Vec<String> = Vec::with_capacity(series_values.len());
            let mut col_vecs: Vec<Vec<Option<f64>>> = Vec::with_capacity(series_values.len());

            for (idx, id) in series_values.iter().enumerate() {
                let series = ctx.get_data_series(id)?;
                let label = series.name().to_string();
                labels.push(if label.is_empty() {
                    format!("Series {}", idx + 1)
                } else {
                    label
                });

                let cast = series
                    .cast(&polars::prelude::DataType::Float64)
                    .map_err(|e| format!("Correlation Plot: cannot cast series {} to Float64: {}", labels.last().unwrap(), e))?;
                let f64_chunk = cast.f64().map_err(|e| format!("Correlation Plot: series is not numeric: {}", e))?;
                let vec: Vec<Option<f64>> = f64_chunk
                    .into_iter()
                    .map(|v| v.filter(|x| x.is_finite()))
                    .collect();
                col_vecs.push(vec);
            }

            let n = labels.len();
            let nrows = col_vecs.first().map(|v| v.len()).unwrap_or(0);
            if nrows < 2 {
                return Err("Correlation Plot: need at least 2 valid rows for correlation".to_string());
            }

            let mut matrix = vec![vec![0.0_f64; n]; n];
            let mut p_matrix = vec![vec![f64::NAN; n]; n];
            for i in 0..n {
                for j in 0..n {
                    let (a, b): (Vec<f64>, Vec<f64>) = (0..nrows)
                        .filter_map(|r| {
                            let va = col_vecs[i].get(r).copied().flatten();
                            let vb = col_vecs[j].get(r).copied().flatten();
                            match (va, vb) {
                                (Some(x), Some(y)) => Some((x, y)),
                                _ => None,
                            }
                        })
                        .unzip();
                    let val = pearson_corr_manual(&a, &b);
                    let sample_n = a.len();
                    matrix[i][j] = if val.is_finite() { val } else { 0.0 };
                    p_matrix[i][j] = pearson_p_value(val, sample_n);
                }
            }

            let plot_data = CorrelationPlotData {
                labels,
                matrix,
                p_matrix,
            };

            let json = serde_json::to_string(&plot_data)
                .map_err(|e| format!("Correlation Plot: serialize failed: {}", e))?;
            ctx.open_window("correlation".to_string(), json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
