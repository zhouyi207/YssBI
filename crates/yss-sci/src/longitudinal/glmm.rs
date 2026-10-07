use super::*;
use statrs::function::gamma::ln_gamma;
use yss_sci_contract::execution::ScientificComputationError as Error;
use yss_sci_contract::regression::models::IterationOptions;

fn softplus(x: f64) -> f64 {
    x.max(0.0) + (-x.abs()).exp().ln_1p()
}

fn observation(y: f64, eta: f64, family: ResponseFamily, alpha: f64) -> Result<(f64, f64, f64)> {
    let (mu, _, variance) = mean_variance(eta, family, alpha)?;
    let (nll, score, curvature) = match family {
        ResponseFamily::Binomial => (softplus(eta) - y * eta, y - mu, variance),
        ResponseFamily::Poisson => (mu - y * eta + ln_gamma(y + 1.0), y - mu, mu),
        ResponseFamily::NegativeBinomial => {
            let size = 1.0 / alpha;
            let log_ratio = softplus(eta - size.ln());
            let ll = ln_gamma(y + size) - ln_gamma(size) - ln_gamma(y + 1.0) - size * log_ratio
                + y * (eta - size.ln() - log_ratio);
            (
                -ll,
                (y - mu) / (1.0 + alpha * mu),
                mu * (1.0 + alpha * y) / (1.0 + alpha * mu).powi(2),
            )
        }
        ResponseFamily::Gaussian => return Err(parameter()),
    };
    Ok((finite(nll)?, finite(score)?, finite(curvature)?))
}

struct Laplace {
    nll: f64,
    modes: Vec<f64>,
    fitted: Vec<f64>,
}

fn laplace(
    y: &[f64],
    x: &Mat<f64>,
    clusters: &[Vec<usize>],
    parameters: &[f64],
    family: ResponseFamily,
    control: &Control,
) -> Result<Laplace> {
    control.check()?;
    let p = x.ncols();
    let sd = parameters[p];
    let alpha = if family == ResponseFamily::NegativeBinomial {
        parameters[p + 1].exp()
    } else {
        0.0
    };
    if !sd.is_finite()
        || (family == ResponseFamily::NegativeBinomial && (!alpha.is_finite() || alpha <= 0.0))
    {
        return Err(failed());
    }
    let eta = fitted(x, &parameters[..p]);
    let mut fitted_values = vec![0.0; y.len()];
    let mut modes = Vec::with_capacity(clusters.len());
    let mut nll = 0.0;
    for rows in clusters {
        control.check()?;
        // Standard-normal modes remain defined when the variance component is zero.
        let conditional = |u: f64| -> Result<(f64, f64, f64)> {
            let mut value = u * u / 2.0;
            let mut score = -u;
            let mut curvature = 1.0;
            for &i in rows {
                control.check()?;
                let (v, s, c) = observation(y[i], eta[i] + sd * u, family, alpha)?;
                value += v;
                score += sd * s;
                curvature += sd * sd * c;
            }
            Ok((finite(value)?, finite(score)?, finite(curvature)?))
        };
        let mut u = 0.0;
        let mut converged = false;
        for _ in 0..100 {
            let (value, score, curvature) = conditional(u)?;
            let step = score / curvature;
            if step.abs() <= 1e-10 * (1.0 + u.abs()) {
                converged = true;
                break;
            }
            let mut scale = 1.0;
            let mut accepted = false;
            for _ in 0..40 {
                match conditional(u + scale * step) {
                    Ok((next, _, _)) if next <= value + 1e-12 * (1.0 + value.abs()) => {
                        u += scale * step;
                        accepted = true;
                        break;
                    }
                    Err(e @ (Error::Cancelled | Error::DeadlineExceeded)) => return Err(e),
                    _ => scale *= 0.5,
                }
            }
            if !accepted {
                return Err(failed());
            }
        }
        if !converged {
            return Err(failed());
        }
        let (value, _, curvature) = conditional(u)?;
        nll += value + 0.5 * curvature.ln();
        modes.push(sd * u);
        for &i in rows {
            fitted_values[i] = mean_variance(eta[i] + sd * u, family, alpha)?.0;
        }
    }
    Ok(Laplace {
        nll: finite(nll)?,
        modes,
        fitted: fitted_values,
    })
}

