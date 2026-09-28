pub mod types;
use crate::error::computation_failed;
use yss_sci_contract::causal::iv::InstrumentalVariableKind;
use yss_sci_contract::{SciError, SciOperationCode};

pub fn fit_instrumental_variables(
    kind: InstrumentalVariableKind,
    response: Vec<f64>,
    exogenous: Vec<f64>,
    endogenous: Vec<f64>,
    instruments: Vec<f64>,
) -> Result<serde_json::Value, SciError> {
    let fit = yss_sci::causal::iv::fit::fit_instrumental_variables(
        kind,
        response,
        exogenous,
        endogenous,
        instruments,
    )?;
    serde_json::to_value(fit)
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}
