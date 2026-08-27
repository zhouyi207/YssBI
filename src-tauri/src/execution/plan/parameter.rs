use std::collections::BTreeMap;
use thiserror::Error;

use super::identity::PlanResourceId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum InvalidPlanParameterId {
    #[error("parameter identity is empty")]
    Empty,
    #[error("parameter identity has surrounding whitespace")]
    SurroundingWhitespace,
    #[error("parameter identity contains a NUL")]
    Nul,
}

fn validate(value: Box<str>) -> Result<Box<str>, InvalidPlanParameterId> {
    if value.is_empty() {
        return Err(InvalidPlanParameterId::Empty);
    }
    if value.trim() != value.as_ref() {
        return Err(InvalidPlanParameterId::SurroundingWhitespace);
    }
    if value.contains('\0') {
        return Err(InvalidPlanParameterId::Nul);
    }
    Ok(value)
}

macro_rules! parameter_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: Box<str>) -> Result<Self, InvalidPlanParameterId> {
                Ok(Self(validate(value)?))
            }

            pub fn from_existing(value: Box<str>) -> Self {
                debug_assert!(validate(value.clone()).is_ok());
                Self(value)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

parameter_id!(PlanParameterSchemaId);
parameter_id!(PlanParameterFieldId);
parameter_id!(CompiledParameterHandle);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanonicalDecimal(f64);

impl CanonicalDecimal {
    pub fn try_new(value: f64) -> Result<Self, CanonicalDecimalError> {
        if !value.is_finite() {
            return Err(CanonicalDecimalError::NonFinite);
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    pub const fn value(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum CanonicalDecimalError {
    #[error("canonical decimal is not finite")]
    NonFinite,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlanParameterScalar {
    Null,
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Decimal(CanonicalDecimal),
    String(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlanParameterValue {
    Scalar(PlanParameterScalar),
    Resource(PlanResourceId),
    List(Box<[PlanParameterValue]>),
    Record(BTreeMap<PlanParameterFieldId, PlanParameterValue>),
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
pub enum CompiledParameterBundleError {
    #[error("compiled parameter handle is duplicated")]
    DuplicateHandle { handle: CompiledParameterHandle },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledParameterBundle {
    basis: super::basis::PlanCompilationBasis,
    entries: BTreeMap<CompiledParameterHandle, PlanParameterPayload>,
}

impl CompiledParameterBundle {
    pub fn basis(&self) -> &super::basis::PlanCompilationBasis {
        &self.basis
    }

    pub fn entries(&self) -> &BTreeMap<CompiledParameterHandle, PlanParameterPayload> {
        &self.entries
    }
}

pub struct CompiledParameterBundleBuilder {
    basis: super::basis::PlanCompilationBasis,
    entries: BTreeMap<CompiledParameterHandle, PlanParameterPayload>,
}

impl CompiledParameterBundleBuilder {
    pub fn new(basis: super::basis::PlanCompilationBasis) -> Self {
        Self {
            basis,
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        handle: CompiledParameterHandle,
        payload: PlanParameterPayload,
    ) -> Result<(), CompiledParameterBundleError> {
        if self.entries.contains_key(&handle) {
            return Err(CompiledParameterBundleError::DuplicateHandle { handle });
        }
        self.entries.insert(handle, payload);
        Ok(())
    }

    pub fn freeze(self) -> CompiledParameterBundle {
        CompiledParameterBundle {
            basis: self.basis,
            entries: self.entries,
        }
    }
}
