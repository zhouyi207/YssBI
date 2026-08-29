use std::sync::Arc;

use thiserror::Error;

use crate::execution::plan::PlanGraphRevision;
use crate::execution::plan::ResultCategory;
use crate::execution::value::RuntimeValue;

use super::run_registry::RunId;

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

#[derive(Clone, Debug, PartialEq)]
pub enum StoredResult {
    Runtime(RuntimeValue),
    Scalar(f64),
    Text(Box<str>),
    Empty,
    Categorized {
        value: Box<StoredResult>,
        category: ResultCategory,
    },
}

impl StoredResult {
    pub(crate) fn with_category(value: StoredResult, category: ResultCategory) -> Self {
        Self::Categorized {
            value: Box::new(value),
            category,
        }
    }

    pub fn value(&self) -> &StoredResult {
        match self {
            Self::Categorized { value, .. } => value.value(),
            value => value,
        }
    }

    pub const fn category(&self) -> ResultCategory {
        match self {
            Self::Categorized { category, .. } => *category,
            Self::Runtime(_) | Self::Scalar(_) | Self::Text(_) | Self::Empty => {
                ResultCategory::Value
            }
        }
    }
}

/// Neutral activation identity retained by pin history.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActivationId(u64);

impl ActivationId {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultUsage {
    Produced,
    Reused {
        original_activation_id: ActivationId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinResultEntry {
    result_id: ResultId,
    run_id: RunId,
    activation_id: ActivationId,
    graph_revision: PlanGraphRevision,
    created_at_ms: u64,
    usage: ResultUsage,
}

impl PinResultEntry {
    pub(in crate::execution) fn new(
        result_id: ResultId,
        run_id: RunId,
        activation_id: ActivationId,
        graph_revision: PlanGraphRevision,
        created_at_ms: u64,
        usage: ResultUsage,
    ) -> Self {
        Self {
            result_id,
            run_id,
            activation_id,
            graph_revision,
            created_at_ms,
            usage,
        }
    }

    pub fn result_id(&self) -> ResultId {
        self.result_id
    }

    pub fn run_id(&self) -> RunId {
        self.run_id
    }

    pub fn activation_id(&self) -> ActivationId {
        self.activation_id
    }

    pub fn graph_revision(&self) -> PlanGraphRevision {
        self.graph_revision
    }

    pub const fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    pub const fn usage(&self) -> ResultUsage {
        self.usage
    }
}

/// A result and its pin-history entry captured from one Execution store view.
#[derive(Clone, Debug)]
pub struct PinResultHistorySnapshot {
    entry: PinResultEntry,
    result: Arc<StoredResult>,
}

impl PinResultHistorySnapshot {
    pub(in crate::execution) fn new(entry: PinResultEntry, result: Arc<StoredResult>) -> Self {
        Self { entry, result }
    }

    pub(crate) fn into_parts(self) -> (PinResultEntry, Arc<StoredResult>) {
        (self.entry, self.result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ExecutionResultQueryError {
    #[error("pin history references a missing result")]
    ResultSourceReadFailed { result_id: ResultId },
}
