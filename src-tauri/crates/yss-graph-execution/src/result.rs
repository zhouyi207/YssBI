use std::collections::BTreeMap;
use std::sync::Arc;

use crate::identity::ExecutionSessionId;
use crate::plan::{PlanNodeId, PlanOutputRef, PlanPortAddress, ResultCategory};
use yss_node_kernel::RuntimeValue;

use super::run_registry::RunId;

pub mod analysis;

/// Opaque identity for an Execution-owned result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResultId(u64);

impl ResultId {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultReference {
    pub execution_session_id: ExecutionSessionId,
    pub result_id: ResultId,
}

/// Application maps Graph semantics and resource versions into this neutral execution input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputResultInputs {
    pub fingerprint: [u8; 32],
    pub bindings: BTreeMap<PlanPortAddress, Box<[PlanOutputRef]>>,
    pub resources: BTreeMap<Box<str>, Option<[u8; 32]>>,
    pub available: bool,
}

impl OutputResultInputs {
    pub(crate) fn sources(&self) -> impl Iterator<Item = &PlanOutputRef> {
        self.bindings.values().flat_map(|sources| sources.iter())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphResultInputs {
    pub semantic_input_hash: [u8; 32],
    pub outputs: BTreeMap<PlanOutputRef, OutputResultInputs>,
    pub observers: BTreeMap<PlanNodeId, OutputResultInputs>,
}

/// A run captures this before preparation. Undo cannot renew an obsolete admission.
#[derive(Clone, Debug)]
pub struct ResultRunBasis {
    pub(crate) graph: Box<str>,
    pub(crate) revision: uuid::Uuid,
    pub(crate) inputs: GraphResultInputs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultCacheState {
    Missing,
    Stale,
    Valid { result_id: ResultId },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConnectionCacheState {
    New,
    Stale,
    Valid,
}

pub struct ConnectionResultState {
    pub output: PlanOutputRef,
    pub input: PlanPortAddress,
    pub state: ConnectionCacheState,
}

pub struct GraphResultCacheState {
    pub outputs: BTreeMap<PlanOutputRef, ResultCacheState>,
    pub connections: Vec<ConnectionResultState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ResultRetentionError {
    #[error("result is no longer available")]
    Unavailable,
    #[error("lease is already bound to another result or owner")]
    LeaseConflict,
    #[error("lease belongs to another owner")]
    WrongOwner,
    #[error("lease owner has closed")]
    OwnerClosed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredResult {
    value: RuntimeValue,
    category: ResultCategory,
    contract: Option<Arc<crate::plan::PlanOutputContract>>,
}

impl StoredResult {
    pub fn new(value: RuntimeValue) -> Self {
        Self {
            value,
            category: ResultCategory::Value,
            contract: None,
        }
    }

    pub(crate) fn with_category(mut self, category: ResultCategory) -> Self {
        self.category = category;
        self
    }

    pub fn value(&self) -> &RuntimeValue {
        &self.value
    }

    pub fn with_output_contract(mut self, contract: crate::plan::PlanOutputContract) -> Self {
        self.category = contract.category;
        self.contract = Some(Arc::new(contract));
        self
    }

    pub fn output_contract(&self) -> Option<&crate::plan::PlanOutputContract> {
        self.contract.as_deref()
    }

    pub const fn category(&self) -> ResultCategory {
        self.category
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultProvenance {
    execution_session_id: ExecutionSessionId,
    result_id: ResultId,
    run_id: RunId,
    created_at_ms: u64,
}

impl ResultProvenance {
    pub(crate) fn produced(
        execution_session_id: ExecutionSessionId,
        result_id: ResultId,
        run_id: RunId,
        created_at_ms: u64,
    ) -> Self {
        Self {
            execution_session_id,
            result_id,
            run_id,
            created_at_ms,
        }
    }

    pub fn result_id(&self) -> ResultId {
        self.result_id
    }

    pub fn reference(&self) -> ResultReference {
        ResultReference {
            execution_session_id: self.execution_session_id,
            result_id: self.result_id,
        }
    }

    pub fn run_id(&self) -> RunId {
        self.run_id
    }

    pub const fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }
}

/// One immutable result read view, retained by its output or an explicit report lease.
#[derive(Clone, Debug)]
pub struct StoredResultSnapshot {
    value: Arc<StoredResult>,
    output: PlanOutputRef,
    provenance: ResultProvenance,
}

impl StoredResultSnapshot {
    pub(crate) fn new(
        value: Arc<StoredResult>,
        output: PlanOutputRef,
        provenance: ResultProvenance,
    ) -> Self {
        Self {
            value,
            output,
            provenance,
        }
    }

    pub fn value(&self) -> &Arc<StoredResult> {
        &self.value
    }

    pub fn output(&self) -> &PlanOutputRef {
        &self.output
    }

    pub fn provenance(&self) -> &ResultProvenance {
        &self.provenance
    }
}
