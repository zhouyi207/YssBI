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
    #[serde(
        with = "ordered_map_entries",
        bound(
            serialize = "TypeFact: Serialize",
            deserialize = "TypeFact: Deserialize<'de>"
        )
    )]
    pub partial_types: TypeFacts<PortAddress, TypeFact>,
    #[serde(
        with = "ordered_map_entries",
        bound(
            serialize = "SchemaFact: Serialize",
            deserialize = "SchemaFact: Deserialize<'de>"
        )
    )]
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

mod ordered_map_entries {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S, K, V>(values: &BTreeMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        K: Serialize,
        V: Serialize,
    {
        values.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
    {
        let entries = Vec::<(K, V)>::deserialize(deserializer)?;
        let mut values = BTreeMap::new();
        for (key, value) in entries {
            if values.insert(key, value).is_some() {
                return Err(serde::de::Error::custom("duplicate ordered map key"));
            }
        }
        Ok(values)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{ResourceVersion, ResourceVersionSet};
    use crate::node_system::document::{ConnectionId, GraphRevision, NodeId, PortAddress};
    use crate::node_system::protocol::{SchemaExpr, TypeExpr, TypeId};
    use crate::node_system::registry::RegistryFingerprint;
    use uuid::Uuid;

    type TestSnapshot = AnalysisSnapshot<
        GraphRevision,
        NodeId,
        PortAddress,
        ConnectionId,
        Box<str>,
        serde_json::Value,
        TypeExpr,
        SchemaExpr,
    >;

    fn address(node: u128, port: &str) -> PortAddress {
        PortAddress::declared(
            NodeId::from_uuid(Uuid::from_u128(node)),
            PortKey::new(port).unwrap(),
        )
    }

    fn snapshot() -> TestSnapshot {
        let first = address(1, "first");
        let second = address(2, "second");
        let mut partial_types = BTreeMap::new();
        partial_types.insert(
            second.clone(),
            TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
        );
        partial_types.insert(
            first.clone(),
            TypeExpr::Concrete(TypeId::new("core.integer").unwrap()),
        );
        let mut partial_schemas = BTreeMap::new();
        partial_schemas.insert(second, SchemaExpr::Input(PortKey::new("source_b").unwrap()));
        partial_schemas.insert(first, SchemaExpr::Input(PortKey::new("source_a").unwrap()));

        AnalysisSnapshot {
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(5),
                registry_fingerprint: RegistryFingerprint::from_bytes([3; 32]),
                resource_versions: ResourceVersionSet::from([(
                    crate::node_system::analysis::ResourceKey::new("resource.test"),
                    ResourceVersion::new("1"),
                )]),
            },
            nodes: Box::new([]),
            resolved_interfaces: Box::new([]),
            partial_types,
            partial_schemas,
            diagnostics: Box::new([]),
        }
    }

    fn json_entry<K: Serialize, V: Serialize>(key: &K, value: &V) -> serde_json::Value {
        serde_json::Value::Array(vec![
            serde_json::to_value(key).unwrap(),
            serde_json::to_value(value).unwrap(),
        ])
    }

    #[test]
    fn partial_fact_maps_serialize_as_ordered_entry_arrays() {
        let snapshot = snapshot();
        let first = address(1, "first");
        let second = address(2, "second");
        let json = serde_json::to_value(&snapshot).unwrap();

        assert_eq!(
            json["partial_types"],
            serde_json::Value::Array(vec![
                json_entry(
                    &first,
                    &TypeExpr::Concrete(TypeId::new("core.integer").unwrap()),
                ),
                json_entry(
                    &second,
                    &TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
                ),
            ])
        );
        assert_eq!(
            json["partial_schemas"],
            serde_json::Value::Array(vec![
                json_entry(
                    &first,
                    &SchemaExpr::Input(PortKey::new("source_a").unwrap()),
                ),
                json_entry(
                    &second,
                    &SchemaExpr::Input(PortKey::new("source_b").unwrap()),
                ),
            ])
        );
    }

    #[test]
    fn partial_fact_map_json_roundtrips() {
        let snapshot = snapshot();
        let encoded = serde_json::to_vec(&snapshot).unwrap();
        let decoded: TestSnapshot = serde_json::from_slice(&encoded).unwrap();

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn partial_fact_map_json_rejects_duplicate_keys() {
        let mut json = serde_json::to_value(snapshot()).unwrap();
        let duplicate = json["partial_types"][0].clone();
        json["partial_types"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);

        let error = serde_json::from_value::<TestSnapshot>(json).unwrap_err();

        assert_eq!(error.to_string(), "duplicate ordered map key");
    }
}

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
