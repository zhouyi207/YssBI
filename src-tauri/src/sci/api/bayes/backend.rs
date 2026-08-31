use std::sync::Arc;

use polars::prelude::DataFrame;

use yss_bayes_model::BayesModelSpec;
use yss_bayes_result::{InferenceResult, TaskErrorDetails, TaskProgress};

pub type BayesProgressCallback = Arc<dyn Fn(TaskProgress) + Send + Sync>;

pub struct BayesBackendRequest {
    pub task_id: String,
    pub spec: BayesModelSpec,
    pub input_table: Option<DataFrame>,
    pub progress: Option<BayesProgressCallback>,
}

impl BayesBackendRequest {
    pub fn new(
        task_id: impl Into<String>,
        spec: BayesModelSpec,
        input_table: Option<DataFrame>,
    ) -> Self {
        Self::with_progress(task_id, spec, input_table, None)
    }

    pub fn with_progress(
        task_id: impl Into<String>,
        spec: BayesModelSpec,
        input_table: Option<DataFrame>,
        progress: Option<BayesProgressCallback>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            spec,
            input_table,
            progress,
        }
    }
}

pub trait BayesBackend: Send + Sync {
    fn fit(&self, request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError>;

    fn cancel(&self, _task_id: &str) -> Result<(), BayesBackendError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BayesBackendError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub details: Option<TaskErrorDetails>,
}

impl BayesBackendError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
            details: None,
        }
    }

    pub fn with_detail(
        code: impl Into<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: Some(detail.into()),
            details: None,
        }
    }

    pub fn with_safe_details(mut self, details: TaskErrorDetails) -> Self {
        if !details.is_empty() {
            self.details = Some(details);
        }
        self
    }
}

impl std::fmt::Display for BayesBackendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BayesBackendError {}

#[derive(Debug, Default)]
pub struct PlaceholderBayesBackend;

impl BayesBackend for PlaceholderBayesBackend {
    fn fit(&self, _request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
        Err(BayesBackendError::new(
            "bayes_backend_not_configured",
            "Bayesian inference backend is not configured.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{BayesBackend, BayesBackendRequest, PlaceholderBayesBackend};
    use yss_bayes_model::BayesModelSpec;

    fn spec() -> BayesModelSpec {
        serde_json::from_value(serde_json::json!({
            "dataset": { "sourceType": "table", "sourceId": "demo" },
            "response": {
                "expression": { "type": "data_variable", "name": "y" },
                "dataVariables": { "y": "response" }
            },
            "predictor": { "type": "parameter", "name": "a" },
            "dataVariables": {},
            "likelihood": {
                "type": "normal",
                "mean": { "source": "predictor" },
                "sigma": { "parameter": "sigma" }
            },
            "parameters": [
                { "name": "a", "constraint": { "type": "real" }, "prior": { "distribution": "normal", "args": [0.0, 10.0] } },
                { "name": "sigma", "constraint": { "type": "positive" }, "prior": { "distribution": "exponential", "args": [1.0] } }
            ],
            "sampler": {
                "algorithm": "nuts",
                "chains": 1,
                "samples": 10,
                "warmup": 5,
                "seed": null,
                "targetAccept": 0.8,
                "maxTreeDepth": 10,
                "saveSamples": false
            },
            "displayFormula": "y ~ Normal(a, sigma)"
        }))
        .expect("backend test model must deserialize")
    }

    #[test]
    fn placeholder_backend_returns_configuration_error() {
        let error = PlaceholderBayesBackend
            .fit(BayesBackendRequest::new("task-1", spec(), None))
            .expect_err("placeholder backend must fail");
        assert_eq!(error.code, "bayes_backend_not_configured");
    }
}
