//! Fixed/mixed-effects inverse-variance regression; pooling is its intercept-only case.
use super::data::*;
use crate::inference::intervals::validate_confidence;
use crate::regression::models::common::{Design, coefficient_table, inverse, names, validate};
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_linalg::Mat;

struct WeightedFit {
    beta: Vec<f64>,
    gram_inverse: Mat<f64>,
    q: f64,
    weight_scale: f64,
    weight_total: f64,
}

struct PreparedMeta {
    design: Design,
    fit: WeightedFit,
    baseline_q: f64,
    tau: f64,
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
    let mut w = variances
        .iter()
        .map(|v| finite(1. / (v + tau)))
        .collect::<Result<Vec<_>>>()?;
    let scale = w.iter().copied().fold(0., f64::max);
    for weight in &mut w {
        *weight /= scale;
    }
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
    let mut q = 0.;
    for i in 0..n {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let prediction = finite((0..p).map(|j| design.x[(i, j)] * beta[j]).sum())?;
        let residual = finite(y[i] - prediction)?;
        q += (residual / (variances[i] + tau).sqrt()).powi(2);
    }
    let total = w.iter().sum::<f64>();
    Ok(WeightedFit {
        beta,
        gram_inverse: inverse,
        q: finite(q)?,
        weight_scale: scale,
        weight_total: total,
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
    let p = design.x.ncols();
    let mut projection_trace = 0.;
    for (i, &variance) in v.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let weight = (1. / variance) / initial.weight_scale;
        let hat = (0..p)
            .map(|j| {
                design.x[(i, j)]
                    * (0..p)
                        .map(|k| initial.gram_inverse[(j, k)] * design.x[(i, k)])
                        .sum::<f64>()
            })
            .sum::<f64>();
        projection_trace += weight * weight * hat;
    }
    let normalized_denominator = finite(initial.weight_total - projection_trace)?;
    if normalized_denominator <= 0. {
        return Err(failed());
    }
    let denominator = initial.weight_scale * normalized_denominator;
    // Keep the normalized denominator separate when restoring its scale would overflow.
    let dl = finite(if denominator.is_finite() {
        (initial.q - df) / denominator
    } else {
        (initial.q - df) / initial.weight_scale / normalized_denominator
    })?;
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

fn prepare(
    y: &[f64],
    variances: &[f64],
    moderators: &[Vec<f64>],
    estimator: MetaEstimator,
    confidence: Option<f64>,
    control: &Control,
) -> Result<PreparedMeta> {
    let n = study_data(y, variances, control)?;
    validate(y, moderators, control)?;
    if let Some(confidence) = confidence {
        validate_confidence(confidence)?;
    }
    let design = Design::new(moderators, n, true, true, true, control)?;
    let zero = fit_at(y, variances, &design, 0., control)?;
    let baseline_q = zero.q;
    let tau = heterogeneity_variance(y, variances, &design, &zero, estimator, control)?;
    let fit = if tau == 0. {
        zero
    } else {
        drop(zero);
        fit_at(y, variances, &design, tau, control)?
    };
    Ok(PreparedMeta {
        design,
        fit,
        baseline_q,
        tau,
    })
}

impl PreparedMeta {
    fn heterogeneity(&self) -> Result<Heterogeneity> {
        let df = self.design.x.nrows() - self.design.x.ncols();
        Ok(Heterogeneity {
            q: self.baseline_q,
            degrees_of_freedom: df,
            p_value: ChiSquared::new(df as f64)
                .map_err(|_| failed())?
                .sf(self.baseline_q),
            i_squared_percent: if self.baseline_q > 0. {
                ((self.baseline_q - df as f64) / self.baseline_q).max(0.) * 100.
            } else {
                0.
            },
            h_squared: (self.baseline_q / df as f64).max(1.),
            tau_squared: self.tau,
        })
    }

    fn summary(&self, options: MetaOptions, control: &Control) -> Result<MetaSummary> {
        let (design, result, tau) = (&self.design, &self.fit, self.tau);
        let (n, p) = (design.x.nrows(), design.x.ncols());
        let df = n - p;
        let scale = if options.inference == MetaInference::KnappHartung {
            result.q / df as f64
        } else {
            1.
        };
        let covariance = Mat::from_fn(p, p, |j, k| {
            (result.gram_inverse[(j, k)] / result.weight_scale) * scale
        });
        let (beta, covariance) = design.raw(&result.beta, Some(covariance));
        let covariance = covariance.ok_or_else(failed)?;
        let t_df = (options.inference == MetaInference::KnappHartung).then_some(df);
        let coefficients = coefficient_table(
            &beta,
            names(p - 1, true),
            Some(&covariance),
            t_df,
            options.confidence_level,
        )?;
        let prediction_interval = if p == 1 && options.estimator != MetaEstimator::Fixed && n > 2 {
            let q = critical(options.confidence_level, Some((n - 2) as f64))?;
            let width = finite(q * (tau + covariance[(0, 0)]).sqrt())?;
            Some([finite(beta[0] - width)?, finite(beta[0] + width)?])
        } else {
            None
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
            heterogeneity: self.heterogeneity()?,
            residual_q: result.q,
            prediction_interval,
        };
        control.check()?;
        Ok(summary)
    }
}

pub fn summary(
    y: &[f64],
    variances: &[f64],
    moderators: &[Vec<f64>],
    options: MetaOptions,
    control: &Control,
) -> Result<MetaSummary> {
    prepare(
        y,
        variances,
        moderators,
        options.estimator,
        Some(options.confidence_level),
        control,
    )?
    .summary(options, control)
}

pub fn heterogeneity(
    y: &[f64],
    variances: &[f64],
    estimator: MetaEstimator,
    control: &Control,
) -> Result<Heterogeneity> {
    let result = prepare(y, variances, &[], estimator, None, control)?.heterogeneity()?;
    control.check()?;
    Ok(result)
}

pub(super) fn pooled_estimate(
    y: &[f64],
    variances: &[f64],
    estimator: MetaEstimator,
    confidence: f64,
    control: &Control,
) -> Result<f64> {
    let prepared = prepare(y, variances, &[], estimator, Some(confidence), control)?;
    let (beta, _) = prepared.design.raw(&prepared.fit.beta, None);
    control.check()?;
    finite(beta[0])
}

pub fn fit(
    y: &[f64],
    variances: &[f64],
    moderators: &[Vec<f64>],
    options: MetaOptions,
    control: &Control,
) -> Result<MetaFit> {
    let prepared = prepare(
        y,
        variances,
        moderators,
        options.estimator,
        Some(options.confidence_level),
        control,
    )?;
    let summary = prepared.summary(options, control)?;
    let normal_q = critical(options.confidence_level, None)?;
    let p = prepared.design.x.ncols();
    let studies = (0..y.len())
        .map(|i| {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            let fitted = finite(
                (0..p)
                    .map(|j| prepared.design.x[(i, j)] * prepared.fit.beta[j])
                    .sum(),
            )?;
            Ok(MetaStudy {
                effect: study(i, y[i], variances[i], normal_q)?,
                weight: (1. / (variances[i] + prepared.tau))
                    / prepared.fit.weight_scale
                    / prepared.fit.weight_total,
                fitted,
                residual: finite(y[i] - fitted)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    control.check()?;
    Ok(MetaFit { summary, studies })
}
