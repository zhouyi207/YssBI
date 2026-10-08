use super::*;
use crate::plots::cartesian::{CartesianKind, CartesianOptions};
use yss_application::{
    chart::{ChartPlotResult, PlotAxisFormat, PlotPoint},
    graph::results::{
        plot::CorrelogramPoint,
        report::{HypothesisTestOutput, ResultAnalysisProjection, SerialTestsOutput},
    },
};
use yss_node_kernel::RuntimeValue;

pub(super) enum AnalysisData {
    Diagnostics {
        tests: Vec<(String, Result<Arc<RuntimeValue>, String>)>,
        density: Option<Arc<CartesianData>>,
        reason: Option<String>,
    },
    Acf {
        acf: Arc<CorrelogramData>,
        pacf: Arc<CorrelogramData>,
        observations: usize,
    },
    Serial(SerialTestsOutput),
    Hypothesis(HypothesisTestOutput),
}

pub(super) fn prepare(result: ResultAnalysisProjection) -> anyhow::Result<AnalysisData> {
    Ok(match result {
        ResultAnalysisProjection::Diagnostics(value) => {
            let tests = value
                .tests
                .into_iter()
                .map(|test| {
                    let content = match test.unavailable_reason {
                        Some(reason) => Err(reason),
                        None => Ok(Arc::new(RuntimeValue::try_from(
                            test.value.unwrap_or_default(),
                        )?)),
                    };
                    Ok((test.name, content))
                })
                .collect::<anyhow::Result<_>>()?;
            let density = if value.leverage_unavailable_reason.is_none() {
                anyhow::ensure!(
                    !value.leverage_density.is_empty()
                        && value
                            .leverage_density
                            .iter()
                            .all(|p| p.x.is_finite() && p.y.is_finite()),
                    "invalid leverage density"
                );
                Some(Arc::new(CartesianData::new(
                    series(
                        value
                            .leverage_density
                            .into_iter()
                            .map(|p| PlotPoint { x: p.x, y: p.y })
                            .collect(),
                    ),
                    CartesianOptions {
                        x_min: Some(0.),
                        ..CartesianOptions::new(CartesianKind::Density)
                    },
                )))
            } else {
                None
            };
            AnalysisData::Diagnostics {
                tests,
                density,
                reason: value.leverage_unavailable_reason,
            }
        }
        ResultAnalysisProjection::AcfPacf(value) => {
            anyhow::ensure!(
                value.ci_half_width.is_finite()
                    && value.ci_half_width >= 0.
                    && !value.acf.is_empty()
                    && !value.pacf.is_empty()
                    && value.acf.iter().chain(&value.pacf).all(|v| v.is_finite()),
                "invalid correlogram"
            );
            let bars = |values: Vec<f64>, start| {
                values
                    .into_iter()
                    .enumerate()
                    .map(|(i, value)| CorrelogramPoint {
                        lag: i + start,
                        value,
                        q_stat: None,
                        p_value: None,
                    })
                    .collect()
            };
            AnalysisData::Acf {
                acf: Arc::new(CorrelogramData::new(
                    bars(value.acf, 0),
                    value.ci_half_width,
                )),
                pacf: Arc::new(CorrelogramData::new(
                    bars(value.pacf, 1),
                    value.ci_half_width,
                )),
                observations: value.n,
            }
        }
        ResultAnalysisProjection::SerialTests(value) => AnalysisData::Serial(value),
        ResultAnalysisProjection::Hypothesis(value) => AnalysisData::Hypothesis(value),
        ResultAnalysisProjection::ResidualPlot(_) => {
            anyhow::bail!("residuals have their own query view")
        }
    })
}

pub(in crate::results::report) fn series(data: Vec<PlotPoint>) -> ChartPlotResult {
    ChartPlotResult {
        data,
        x_label: None,
        y_label: None,
        x_format: PlotAxisFormat::Number,
        y_format: PlotAxisFormat::Number,
    }
}
