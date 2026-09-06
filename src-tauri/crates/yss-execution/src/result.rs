use std::sync::Arc;

use crate::plan::{PlanOutputRef, ResultCategory};
use crate::value::RuntimeValue;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultProvenance {
    result_id: ResultId,
    run_id: RunId,
    created_at_ms: u64,
}

impl ResultProvenance {
    pub(crate) fn produced(result_id: ResultId, run_id: RunId, created_at_ms: u64) -> Self {
        Self {
            result_id,
            run_id,
            created_at_ms,
        }
    }

    pub fn result_id(&self) -> ResultId {
        self.result_id
    }

    pub fn run_id(&self) -> RunId {
        self.run_id
    }

    pub const fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }
}

/// One coherent view of the current output value and its provenance.
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
