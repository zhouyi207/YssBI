pub(super) use crate::regression::models::common::{
    Design, Result, check_iteration, coefficient_table, failed, finite, fitted, hessian, inverse,
    least_squares, minimize, names, parameter, transform, validate,
};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_contract::causal::models::{BootstrapOptions, CausalWaldTest};
pub(super) use yss_sci_contract::execution::ScientificExecutionControl as Control;
pub(super) use yss_sci_linalg::{Col, Mat};

pub(super) fn mean(values: &[f64]) -> f64 {
    values.iter().map(|v| v / values.len() as f64).sum()
}
pub(super) fn rows(matrix: &Mat<f64>) -> Vec<Vec<f64>> {
    (0..matrix.nrows())
        .map(|i| (0..matrix.ncols()).map(|j| matrix[(i, j)]).collect())
        .collect()
}
pub(super) fn binary(values: &[f64]) -> Result<usize> {
    if values.iter().any(|&v| v != 0.0 && v != 1.0) {
        return Err(parameter());
    }
    let treated = values.iter().filter(|&&v| v == 1.0).count();
    if treated == 0 || treated == values.len() {
        return Err(parameter());
    }
    Ok(treated)
}
pub(super) fn chi_square(statistic: f64, df: usize) -> Result<CausalWaldTest> {
    if df == 0 || !statistic.is_finite() || statistic < 0.0 {
        return Err(failed());
    }
    Ok(CausalWaldTest {
        statistic,
        degrees_of_freedom: df,
        p_value: ChiSquared::new(df as f64)
            .map_err(|_| failed())?
            .sf(statistic),
    })
}

/// Resample whole observations and refit every stage. Failed replicates are errors,
/// never silently dropped. Online covariance avoids retaining B full model results.
pub(super) fn bootstrap_covariance(
    n: usize,
    width: usize,
    options: BootstrapOptions,
    control: &Control,
    fit: impl Fn(&[usize]) -> Result<Vec<f64>>,
) -> Result<Option<Mat<f64>>> {
    if options.replications == 0 {
        return Ok(None);
    }
    if options.replications < 2 {
        return Err(parameter());
    }
    let mut rng = StdRng::seed_from_u64(options.seed);
    let mut indices = vec![0; n];
    let mut average = vec![0.0; width];
    let mut scales = vec![0.0; width];
    let mut covariance = Mat::zeros(width, width);
    for b in 0..options.replications {
        control.check()?;
        for (i, index) in indices.iter_mut().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            *index = rng.random_range(0..n);
        }
        let mut values = fit(&indices)?;
        if values.len() != width || values.iter().any(|v| !v.is_finite()) {
            return Err(failed());
        }
        // Keep online moments in bounded coefficient coordinates. Rescaling
        // existing moments preserves both very large and subnormal covariance.
        for j in 0..width {
            if j.is_multiple_of(256) {
                control.check()?;
            }
            let scale = values[j].abs();
            if scale > scales[j] {
                let ratio = scales[j] / scale;
                average[j] *= ratio;
                for k in 0..width {
                    covariance[(j, k)] *= ratio;
                    covariance[(k, j)] *= ratio;
                }
                scales[j] = scale;
            }
            values[j] = if scales[j] == 0.0 {
                0.0
            } else {
                values[j] / scales[j]
            };
        }
        let delta = values
            .iter()
            .zip(&average)
            .map(|(v, m)| v - m)
            .collect::<Vec<_>>();
        for j in 0..width {
            average[j] += delta[j] / (b + 1) as f64;
            values[j] -= average[j];
        }
        for j in 0..width {
            if j.is_multiple_of(256) {
                control.check()?;
            }
            for k in 0..width {
                covariance[(j, k)] += delta[j] * values[k];
            }
        }
    }
    for j in 0..width {
        control.check()?;
        for k in 0..=j {
            let normalized =
                covariance[(j, k)].midpoint(covariance[(k, j)]) / (options.replications - 1) as f64;
            // Apply the larger coordinate first to preserve small cross terms;
            // do not form an overflowing or underflowing product of scales.
            let value = finite(normalized * scales[j].max(scales[k]) * scales[j].min(scales[k]))?;
            covariance[(j, k)] = value;
            covariance[(k, j)] = value;
        }
    }
    Ok(Some(covariance))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_covariance_preserves_minimum_positive_units() {
        use std::cell::RefCell;
        use std::time::{Duration, Instant};
        use yss_sci_contract::execution::ScientificCancellationToken;

        let control = Control {
            cancellation: ScientificCancellationToken::new(),
            deadline: Instant::now() + Duration::from_secs(60),
        };
        let amplitude = (4.0 * f64::from_bits(1)).sqrt();
        let samples = RefCell::new(Vec::new());
        let covariance = bootstrap_covariance(
            2,
            1,
            BootstrapOptions {
                replications: 64,
                seed: 17,
            },
            &control,
            |indices| {
                let average = indices
                    .iter()
                    .map(|&i| if i == 0 { 1.0 } else { -1.0 })
                    .sum::<f64>()
                    / indices.len() as f64;
                samples.borrow_mut().push(average);
                Ok(vec![average * amplitude])
            },
        )
        .unwrap()
        .unwrap();
        let samples = samples.into_inner();
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let normalized_variance = samples
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (samples.len() - 1) as f64;
        let expected = normalized_variance * amplitude * amplitude;
        assert!(expected > 0.0 && expected.is_subnormal());
        assert!(covariance[(0, 0)] > 0.0);
        assert_eq!(covariance[(0, 0)], expected);
    }
}
