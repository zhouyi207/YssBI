use super::run_output::RunOutputMessage;
use super::{RelationalErrorCode, ResultId, RunError, RunId, RunPhase};
use crate::execution::plan::legacy::GraphOutputRef;
use crate::graph_document::GraphResourcePath;
use crate::project::ProjectSessionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRunIdentity {
    pub project_session_id: ProjectSessionId,
    pub graph_path: GraphResourcePath,
    pub run_id: RunId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub run: GraphRunIdentity,
    pub kind: RunEventKind,
}

macro_rules! define_run_event_kind {
    ($($variant:ident $({ $($field:ident: $field_type:ty),* $(,)? })?),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "type", rename_all = "camelCase")]
        pub enum RunEventKind {
            $($variant $({ $($field: $field_type),* })?),*
        }

        #[cfg(test)]
        pub(crate) const RUN_EVENT_KIND_VARIANT_COUNT: usize =
            [$(stringify!($variant)),*].len();
    };
}

define_run_event_kind! {
    RunStarted,
    RunCompleted,
    RunErrored { outcome: RunErrorOutcome },
    RunCancelled,
    PinPreviewResultReady {
        output: GraphOutputRef,
        generation: u64,
        result_id: ResultId,
    },
    OpenResultWindow { result_id: ResultId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrdinaryRunErrorCode {
    InvalidPlan,
    Cancelled,
    ActivationIdExhausted,
    RuntimeIdExhausted,

    KernelNotFound,
    KernelFailed,
    RelationalBackendNotFound,
    RelationalOperatorInvalid,
    RelationalColumnMissing,
    RelationalTypeMismatch,
    RelationalInputShapeInvalid,
    RelationalHintInvalid,
    Stream,
    MissingValue,
    InvalidCondition,
    OutputCount,
    OperationAlreadyExecuted,
    UnsatisfiedEffectDependency,
    LoopLimitExceeded,
    FunctionPlanNotFound,
    FunctionPlanFailed,
    RecursionLimitExceeded,
    ProjectDraining,
    ResourceSnapshotMismatch,
    ResourceAcquire,
}

impl OrdinaryRunErrorCode {
    pub const fn public_message(self) -> &'static str {
        match self {
            Self::InvalidPlan => "execution plan is invalid",
            Self::Cancelled => "run was cancelled",
            Self::ActivationIdExhausted => "activation identity space is exhausted",
            Self::RuntimeIdExhausted => "runtime identity space is exhausted",

            Self::KernelNotFound => "required kernel is unavailable",
            Self::KernelFailed => "operation failed",
            Self::RelationalBackendNotFound => "relational backend is unavailable",
            Self::RelationalOperatorInvalid => "relational operator is invalid",
            Self::RelationalColumnMissing => "relational column is missing",
            Self::RelationalTypeMismatch => "relational types do not match",
            Self::RelationalInputShapeInvalid => "relational input shape is invalid",
            Self::RelationalHintInvalid => "relational pushdown metadata is invalid",
            Self::Stream => "runtime stream failed",
            Self::MissingValue => "runtime value is unavailable",
            Self::InvalidCondition => "runtime condition is invalid",
            Self::OutputCount => "operation returned an invalid output count",
            Self::OperationAlreadyExecuted => "operation executed more than once",
            Self::UnsatisfiedEffectDependency => "effect dependency is unsatisfied",
            Self::LoopLimitExceeded => "loop iteration limit exceeded",
            Self::FunctionPlanNotFound => "function plan is unavailable",
            Self::FunctionPlanFailed => "function plan failed",
            Self::RecursionLimitExceeded => "call recursion limit exceeded",
            Self::ProjectDraining => "project is draining",
            Self::ResourceSnapshotMismatch => "project resource snapshot changed",
            Self::ResourceAcquire => "run resource is unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunErrorCode {
    InvalidPlan,
    Cancelled,
    ActivationIdExhausted,
    RuntimeIdExhausted,
    DeadlineExceeded,
    KernelNotFound,
    KernelFailed,
    RelationalBackendNotFound,
    RelationalOperatorInvalid,
    RelationalColumnMissing,
    RelationalTypeMismatch,
    RelationalInputShapeInvalid,
    RelationalHintInvalid,
    Stream,
    MissingValue,
    InvalidCondition,
    OutputCount,
    OperationAlreadyExecuted,
    UnsatisfiedEffectDependency,
    LoopLimitExceeded,
    FunctionPlanNotFound,
    FunctionPlanFailed,
    RecursionLimitExceeded,
    ProjectDraining,
    ResourceSnapshotMismatch,
    ResourceAcquire,
}

impl From<&RunError> for RunErrorCode {
    fn from(error: &RunError) -> Self {
        match RunErrorOutcome::from(error) {
            RunErrorOutcome::DeadlineExceeded { .. } => Self::DeadlineExceeded,
            RunErrorOutcome::Ordinary { code } => match code {
                OrdinaryRunErrorCode::InvalidPlan => Self::InvalidPlan,
                OrdinaryRunErrorCode::Cancelled => Self::Cancelled,
                OrdinaryRunErrorCode::ActivationIdExhausted => Self::ActivationIdExhausted,
                OrdinaryRunErrorCode::RuntimeIdExhausted => Self::RuntimeIdExhausted,
                OrdinaryRunErrorCode::KernelNotFound => Self::KernelNotFound,
                OrdinaryRunErrorCode::KernelFailed => Self::KernelFailed,
                OrdinaryRunErrorCode::RelationalBackendNotFound => Self::RelationalBackendNotFound,
                OrdinaryRunErrorCode::RelationalOperatorInvalid => Self::RelationalOperatorInvalid,
                OrdinaryRunErrorCode::RelationalColumnMissing => Self::RelationalColumnMissing,
                OrdinaryRunErrorCode::RelationalTypeMismatch => Self::RelationalTypeMismatch,
                OrdinaryRunErrorCode::RelationalInputShapeInvalid => {
                    Self::RelationalInputShapeInvalid
                }
                OrdinaryRunErrorCode::RelationalHintInvalid => Self::RelationalHintInvalid,
                OrdinaryRunErrorCode::Stream => Self::Stream,
                OrdinaryRunErrorCode::MissingValue => Self::MissingValue,
                OrdinaryRunErrorCode::InvalidCondition => Self::InvalidCondition,
                OrdinaryRunErrorCode::OutputCount => Self::OutputCount,
                OrdinaryRunErrorCode::OperationAlreadyExecuted => Self::OperationAlreadyExecuted,
                OrdinaryRunErrorCode::UnsatisfiedEffectDependency => {
                    Self::UnsatisfiedEffectDependency
                }
                OrdinaryRunErrorCode::LoopLimitExceeded => Self::LoopLimitExceeded,
                OrdinaryRunErrorCode::FunctionPlanNotFound => Self::FunctionPlanNotFound,
                OrdinaryRunErrorCode::FunctionPlanFailed => Self::FunctionPlanFailed,
                OrdinaryRunErrorCode::RecursionLimitExceeded => Self::RecursionLimitExceeded,
                OrdinaryRunErrorCode::ProjectDraining => Self::ProjectDraining,
                OrdinaryRunErrorCode::ResourceSnapshotMismatch => Self::ResourceSnapshotMismatch,
                OrdinaryRunErrorCode::ResourceAcquire => Self::ResourceAcquire,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunErrorOutcome {
    Ordinary { code: OrdinaryRunErrorCode },
    DeadlineExceeded { phase: RunPhase },
}

impl RunErrorOutcome {
    pub const fn code(self) -> RunErrorCode {
        match self {
            Self::DeadlineExceeded { .. } => RunErrorCode::DeadlineExceeded,
            Self::Ordinary { code } => match code {
                OrdinaryRunErrorCode::InvalidPlan => RunErrorCode::InvalidPlan,
                OrdinaryRunErrorCode::Cancelled => RunErrorCode::Cancelled,
                OrdinaryRunErrorCode::ActivationIdExhausted => RunErrorCode::ActivationIdExhausted,
                OrdinaryRunErrorCode::RuntimeIdExhausted => RunErrorCode::RuntimeIdExhausted,
                OrdinaryRunErrorCode::KernelNotFound => RunErrorCode::KernelNotFound,
                OrdinaryRunErrorCode::KernelFailed => RunErrorCode::KernelFailed,
                OrdinaryRunErrorCode::RelationalBackendNotFound => {
                    RunErrorCode::RelationalBackendNotFound
                }
                OrdinaryRunErrorCode::RelationalOperatorInvalid => {
                    RunErrorCode::RelationalOperatorInvalid
                }
                OrdinaryRunErrorCode::RelationalColumnMissing => {
                    RunErrorCode::RelationalColumnMissing
                }
                OrdinaryRunErrorCode::RelationalTypeMismatch => {
                    RunErrorCode::RelationalTypeMismatch
                }
                OrdinaryRunErrorCode::RelationalInputShapeInvalid => {
                    RunErrorCode::RelationalInputShapeInvalid
                }
                OrdinaryRunErrorCode::RelationalHintInvalid => RunErrorCode::RelationalHintInvalid,
                OrdinaryRunErrorCode::Stream => RunErrorCode::Stream,
                OrdinaryRunErrorCode::MissingValue => RunErrorCode::MissingValue,
                OrdinaryRunErrorCode::InvalidCondition => RunErrorCode::InvalidCondition,
                OrdinaryRunErrorCode::OutputCount => RunErrorCode::OutputCount,
                OrdinaryRunErrorCode::OperationAlreadyExecuted => {
                    RunErrorCode::OperationAlreadyExecuted
                }
                OrdinaryRunErrorCode::UnsatisfiedEffectDependency => {
                    RunErrorCode::UnsatisfiedEffectDependency
                }
                OrdinaryRunErrorCode::LoopLimitExceeded => RunErrorCode::LoopLimitExceeded,
                OrdinaryRunErrorCode::FunctionPlanNotFound => RunErrorCode::FunctionPlanNotFound,
                OrdinaryRunErrorCode::FunctionPlanFailed => RunErrorCode::FunctionPlanFailed,
                OrdinaryRunErrorCode::RecursionLimitExceeded => {
                    RunErrorCode::RecursionLimitExceeded
                }
                OrdinaryRunErrorCode::ProjectDraining => RunErrorCode::ProjectDraining,
                OrdinaryRunErrorCode::ResourceSnapshotMismatch => {
                    RunErrorCode::ResourceSnapshotMismatch
                }
                OrdinaryRunErrorCode::ResourceAcquire => RunErrorCode::ResourceAcquire,
            },
        }
    }
}

impl From<&RunError> for RunErrorOutcome {
    fn from(error: &RunError) -> Self {
        match error {
            RunError::DeadlineExceeded { phase } => Self::DeadlineExceeded { phase: *phase },
            error => Self::Ordinary {
                code: OrdinaryRunErrorCode::from(error),
            },
        }
    }
}

impl From<&RunError> for OrdinaryRunErrorCode {
    fn from(error: &RunError) -> Self {
        match error {
            RunError::InvalidPlan(_) => Self::InvalidPlan,
            RunError::MemoizationRetry => {
                unreachable!("memoization retry is internal to scheduling")
            }
            RunError::Cancelled => Self::Cancelled,
            RunError::ActivationIdExhausted => Self::ActivationIdExhausted,
            RunError::RuntimeIdExhausted => Self::RuntimeIdExhausted,
            RunError::DeadlineExceeded { .. } => {
                unreachable!("deadline errors use RunErrorOutcome::DeadlineExceeded")
            }
            RunError::KernelNotFound(_) => Self::KernelNotFound,
            RunError::KernelFailed { .. } => Self::KernelFailed,
            RunError::RelationalBackendNotFound(_) => Self::RelationalBackendNotFound,
            RunError::RelationalAcquire { code, .. } | RunError::RelationalFailed { code, .. } => {
                match code {
                    RelationalErrorCode::OperatorInvalid => Self::RelationalOperatorInvalid,
                    RelationalErrorCode::ColumnMissing => Self::RelationalColumnMissing,
                    RelationalErrorCode::TypeMismatch => Self::RelationalTypeMismatch,
                    RelationalErrorCode::InputShapeInvalid => Self::RelationalInputShapeInvalid,
                    RelationalErrorCode::HintInvalid => Self::RelationalHintInvalid,
                    RelationalErrorCode::Cancelled => Self::Cancelled,
                    RelationalErrorCode::DeadlineExceeded => {
                        unreachable!("deadline relational errors map to RunError::DeadlineExceeded")
                    }
                }
            }
            RunError::Stream(_) => Self::Stream,
            RunError::MissingValue(_) | RunError::UpstreamResultFailed { .. } => Self::MissingValue,
            RunError::UpstreamResultCancelled { .. } => Self::Cancelled,
            RunError::InvalidCondition { .. } => Self::InvalidCondition,
            RunError::OutputCount { .. } => Self::OutputCount,
            RunError::OperationAlreadyExecuted { .. } => Self::OperationAlreadyExecuted,
            RunError::UnsatisfiedEffectDependency { .. } => Self::UnsatisfiedEffectDependency,
            RunError::LoopLimitExceeded { .. } => Self::LoopLimitExceeded,
            RunError::FunctionPlanNotFound(_) => Self::FunctionPlanNotFound,
            RunError::FunctionPlanFailed(_) => Self::FunctionPlanFailed,
            RunError::RecursionLimitExceeded { .. } => Self::RecursionLimitExceeded,
            RunError::ProjectDraining(_) => Self::ProjectDraining,
            RunError::ResourceSnapshotMismatch(_) => Self::ResourceSnapshotMismatch,
            RunError::ResourceAcquire { .. } => Self::ResourceAcquire,
        }
    }
}

pub trait RunEventSink: Send + Sync {
    fn record(&self, event: RunEvent);

    fn record_run_output(&self, _: RunOutputMessage) {}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopRunEventSink;

impl RunEventSink for NoopRunEventSink {
    fn record(&self, _: RunEvent) {}
}

pub static NOOP_RUN_EVENT_SINK: NoopRunEventSink = NoopRunEventSink;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_document::GraphResourcePath;
    use crate::node_system::runtime::RunId;
    use crate::project::ProjectSessionId;

    #[test]
    fn events_carry_only_graph_run_identity_and_kind() {
        let event = RunEvent {
            run: GraphRunIdentity {
                project_session_id: ProjectSessionId::new("session"),
                graph_path: GraphResourcePath::new("events/main.yssbi-event").unwrap(),
                run_id: RunId::new(15),
            },
            kind: RunEventKind::RunStarted,
        };

        let json = serde_json::to_value(event).unwrap();
        let object = json.as_object().unwrap();
        assert_eq!(object.len(), 2);
        assert!(object.contains_key("run"));
        assert!(object.contains_key("kind"));
        assert_eq!(RUN_EVENT_KIND_VARIANT_COUNT, 6);
    }

    #[test]
    fn error_events_reduce_runtime_errors_to_stable_codes() {
        let error = RunError::KernelFailed {
            operation: crate::execution::plan::legacy::OperationIndex::new(3),
            kind: crate::node_system::runtime::KernelErrorKind::Permanent,
            message: "sensitive literal".into(),
        };
        let relational = RunError::RelationalFailed {
            operation: crate::execution::plan::legacy::OperationIndex::new(4),
            code: RelationalErrorCode::TypeMismatch,
            message: "sensitive backend detail".into(),
        };

        assert_eq!(RunErrorCode::from(&error), RunErrorCode::KernelFailed);
        assert_eq!(
            RunErrorCode::from(&relational),
            RunErrorCode::RelationalTypeMismatch
        );
        let wire = serde_json::to_string(&RunErrorCode::from(&relational)).unwrap();
        assert_eq!(wire, "\"relationalTypeMismatch\"");
        assert!(!wire.contains("sensitive backend detail"));
        assert!(serde_json::from_str::<RunErrorCode>("\"relationalAcquire\"").is_err());
        assert!(serde_json::from_str::<RunErrorCode>("\"relationalFailed\"").is_err());
    }
}
