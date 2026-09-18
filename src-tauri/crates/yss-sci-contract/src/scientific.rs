use crate::regression::OlsOptions;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[derive(Clone, Default)]
pub struct ScientificCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl ScientificCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn from_shared(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }
}

/// Admission control for a synchronous scientific computation.
///
/// Runtime functions sample cancellation and the deadline before dispatch. This
/// contract does not claim cooperative interruption after computation starts.
pub struct ScientificExecutionControl {
    pub cancellation: ScientificCancellationToken,
    pub deadline: Instant,
}

impl ScientificExecutionControl {
    pub fn from_shared(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation: ScientificCancellationToken::from_shared(cancellation),
            deadline,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcfPacfRequest {
    pub values: Vec<f64>,
    pub max_lag: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcfPacfResult {
    pub acf: Vec<f64>,
    pub pacf: Vec<f64>,
    pub n: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OlsRequest {
    pub response: Vec<f64>,
    pub predictors: Vec<Vec<f64>>,
    pub options: OlsOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinearRegressionMethod {
    Ols,
    Wls {
        weights: Vec<f64>,
    },
    /// Relative error covariance, in row-major order and fitted observation order.
    Gls {
        sigma: Vec<Vec<f64>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearRegressionRequest {
    pub response: Vec<f64>,
    pub predictors: Vec<Vec<f64>>,
    pub options: OlsOptions,
    pub method: LinearRegressionMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearRegressionResult {
    pub constant: bool,
    pub coefficients: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    /// Design columns in fitted parameter order; each column retains observation order.
    pub design: Vec<Vec<f64>>,
    pub report: crate::regression::report::LinearRegressionSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScientificInputViolation {
    EmptyInput,
    NonFiniteInput,
    ShapeMismatch,
    ParameterOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScientificComputationError {
    #[error("scientific input is invalid")]
    InvalidInput { violation: ScientificInputViolation },
    #[error("scientific execution was cancelled")]
    Cancelled,
    #[error("scientific execution deadline was exceeded")]
    DeadlineExceeded,
    #[error("scientific computation failed")]
    ComputationFailed,
}

#[cfg(test)]
mod tests;
