//! Numerical panel-model fits.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelFit {
    pub family: &'static str,
    pub coefficients: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub p_values: Vec<f64>,
    pub r2: f64,
    pub adjusted_r2: f64,
    pub observations: usize,
    pub entities: usize,
    pub time_periods: usize,
}
