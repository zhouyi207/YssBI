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

#[derive(Debug, Serialize)]
pub struct HypothesisTestResponseDto {
    pub test_type: String,
    pub h0_form: String,
    pub h1_form: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

impl From<yss_application::hypothesis::HypothesisTestOutput> for HypothesisTestResponseDto {
    fn from(out: yss_application::hypothesis::HypothesisTestOutput) -> Self {
        Self {
            test_type: out.test_type,
            h0_form: out.h0_form,
            h1_form: out.h1_form,
            alternative: out.alternative,
            r_beta_minus_r: out.r_beta_minus_r,
            stat: out.stat,
            df1: out.df1,
            df2: out.df2,
            p_value: out.p_value,
        }
    }
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
