//! Instrumental-variable estimator selection and numerical fit.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentalVariableKind {
    TwoStageLeastSquares,
    LimitedInformationMaximumLikelihood,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentalVariableFit {
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub p_values: Vec<f64>,
    pub r2: f64,
    pub adjusted_r2: f64,
    pub first_stage_min_eigenvalue: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kappa: Option<f64>,
}
