pub(super) use crate::regression::models::common::{
    Design, Result, check_iteration, coefficient_table, failed, finite, fitted, hessian, inverse,
    least_squares, minimize, names, normal_log_cdf, parameter, validate,
};
use statrs::distribution::{ChiSquared, ContinuousCDF};
pub(super) use yss_sci_contract::execution::ScientificExecutionControl as Control;
use yss_sci_contract::{regression::models::RegressionCoefficient, survival::*};
pub(super) use yss_sci_linalg::Mat;

pub(super) const Z95: f64 = 1.959963984540054;
pub(super) fn data(time: &[f64], event: &[f64], x: &[Vec<f64>], control: &Control) -> Result<()> {
    validate(time, x, control)?;
    if event.len() != time.len() {
        return Err(crate::regression::models::common::invalid(
            yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
        ));
    }
    if time.iter().any(|&v| v <= 0.0) || event.iter().any(|&v| v != 0.0 && v != 1.0) {
        return Err(parameter());
    }
    Ok(())
}
pub(super) fn groups(group: &[usize], n: usize) -> Result<Vec<usize>> {
    if group.len() != n {
        return Err(parameter());
    }
    Ok(group
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect())
}
pub(super) fn rows(m: &Mat<f64>) -> Vec<Vec<f64>> {
    (0..m.nrows())
        .map(|i| (0..m.ncols()).map(|j| m[(i, j)]).collect())
        .collect()
}
pub(super) fn chi_square(statistic: f64, df: usize) -> Result<SurvivalTest> {
    if df == 0 || !statistic.is_finite() || statistic < 0.0 {
        return Err(failed());
    }
    Ok(SurvivalTest {
        statistic,
        degrees_of_freedom: df,
        p_value: ChiSquared::new(df as f64)
            .map_err(|_| failed())?
            .sf(statistic),
    })
}
pub(super) fn ratios(coefficients: &[RegressionCoefficient]) -> Result<Vec<RatioEstimate>> {
    coefficients
        .iter()
        .map(|c| {
            Ok(RatioEstimate {
                term: c.term.clone(),
                estimate: finite(c.estimate.exp())?,
                confidence_interval: c
                    .confidence_interval
                    .map(|[l, u]| Ok([finite(l.exp())?, finite(u.exp())?]))
                    .transpose()?,
            })
        })
        .collect()
}
pub(super) fn positive_horizon(horizon: f64) -> Result<()> {
    if !horizon.is_finite() || horizon <= 0.0 {
        Err(parameter())
    } else {
        Ok(())
    }
}
