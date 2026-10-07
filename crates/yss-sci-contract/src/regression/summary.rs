//! Contents and analysis parameters for a summary of an already fitted linear model.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinearSummaryOptions {
    pub equation: bool,
    pub model_summary: bool,
    pub anova: bool,
    pub coefficient_table: bool,
    pub coefficient_chart: bool,
    pub diagnostics: bool,
    pub residual_plot: bool,
    pub observations: bool,
    pub acf_pacf: bool,
    pub acf_max_lag: usize,
    pub serial_tests: bool,
    pub serial_lags: usize,
    pub bg_nomiss0: bool,
    pub hypothesis_test: bool,
    pub hypothesis: String,
}

impl Default for LinearSummaryOptions {
    fn default() -> Self {
        Self {
            equation: true,
            model_summary: true,
            anova: true,
            coefficient_table: true,
            coefficient_chart: false,
            diagnostics: false,
            residual_plot: false,
            observations: false,
            acf_pacf: false,
            acf_max_lag: 1,
            serial_tests: false,
            serial_lags: 1,
            bg_nomiss0: true,
            hypothesis_test: false,
            hypothesis: "x1 = 0".into(),
        }
    }
}
