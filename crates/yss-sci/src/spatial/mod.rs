//! Spatial weights and Gaussian spatial models, independent of graph identity.
pub mod moran;
pub mod regression;
pub mod weights;
pub(super) use crate::regression::models::common::{Result, failed, finite, invalid, parameter};
pub(super) use yss_sci_contract::execution::{
    ScientificExecutionControl as Control, ScientificInputViolation as Violation,
};

pub(super) fn lag(w: &[Vec<f64>], x: &[f64], control: &Control) -> Result<Vec<f64>> {
    let n = w.len();
    if n == 0 || !x.len().is_multiple_of(n) {
        return Err(invalid(Violation::ShapeMismatch));
    }
    let mut out = Vec::with_capacity(x.len());
    for block in x.chunks_exact(n) {
        for row in w {
            control.check()?;
            out.push(finite(row.iter().zip(block).map(|(a, b)| a * b).sum())?);
        }
    }
    Ok(out)
}
