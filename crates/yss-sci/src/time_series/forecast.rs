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
fn normalized_series(y: &[f64], control: &Control) -> Result<(Vec<f64>, f64)> {
    let mut scale: f64 = 0.0;
    for (i, value) in y.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        scale = scale.max(value.abs());
    }
    if scale == 0.0 {
        scale = 1.0;
    }
    let mut response = Vec::with_capacity(y.len());
    for (i, value) in y.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        response.push(value / scale);
    }
    Ok((response, scale))
}
fn root_mean_square(values: impl Iterator<Item = f64>, control: &Control) -> Result<(f64, usize)> {
    let mut count = 0;
    let mut max: f64 = 0.0;
    let mut sum = 0.0;
    for (i, value) in values.enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let magnitude = finite(value)?.abs();
        if magnitude > max {
            // Rescale the accumulated squares when a larger residual arrives.
            sum = 1.0 + sum * (max / magnitude).powi(2);
            max = magnitude;
        } else if max > 0.0 {
            sum += (magnitude / max).powi(2);
        }
        count += 1;
    }
    if count == 0 {
        return Err(parameter());
    }
    Ok((finite(max * (sum / count as f64).sqrt())?, count))
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
    control: &Control,
) -> Result<ForecastResult> {
    let mut residuals = Vec::with_capacity(y.len());
    for (i, (value, fit)) in y.iter().zip(&fitted).enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        residuals.push(fit.map(|fit| value - fit));
    }
    let (rms, n) = root_mean_square(residuals.iter().flatten().copied(), control)?;
    let variance = finite(rms * rms)?;
    for (i, v) in fitted
        .iter()
        .flatten()
        .chain(&forecasts)
        .chain(residuals.iter().flatten())
        .chain(parameters.iter().map(|p| &p.estimate))
        .enumerate()
    {
        if i % 1024 == 0 {
            control.check()?;
        }
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
    let ll =
        finite(-0.5 * n * ((2.0 * std::f64::consts::PI).ln() + r.innovation_variance.ln() + 1.0))?;
    r.log_likelihood = Some(ll);
    r.aic = Some(finite(-2.0 * ll + 2.0 * parameters as f64)?);
    r.bic = Some(finite(-2.0 * ll + n.ln() * parameters as f64)?);
    Ok(())
}
