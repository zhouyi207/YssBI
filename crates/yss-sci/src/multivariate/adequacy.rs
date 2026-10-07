//! Correlation-matrix diagnostics shared by standalone validity screening and factor extraction.
use super::common::*;
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_contract::{execution::*, multivariate::FactorabilityReport};
use yss_sci_linalg::{Mat, MatrixExt, Solve};
pub fn sampling_adequacy(
    columns: &[Vec<f64>],
    control: &ScientificExecutionControl,
) -> Result<FactorabilityReport> {
    let prepared = prepare(columns, true, control)?;
    let (n, p) = (prepared.matrix.nrows(), columns.len());
    if p < 2 || n <= p {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let correlation = covariance(&prepared.matrix);
    full_rank(&correlation, control)?;
    let factor = correlation
        .checked_cholesky()
        .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?;
    let inverse = factor.solve(&Mat::identity(p, p));
    let lower = factor.lower();
    let log_determinant = (0..p).map(|i| 2. * lower[(i, i)].ln()).sum();
    assess(&correlation, &inverse, n, log_determinant, control)
}
pub(super) fn assess(
    correlation: &Mat<f64>,
    inverse: &Mat<f64>,
    n: usize,
    log_determinant: f64,
    control: &ScientificExecutionControl,
) -> Result<FactorabilityReport> {
    let p = correlation.nrows();
    let mut correlations = vec![0.; p];
    let mut partials = vec![0.; p];
    for i in 0..p {
        control.check()?;
        for j in 0..i {
            let c = correlation[(i, j)].powi(2);
            let partial = inverse[(i, j)] / inverse[(i, i)].sqrt() / inverse[(j, j)].sqrt();
            let q = finite(partial.powi(2))?;
            correlations[i] += c;
            correlations[j] += c;
            partials[i] += q;
            partials[j] += q;
        }
    }
    let ratio = |c: f64, q: f64| if c + q > 0. { Some(c / (c + q)) } else { None };
    let kmo = ratio(correlations.iter().sum(), partials.iter().sum());
    let item_msa = correlations
        .iter()
        .zip(partials)
        .map(|(&c, q)| ratio(c, q))
        .collect();
    let bartlett_chi_square =
        finite(-((n - 1) as f64 - (2. * p as f64 + 5.) / 6.) * log_determinant)?.max(0.);
    let bartlett_df = p.checked_mul(p - 1).ok_or_else(failed)? / 2;
    let bartlett_p_value = ChiSquared::new(bartlett_df as f64)
        .map_err(|_| failed())?
        .sf(bartlett_chi_square);
    Ok(FactorabilityReport {
        observations: n,
        variables: p,
        kmo,
        item_msa,
        bartlett_chi_square,
        bartlett_df,
        bartlett_p_value: finite(bartlett_p_value)?,
    })
}
