//! Additive ratings-based conjoint with within-attribute zero-sum part-worths.
use super::data::*;
use crate::regression::models::common::{failed, least_squares};
use yss_sci_contract::decision::conjoint::*;
use yss_sci_linalg::{Mat, matrix_rank};

pub fn fit(response: &[f64], factors: &[Vec<usize>], control: &Control) -> Result<ConjointResult> {
    validate(response, &[], control)?;
    let n = response.len();
    let levels = level_counts(factors, n, control)?;
    let k = levels
        .iter()
        .try_fold(1usize, |s, &l| s.checked_add(l - 1))
        .ok_or_else(parameter)?;
    if n < k {
        return Err(parameter());
    }
    let mut design = Mat::from_fn(n, k, |_, j| if j == 0 { 1. } else { 0. });
    let mut offset = 1;
    for (factor, &count) in factors.iter().zip(&levels) {
        for (i, &level) in factor.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if level > 0 {
                design[(i, offset + level - 1)] = 1.;
            }
        }
        offset += count - 1;
    }
    control.check()?;
    if matrix_rank(design.as_ref()).map_err(|_| failed())?.0 != k {
        return Err(parameter());
    }
    let scale = response
        .iter()
        .map(|v| v.abs())
        .fold(0., f64::max)
        .max(f64::MIN_POSITIVE);
    let scaled = response.iter().map(|v| v / scale).collect::<Vec<_>>();
    let (mut beta, inverse) = least_squares(&design, &scaled, None, control)?;
    // A constant response has exactly zero identifiable part-worths, not roundoff-sized importance.
    let constant_response = response.iter().all(|v| *v == response[0]);
    if constant_response {
        beta.fill(0.);
        beta[0] = scaled[0];
    }
    let mean = scaled.iter().map(|v| v / n as f64).sum::<f64>();
    let mut sse = 0.;
    let mut sst = 0.;
    let mut rows = Vec::with_capacity(n);
    for (i, &observed) in scaled.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let fitted = finite((0..k).map(|j| design[(i, j)] * beta[j]).sum())?;
        let residual = observed - fitted;
        sse += residual * residual;
        sst += (observed - mean).powi(2);
        rows.push(ConjointPrediction {
            observation: i + 1,
            observed: response[i],
            fitted: finite(fitted * scale)?,
            residual: finite(residual * scale)?,
        });
    }
    finite(sse)?;
    finite(sst)?;
    let df = n - k;
    let variance = (df > 0).then(|| sse / df as f64);
    let mut attributes = Vec::with_capacity(levels.len());
    let mut offset = 1;
    let mut intercept = beta[0];
    for &count in &levels {
        control.check()?;
        let (mut part, average) =
            attribute(count, offset, &beta, &inverse, variance, scale, control)?;
        part.attribute = attributes.len() + 1;
        attributes.push(part);
        intercept += average;
        offset += count - 1;
    }
    let largest = attributes.iter().map(|a| a.range).fold(0., f64::max);
    if largest > 0. {
        let total = attributes.iter().map(|a| a.range / largest).sum::<f64>();
        for attribute in &mut attributes {
            attribute.importance_percent = Some(100. * (attribute.range / largest) / total);
        }
    }
    let r_squared = if !constant_response && sst > 0. {
        let r = finite(1. - sse / sst)?;
        if !(-1e-8..=1. + 1e-8).contains(&r) {
            return Err(failed());
        }
        Some(r.clamp(0., 1.))
    } else {
        None
    };
    Ok(ConjointResult {
        summary: ConjointSummary {
            method: "additive_ratings_ols",
            observations: n,
            parameters: k,
            residual_degrees_of_freedom: df,
            centered_intercept: finite(intercept * scale)?,
            r_squared,
            attributes,
        },
        rows,
    })
}
fn level_counts(factors: &[Vec<usize>], n: usize, control: &Control) -> Result<Vec<usize>> {
    if factors.is_empty() {
        return Err(parameter());
    }
    factors
        .iter()
        .map(|factor| {
            control.check()?;
            if factor.len() != n {
                return Err(parameter());
            }
            let count = factor
                .iter()
                .copied()
                .max()
                .and_then(|v| v.checked_add(1))
                .ok_or_else(parameter)?;
            if count > n {
                return Err(parameter());
            }
            let mut seen = vec![false; count];
            for (i, &code) in factor.iter().enumerate() {
                if i.is_multiple_of(1024) {
                    control.check()?;
                }
                seen[code] = true;
            }
            if seen.contains(&false) {
                return Err(parameter());
            }
            Ok(count)
        })
        .collect()
}
fn attribute(
    count: usize,
    offset: usize,
    beta: &[f64],
    inverse: &Mat<f64>,
    variance: Option<f64>,
    scale: f64,
    control: &Control,
) -> Result<(ConjointAttribute, f64)> {
    let end = offset + count - 1;
    let average = beta[offset..end]
        .iter()
        .map(|v| v / count as f64)
        .sum::<f64>();
    let mut covariance_sum = 0.;
    for i in offset..end {
        control.check()?;
        covariance_sum += (offset..end).map(|j| inverse[(i, j)]).sum::<f64>();
    }
    let covariance_mean = covariance_sum / (count as f64).powi(2);
    let mut levels = Vec::with_capacity(count);
    for level in 0..count {
        control.check()?;
        let utility = if level == 0 {
            -average
        } else {
            beta[offset + level - 1] - average
        };
        let standard_error = variance
            .map(|variance| {
                let (diagonal, row_mean) = if level == 0 {
                    (0., 0.)
                } else {
                    let i = offset + level - 1;
                    (
                        inverse[(i, i)],
                        (offset..end)
                            .map(|j| inverse[(i, j)] / count as f64)
                            .sum::<f64>(),
                    )
                };
                let v = diagonal - 2. * row_mean + covariance_mean;
                let tolerance = 128.
                    * f64::EPSILON
                    * (diagonal.abs() + 2. * row_mean.abs() + covariance_mean.abs());
                if !v.is_finite() || v < -tolerance {
                    return Err(failed());
                }
                finite((v.max(0.) * variance).sqrt() * scale)
            })
            .transpose()?;
        levels.push(PartWorth {
            level: level + 1,
            utility: finite(utility * scale)?,
            standard_error,
        });
    }
    let minimum = levels
        .iter()
        .map(|l| l.utility)
        .fold(f64::INFINITY, f64::min);
    let maximum = levels
        .iter()
        .map(|l| l.utility)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok((
        ConjointAttribute {
            attribute: 0,
            range: finite(maximum - minimum)?,
            importance_percent: None,
            levels,
        },
        average,
    ))
}
