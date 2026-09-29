//! Descriptive statistical reports over neutral numeric slices.
use serde::Serialize;
use yss_sci_contract::execution::{ScientificComputationError, ScientificExecutionControl};

#[derive(Debug, Serialize)]
pub struct TheilReport {
    pub theil_t: f64,
    pub form: &'static str,
    /// Number of supplied individuals or groups, including zero-weight groups.
    pub observations: usize,
}

pub fn theil(
    values: &[f64],
    weights: Option<&[f64]>,
    control: &ScientificExecutionControl,
) -> Result<TheilReport, ScientificComputationError> {
    let theil_t = yss_sci::descriptive::theil_t(values, weights, control)?;
    Ok(TheilReport {
        theil_t,
        form: if weights.is_some() {
            "grouped"
        } else {
            "individual"
        },
        observations: values.len(),
    })
}
