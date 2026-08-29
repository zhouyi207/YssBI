use super::{CompilationBasis, DiagnosticSeverity, NodeDiagnostic, ValidatedSemanticGraph};
use crate::graph::protocol::{
    NodeTypeId, ParameterKey, PortDirection, PortKey, PortKind, TypeExpr,
};
use crate::graph::registry::ProtocolFingerprint;
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
    #[serde(with = "ordered_map_entries")]
    pub resolved_schemas: SchemaFacts<PortAddress, crate::graph::protocol::ResolvedSchemaFact>,
    pub diagnostics: Box<[NodeDiagnostic<NodeId, PortAddress, ConnectionId, ResourceIdentity>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedNode<NodeId, ParameterValue> {
    pub node_id: NodeId,
    pub node_type_id: NodeTypeId,
    pub protocol_fingerprint: ProtocolFingerprint,
    pub normalized_parameters: BTreeMap<ParameterKey, ParameterValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_title: Option<Box<str>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_label: Option<Box<str>>,
    pub value_type: TypeExpr,
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

pub(super) mod ordered_map_entries {
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
    use crate::graph::analysis::contracts::{ResourceVersion, ResourceVersionSet};
    use crate::graph::protocol::{SchemaExpr, TypeExpr, TypeId};
    use crate::graph::registry::RegistryFingerprint;
    use crate::graph_document::{ConnectionId, GraphRevision, NodeId, PortAddress};
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
    type TestSemantic = ValidatedSemanticGraph<
        GraphRevision,
        NodeId,
        PortAddress,
        ConnectionId,
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
        partial_schemas.insert(
            second.clone(),
            SchemaExpr::Input(PortKey::new("source_b").unwrap()),
        );
        partial_schemas.insert(
            first.clone(),
            SchemaExpr::Input(PortKey::new("source_a").unwrap()),
        );
        let resolved_schemas = BTreeMap::from([
            (
                second,
                crate::graph::protocol::ResolvedSchemaFact::new(
                    SchemaExpr::Input(PortKey::new("source_b").unwrap()),
                    [crate::graph::protocol::SchemaField {
                        name: crate::graph::protocol::SchemaColumnRef("name".into()),
                        scalar_type: crate::graph::protocol::RelationalScalarType::String,
                        lineage: None,
                    }],
                ),
            ),
            (
                first,
                crate::graph::protocol::ResolvedSchemaFact::new(
                    SchemaExpr::Input(PortKey::new("source_a").unwrap()),
                    [crate::graph::protocol::SchemaField {
                        name: crate::graph::protocol::SchemaColumnRef("amount".into()),
                        scalar_type: crate::graph::protocol::RelationalScalarType::Float64,
                        lineage: None,
                    }],
                ),
            ),
        ]);

        AnalysisSnapshot {
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(5),
                registry_fingerprint: RegistryFingerprint::from_bytes([3; 32]),
                resource_versions: ResourceVersionSet::from([(
                    crate::graph::analysis::contracts::ResourceKey::new("resource.test"),
                    ResourceVersion::new("1"),
                )]),
                resource_observations: Default::default(),
            },
            nodes: Box::new([]),
            resolved_interfaces: Box::new([]),
            partial_types,
            partial_schemas,
            resolved_schemas,
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
            json["resolved_schemas"],
            serde_json::Value::Array(vec![
                json_entry(&first, &snapshot.resolved_schemas[&first]),
                json_entry(&second, &snapshot.resolved_schemas[&second]),
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
    fn non_empty_typed_schema_serialization_and_digest_are_deterministic_and_sensitive() {
        let baseline = snapshot();
        let mut reordered = baseline.clone();
        reordered.resolved_schemas = baseline
            .resolved_schemas
            .iter()
            .rev()
            .map(|(address, fact)| (address.clone(), fact.clone()))
            .collect();

        let baseline_semantic = TestSemantic {
            basis: baseline.basis.clone(),
            nodes: Box::new([]),
            dependencies: Box::new([]),
            resolved_schemas: baseline.resolved_schemas.clone(),
        };
        let reordered_semantic = TestSemantic {
            basis: reordered.basis.clone(),
            nodes: Box::new([]),
            dependencies: Box::new([]),
            resolved_schemas: reordered.resolved_schemas.clone(),
        };

        assert_eq!(
            serde_json::to_vec(&baseline).unwrap(),
            serde_json::to_vec(&reordered).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&baseline_semantic).unwrap(),
            serde_json::to_vec(&reordered_semantic).unwrap()
        );
        assert_eq!(
            crate::graph::registry::hash_canonical("yssbi.analysis-snapshot.test.v1", &baseline,)
                .unwrap(),
            crate::graph::registry::hash_canonical("yssbi.analysis-snapshot.test.v1", &reordered,)
                .unwrap()
        );
        assert_eq!(
            crate::graph::registry::hash_canonical(
                "yssbi.validated-semantic-graph.test.v1",
                &baseline_semantic,
            )
            .unwrap(),
            crate::graph::registry::hash_canonical(
                "yssbi.validated-semantic-graph.test.v1",
                &reordered_semantic,
            )
            .unwrap()
        );

        let mut changed = baseline.clone();
        changed
            .resolved_schemas
            .get_mut(&address(1, "first"))
            .unwrap()
            .fields[0]
            .scalar_type = crate::graph::protocol::RelationalScalarType::Int64;
        let changed_semantic = TestSemantic {
            basis: changed.basis.clone(),
            nodes: Box::new([]),
            dependencies: Box::new([]),
            resolved_schemas: changed.resolved_schemas.clone(),
        };

        assert_ne!(
            serde_json::to_vec(&baseline).unwrap(),
            serde_json::to_vec(&changed).unwrap()
        );
        assert_ne!(
            serde_json::to_vec(&baseline_semantic).unwrap(),
            serde_json::to_vec(&changed_semantic).unwrap()
        );
        assert_ne!(
            crate::graph::registry::hash_canonical("yssbi.analysis-snapshot.test.v1", &baseline,)
                .unwrap(),
            crate::graph::registry::hash_canonical("yssbi.analysis-snapshot.test.v1", &changed,)
                .unwrap()
        );
        assert_ne!(
            crate::graph::registry::hash_canonical(
                "yssbi.validated-semantic-graph.test.v1",
                &baseline_semantic,
            )
            .unwrap(),
            crate::graph::registry::hash_canonical(
                "yssbi.validated-semantic-graph.test.v1",
                &changed_semantic,
            )
            .unwrap()
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
