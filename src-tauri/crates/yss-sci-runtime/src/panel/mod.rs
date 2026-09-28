//! Panel estimation and report records.
pub mod types;
use crate::error::computation_failed;
use yss_sci_contract::{SciError, SciOperationCode};

pub fn fit_panel(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
) -> Result<serde_json::Value, SciError> {
    let fit = yss_sci::panel::fit::fit_panel(response, predictors, entity, time)?;
    serde_json::to_value(fit).map_err(|_| computation_failed(SciOperationCode::Panel))
}
