use serde::{Deserialize, Serialize};

use super::{ParameterKey, PortKey, TypeConstructorId, TypeId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResolvedType {
    Nominal(TypeId),
    Applied {
        constructor: TypeConstructorId,
        arguments: Box<[ResolvedType]>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeDomain(Box<[ResolvedType]>);

impl TypeDomain {
    pub fn new(types: impl IntoIterator<Item = ResolvedType>) -> Option<Self> {
        let mut types = types.into_iter().collect::<Vec<_>>();
        types.sort();
        types.dedup();
        (!types.is_empty()).then(|| Self(types.into_boxed_slice()))
    }

    pub fn singleton(value: ResolvedType) -> Self {
        Self(Box::new([value]))
    }

    pub fn types(&self) -> &[ResolvedType] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeUnknownReason {
    UnconnectedInput,
    UnresolvedUpstream,
    MissingResource,
    UnsupportedDeclaration,
    OrphanedPort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeConflict {
    InputNotAccepted,
    IncompatibleInputs,
    MissingParameter,
    UnsupportedParameter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeState {
    Exact(ResolvedType),
    Constrained(TypeDomain),
    Unknown(TypeUnknownReason),
    Conflict(TypeConflict),
}

impl TypeState {
    pub fn exact(&self) -> Option<&ResolvedType> {
        match self {
            Self::Exact(value) => Some(value),
            Self::Constrained(_) | Self::Unknown(_) | Self::Conflict(_) => None,
        }
    }

    pub fn domain(&self) -> Option<&[ResolvedType]> {
        match self {
            Self::Exact(value) => Some(std::slice::from_ref(value)),
            Self::Constrained(domain) => Some(domain.types()),
            Self::Unknown(_) | Self::Conflict(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortSelector {
    Declared(PortKey),
    AllInstances(PortKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumericPromotionRule {
    Widen,
    Float64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeRule {
    AnySeriesElseScalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NodeTypingSpec {
    #[default]
    Fixed,
    Identity {
        input: PortKey,
        output: PortKey,
    },
    NumericFold {
        inputs: Box<[PortSelector]>,
        output: PortKey,
        promotion: NumericPromotionRule,
        shape: ShapeRule,
    },
    ShapePreservingFloat {
        input: PortKey,
        output: PortKey,
    },
    ParameterOutput {
        parameter: ParameterKey,
        output: PortKey,
    },
    VariableOutput {
        parameter: ParameterKey,
        output: PortKey,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputCoercionKind {
    WidenInt64ToFloat64,
    BroadcastScalarToSeries,
}
