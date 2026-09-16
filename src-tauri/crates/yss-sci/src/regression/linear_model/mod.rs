pub mod gls;
pub mod iv2sls;
pub mod ivliml;
pub mod ols;
pub mod prais;
pub mod wls;

pub use gls::*;
pub use iv2sls::*;
pub use ivliml::*;
pub use ols::*;
pub use prais::*;
pub use wls::*;

/// Joint test of all coefficients except the intercept, using the fitted covariance.
fn overall_f_test(
    betas: &yss_sci_linalg::Col<f64>,
    covariance: &yss_sci_linalg::Mat<f64>,
    constant: bool,
    df_residual: usize,
    classical_f: Option<f64>,
) -> Result<(f64, f64), String> {
    use statrs::distribution::{ContinuousCDF, FisherSnedecor};
    use yss_sci_linalg::{MatrixExt, Solve};

    let start = usize::from(constant);
    let restrictions = betas.nrows() - start;
    if restrictions == 0 {
        return Ok((0.0, 1.0));
    }
    let f = if let Some(f) = classical_f {
        f.max(0.0)
    } else {
        let beta = betas.subrows(start, restrictions);
        let covariance = covariance.submatrix(start, start, restrictions, restrictions);
        let factor = covariance.checked_cholesky().map_err(|error| {
            format!("Overall Wald test covariance decomposition failed: {error}")
        })?;
        let solution = factor.solve(&beta);
        let wald = beta.transpose() * solution.as_ref();
        if !wald.is_finite() || wald < 0.0 {
            return Err("Overall Wald test produced an invalid statistic".into());
        }
        wald / restrictions as f64
    };
    let distribution = FisherSnedecor::new(restrictions as f64, df_residual as f64)
        .map_err(|error| format!("Overall F distribution: {error}"))?;
    Ok((f, distribution.sf(f)))
}

/// Total variation after whitening, centered along the transformed intercept.
fn transformed_total_ss(
    y: &yss_sci_linalg::Col<f64>,
    intercept: Option<&yss_sci_linalg::Col<f64>>,
) -> f64 {
    match intercept {
        Some(c) => {
            let mean = y.iter().zip(c.iter()).map(|(y, c)| y * c).sum::<f64>()
                / c.iter().map(|c| c * c).sum::<f64>();
            y.iter()
                .zip(c.iter())
                .map(|(y, c)| (y - mean * c).powi(2))
                .sum()
        }
        None => y.iter().map(|y| y * y).sum(),
    }
}
