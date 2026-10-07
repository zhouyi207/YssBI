//! OLS/WLS observation diagnostics using the fitted design and precision weights.
use crate::regression::models::common::{Result, failed, finite, parameter, validate};
use yss_sci_contract::{
    diagnostics::model::{InfluenceOutput, InfluenceSummary},
    execution::ScientificExecutionControl as Control,
    regression::linear::LinearRegressionResult,
};
use yss_sci_linalg::Mat;

pub(super) fn validate_linear(model: &LinearRegressionResult, control: &Control) -> Result<()> {
    validate(&model.residuals, &model.design, control)?;
    let n = model.residuals.len();
    let k = model.design.len();
    if k == 0
        || n <= k
        || model.coefficients.len() != k
        || model.fitted.len() != n
        || model.fitted.iter().any(|v| !v.is_finite())
        || !matches!(
            model.report.model_basic_info.model_type.as_str(),
            "OLS" | "WLS"
        )
        || (model.report.model_basic_info.model_type == "WLS") != model.weights.is_some()
        || model
            .weights
            .as_ref()
            .is_some_and(|w| w.len() != n || w.iter().any(|v| !v.is_finite() || *v <= 0.0))
    {
        return Err(parameter());
    }
    Ok(())
}

pub fn influence(model: &LinearRegressionResult, control: &Control) -> Result<InfluenceOutput> {
    validate_linear(model, control)?;
    let n = model.residuals.len();
    let k = model.design.len();
    let weighted = model
        .residuals
        .iter()
        .enumerate()
        .map(|(i, u)| finite(u * model.weights.as_ref().map_or(1.0, |w| w[i].sqrt())))
        .collect::<Result<Vec<_>>>()?;
    let ss = finite(weighted.iter().map(|v| v * v).sum())?;
    let variance = ss / (n - k) as f64;
    // Column scaling preserves the hat matrix while avoiding units-driven conditioning.
    let scales = model
        .design
        .iter()
        .map(|c| c.iter().map(|v| v.abs()).fold(0.0, f64::max))
        .collect::<Vec<_>>();
    if scales.contains(&0.0) {
        return Err(parameter());
    }
    let x = Mat::from_fn(n, k, |i, j| {
        model.design[j][i] / scales[j] * model.weights.as_ref().map_or(1.0, |w| w[i].sqrt())
    });
    control.check()?;
    let mut leverage = super::leverage::leverage(&x).map_err(|_| failed())?;
    control.check()?;
    let mut standardized_residuals = Vec::with_capacity(n);
    let mut studentized_residuals = Vec::with_capacity(n);
    let mut cooks_distance = Vec::with_capacity(n);
    for i in 0..n {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let h = finite(leverage[i])?;
        if !(-1e-8..=1.0 + 1e-8).contains(&h) {
            return Err(failed());
        }
        leverage[i] = h.clamp(0.0, 1.0);
        let remaining = 1.0 - leverage[i];
        if variance <= 0.0 || remaining <= f64::EPSILON * n as f64 {
            standardized_residuals.push(None);
            studentized_residuals.push(None);
            cooks_distance.push(None);
            continue;
        }
        let r = finite(weighted[i] / (variance * remaining).sqrt())?;
        standardized_residuals.push(Some(r));
        cooks_distance.push(Some(finite(r * r / k as f64 * leverage[i] / remaining)?));
        let deleted_ss = ss - weighted[i] * weighted[i] / remaining;
        studentized_residuals.push(if n > k + 1 && deleted_ss > 0.0 {
            Some(finite(
                weighted[i] / (deleted_ss / (n - k - 1) as f64 * remaining).sqrt(),
            )?)
        } else {
            None
        });
    }
    let mean = finite(model.residuals.iter().map(|u| u / n as f64).sum())?;
    let sd = finite(
        (model
            .residuals
            .iter()
            .map(|u| (u - mean).powi(2))
            .sum::<f64>()
            / (n - 1) as f64)
            .sqrt(),
    )?;
    Ok(InfluenceOutput {
        summary: InfluenceSummary {
            observations: n,
            parameters: k,
            residual_degrees_of_freedom: n - k,
            residual_mean: mean,
            residual_sd: sd,
            residual_minimum: model
                .residuals
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min),
            residual_maximum: model
                .residuals
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
            weighted_residual_sum_squares: ss,
            residual_standard_error: variance.sqrt(),
            maximum_leverage: leverage.iter().copied().fold(0.0, f64::max),
            maximum_cooks_distance: cooks_distance.iter().flatten().copied().reduce(f64::max),
        },
        leverage,
        standardized_residuals,
        studentized_residuals,
        cooks_distance,
    })
}