/// Random-intercept GLMM using ML with a one-dimensional Laplace integral per group.
pub fn generalized_mixed(
    y: &[f64],
    predictors: &[Vec<f64>],
    group: &Grouping,
    family: ResponseFamily,
    constant: bool,
    iteration: IterationOptions,
    control: &Control,
) -> Result<LongitudinalResult> {
    check_iteration(iteration)?;
    let design = prepare(y, predictors, constant, control)?;
    validate_response(y, family)?;
    if family == ResponseFamily::Gaussian {
        return Err(parameter());
    }
    let clusters = rows(group, y.len(), control)?;
    let transformed = y
        .iter()
        .map(|&v| {
            if family == ResponseFamily::Binomial {
                if v == 1.0 { 1.0 } else { -1.0 }
            } else {
                (v + 0.5).ln()
            }
        })
        .collect::<Vec<_>>();
    let mut initial = least_squares(&design.x, &transformed, None, control)?.0;
    initial.push(0.7);
    if family == ResponseFamily::NegativeBinomial {
        initial.push(-1.0);
    }
    let objective = |parameters: &[f64]| {
        laplace(y, &design.x, &clusters, parameters, family, control)
            .map(|f| f.nll / y.len() as f64)
    };
    let minimum = minimize(&objective, initial, iteration, control)?;
    let fit = laplace(y, &design.x, &clusters, &minimum.beta, family, control)?;
    let information = hessian(
        &|b| laplace(y, &design.x, &clusters, b, family, control).map(|f| f.nll),
        &minimum.beta,
        control,
    )?;
    let covariance = inverse(&information)?;
    let p = design.x.ncols();
    let fixed_covariance = Mat::from_fn(p, p, |i, j| covariance[(i, j)]);
    let fixed_fitted = fitted(&design.x, &minimum.beta[..p])
        .into_iter()
        .map(|eta| mean_variance(eta, family, 0.0).map(|m| m.0))
        .collect::<Result<Vec<_>>>()?;
    let (beta, covariance) = design.raw(&minimum.beta[..p], Some(fixed_covariance));
    let covariance = covariance.unwrap();
    let variance = finite(minimum.beta[p].powi(2))?;
    control.check()?;
    Ok(LongitudinalResult {
        method: "glmm_laplace_ml".into(),
        family,
        observations: y.len(),
        group_counts: vec![group.levels],
        coefficients: coefficient_table(
            &beta,
            names(predictors.len(), constant),
            Some(&covariance),
            None,
        )?,
        coefficient_covariance: covariance_rows(&covariance),
        inference: "laplace_observed_information_wald_normal".into(),
        iterations: minimum.iterations,
        working_correlation: None,
        correlation: None,
        scale: 1.0,
        log_likelihood: Some(-fit.nll),
        aic: Some(2.0 * fit.nll + 2.0 * minimum.beta.len() as f64),
        negative_binomial_alpha: if family == ResponseFamily::NegativeBinomial {
            Some(finite(minimum.beta[p + 1].exp())?)
        } else {
            None
        },
        variance_components: vec![VarianceComponent {
            grouping: 1,
            term: 0,
            variance,
            boundary: variance <= 1e-8,
        }],
        random_effects: fit
            .modes
            .into_iter()
            .enumerate()
            .map(|(i, estimate)| RandomEffect {
                grouping: 1,
                level: i + 1,
                term: 0,
                estimate,
            })
            .collect(),
        fixed_fitted,
        residuals: y.iter().zip(&fit.fitted).map(|(y, m)| y - m).collect(),
        fitted_values: fit.fitted,
    })
}
