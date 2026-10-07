//! Descriptive statistical reports over neutral numeric slices.
use serde::Serialize;
use yss_sci_contract::descriptive::{DagumResult, GiniResult};
use yss_sci_contract::execution::{ScientificComputationError, ScientificExecutionControl};

pub fn gini(
    values: &[f64],
    control: &ScientificExecutionControl,
) -> Result<GiniResult, ScientificComputationError> {
    yss_sci::descriptive::gini(values, control)
}

pub fn dagum_gini(
    values: &[f64],
    groups: &[usize],
    control: &ScientificExecutionControl,
) -> Result<DagumResult, ScientificComputationError> {
    yss_sci::descriptive::dagum_gini(values, groups, control)
}

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
