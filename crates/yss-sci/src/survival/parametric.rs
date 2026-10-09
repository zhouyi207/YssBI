use super::common::*;
use yss_sci_contract::survival::*;

fn softplus(z: f64) -> f64 {
    if z > 0.0 {
        z + (-z).exp().ln_1p()
    } else {
        z.exp().ln_1p()
    }
}
fn log_survival(z: f64, distribution: AftDistribution) -> f64 {
    match distribution {
        AftDistribution::Exponential | AftDistribution::Weibull => -z.exp(),
        AftDistribution::Lognormal => normal_log_cdf(-z),
        AftDistribution::Loglogistic => -softplus(z),
    }
}
pub fn fit(
    time: &[f64],
    event: &[f64],
    predictors: &[Vec<f64>],
    options: AftOptions,
    control: &Control,
) -> Result<AftResult> {
    data(time, event, predictors, control)?;
    positive_horizon(options.horizon)?;
    check_iteration(options.iteration)?;
    let events = event.iter().filter(|&&v| v == 1.0).count();
    if events == 0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let n = time.len();
    let design = Design::new(predictors, n, true, true, true, control)?;
    let p = design.x.ncols();
    let estimated_scale = options.distribution != AftDistribution::Exponential;
    let log_time = time.iter().map(|t| t.ln()).collect::<Vec<_>>();
    let (mut initial, _) = least_squares(&design.x, &log_time, None, control)?;
    if estimated_scale {
        let predicted = fitted(&design.x, &initial);
        let scale = (predicted
            .iter()
            .zip(&log_time)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        initial.push(scale.max(0.1).ln());
    }
    let objective = |theta: &[f64]| -> Result<f64> {
        control.check()?;
        let sigma = if estimated_scale { theta[p].exp() } else { 1.0 };
        if !sigma.is_finite() || sigma <= 0.0 {
            return Ok(f64::MAX / 1e10);
        }
        let eta = fitted(&design.x, &theta[..p]);
        let mut ll = 0.0;
        for i in 0..n {
            if i.is_multiple_of(256) {
                control.check()?;
            }
            let z = (log_time[i] - eta[i]) / sigma;
            ll += if event[i] == 0.0 {
                log_survival(z, options.distribution)
            } else {
                -sigma.ln() - log_time[i]
                    + match options.distribution {
                        AftDistribution::Exponential | AftDistribution::Weibull => z - z.exp(),
                        AftDistribution::Lognormal => {
                            -0.5 * z * z - 0.5 * (2.0 * std::f64::consts::PI).ln()
                        }
                        AftDistribution::Loglogistic => z - 2.0 * softplus(z),
                    }
            };
        }
        let value = -ll / n as f64;
        Ok(if value.is_finite() {
            value
        } else {
            f64::MAX / 1e10
        })
    };
    let minimum = minimize(&objective, initial, options.iteration, control)?;
    let theta = &minimum.beta;
    let sigma = if estimated_scale {
        finite(theta[p].exp())?
    } else {
        1.0
    };
    let q = theta.len();
    let h = hessian(&objective, theta, control)?;
    let standardized = inverse(&Mat::from_fn(q, q, |j, k| h[(j, k)] * n as f64))?;
    let raw_jacobian = design.raw_jacobian(1.0);
    let jacobian = Mat::from_fn(q, q, |j, k| {
        if j < p && k < p {
            raw_jacobian[(j, k)]
        } else if j == k {
            1.0
        } else {
            0.0
        }
    });
    let covariance = jacobian.as_ref() * standardized.as_ref() * jacobian.transpose();
    let (raw, _) = design.raw(&theta[..p], None);
    let coefficient_cov = Mat::from_fn(p, p, |j, k| covariance[(j, k)]);
    let coefficients = coefficient_table(&raw, names(p - 1, true), Some(&coefficient_cov), None)?;
    let eta = fitted(&design.x, &theta[..p]);
    let median_survival = eta
        .iter()
        .map(|&v| {
            let offset = match options.distribution {
                AftDistribution::Exponential | AftDistribution::Weibull => {
                    sigma * std::f64::consts::LN_2.ln()
                }
                _ => 0.0,
            };
            finite((v + offset).exp())
        })
        .collect::<Result<Vec<_>>>()?;
    let event_probabilities = eta
        .iter()
        .map(|&v| {
            finite(-log_survival((options.horizon.ln() - v) / sigma, options.distribution).exp_m1())
        })
        .collect::<Result<Vec<_>>>()?;
    let ll = finite(-minimum.value * n as f64)?;
    Ok(AftResult {
        distribution: options.distribution,
        observations: n,
        events,
        iterations: minimum.iterations,
        time_ratios: ratios(&coefficients)?,
        coefficients,
        covariance: rows(&covariance),
        scale: sigma,
        scale_standard_error: if estimated_scale {
            Some(finite(sigma * covariance[(p, p)].sqrt())?)
        } else {
            None
        },
        log_likelihood: ll,
        aic: finite(-2.0 * ll + 2.0 * q as f64)?,
        bic: finite(-2.0 * ll + (n as f64).ln() * q as f64)?,
        median_survival,
        horizon: options.horizon,
        event_probabilities,
    })
}
