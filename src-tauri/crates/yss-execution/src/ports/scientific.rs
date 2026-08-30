use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use serde_json::Value;

use crate::settings::ExecutionSettings;

#[derive(Clone, Default)]
pub struct BackendCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl BackendCancellationToken {
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

/// Admission control for a synchronous scientific backend call.
///
/// Implementations sample cancellation and the deadline before dispatch. This
/// contract does not claim cooperative interruption after computation starts.
pub struct BackendExecutionControl {
    pub cancellation: BackendCancellationToken,
    pub deadline: Instant,
}

impl BackendExecutionControl {
    pub fn from_shared(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation: BackendCancellationToken::from_shared(cancellation),
            deadline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionRegressionKind {
    Ols,
    Gls,
    Logit,
    Probit,
    Prais,
    Wls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionInstrumentalVariableKind {
    TwoStageLeastSquares,
    LimitedInformationMaximumLikelihood,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExecutionStatisticalTrend {
    None,
    #[default]
    Constant,
    Trend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticsOperation {
    Regression {
        kind: ExecutionRegressionKind,
    },
    InstrumentalVariables {
        kind: ExecutionInstrumentalVariableKind,
    },
    Panel,
    PanelDidTwfe,
    AugmentedDickeyFuller,
    VarFit,
    VarLagOrder,
    VecFit,
    VecRankTest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticsParameters {
    pub weights: Option<Vec<f64>>,
    pub lags: usize,
    pub max_lags: usize,
    pub rank: usize,
    pub trend: ExecutionStatisticalTrend,
}

impl Default for StatisticsParameters {
    fn default() -> Self {
        Self {
            weights: None,
            lags: 1,
            max_lags: 4,
            rank: 1,
            trend: ExecutionStatisticalTrend::Constant,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticsRequest {
    pub operation: StatisticsOperation,
    pub parameters: StatisticsParameters,
    pub inputs: Vec<Vec<f64>>,
    pub settings: ExecutionSettings,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticsResult {
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KernelDensityRequest {
    pub values: Vec<f64>,
    pub grid_points: usize,
    pub min_x: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KdePoint {
    pub x: f64,
    pub density: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KernelDensityResult {
    pub points: Vec<KdePoint>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScientificInputViolation {
    EmptyInput,
    NonFiniteInput,
    ShapeMismatch,
    ParameterOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScientificBackendError {
    #[error("scientific input is invalid")]
    InvalidInput { violation: ScientificInputViolation },
    #[error("scientific execution was cancelled")]
    Cancelled,
    #[error("scientific execution deadline was exceeded")]
    DeadlineExceeded,
    #[error("scientific backend is unavailable")]
    Unavailable,
    #[error("scientific computation failed")]
    ComputationFailed,
}

/// Execution's scientific backend boundary.
///
/// Every method applies [`BackendExecutionControl`] at admission. A concrete
/// backend may add real cooperative checkpoints, but callers cannot infer them
/// from this synchronous port.
pub trait ScientificBackend: Send + Sync {
    fn statistics(
        &self,
        request: StatisticsRequest,
        control: &BackendExecutionControl,
    ) -> Result<StatisticsResult, ScientificBackendError>;

    fn kernel_density(
        &self,
        request: KernelDensityRequest,
        control: &BackendExecutionControl,
    ) -> Result<KernelDensityResult, ScientificBackendError>;

    fn acf_pacf(
        &self,
        request: AcfPacfRequest,
        control: &BackendExecutionControl,
    ) -> Result<AcfPacfResult, ScientificBackendError>;
}

#[cfg(test)]
mod tests;
