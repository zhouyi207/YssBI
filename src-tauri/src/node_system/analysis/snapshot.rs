use super::{CompilationBasis, DiagnosticSeverity, NodeDiagnostic, ValidatedSemanticGraph};
use crate::node_system::protocol::{NodeTypeId, ParameterKey, PortDirection, PortKey, PortKind};
use crate::node_system::registry::ProtocolFingerprint;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type TypeFacts<PortAddress, TypeFact> = BTreeMap<PortAddress, TypeFact>;
pub type SchemaFacts<PortAddress, SchemaFact> = BTreeMap<PortAddress, SchemaFact>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisSnapshot<
    GraphRevision,
    NodeId,
    PortAddress: Ord,
    ConnectionId,
    ResourceIdentity,
    ParameterValue,
    TypeFact,
    SchemaFact,
> {
    pub basis: CompilationBasis<GraphRevision>,
    pub nodes: Box<[AnalyzedNode<NodeId, ParameterValue>]>,
    pub resolved_interfaces: Box<[ResolvedInterface<NodeId, PortAddress>]>,
    pub partial_types: TypeFacts<PortAddress, TypeFact>,
    pub partial_schemas: SchemaFacts<PortAddress, SchemaFact>,
    pub diagnostics: Box<[NodeDiagnostic<NodeId, PortAddress, ConnectionId, ResourceIdentity>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedNode<NodeId, ParameterValue> {
    pub node_id: NodeId,
    pub node_type_id: NodeTypeId,
    pub protocol_fingerprint: ProtocolFingerprint,
    pub normalized_parameters: BTreeMap<ParameterKey, ParameterValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedInterface<NodeId, PortAddress> {
    pub node_id: NodeId,
    pub ports: Box<[ResolvedPort<PortAddress>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedPort<PortAddress> {
    pub address: PortAddress,
    pub template: PortKey,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub status: ResolvedPortStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolvedPortStatus {
    Resolved,
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    BlockingDiagnostics { count: usize },
    BasisMismatch,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockingDiagnostics { count } => {
                write!(
                    formatter,
                    "analysis contains {count} blocking diagnostic(s)"
                )
            }
            Self::BasisMismatch => {
                formatter.write_str("semantic graph basis does not match analysis")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

impl<
    GraphRevision: PartialEq,
    NodeId,
    PortAddress: Ord,
    ConnectionId,
    ResourceIdentity,
    ParameterValue,
    TypeFact,
    SchemaFact,
>
    AnalysisSnapshot<
        GraphRevision,
        NodeId,
        PortAddress,
        ConnectionId,
        ResourceIdentity,
        ParameterValue,
        TypeFact,
        SchemaFact,
    >
{
    pub fn has_blocking_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }

    pub fn validated<ResolvedType, ResolvedSchema>(
        &self,
        graph: ValidatedSemanticGraph<
            GraphRevision,
            NodeId,
            PortAddress,
            ConnectionId,
            ParameterValue,
            ResolvedType,
            ResolvedSchema,
        >,
    ) -> Result<
        ValidatedSemanticGraph<
            GraphRevision,
            NodeId,
            PortAddress,
            ConnectionId,
            ParameterValue,
            ResolvedType,
            ResolvedSchema,
        >,
        ValidationError,
    > {
        let blocking_count = self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_blocking())
            .count();
        if blocking_count != 0 {
            return Err(ValidationError::BlockingDiagnostics {
                count: blocking_count,
            });
        }
        if graph.basis != self.basis {
            return Err(ValidationError::BasisMismatch);
        }
        Ok(graph)
    }
}
