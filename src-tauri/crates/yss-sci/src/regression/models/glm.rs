//! Shared controlled GLM fitting, including positive prior weights for survey estimators.
use super::common::*;
use super::likelihood::logistic;
use statrs::distribution::{Continuous, ContinuousCDF, Normal};
use statrs::function::gamma::ln_gamma;
use yss_sci_contract::{execution::ScientificExecutionControl as Control, regression::models::*};
use yss_sci_linalg::Mat;
pub fn glm(
    y: &[f64],
    predictors: &[Vec<f64>],
    options: GlmOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    fit(y, predictors, options, None, control)
}
/// The survey owner replaces model-based covariance with design-based linearization.
pub(crate) fn weighted_glm(
    y: &[f64],
    predictors: &[Vec<f64>],
    weights: &[f64],
    options: GlmOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    fit(y, predictors, options, Some(weights), control)
}
fn mean_link(eta: f64, link: GlmLink) -> Result<(f64, f64)> {
    let (mu, d) = match link {
        GlmLink::Identity => (eta, 1.0),
        GlmLink::Log => {
            let mu = eta.exp();
            (mu, mu)
        }
        GlmLink::Logit => {
            let mu = logistic(eta);
            (mu, mu * (1.0 - mu))
        }
        GlmLink::Probit => {
            let norm = Normal::new(0.0, 1.0).expect("normal");
            (norm.cdf(eta), norm.pdf(eta))
        }
        GlmLink::Cloglog => {
            let e = eta.exp();
            (-(-e).exp_m1(), (eta - e).exp())
        }
    };
    if !mu.is_finite() || !d.is_finite() || d <= 0.0 {
        return Err(failed());
    }
    Ok((mu, d))
}
fn variance(mu: f64, family: GlmFamily) -> f64 {
    match family {
        GlmFamily::Gaussian => 1.0,
        GlmFamily::Binomial => mu * (1.0 - mu),
        GlmFamily::Poisson => mu,
        GlmFamily::Gamma => mu * mu,
        GlmFamily::InverseGaussian => mu.powi(3),
    }
}
fn unit_deviance(y: f64, mu: f64, family: GlmFamily) -> f64 {
    match family {
        GlmFamily::Gaussian => (y - mu).powi(2),
        GlmFamily::Binomial => {
            2.0 * (if y == 0.0 { 0.0 } else { y * (y / mu).ln() }
                + if y == 1.0 {
                    0.0
                } else {
                    (1.0 - y) * ((1.0 - y) / (1.0 - mu)).ln()
                })
        }
        GlmFamily::Poisson => {
            2.0 * (if y == 0.0 {
                mu
            } else {
                y * (y / mu).ln() - y + mu
            })
        }
        GlmFamily::Gamma => 2.0 * ((y - mu) / mu - (y / mu).ln()),
        GlmFamily::InverseGaussian => (y - mu).powi(2) / (y * mu * mu),
    }
}
fn fit(
    y: &[f64],
    predictors: &[Vec<f64>],
    options: GlmOptions,
    prior_weights: Option<&[f64]>,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    check_iteration(options.iteration)?;
    if prior_weights
        .is_some_and(|w| w.len() != y.len() || w.iter().any(|v| !v.is_finite() || *v <= 0.))
    {
        return Err(parameter());
    }
    let prior = |i: usize| prior_weights.map_or(1., |w| w[i]);
    let valid = matches!(
        (options.family, options.link),
        (GlmFamily::Gaussian, GlmLink::Identity | GlmLink::Log)
            | (
                GlmFamily::Binomial,
                GlmLink::Logit | GlmLink::Probit | GlmLink::Cloglog
            )
            | (
                GlmFamily::Poisson | GlmFamily::Gamma | GlmFamily::InverseGaussian,
                GlmLink::Log
            )
    );
    if !valid
        || (options.fractional && options.family != GlmFamily::Binomial)
        || y.iter().any(|&v| match options.family {
            GlmFamily::Binomial => {
                if options.fractional {
                    !(0.0..=1.0).contains(&v)
                } else {
                    v != 0.0 && v != 1.0
                }
            }
            GlmFamily::Poisson => v < 0.0 || v.fract() != 0.0,
            GlmFamily::Gamma | GlmFamily::InverseGaussian => v <= 0.0,
            GlmFamily::Gaussian => false,
        })
    {
        return Err(parameter());
    }
    let design = Design::new(predictors, y.len(), options.constant, true, true, control)?;
    let x = &design.x;
    let p = x.ncols();
    let n = y.len();
    let ym = if let Some(w) = prior_weights {
        let total = finite(w.iter().sum())?;
        finite(y.iter().zip(w).map(|(y, w)| y * (w / total)).sum())?
    } else {
        mean(y)
    };
    if options.family == GlmFamily::Binomial && (ym <= 0.0 || ym >= 1.0) {
        return Err(parameter());
    }
    if options.link == GlmLink::Log && ym <= 0.0 {
        return Err(parameter());
    }
    let mut beta = vec![0.0; p];
    if options.constant {
        beta[0] = match options.link {
            GlmLink::Identity => ym,
            GlmLink::Log => ym.ln(),
            GlmLink::Logit => (ym / (1.0 - ym)).ln(),
            GlmLink::Probit => Normal::new(0.0, 1.0).expect("normal").inverse_cdf(ym),
            GlmLink::Cloglog => (-(1.0 - ym).ln()).ln(),
        };
    }
    let evaluate = |b: &[f64]| -> Result<(Vec<f64>, Vec<f64>, f64)> {
        let eta = fitted(x, b);
        let mut mu = Vec::with_capacity(n);
        let mut d = Vec::with_capacity(n);
        let mut dev = 0.0;
        for i in 0..n {
            if i % 1024 == 0 {
                control.check()?;
            }
            let (m, derivative) = mean_link(eta[i], options.link)?;
            let v = variance(m, options.family);
            if v <= 0.0 || !v.is_finite() {
                return Err(failed());
            }
            dev += prior(i) * unit_deviance(y[i], m, options.family);
            mu.push(m);
            d.push(derivative);
        }
        Ok((mu, d, finite(dev)?))
    };
    let mut current = evaluate(&beta)?;
    let mut done = None;
    for iter in 1..=options.iteration.max_iterations {
        control.check()?;
        let eta = fitted(x, &beta);
        let weights = (0..n)
            .map(|i| prior(i) * current.1[i].powi(2) / variance(current.0[i], options.family))
            .collect::<Vec<_>>();
        let z = (0..n)
            .map(|i| eta[i] + (y[i] - current.0[i]) / current.1[i])
            .collect::<Vec<_>>();
        let (proposal, _) = least_squares(x, &z, Some(&weights), control)?;
        let mut accepted = None;
        let mut step = 1.0;
        for _ in 0..40 {
            let trial = beta
                .iter()
                .zip(&proposal)
                .map(|(a, b)| a + step * (b - a))
                .collect::<Vec<_>>();
            match evaluate(&trial) {
                Ok(v) if v.2 <= current.2 + 1e-10 * (1.0 + current.2.abs()) => {
                    accepted = Some((trial, v));
                    break;
                }
                Err(
                    e @ (yss_sci_contract::execution::ScientificComputationError::Cancelled
                    | yss_sci_contract::execution::ScientificComputationError::DeadlineExceeded),
                ) => return Err(e),
                _ => {
                    step *= 0.5;
                }
            }
        }
        let (next, new) = accepted.ok_or_else(failed)?;
        let change = next
            .iter()
            .zip(&beta)
            .map(|(a, b)| (a - b).abs() / (1.0 + b.abs()))
            .fold(0.0, f64::max);
        beta = next;
        current = new;
        if change <= options.iteration.tolerance {
            done = Some(iter);
            break;
        }
    }
    let iterations = done.ok_or_else(failed)?;
    let (mu, derivative, deviance) = current;
    let weights = (0..n)
        .map(|i| prior(i) * derivative[i].powi(2) / variance(mu[i], options.family))
        .collect::<Vec<_>>();
    let bread = inverse(&gram(x, Some(&weights), control)?)?;
    let dispersion = match options.family {
        GlmFamily::Gaussian | GlmFamily::Gamma | GlmFamily::InverseGaussian => {
            (0..n)
                .map(|i| prior(i) * (y[i] - mu[i]).powi(2) / variance(mu[i], options.family))
                .sum::<f64>()
                / (n - p) as f64
        }
        _ => 1.0,
    };
    let cov = if options.fractional {
        let scores = (0..n)
            .map(|i| {
                (prior(i) * (y[i] - mu[i]) * derivative[i] / variance(mu[i], options.family))
                    .powi(2)
            })
            .collect::<Vec<_>>();
        bread.as_ref() * gram(x, Some(&scores), control)?.as_ref() * bread.as_ref()
    } else {
        Mat::from_fn(p, p, |i, j| bread[(i, j)] * dispersion)
    };
    let (raw, cov) = design.raw(&beta, Some(cov));
    let mut r = result(
        if options.fractional {
            "fractional_response"
        } else {
            "glm"
        },
        y,
        mu.clone(),
        raw,
        names(predictors.len(), options.constant),
        cov,
        options.constant,
        if options.family == GlmFamily::Gaussian {
            Some(n - p)
        } else {
            None
        },
        RegressionDetails::Glm {
            family: options.family,
            link: options.link,
            dispersion: finite(dispersion)?,
            deviance,
            covariance_method: if options.fractional {
                "HC0 sandwich".into()
            } else {
                "model Fisher information".into()
            },
        },
    )?;
    r.statistics.df_residual = Some(n - p);
    r.iterations = iterations;
    if options.family != GlmFamily::Gaussian {
        r.statistics.r_squared = None;
        r.statistics.adjusted_r_squared = None;
    }
    if !options.fractional && prior_weights.is_none() {
        if options.family == GlmFamily::Gaussian {
            gaussian_likelihood(&mut r)?;
        } else {
            let ll = (0..n)
                .map(|i| match options.family {
                    GlmFamily::Binomial => {
                        if y[i] == 1.0 {
                            mu[i].ln()
                        } else {
                            (-mu[i]).ln_1p()
                        }
                    }
                    GlmFamily::Poisson => y[i] * mu[i].ln() - mu[i] - ln_gamma(y[i] + 1.0),
                    GlmFamily::Gamma => {
                        let a = 1.0 / dispersion;
                        a * (a / mu[i]).ln() - ln_gamma(a) + (a - 1.0) * y[i].ln()
                            - a * y[i] / mu[i]
                    }
                    GlmFamily::InverseGaussian => {
                        -0.5 * ((2.0 * std::f64::consts::PI * dispersion).ln()
                            + 3.0 * y[i].ln()
                            + (y[i] - mu[i]).powi(2) / (dispersion * y[i] * mu[i] * mu[i]))
                    }
                    GlmFamily::Gaussian => unreachable!(),
                })
                .sum::<f64>();
            if ll.is_finite() {
                likelihood_statistics(
                    &mut r,
                    ll,
                    p + usize::from(matches!(
                        options.family,
                        GlmFamily::Gamma | GlmFamily::InverseGaussian
                    )),
                );
            }
        }
    }
    control.check()?;
    Ok(r)
}
