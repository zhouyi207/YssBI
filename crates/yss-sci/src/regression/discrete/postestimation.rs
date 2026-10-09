//! Binary-link postestimation. Effects are derivatives for numeric regressors;
//! the delta method differentiates the averaged effect, not observation-wise SEs.
use crate::regression::postestimation::evaluation::{
    EvaluationDesign, binary_link, delta_standard_error,
};
use statrs::distribution::{ContinuousCDF, Normal};
use yss_sci_contract::regression::{discrete::*, fit::*};
use yss_sci_contract::{SciError, SciOperationCode, execution::ScientificInputViolation};
fn invalid() -> SciError {
    SciError::InvalidInput {
        operation: SciOperationCode::Regression,
        violation: ScientificInputViolation::ParameterOutOfRange,
    }
}
fn link(fit: &RegressionFit) -> Result<BinaryRegressionLink, SciError> {
    match fit.statistics {
        RegressionStatistics::Binary { link, .. } => Ok(link),
        _ => Err(invalid()),
    }
}
fn validate(fit: &RegressionFit) -> Result<(), SciError> {
    link(fit)?;
    let k = fit.coefficients.len();
    let n = fit.fitted.len();
    if n == 0
        || k == 0
        || fit.parameter_names.len() != k
        || fit.design.len() != k
        || fit
            .design
            .iter()
            .any(|x| x.len() != n || x.iter().any(|v| !v.is_finite()))
        || fit.coefficients.iter().any(|v| !v.is_finite())
        || fit.statistics.coefficient_statistics().covariance.len() != k
        || fit
            .statistics
            .coefficient_statistics()
            .covariance
            .iter()
            .any(|r| r.len() != k || r.iter().any(|v| !v.is_finite()))
    {
        return Err(invalid());
    }
    let stats = fit.statistics.coefficient_statistics();
    if [
        stats.standard_errors.len(),
        stats.statistic_values.len(),
        stats.p_values.len(),
        stats.confidence_interval_lower.len(),
        stats.confidence_interval_upper.len(),
    ]
    .iter()
    .any(|n| *n != k)
        || stats
            .standard_errors
            .iter()
            .chain(&stats.statistic_values)
            .chain(&stats.p_values)
            .chain(&stats.confidence_interval_lower)
            .chain(&stats.confidence_interval_upper)
            .any(|v| !v.is_finite())
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn odds_ratios(fit: &RegressionFit) -> Result<Vec<EffectInference>, SciError> {
    validate(fit)?;
    if link(fit)? != BinaryRegressionLink::Logit {
        return Err(invalid());
    }
    let stats = fit.statistics.coefficient_statistics();
    fit.coefficients
        .iter()
        .enumerate()
        .map(|(j, b)| {
            let estimate = b.exp();
            let standard_error = estimate * stats.standard_errors[j];
            let ci_lower = stats.confidence_interval_lower[j].exp();
            let ci_upper = stats.confidence_interval_upper[j].exp();
            if [estimate, standard_error, ci_lower, ci_upper]
                .iter()
                .any(|v| !v.is_finite())
            {
                return Err(invalid());
            }
            // The null is OR=1; use beta's z, not (exp(beta)-1)/delta-SE.
            Ok(EffectInference {
                variable: if fit.constant && j == 0 {
                    "Baseline odds (_cons)".into()
                } else {
                    fit.parameter_names[j].clone()
                },
                estimate,
                standard_error,
                z_value: Some(stats.statistic_values[j]),
                p_value: Some(stats.p_values[j]),
                ci_lower,
                ci_upper,
            })
        })
        .collect()
}
pub fn classification(fit: &RegressionFit, cutoff: f64) -> Result<BinaryClassification, SciError> {
    link(fit)?;
    if !cutoff.is_finite()
        || !(0.0..=1.0).contains(&cutoff)
        || fit.fitted.is_empty()
        || fit.fitted.len() != fit.residuals.len()
    {
        return Err(invalid());
    }
    let (mut tp, mut fp, mut fn_, mut tn) = (0, 0, 0, 0);
    for (&p, &r) in fit.fitted.iter().zip(&fit.residuals) {
        let y = p + r;
        if !p.is_finite()
            || !(0.0..=1.0).contains(&p)
            || !y.is_finite()
            || (y.abs() > 1e-12 && (y - 1.0).abs() > 1e-12)
        {
            return Err(invalid());
        }
        match (p >= cutoff, y > 0.5) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => tn += 1,
        }
    }
    let ratio = |a: usize, b: usize| (b > 0).then(|| a as f64 / b as f64);
    let n = fit.fitted.len() as f64;
    Ok(BinaryClassification {
        cutoff,
        true_positive: tp,
        false_positive: fp,
        false_negative: fn_,
        true_negative: tn,
        sensitivity: ratio(tp, tp + fn_),
        specificity: ratio(tn, tn + fp),
        positive_predictive_value: ratio(tp, tp + fp),
        negative_predictive_value: ratio(tn, tn + fn_),
        accuracy: (tp + tn) as f64 / n,
        error_rate: (fp + fn_) as f64 / n,
    })
}
pub fn marginal_effects(
    fit: &RegressionFit,
    options: MarginalOptions,
    control: &yss_sci_contract::execution::ScientificExecutionControl,
) -> Result<MarginalEffects, yss_sci_contract::execution::ScientificComputationError> {
    use yss_sci_contract::execution::{ScientificComputationError, ScientificInputViolation};
    let invalid = || ScientificComputationError::InvalidInput {
        violation: ScientificInputViolation::ParameterOutOfRange,
    };
    control.check()?;
    validate(fit).map_err(|_| invalid())?;
    let link = link(fit).map_err(|_| invalid())?;
    let k = fit.coefficients.len();
    let normal = Normal::new(0.0, 1.0).map_err(|_| invalid())?;
    let grid = EvaluationDesign::new(
        &fit.design,
        &fit.parameter_names,
        fit.constant,
        options.evaluation,
        &options.at,
        control,
    )?;
    let rows = grid.rows;
    let mut values = vec![0.0; k];
    let mut gradients = vec![vec![0.0; k]; k];
    for i in 0..rows {
        control.check()?;
        let x: Vec<f64> = (0..k).map(|j| grid.value(i, j)).collect();
        let eta: f64 = x.iter().zip(&fit.coefficients).map(|(x, b)| x * b).sum();
        if !eta.is_finite() {
            return Err(invalid());
        }
        let (p, d, dd, logit_q) = binary_link(link, eta, &normal);
        let log_y = matches!(options.method, MarginalMethod::Eyex | MarginalMethod::Eydx);
        if log_y && p <= 0.0 && link == BinaryRegressionLink::Probit {
            return Err(invalid());
        }
        let (h, dh) = if log_y && link == BinaryRegressionLink::Logit {
            (logit_q, -d)
        } else if log_y {
            (d / p, dd / p - (d / p) * (d / p))
        } else {
            (d, dd)
        };
        for j in usize::from(fit.constant)..k {
            let multiplier =
                if matches!(options.method, MarginalMethod::Eyex | MarginalMethod::Dyex) {
                    x[j]
                } else {
                    1.0
                };
            values[j] += fit.coefficients[j] * h * multiplier / rows as f64;
            for (l, &xl) in x.iter().enumerate() {
                gradients[j][l] += (f64::from(l == j) * h + fit.coefficients[j] * dh * xl)
                    * multiplier
                    / rows as f64;
            }
        }
    }
    let cov = &fit.statistics.coefficient_statistics().covariance;
    let critical = normal.inverse_cdf(0.975);
    let mut coefficients = Vec::new();
    for j in usize::from(fit.constant)..k {
        control.check()?;
        let se = delta_standard_error(&gradients[j], cov)?;
        if !values[j].is_finite() {
            return Err(invalid());
        }
        let z = (se > 0.0).then(|| values[j] / se);
        if z.is_some_and(|value| !value.is_finite())
            || !(values[j] - critical * se).is_finite()
            || !(values[j] + critical * se).is_finite()
        {
            return Err(invalid());
        }
        coefficients.push(EffectInference {
            variable: fit.parameter_names[j].clone(),
            estimate: values[j],
            standard_error: se,
            z_value: z,
            p_value: z.map(crate::distribution::normal_two_sided_p),
            ci_lower: values[j] - critical * se,
            ci_upper: values[j] + critical * se,
        });
    }
    Ok(MarginalEffects {
        evaluation: options.evaluation,
        method: options.method,
        at: options.at,
        coefficients,
    })
}
