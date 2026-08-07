use super::relational::RelationalFragment;
use super::{CompileCancellationToken, CompileCancelled};
use crate::node_system::document::{NodeId, PortAddress};
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, KernelHandle, RelationalBackendId,
    RelationalOperatorIndex, ResourceId, ValueRef,
};
use crate::node_system::protocol::{
    EffectSemantics, NodeProtocol, ParameterEditorSpec, ParameterKey, TypeExpr, TypeId,
};
use crate::node_system::registry::{ImplementationKind, LeafImplementation, PreparedNominalValue};
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Registry-owned executable implementation for one node protocol.
pub struct NodeImplementation {
    pub lowerer: Box<dyn NodeLowerer>,
    identity: Box<str>,
}

impl NodeImplementation {
    pub fn new(lowerer: impl NodeLowerer + 'static) -> Self {
        Self {
            identity: std::any::type_name_of_val(&lowerer).into(),
            lowerer: Box::new(lowerer),
        }
    }
}

impl crate::node_system::registry::NodeImplementation for NodeImplementation {
    fn capability(&self) -> ImplementationKind {
        ImplementationKind::CompilerLowering
    }

    fn implementation_identity(&self) -> &str {
        &self.identity
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl From<NodeImplementation> for LeafImplementation {
    fn from(implementation: NodeImplementation) -> Self {
        Self::from_arc(Arc::new(implementation))
    }
}

impl From<Arc<NodeImplementation>> for LeafImplementation {
    fn from(implementation: Arc<NodeImplementation>) -> Self {
        Self::from_arc(implementation)
    }
}

#[derive(Debug, Clone)]
pub enum PreparedParameterValue {
    Null,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(Box<str>),
    Resource(ResourceId),
    Nominal(PreparedNominalValue),
    Collection(Box<[PreparedParameterValue]>),
    Object(BTreeMap<Box<str>, PreparedParameterValue>),
}

/// Immutable information supplied to a leaf node lowerer.
pub struct ValidatedNodeConfig {
    prepared: BTreeMap<ParameterKey, PreparedParameterValue>,
}

impl ValidatedNodeConfig {
    pub(crate) fn empty() -> Self {
        Self {
            prepared: BTreeMap::new(),
        }
    }

    pub(crate) fn from_analysis(
        protocol: &NodeProtocol,
        parameters: BTreeMap<ParameterKey, serde_json::Value>,
        prepare_nominal: impl Fn(
            &TypeId,
            &serde_json::Value,
        ) -> Option<Result<PreparedNominalValue, String>>,
    ) -> Self {
        let mut prepared = BTreeMap::new();
        for spec in protocol.parameters.parameters.iter() {
            let Some(value) = parameters.get(&spec.key) else {
                continue;
            };
            let value = if spec.editor == ParameterEditorSpec::Resource {
                value
                    .as_str()
                    .and_then(|value| ResourceId::new(value).ok())
                    .map(PreparedParameterValue::Resource)
            } else if let TypeExpr::Concrete(type_id) = &spec.value_type {
                match type_id.as_str() {
                    "core.bool" => value.as_bool().map(PreparedParameterValue::Bool),
                    "core.int64" => value.as_i64().map(PreparedParameterValue::Int64),
                    "core.float64" => value.as_f64().map(PreparedParameterValue::Float64),
                    "core.string" => value
                        .as_str()
                        .map(|value| PreparedParameterValue::String(value.into())),
                    _ => prepare_nominal(type_id, value)
                        .and_then(Result::ok)
                        .map(PreparedParameterValue::Nominal)
                        .or_else(|| prepare_json_value(value)),
                }
            } else {
                prepare_json_value(value)
            };
            if let Some(value) = value {
                prepared.insert(spec.key.clone(), value);
            }
        }
        Self { prepared }
    }

    pub fn value(&self, key: &ParameterKey) -> Option<&PreparedParameterValue> {
        self.prepared.get(key)
    }

    pub fn int64(&self, key: &ParameterKey) -> Option<i64> {
        match self.value(key)? {
            PreparedParameterValue::Int64(value) => Some(*value),
            _ => None,
        }
    }

    pub fn string(&self, key: &ParameterKey) -> Option<&str> {
        match self.value(key)? {
            PreparedParameterValue::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn resource(&self, key: &ParameterKey) -> Option<&ResourceId> {
        match self.value(key)? {
            PreparedParameterValue::Resource(value) => Some(value),
            _ => None,
        }
    }

    pub fn nominal<T: Any + Send + Sync>(&self, key: &ParameterKey) -> Option<&T> {
        match self.value(key)? {
            PreparedParameterValue::Nominal(value) => value.downcast_ref(),
            _ => None,
        }
    }

    pub fn collection(&self, key: &ParameterKey) -> Option<&[PreparedParameterValue]> {
        match self.value(key)? {
            PreparedParameterValue::Collection(value) => Some(value),
            _ => None,
        }
    }
}

fn prepare_json_value(value: &serde_json::Value) -> Option<PreparedParameterValue> {
    Some(match value {
        serde_json::Value::Null => PreparedParameterValue::Null,
        serde_json::Value::Bool(value) => PreparedParameterValue::Bool(*value),
        serde_json::Value::Number(value) if value.is_i64() => {
            PreparedParameterValue::Int64(value.as_i64()?)
        }
        serde_json::Value::Number(value) if value.is_u64() => {
            PreparedParameterValue::UInt64(value.as_u64()?)
        }
        serde_json::Value::Number(value) => PreparedParameterValue::Float64(value.as_f64()?),
        serde_json::Value::String(value) => PreparedParameterValue::String(value.as_str().into()),
        serde_json::Value::Array(values) => PreparedParameterValue::Collection(
            values
                .iter()
                .map(prepare_json_value)
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
        ),
        serde_json::Value::Object(values) => PreparedParameterValue::Object(
            values
                .iter()
                .map(|(key, value)| Some((key.as_str().into(), prepare_json_value(value)?)))
                .collect::<Option<BTreeMap<_, _>>>()?,
        ),
    })
}

/// Immutable information supplied to a leaf node lowerer after Analysis has
/// validated and prepared the node configuration.
pub struct LoweringContext<'a> {
    pub cancellation: &'a CompileCancellationToken,
    pub node_id: NodeId,
    pub protocol: &'a NodeProtocol,
    pub parameters: &'a ValidatedNodeConfig,
    pub inputs: &'a [(PortAddress, ValueRef)],
    pub outputs: &'a [(PortAddress, ValueRef)],
}

/// Lowering result for one leaf node.
///
/// Parameters remain common to every backend, while `kernel` explicitly carries
/// scalar, relational, or opaque-kernel output. Relational lowering emits a
/// fragment rather than guessing an `ExecutionPlan`-local subplan index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredNode {
    pub kernel: LoweredKernel,
    pub parameters: CompiledParameterHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredKernel {
    /// Compact form retained for built-in native kernels with no fragment metadata.
    Native(KernelHandle),
    Scalar(ScalarFragment),
    Relational(RelationalNodeFragment),
    Kernel(KernelFragment),
}

impl LoweredKernel {
    pub fn metadata(&self) -> Option<&FragmentMetadata> {
        match self {
            Self::Native(_) => None,
            Self::Scalar(fragment) => Some(&fragment.metadata),
            Self::Relational(fragment) => Some(&fragment.metadata),
            Self::Kernel(fragment) => Some(&fragment.metadata),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFragment {
    pub kernel: KernelHandle,
    pub metadata: FragmentMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelFragment {
    pub kernel: KernelHandle,
    pub metadata: FragmentMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalNodeFragment {
    pub backend: RelationalBackendId,
    pub fragment: RelationalFragment,
    pub inputs: Box<[RelationalInputBinding]>,
    pub metadata: FragmentMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalInputBinding {
    pub port: PortAddress,
    pub operator: RelationalOperatorIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentMetadata {
    pub effect: EffectSemantics,
    pub resources: Box<[CompiledResourceRequirement]>,
    pub results: Box<[FragmentResult]>,
}

impl Default for FragmentMetadata {
    fn default() -> Self {
        Self {
            effect: EffectSemantics::None,
            resources: Box::new([]),
            results: Box::new([]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentResult {
    pub name: Box<str>,
    pub output: PortAddress,
}

pub trait NodeLowerer: Send + Sync {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringInvariant {
    InvalidStaticHandle,
    InvalidPreparedConfiguration,
    MissingMaterializedPort,
    StructuralNodeReachedLeafLowerer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    Cancelled(CompileCancelled),
    InternalInvariant(LoweringInvariant),
    DeadlineExceeded,
    ResourceExhausted,
}

impl LoweringError {
    pub const fn internal(invariant: LoweringInvariant) -> Self {
        Self::InternalInvariant(invariant)
    }
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled(error) => error.fmt(formatter),
            Self::InternalInvariant(invariant) => {
                write!(formatter, "lowering invariant failed: {invariant:?}")
            }
            Self::DeadlineExceeded => formatter.write_str("lowering deadline exceeded"),
            Self::ResourceExhausted => formatter.write_str("lowering resource exhausted"),
        }
    }
}

impl std::error::Error for LoweringError {}
