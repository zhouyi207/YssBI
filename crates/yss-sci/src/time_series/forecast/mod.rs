//! Conditional univariate estimation, forecasting and stationarity diagnostics.
mod arima;
mod diagnostics;
mod elementary;
mod smoothing;
mod volatility;
pub use arima::arima;
pub use diagnostics::{kpss, phillips_perron};
pub use elementary::{ecm, grey_prediction, markov_prediction};
pub use smoothing::exponential_smoothing;
pub use volatility::volatility;

use crate::regression::models::common::*;
use yss_sci_contract::execution::ScientificExecutionControl as Control;
use yss_sci_contract::time_series::forecast::*;

fn estimate(term: impl Into<String>, estimate: f64) -> ParameterEstimate {
    ParameterEstimate {
        term: term.into(),
        estimate,
    }
}
fn mean(y: &[f64]) -> f64 {
    y.iter().map(|v| v / y.len() as f64).sum()
}
fn scale(y: &[f64]) -> Result<f64> {
    let m = mean(y);
    let max = y.iter().map(|v| (v - m).abs()).fold(0.0, f64::max);
    if max == 0.0 || !max.is_finite() {
        return Err(parameter());
    }
    let s = finite(
        max * (y.iter().map(|v| ((v - m) / max).powi(2)).sum::<f64>() / y.len() as f64).sqrt(),
    )?;
    if s > 0.0 { Ok(s) } else { Err(parameter()) }
}
fn horizon(n: usize, h: usize) -> Result<()> {
    if h == 0 || n.checked_add(h).is_none() {
        return Err(parameter());
    }
    Ok(())
}
fn report(
    method: &str,
    y: &[f64],
    fitted: Vec<Option<f64>>,
    forecasts: Vec<f64>,
    parameters: Vec<ParameterEstimate>,
    iterations: usize,
) -> Result<ForecastResult> {
    let residuals: Vec<_> = y
        .iter()
        .zip(&fitted)
        .map(|(v, f)| f.map(|f| v - f))
        .collect();
    let n = residuals.iter().flatten().count();
    if n == 0 {
        return Err(parameter());
    }
    let variance = finite(residuals.iter().flatten().map(|v| v * v).sum::<f64>() / n as f64)?;
    for v in fitted
        .iter()
        .flatten()
        .chain(&forecasts)
        .chain(residuals.iter().flatten())
        .chain(parameters.iter().map(|p| &p.estimate))
    {
        finite(*v)?;
    }
    Ok(ForecastResult {
        method: method.into(),
        observations: y.len(),
        effective_observations: n,
        parameters,
        fitted,
        residuals,
        forecasts,
        lower: None,
        upper: None,
        innovation_variance: variance,
        log_likelihood: None,
        aic: None,
        bic: None,
        iterations,
    })
}
fn likelihood(r: &mut ForecastResult, parameters: usize) -> Result<()> {
    if r.innovation_variance <= 0.0 {
        return Err(parameter());
    }
    let n = r.effective_observations as f64;
    let ll = finite(-0.5 * n * ((2.0 * std::f64::consts::PI * r.innovation_variance).ln() + 1.0))?;
    r.log_likelihood = Some(ll);
    r.aic = Some(finite(-2.0 * ll + 2.0 * parameters as f64)?);
    r.bic = Some(finite(-2.0 * ll + n.ln() * parameters as f64)?);
    Ok(())
}
