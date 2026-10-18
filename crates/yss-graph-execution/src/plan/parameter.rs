use std::collections::BTreeMap;
use thiserror::Error;

use super::identity::{PlanParameterHandle, PlanParameterSchemaId, PlanResourceId};

/// Immutable literal trees are prepared once; resources are authorized separately per run.
#[derive(Clone, Debug, PartialEq)]
pub enum PlanParameterValue {
    Literal(std::sync::Arc<yss_node_kernel::RuntimeValue>),
    Resource(PlanResourceId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanParameterPayload {
    schema: PlanParameterSchemaId,
    value: PlanParameterValue,
}

impl PlanParameterPayload {
    pub fn new(schema: PlanParameterSchemaId, value: PlanParameterValue) -> Self {
        Self { schema, value }
    }

    pub fn schema(&self) -> &PlanParameterSchemaId {
        &self.schema
    }

    pub fn value(&self) -> &PlanParameterValue {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum PlanParameterBundleError {
    #[error("prepared parameter handle is duplicated")]
    DuplicateHandle { handle: PlanParameterHandle },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanParameterBundle {
    basis: super::basis::PlanBasis,
    entries: BTreeMap<PlanParameterHandle, PlanParameterPayload>,
}

impl PlanParameterBundle {
    pub fn basis(&self) -> &super::basis::PlanBasis {
        &self.basis
    }

    pub fn entries(&self) -> &BTreeMap<PlanParameterHandle, PlanParameterPayload> {
        &self.entries
    }
}

pub struct PlanParameterBundleBuilder {
    basis: super::basis::PlanBasis,
    entries: BTreeMap<PlanParameterHandle, PlanParameterPayload>,
}

impl PlanParameterBundleBuilder {
    pub fn new(basis: super::basis::PlanBasis) -> Self {
        Self {
            basis,
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        handle: PlanParameterHandle,
        payload: PlanParameterPayload,
    ) -> Result<(), PlanParameterBundleError> {
        if self.entries.contains_key(&handle) {
            return Err(PlanParameterBundleError::DuplicateHandle { handle });
        }
        self.entries.insert(handle, payload);
        Ok(())
    }

    pub fn freeze(self) -> PlanParameterBundle {
        PlanParameterBundle {
            basis: self.basis,
            entries: self.entries,
        }
    }
}
