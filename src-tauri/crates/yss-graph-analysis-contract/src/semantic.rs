use super::CompilationBasis;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use yss_graph_protocol::NodeTypeId;
use yss_graph_registry::ProtocolFingerprint;

pub type ValidatedSemanticNodeSet<
    NodeId,
    PortAddress,
    ParameterValue,
    ResolvedType,
    ResolvedSchema,
> = Box<[ValidatedSemanticNode<NodeId, PortAddress, ParameterValue, ResolvedType, ResolvedSchema>]>;

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
    pub nodes:
        ValidatedSemanticNodeSet<NodeId, PortAddress, ParameterValue, ResolvedType, ResolvedSchema>,
    pub dependencies: Box<[ValueEdge<PortAddress, ConnectionId>]>,
    #[serde(
        with = "super::snapshot::ordered_map_entries",
        bound(
            serialize = "PortAddress: Serialize",
            deserialize = "PortAddress: Deserialize<'de> + Ord"
        )
    )]
    pub resolved_schemas: BTreeMap<PortAddress, yss_graph_protocol::ResolvedSchemaFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedSemanticNode<NodeId, PortAddress, ParameterValue, ResolvedType, ResolvedSchema>
{
    pub node_id: NodeId,
    pub node_type_id: NodeTypeId,
    pub protocol_fingerprint: ProtocolFingerprint,
    pub normalized_parameters: BTreeMap<yss_graph_protocol::ParameterKey, ParameterValue>,
    pub ports: Box<[ValidatedSemanticPort<PortAddress, ResolvedType, ResolvedSchema>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedSemanticPort<PortAddress, ResolvedType, ResolvedSchema> {
    pub address: PortAddress,
    pub resolved_type: Option<ResolvedType>,
    pub resolved_schema: Option<ResolvedSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueEdge<PortAddress, ConnectionId> {
    pub connection_id: ConnectionId,
    pub source: PortAddress,
    pub target: PortAddress,
}
