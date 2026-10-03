//! Validation and confidence conventions shared by study-level computations.
pub(super) use crate::inference::intervals::critical;
pub(super) use crate::regression::models::common::{Result, failed, finite, parameter};
pub(super) use yss_sci_contract::{execution::ScientificExecutionControl as Control, meta::*};

pub(super) fn aligned(columns: &[&[f64]], control: &Control) -> Result<usize> {
    control.check()?;
    let n = columns.first().map_or(0, |c| c.len());
    if n == 0 {
        return Err(parameter());
    }
    if columns.iter().any(|c| c.len() != n) {
        return Err(
            yss_sci_contract::execution::ScientificComputationError::InvalidInput {
                violation: yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
            },
        );
    }
    for col in columns {
        for (i, v) in col.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if !v.is_finite() {
                return Err(parameter());
            }
        }
    }
    Ok(n)
}
pub(super) fn study_data(y: &[f64], v: &[f64], control: &Control) -> Result<usize> {
    let n = aligned(&[y, v], control)?;
    if v.iter().any(|v| *v <= 0.) {
        return Err(parameter());
    }
    Ok(n)
}
pub(super) fn size(n: f64, minimum: f64) -> Result<()> {
    if n < minimum || n.fract() != 0. {
        return Err(parameter());
    }
    Ok(())
}
pub(super) fn study(i: usize, effect: f64, variance: f64, q: f64) -> Result<StudyEffect> {
    let effect = finite(effect)?;
    let variance = finite(variance)?;
    if variance <= 0. {
        return Err(parameter());
    }
    let standard_error = variance.sqrt();
    Ok(StudyEffect {
        study: i + 1,
        effect,
        variance,
        standard_error,
        lower: finite(effect - q * standard_error)?,
        upper: finite(effect + q * standard_error)?,
    })
}
