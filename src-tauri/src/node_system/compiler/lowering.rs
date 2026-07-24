use super::CompileCancellationToken;
use super::relational::RelationalFragment;
use crate::node_system::document::{NodeId, PortAddress};
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, KernelHandle, RelationalBackendId,
    RelationalOperatorIndex, ValueRef,
};
use crate::node_system::protocol::{EffectSemantics, NodeProtocol};
use crate::node_system::registry::{ImplementationKind, LeafImplementation};
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

/// Immutable information supplied to a leaf node lowerer.
pub struct LoweringContext<'a> {
    pub cancellation: &'a CompileCancellationToken,
    pub node_id: NodeId,
    pub protocol: &'a NodeProtocol,
    pub parameters: &'a BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringError {
    pub message: Box<str>,
}

impl LoweringError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LoweringError {}
