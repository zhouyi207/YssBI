use std::sync::Arc;
use std::time::Instant;

use yss_tabular_contract::TabularSnapshot;

use super::scientific::BackendCancellationToken;
use crate::plan::{ExecutionPlan, PlanResourceId};

#[derive(Clone)]
pub struct RelationalExecutionControl {
    pub cancellation: BackendCancellationToken,
    pub deadline: Instant,
}

#[derive(Clone)]
pub struct RelationalRequest {
    pub plan: Arc<ExecutionPlan>,
    pub subplan_index: usize,
    pub database: PlanResourceId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelationalResult {
    pub rows: TabularSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RelationalError {
    #[error("relational request is invalid")]
    InvalidRequest,
    #[error("relational backend is unavailable")]
    Unavailable,
    #[error("relational execution was cancelled")]
    Cancelled,
    #[error("relational execution deadline was exceeded")]
    DeadlineExceeded,
    #[error("relational computation failed")]
    ComputationFailed,
}

pub trait RelationalBackend: Send + Sync {
    fn execute(
        &self,
        request: RelationalRequest,
        control: &RelationalExecutionControl,
    ) -> Result<RelationalResult, RelationalError>;
}
