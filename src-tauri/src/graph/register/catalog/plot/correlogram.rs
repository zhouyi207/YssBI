//! Correlogram 节点：接收一个 DataSeries，计算 ACF & PACF 及累积 Ljung-Box Q 检验，打开 Plot 窗口

use crate::execution::{ExecutionEffect, PlotChart};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataType, DataValue};
use crate::sci::api::time_series::acf_pacf::{AcfPacfInput, compute_acf_pacf};
use crate::sci::engine::SciContext;
use serde::Serialize;
use statrs::distribution::{ChiSquared, ContinuousCDF};
use std::sync::Arc;

#[derive(Serialize)]
struct CorrelogramPlotData {
    acf: Vec<CorrelogramDatum>,
    pacf: Vec<CorrelogramDatum>,
    ci_half_width: f64,
    n: usize,
}

#[derive(Serialize)]
struct CorrelogramDatum {
    lag: usize,
    value: f64,
    /// Ljung-Box Q statistic cumulative up to this lag
    q_stat: f64,
    /// p-value of the Q statistic (χ²(lag) distribution)
    p_value: f64,
}

const DEFAULT_MAX_LAG: usize = 20;

/// Cumulative Ljung-Box Q: Q(h) = n(n+2) Σ_{k=1}^{h} ρ̂²_k / (n-k), p from χ²(h)
fn cumulative_ljung_box(acf_vals: &[f64], n: usize) -> Vec<(f64, f64)> {
    let nf = n as f64;
    let mut cum = 0.0;
    let mut result = Vec::with_capacity(acf_vals.len());
    for (k, &rho_k) in acf_vals.iter().enumerate() {
        let lag = k + 1;
        cum += rho_k.powi(2) / (n - lag).max(1) as f64;
        let q = nf * (nf + 2.0) * cum;
        let p = ChiSquared::new(lag as f64)
            .map(|d| 1.0 - d.cdf(q))
            .unwrap_or(f64::NAN);
        result.push((q, p));
    }
    result
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Correlogram (ACF & PACF)", vec!["Plot".to_string()])
        .with_ui_style("plot")
        .with_documentation(docs::plot::CORRELOGRAM_ZH, docs::plot::CORRELOGRAM_EN)
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::data_input(
                "DataSeries",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "Lags",
                    DataRole::Inputs(1),
                    PinDataTypeDefinition::concrete(DataType::Int64),
                )
                .with_default_value(DataValue::Int64(DEFAULT_MAX_LAG as i64)),
            ),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let input = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;
            let series_id = match &input {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("Correlogram: input must be a DataSeries".to_string()),
            };

            let lag_input = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(1)))?;
            let user_lags: usize = match lag_input {
                DataValue::Int64(v) if v > 0 => v as usize,
                DataValue::Float64(v) if v > 0.0 => v as usize,
                _ => DEFAULT_MAX_LAG,
            };

            let series = ctx.get_data_series(&series_id)?;
            let cast = series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("Correlogram: cannot cast to Float64: {}", e))?;
            let ca = cast.f64().map_err(|e| format!("Correlogram: {}", e))?;

            let values: Vec<f64> = ca.into_no_null_iter().collect();
            let n = values.len();
            if n < 4 {
                return Err("Correlogram: need at least 4 observations".to_string());
            }

            let acf_pacf = compute_acf_pacf(
                &SciContext::rust(),
                AcfPacfInput {
                    residuals: values,
                    max_lag: user_lags,
                },
            )
            .map_err(|e| format!("Correlogram: {}", e))?;
            let acf_vals = acf_pacf.acf;
            let pacf_vals = acf_pacf.pacf;
            let ci_half_width = 1.96 / (n as f64).sqrt();

            // acf_vals[0] = 1.0 (lag 0), skip it; acf_vals[1..] = lag 1..=max_lag
            let acf_no_lag0 = &acf_vals[1..];
            let q_stats = cumulative_ljung_box(acf_no_lag0, n);

            let acf_data: Vec<CorrelogramDatum> = acf_no_lag0
                .iter()
                .enumerate()
                .map(|(i, &value)| {
                    let (q_stat, p_value) = q_stats[i];
                    CorrelogramDatum {
                        lag: i + 1,
                        value,
                        q_stat,
                        p_value,
                    }
                })
                .collect();

            let pacf_data: Vec<CorrelogramDatum> = pacf_vals
                .iter()
                .enumerate()
                .map(|(i, &value)| {
                    let (q_stat, p_value) = q_stats[i];
                    CorrelogramDatum {
                        lag: i + 1,
                        value,
                        q_stat,
                        p_value,
                    }
                })
                .collect();

            let plot_data = CorrelogramPlotData {
                acf: acf_data,
                pacf: pacf_data,
                ci_half_width,
                n,
            };

            let json = serde_json::to_string(&plot_data)
                .map_err(|e| format!("Correlogram: serialize failed: {}", e))?;
            ctx.publish_plot(PlotChart::Correlogram, json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
