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

/// HC3 for a fixed design, including observation weights in both bread and scores.
pub(super) fn hc3(
    x: &Mat<f64>,
    residuals: &[f64],
    weights: Option<&[f64]>,
    bread: &Mat<f64>,
    control: &Control,
) -> Result<Mat<f64>> {
    let p = x.ncols();
    let mut meat = Mat::zeros(p, p);
    for i in 0..x.nrows() {
        if i.is_multiple_of(256) {
            control.check()?;
        }
        let w = weights.map_or(1.0, |w| w[i]);
        let leverage = w
            * (0..p)
                .map(|j| x[(i, j)] * (0..p).map(|k| bread[(j, k)] * x[(i, k)]).sum::<f64>())
                .sum::<f64>();
        if leverage >= 1.0 - 1e-12 {
            return Err(parameter());
        }
        let score = w * residuals[i] / (1.0 - leverage);
        for j in 0..p {
            for k in 0..p {
                meat[(j, k)] += score * score * x[(i, j)] * x[(i, k)];
            }
        }
    }
    Ok(bread.as_ref() * meat.as_ref() * bread.as_ref())
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
    let mut average = vec![0.0; width];
    let mut covariance = Mat::zeros(width, width);
    for b in 0..options.replications {
        control.check()?;
        let indices = (0..n).map(|_| rng.random_range(0..n)).collect::<Vec<_>>();
        let values = fit(&indices)?;
        if values.len() != width || values.iter().any(|v| !v.is_finite()) {
            return Err(failed());
        }
        let delta = values
            .iter()
            .zip(&average)
            .map(|(v, m)| v - m)
            .collect::<Vec<_>>();
        for j in 0..width {
            average[j] += delta[j] / (b + 1) as f64;
        }
        for j in 0..width {
            for k in 0..width {
                covariance[(j, k)] += delta[j] * (values[k] - average[k]);
            }
        }
    }
    Ok(Some(Mat::from_fn(width, width, |j, k| {
        (covariance[(j, k)] + covariance[(k, j)]) / (2.0 * (options.replications - 1) as f64)
    })))
}
