//! Population-average GEE and Gaussian/generalized mixed-effects estimators.
use crate::regression::models::common::*;
use yss_sci_contract::execution::{
    ScientificExecutionControl as Control, ScientificInputViolation as Violation,
};
use yss_sci_contract::longitudinal::*;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve};

mod gee;
mod glmm;
mod mixed;
pub use gee::gee;
pub use glmm::generalized_mixed;
pub use mixed::linear_mixed;
#[cfg(test)]
mod tests;

fn prepare(y: &[f64], x: &[Vec<f64>], constant: bool, control: &Control) -> Result<Design> {
    validate(y, x, control)?;
    Design::new(x, y.len(), constant, true, true, control)
}

fn rows(group: &Grouping, n: usize, control: &Control) -> Result<Vec<Vec<usize>>> {
    control.check()?;
    if group.codes.len() != n {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if group.levels < 2 || group.levels > n {
        return Err(parameter());
    }
    let mut rows = vec![Vec::new(); group.levels];
    for (i, &g) in group.codes.iter().enumerate() {
        control.check()?;
        rows.get_mut(g).ok_or_else(parameter)?.push(i);
    }
    if rows.iter().any(Vec::is_empty) {
        return Err(parameter());
    }
    Ok(rows)
}

fn validate_response(y: &[f64], family: ResponseFamily) -> Result<()> {
    if y.iter().any(|&v| match family {
        ResponseFamily::Gaussian => false,
        ResponseFamily::Binomial => v != 0.0 && v != 1.0,
        ResponseFamily::Poisson | ResponseFamily::NegativeBinomial => v < 0.0 || v.fract() != 0.0,
    }) || (family == ResponseFamily::Binomial && y.iter().all(|&v| v == y[0]))
        || (matches!(
            family,
            ResponseFamily::Poisson | ResponseFamily::NegativeBinomial
        ) && y.iter().all(|&v| v == 0.0))
    {
        return Err(parameter());
    }
    Ok(())
}

fn mean_variance(eta: f64, family: ResponseFamily, alpha: f64) -> Result<(f64, f64, f64)> {
    let (mean, derivative, variance) = match family {
        ResponseFamily::Gaussian => (eta, 1.0, 1.0),
        ResponseFamily::Binomial => {
            let mean = if eta >= 0.0 {
                1.0 / (1.0 + (-eta).exp())
            } else {
                eta.exp() / (1.0 + eta.exp())
            };
            let tail = (-eta.abs()).exp();
            let derivative = tail / (1.0 + tail).powi(2);
            (mean, derivative, derivative)
        }
        ResponseFamily::Poisson => (eta.exp(), eta.exp(), eta.exp()),
        ResponseFamily::NegativeBinomial => {
            let mean = eta.exp();
            (mean, mean, mean + alpha * mean * mean)
        }
    };
    if !mean.is_finite() || !variance.is_finite() || variance <= 0.0 || derivative <= 0.0 {
        return Err(failed());
    }
    Ok((mean, derivative, variance))
}

fn covariance_rows(covariance: &Mat<f64>) -> Vec<Vec<f64>> {
    (0..covariance.nrows())
        .map(|i| {
            (0..covariance.ncols())
                .map(|j| covariance[(i, j)])
                .collect()
        })
        .collect()
}

fn log_determinant(factor: &yss_sci_linalg::Cholesky) -> Result<f64> {
    let lower = factor.lower();
    finite(2.0 * (0..lower.nrows()).map(|i| lower[(i, i)].ln()).sum::<f64>())
}
