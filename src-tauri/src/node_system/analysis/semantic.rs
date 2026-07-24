use super::CompilationBasis;
use crate::node_system::protocol::NodeTypeId;
use crate::node_system::registry::ProtocolFingerprint;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedSemanticGraph<
    GraphRevision,
    NodeId,
    PortAddress,
    ConnectionId,
    ParameterValue,
    ResolvedType,
    ResolvedSchema,
> {
    pub basis: CompilationBasis<GraphRevision>,
    pub nodes: Box<
        [ValidatedSemanticNode<
            NodeId,
            PortAddress,
            ParameterValue,
            ResolvedType,
            ResolvedSchema,
        >],
    >,
    pub dependencies: Box<[SemanticDependency<NodeId, PortAddress, ConnectionId>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedSemanticNode<NodeId, PortAddress, ParameterValue, ResolvedType, ResolvedSchema>
{
    pub node_id: NodeId,
    pub node_type_id: NodeTypeId,
    pub protocol_fingerprint: ProtocolFingerprint,
    pub normalized_parameters: BTreeMap<crate::node_system::protocol::ParameterKey, ParameterValue>,
    pub ports: Box<[ValidatedSemanticPort<PortAddress, ResolvedType, ResolvedSchema>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedSemanticPort<PortAddress, ResolvedType, ResolvedSchema> {
    pub address: PortAddress,
    pub resolved_type: Option<ResolvedType>,
    pub resolved_schema: Option<ResolvedSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticDependency<NodeId, PortAddress, ConnectionId> {
    Value(ValueEdge<PortAddress, ConnectionId>),
    Control(ControlEdge<NodeId, PortAddress, ConnectionId>),
    Effect(EffectDependency<NodeId>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueEdge<PortAddress, ConnectionId> {
    pub connection_id: ConnectionId,
    pub source: PortAddress,
    pub target: PortAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlEdge<NodeId, PortAddress, ConnectionId> {
    pub connection_id: ConnectionId,
    pub source_node: NodeId,
    pub source_port: PortAddress,
    pub target_node: NodeId,
    pub target_port: PortAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectDependency<NodeId> {
    pub predecessor: NodeId,
    pub successor: NodeId,
    pub effect_key: Box<str>,
}
