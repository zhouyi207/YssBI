pub(super) use crate::distribution::normal::log_cdf as normal_log_cdf;
pub(super) use crate::regression::models::common::{
    Design, Result, check_iteration, coefficient_table, failed, finite, fitted, hessian, invalid,
    inverse, least_squares, minimize, names, parameter, validate,
};
use statrs::distribution::{ChiSquared, ContinuousCDF};
pub(super) use yss_sci_contract::execution::ScientificExecutionControl as Control;
pub(super) use yss_sci_contract::execution::ScientificInputViolation as Violation;
use yss_sci_contract::{regression::models::RegressionCoefficient, survival::*};
pub(super) use yss_sci_linalg::Mat;

pub(super) const Z95: f64 = 1.959963984540054;
pub(super) fn data(time: &[f64], event: &[f64], x: &[Vec<f64>], control: &Control) -> Result<()> {
    validate(time, x, control)?;
    binary(event, time.len(), control)?;
    for (i, &value) in time.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        if value <= 0.0 {
            return Err(invalid(Violation::DataOutOfRange));
        }
    }
    Ok(())
}
pub(super) fn binary(values: &[f64], n: usize, control: &Control) -> Result<()> {
    if values.len() != n {
        return Err(invalid(Violation::ShapeMismatch));
    }
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        if !value.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if value != 0.0 && value != 1.0 {
            return Err(invalid(Violation::DataOutOfRange));
        }
    }
    Ok(())
}
pub(super) fn intervals(start: &[f64], stop: &[f64], control: &Control) -> Result<()> {
    if start.len() != stop.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    for (i, (&start, &stop)) in start.iter().zip(stop).enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        if !start.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if start < 0.0 || start >= stop {
            return Err(invalid(Violation::DataOutOfRange));
        }
    }
    Ok(())
}
pub(super) fn group_rows(
    group: &[usize],
    n: usize,
    control: &Control,
) -> Result<std::collections::BTreeMap<usize, Vec<usize>>> {
    if group.len() != n {
        return Err(invalid(Violation::ShapeMismatch));
    }
    let mut rows = std::collections::BTreeMap::<_, Vec<_>>::new();
    for (i, &value) in group.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        rows.entry(value).or_default().push(i);
    }
    Ok(rows)
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
