use super::{ActivationId, StoredValue};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::plan::{
    GraphOutputRef, PlannedValueContract, ResultPresentation, ValueRef,
};
use crate::node_system::runtime::RunId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResultId(u64);

impl ResultId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationProvenance {
    pub run_id: RunId,
    pub activation_id: ActivationId,
    pub graph_path: GraphResourcePath,
    pub graph_revision: GraphRevision,
    pub node_id: NodeId,
    pub created_at_ms: u64,
    pub usage: ResultUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultProvenance {
    pub run_id: RunId,
    pub activation_id: ActivationId,
    pub graph_path: GraphResourcePath,
    pub graph_revision: GraphRevision,
    pub node_id: NodeId,
    pub output: Option<GraphOutputRef>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultProgress {
    pub completed: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultFailureCause {
    Execution,
    Upstream { upstream_result_id: ResultId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultFailure {
    pub message: Box<str>,
    pub cause: ResultFailureCause,
}

impl ResultFailure {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
            cause: ResultFailureCause::Execution,
        }
    }

    pub fn upstream(upstream_result_id: ResultId, message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
            cause: ResultFailureCause::Upstream { upstream_result_id },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultStateKind {
    Pending,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum ResultState {
    Pending(ResultProgress),
    Ready(StoredValue),
    Failed(Arc<ResultFailure>),
    Cancelled,
}

impl ResultState {
    pub const fn kind(&self) -> ResultStateKind {
        match self {
            Self::Pending(_) => ResultStateKind::Pending,
            Self::Ready(_) => ResultStateKind::Ready,
            Self::Failed(_) => ResultStateKind::Failed,
            Self::Cancelled => ResultStateKind::Cancelled,
        }
    }

    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    pub const fn is_terminal(&self) -> bool {
        !self.is_pending()
    }
}

#[derive(Debug, Clone)]
pub struct StoredResult {
    pub id: ResultId,
    pub provenance: ResultProvenance,
    pub value: ValueRef,
    pub presentation: ResultPresentation,
    pub contract: PlannedValueContract,
    pub state: ResultState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOutputDescriptor {
    pub value: ValueRef,
    pub output: Option<GraphOutputRef>,
    pub presentation: ResultPresentation,
    pub contract: PlannedValueContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationResultGroup {
    pub activation_id: ActivationId,
    pub output_result_ids: Box<[ResultId]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultUsage {
    Produced,
    Reused {
        original_activation_id: ActivationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinResultEntry {
    pub result_id: ResultId,
    pub run_id: RunId,
    pub activation_id: ActivationId,
    pub graph_revision: GraphRevision,
    pub created_at_ms: u64,
    pub usage: ResultUsage,
}
