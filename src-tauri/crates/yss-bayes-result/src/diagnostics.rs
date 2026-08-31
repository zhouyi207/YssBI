//! Structured Bayesian inference diagnostics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterSummary {
    parameter: String,
    mean: f64,
    sd: f64,
    median: f64,
    q025: f64,
    q975: f64,
    rhat: Option<f64>,
    ess_bulk: Option<f64>,
    ess_tail: Option<f64>,
}

impl ParameterSummary {
    pub fn parameter(&self) -> &str {
        &self.parameter
    }

    pub fn mean(&self) -> f64 {
        self.mean
    }

    pub fn sd(&self) -> f64 {
        self.sd
    }

    pub fn median(&self) -> f64 {
        self.median
    }

    pub fn q025(&self) -> f64 {
        self.q025
    }

    pub fn q975(&self) -> f64 {
        self.q975
    }

    pub fn rhat(&self) -> Option<f64> {
        self.rhat
    }

    pub fn ess_bulk(&self) -> Option<f64> {
        self.ess_bulk
    }

    pub fn ess_tail(&self) -> Option<f64> {
        self.ess_tail
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InferenceDiagnostics {
    chains: usize,
    draws_per_chain: usize,
    warmup: usize,
    divergences: Option<usize>,
    max_treedepth_hits: Option<usize>,
    warnings: Vec<DiagnosticWarning>,
}

impl InferenceDiagnostics {
    pub fn chains(&self) -> usize {
        self.chains
    }

    pub fn draws_per_chain(&self) -> usize {
        self.draws_per_chain
    }

    pub fn warmup(&self) -> usize {
        self.warmup
    }

    pub fn divergences(&self) -> Option<usize> {
        self.divergences
    }

    pub fn max_treedepth_hits(&self) -> Option<usize> {
        self.max_treedepth_hits
    }

    pub fn warnings(&self) -> &[DiagnosticWarning] {
        &self.warnings
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticWarning {
    code: String,
    metric: DiagnosticMetric,
    value: f64,
    threshold: f64,
    parameter: String,
}

impl DiagnosticWarning {
    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn metric(&self) -> DiagnosticMetric {
        self.metric
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    pub fn parameter(&self) -> &str {
        &self.parameter
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticMetric {
    Rhat,
    EssBulk,
    EssTail,
}

#[cfg(test)]
mod tests {
    use super::{InferenceDiagnostics, ParameterSummary};

    #[test]
    fn diagnostics_reject_unknown_protocol_fields() {
        assert!(
            serde_json::from_value::<ParameterSummary>(serde_json::json!({
                "parameter": "beta",
                "mean": 1.0,
                "sd": 0.5,
                "median": 1.0,
                "q025": 0.1,
                "q975": 1.9,
                "rhat": 1.0,
                "essBulk": 200.0,
                "essTail": 180.0,
                "legacyLabel": "beta"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<InferenceDiagnostics>(serde_json::json!({
                "chains": 4,
                "drawsPerChain": 1000,
                "warmup": 500,
                "divergences": 0,
                "maxTreedepthHits": 0,
                "warnings": [],
                "backendMessage": "private"
            }))
            .is_err()
        );
    }
}
