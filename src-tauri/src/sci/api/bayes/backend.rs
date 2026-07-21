use std::sync::Arc;

use polars::prelude::DataFrame;

use crate::sci::api::bayes::{BayesModelSpec, InferenceResult, TaskProgress};

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
}

impl BayesBackendError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
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
        }
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
            "BAYES_BACKEND_NOT_CONFIGURED",
            "Bayesian inference backend is not configured.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{BayesBackend, BayesBackendRequest, PlaceholderBayesBackend};
    use crate::sci::api::bayes::{
        BayesModelSpec, DatasetRef, DatasetSourceType, Expression, InferenceConfig, LikelihoodSpec,
        ParameterConstraint, ParameterRef, ParameterSpec, PredictorSource, PredictorSourceKind,
        PriorSpec, ResponseSpec, SamplerAlgorithm,
    };
    use std::collections::BTreeMap;

    fn spec() -> BayesModelSpec {
        BayesModelSpec {
            dataset: DatasetRef {
                source_type: DatasetSourceType::Table,
                source_id: "demo".to_string(),
            },
            response: ResponseSpec {
                expression: Expression::DataVariable {
                    name: "y".to_string(),
                },
                data_variables: BTreeMap::from([("y".to_string(), "response".to_string())]),
            },
            predictor: Expression::Parameter {
                name: "a".to_string(),
            },
            data_variables: BTreeMap::new(),
            likelihood: LikelihoodSpec::Normal {
                mean: PredictorSource {
                    source: PredictorSourceKind::Predictor,
                },
                sigma: ParameterRef {
                    parameter: "sigma".to_string(),
                },
            },
            parameters: vec![
                ParameterSpec {
                    name: "a".to_string(),
                    constraint: ParameterConstraint::Real,
                    prior: PriorSpec::Normal([0.0, 10.0]),
                },
                ParameterSpec {
                    name: "sigma".to_string(),
                    constraint: ParameterConstraint::Positive,
                    prior: PriorSpec::Exponential([1.0]),
                },
            ],
            sampler: InferenceConfig {
                algorithm: SamplerAlgorithm::Nuts,
                chains: 1,
                samples: 10,
                warmup: 5,
                seed: None,
                target_accept: Some(0.8),
                max_tree_depth: Some(10),
                save_samples: false,
            },
            display_formula: "y ~ Normal(a, sigma)".to_string(),
        }
    }

    #[test]
    fn placeholder_backend_returns_configuration_error() {
        let error = PlaceholderBayesBackend
            .fit(BayesBackendRequest::new("task-1", spec(), None))
            .expect_err("placeholder backend must fail");
        assert_eq!(error.code, "BAYES_BACKEND_NOT_CONFIGURED");
    }
}
