use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcfPacfRequestDto {
    pub residuals: Vec<f64>,
    pub max_lag: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcfPacfResponseDto {
    pub acf: Vec<f64>,
    pub pacf: Vec<f64>,
    pub n: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerialTestsRequestDto {
    pub residuals: Vec<f64>,
    pub lags: usize,
    #[serde(default)]
    pub exog: Option<Vec<Vec<f64>>>,
    #[serde(default = "default_bg_nomiss0")]
    pub bg_nomiss0: bool,
}

fn default_bg_nomiss0() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct SerialTestWithLagDto {
    pub stat: f64,
    pub p_value: f64,
    pub lags: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DurbinWatsonResultDto {
    pub d: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SerialTestsResponseDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<SerialTestWithLagDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<SerialTestWithLagDto>,
    pub dw: DurbinWatsonResultDto,
}
