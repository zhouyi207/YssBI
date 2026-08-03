use super::{ResultSourceId, RunError};
use crate::node_system::analysis::{CompilationBasis, CorrelationContext};
use crate::node_system::document::GraphRevision;
use crate::node_system::plan::GraphOutputRef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub correlation: CorrelationContext,
    pub basis: CompilationBasis<GraphRevision>,
    pub kind: RunEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunEventKind {
    RunStarted,
    RunCompleted,
    RunErrored {
        code: RunErrorCode,
    },
    RunCancelled,
    OperationStarted {
        operation_index: u32,
        activation_id: u64,
    },
    OperationCompleted {
        operation_index: u32,
        activation_id: u64,
    },
    OperationErrored {
        operation_index: u32,
        activation_id: u64,
        code: RunErrorCode,
    },
    ValueReady {
        value_index: u32,
        source_id: ResultSourceId,
    },
    ResultReady {
        name: Box<str>,
        source_id: ResultSourceId,
    },
    OutputReady {
        output: GraphOutputRef,
        source_id: ResultSourceId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunErrorCode {
    InvalidPlan,
    Cancelled,
    KernelNotFound,
    KernelFailed,
    RelationalBackendNotFound,
    RelationalAcquire,
    RelationalFailed,
    MissingRelationalFragment,
    BridgeFailed,
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
        match error {
            RunError::InvalidPlan(_) => Self::InvalidPlan,
            RunError::Cancelled => Self::Cancelled,
            RunError::KernelNotFound(_) => Self::KernelNotFound,
            RunError::KernelFailed { .. } => Self::KernelFailed,
            RunError::RelationalBackendNotFound(_) => Self::RelationalBackendNotFound,
            RunError::RelationalAcquire { .. } => Self::RelationalAcquire,
            RunError::RelationalFailed { .. } => Self::RelationalFailed,
            RunError::MissingRelationalFragment(_) => Self::MissingRelationalFragment,
            RunError::BridgeFailed(_) => Self::BridgeFailed,
            RunError::Stream(_) => Self::Stream,
            RunError::MissingValue(_) => Self::MissingValue,
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
    use crate::node_system::analysis::{CompileId, ProjectSessionId, RunId};
    use crate::node_system::document::GraphResourcePath;
    use crate::node_system::registry::RegistryFingerprint;
    use std::collections::BTreeMap;

    #[test]
    fn events_carry_exact_basis_without_values_or_display_text() {
        let basis = CompilationBasis {
            graph_revision: GraphRevision::new(13),
            registry_fingerprint: RegistryFingerprint::from_bytes([2; 32]),
            resource_versions: BTreeMap::new(),
        };
        let correlation = CorrelationContext {
            project_session_id: ProjectSessionId::new("session"),
            graph_path: GraphResourcePath("events/main".into()),
            graph_revision: basis.graph_revision,
            registry_fingerprint: basis.registry_fingerprint.clone(),
            resource_versions: basis.resource_versions.clone(),
            compile_id: CompileId::new(14),
            selection_digest: None,
            run_id: Some(RunId::new(15)),
            node_id: None,
            node_type_id: None,
            parent_call: None,
        };
        let event = RunEvent {
            correlation,
            basis: basis.clone(),
            kind: RunEventKind::RunErrored {
                code: RunErrorCode::KernelFailed,
            },
        };

        assert_eq!(event.basis, basis);
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("PortRole"));
        assert!(!json.contains("message"));
        assert!(!json.contains("value"));
    }

    #[test]
    fn error_events_reduce_runtime_errors_to_stable_codes() {
        let error = RunError::KernelFailed {
            operation: crate::node_system::plan::OperationIndex::new(3),
            message: "sensitive literal".into(),
        };

        assert_eq!(RunErrorCode::from(&error), RunErrorCode::KernelFailed);
        assert!(
            !serde_json::to_string(&RunErrorCode::from(&error))
                .unwrap()
                .contains("sensitive literal")
        );
    }
}
