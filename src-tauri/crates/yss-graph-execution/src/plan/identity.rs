use std::fmt;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum InvalidPlanIdentity {
    #[error("plan identity is empty")]
    Empty,
    #[error("plan identity has surrounding whitespace")]
    SurroundingWhitespace,
    #[error("plan identity contains a NUL")]
    Nul,
}

fn validate(value: Box<str>) -> Result<Box<str>, InvalidPlanIdentity> {
    if value.is_empty() {
        return Err(InvalidPlanIdentity::Empty);
    }
    if value.trim() != value.as_ref() {
        return Err(InvalidPlanIdentity::SurroundingWhitespace);
    }
    if value.contains('\0') {
        return Err(InvalidPlanIdentity::Nul);
    }
    Ok(value)
}

macro_rules! plan_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: Box<str>) -> Result<Self, InvalidPlanIdentity> {
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

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

plan_id!(PlanProjectSessionId);
plan_id!(PlanGraphId);
plan_id!(PlanNodeId);
plan_id!(PlanNodeTypeId);
plan_id!(PlanPortAddress);
plan_id!(PlanInputGroupId);
plan_id!(PlanFunctionParameterId);
plan_id!(PlanResourceId);
plan_id!(PlanResourceVersion);
plan_id!(KernelId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlanCompileId(u64);

impl PlanCompileId {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlanRegistryFingerprint([u8; 32]);

impl PlanRegistryFingerprint {
    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanSourceIdentity {
    graph: PlanGraphId,
    node: Option<PlanNodeId>,
    port: Option<PlanPortAddress>,
}

impl PlanSourceIdentity {
    pub fn new(
        graph: PlanGraphId,
        node: Option<PlanNodeId>,
        port: Option<PlanPortAddress>,
    ) -> Self {
        Self { graph, node, port }
    }

    pub fn graph(&self) -> &PlanGraphId {
        &self.graph
    }

    pub fn node(&self) -> Option<&PlanNodeId> {
        self.node.as_ref()
    }

    pub fn port(&self) -> Option<&PlanPortAddress> {
        self.port.as_ref()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlanOutputRef {
    graph: PlanGraphId,
    port: PlanPortAddress,
}

impl PlanOutputRef {
    pub fn new(graph: PlanGraphId, port: PlanPortAddress) -> Self {
        Self { graph, port }
    }

    pub fn graph(&self) -> &PlanGraphId {
        &self.graph
    }

    pub fn port(&self) -> &PlanPortAddress {
        &self.port
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanProvenance {
    source: PlanSourceIdentity,
    basis: super::basis::PlanCompilationBasis,
    compile_id: PlanCompileId,
}

impl PlanProvenance {
    pub fn new(
        source: PlanSourceIdentity,
        basis: super::basis::PlanCompilationBasis,
        compile_id: PlanCompileId,
    ) -> Self {
        Self {
            source,
            basis,
            compile_id,
        }
    }

    pub fn source(&self) -> &PlanSourceIdentity {
        &self.source
    }

    pub fn basis(&self) -> &super::basis::PlanCompilationBasis {
        &self.basis
    }

    pub fn compile_id(&self) -> PlanCompileId {
        self.compile_id
    }
}
