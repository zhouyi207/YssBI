//! Fixed/mixed-effects inverse-variance regression; pooling is its intercept-only case.
use super::data::*;
use crate::regression::models::common::{Design, coefficient_table, inverse, names, validate};
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_linalg::Mat;

struct WeightedFit {
    beta: Vec<f64>,
    covariance: Mat<f64>,
    fitted: Vec<f64>,
    residuals: Vec<f64>,
    weights: Vec<f64>,
    q: f64,
    moment_denominator: f64,
}

fn fit_at(
    y: &[f64],
    variances: &[f64],
    design: &Design,
    tau: f64,
    control: &Control,
) -> Result<WeightedFit> {
    control.check()?;
    let (n, p) = (y.len(), design.x.ncols());
    let raw = variances
        .iter()
        .map(|v| finite(1. / (v + tau)))
        .collect::<Result<Vec<_>>>()?;
    let scale = raw.iter().copied().fold(0., f64::max);
    let w = raw.iter().map(|w| w / scale).collect::<Vec<_>>();
    let gram = Mat::from_fn(p, p, |j, k| {
        (0..n)
            .map(|i| w[i] * design.x[(i, j)] * design.x[(i, k)])
            .sum()
    });
    let inverse = inverse(&gram)?;
    let rhs = (0..p)
        .map(|j| (0..n).map(|i| w[i] * design.x[(i, j)] * y[i]).sum::<f64>())
        .collect::<Vec<_>>();
    let beta = (0..p)
        .map(|j| finite((0..p).map(|k| inverse[(j, k)] * rhs[k]).sum()))
        .collect::<Result<Vec<_>>>()?;
    let mut fitted = Vec::with_capacity(n);
    let mut residuals = Vec::with_capacity(n);
    let mut q = 0.;
    let mut projection_trace = 0.;
    for i in 0..n {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let prediction = finite((0..p).map(|j| design.x[(i, j)] * beta[j]).sum())?;
        let residual = finite(y[i] - prediction)?;
        q += (residual / (variances[i] + tau).sqrt()).powi(2);
        let hat = (0..p)
            .map(|j| {
                design.x[(i, j)]
                    * (0..p)
                        .map(|k| inverse[(j, k)] * design.x[(i, k)])
                        .sum::<f64>()
            })
            .sum::<f64>();
        projection_trace += w[i] * w[i] * hat;
        fitted.push(prediction);
        residuals.push(residual);
    }
    let total = w.iter().sum::<f64>();
    let moment_denominator = finite(scale * (total - projection_trace))?;
    let covariance = Mat::from_fn(p, p, |j, k| inverse[(j, k)] / scale);
    Ok(WeightedFit {
        beta,
        covariance,
        fitted,
        residuals,
        weights: w.iter().map(|v| v / total).collect(),
        q: finite(q)?,
        moment_denominator,
    })
}

fn heterogeneity_variance(
    y: &[f64],
    v: &[f64],
    design: &Design,
    initial: &WeightedFit,
    estimator: MetaEstimator,
    control: &Control,
) -> Result<f64> {
    let df = (y.len() - design.x.ncols()) as f64;
    if estimator == MetaEstimator::Fixed || initial.q <= df {
        return Ok(0.);
    }
    if initial.moment_denominator <= 0. {
        return Err(failed());
    }
    let dl = finite((initial.q - df) / initial.moment_denominator)?;
    if estimator == MetaEstimator::DerSimonianLaird {
        return Ok(dl);
    }
    let variance_scale = v.iter().copied().fold(0., f64::max);
    let (mut low, mut high) = (0., dl.max(variance_scale));
    while fit_at(y, v, design, high, control)?.q > df {
        high = finite(high * 2.)?;
    }
    for _ in 0..128 {
        control.check()?;
        let mid = low + (high - low) / 2.;
        let q = fit_at(y, v, design, mid, control)?.q;
        if (q - df).abs() <= 1e-10 * (1. + df) {
            return Ok(mid);
        }
        if q > df {
            low = mid;
        } else {
            high = mid;
        }
        if high - low <= 1e-12 * high.max(variance_scale) {
            return Ok(low + (high - low) / 2.);
        }
    }
    Err(failed())
}

pub fn fit(
    y: &[f64],
    variances: &[f64],
    moderators: &[Vec<f64>],
    options: MetaOptions,
    control: &Control,
) -> Result<MetaFit> {
    let n = study_data(y, variances, control)?;
    validate(y, moderators, control)?;
    let normal_q = critical(options.confidence_level, None)?;
    let design = Design::new(moderators, n, true, true, true, control)?;
    let p = design.x.ncols();
    let df = n - p;
    let zero = fit_at(y, variances, &design, 0., control)?;
    let tau = heterogeneity_variance(y, variances, &design, &zero, options.estimator, control)?;
    let nonzero;
    let result = if tau == 0. {
        &zero
    } else {
        nonzero = fit_at(y, variances, &design, tau, control)?;
        &nonzero
    };
    let scale = if options.inference == MetaInference::KnappHartung {
        result.q / df as f64
    } else {
        1.
    };
    let covariance = Mat::from_fn(p, p, |j, k| result.covariance[(j, k)] * scale);
    let (beta, covariance) = design.raw(&result.beta, Some(covariance));
    let covariance = covariance.ok_or_else(failed)?;
    let t_df = (options.inference == MetaInference::KnappHartung).then_some(df);
    let q = critical(options.confidence_level, t_df.map(|df| df as f64))?;
    let mut coefficients = coefficient_table(
        &beta,
        names(moderators.len(), true),
        Some(&covariance),
        t_df,
    )?;
    for c in &mut coefficients {
        c.confidence_interval = c
            .standard_error
            .map(|se| [c.estimate - q * se, c.estimate + q * se]);
        if c.confidence_interval
            .is_some_and(|ci| ci.iter().any(|x| !x.is_finite()))
        {
            return Err(failed());
        }
    }
    let prediction_interval = if p == 1 && options.estimator != MetaEstimator::Fixed && n > 2 {
        let q = critical(options.confidence_level, Some((n - 2) as f64))?;
        let width = finite(q * (tau + covariance[(0, 0)]).sqrt())?;
        Some([finite(beta[0] - width)?, finite(beta[0] + width)?])
    } else {
        None
    };
    let h_squared = (zero.q / df as f64).max(1.);
    let heterogeneity = Heterogeneity {
        q: zero.q,
        degrees_of_freedom: df,
        p_value: ChiSquared::new(df as f64).map_err(|_| failed())?.sf(zero.q),
        i_squared_percent: if zero.q > 0. {
            ((zero.q - df as f64) / zero.q).max(0.) * 100.
        } else {
            0.
        },
        h_squared,
        tau_squared: tau,
    };
    let summary = MetaSummary {
        studies: n,
        estimator: options.estimator,
        inference: options.inference,
        confidence_level: options.confidence_level,
        residual_degrees_of_freedom: df,
        coefficients,
        covariance: (0..p)
            .map(|j| (0..p).map(|k| covariance[(j, k)]).collect())
            .collect(),
        heterogeneity,
        residual_q: result.q,
        prediction_interval,
    };
    let studies = (0..n)
        .map(|i| {
            Ok(MetaStudy {
                effect: study(i, y[i], variances[i], normal_q)?,
                weight: result.weights[i],
                fitted: result.fitted[i],
                residual: result.residuals[i],
            })
        })
        .collect::<Result<Vec<_>>>()?;
    control.check()?;
    Ok(MetaFit { summary, studies })
}
