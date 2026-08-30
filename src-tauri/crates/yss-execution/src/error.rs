use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RunPhase {
    Admission,
    PlanValidation,
    ResourcePreparation,
    Execution,
    Finalization,
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("execution admission failed")]
    Admission,
    #[error("execution plan is invalid")]
    Plan,
    #[error("execution backend failed")]
    Backend,
    #[error("execution was cancelled")]
    Cancelled { phase: RunPhase },
    #[error("execution deadline was exceeded")]
    Deadline { phase: RunPhase },
}

#[derive(Debug, Error)]
pub enum RunTerminalOutcome<S, C, F> {
    #[error("execution succeeded")]
    Succeeded(S),
    #[error("execution was cancelled")]
    Cancelled(C),
    #[error("execution failed")]
    Failed(F),
}
