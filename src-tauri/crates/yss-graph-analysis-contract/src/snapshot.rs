use super::{CompilationBasis, NodeDiagnostic, ValidatedSemanticGraph};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use yss_graph_protocol::{NodeTypeId, ParameterKey, PortDirection, PortKey, TypeExpr};
use yss_graph_registry::ProtocolFingerprint;

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
    pub resolved_schemas: SchemaFacts<PortAddress, yss_graph_protocol::ResolvedSchemaFact>,
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

pub type SemanticValidationResult<
    GraphRevision,
    NodeId,
    PortAddress,
    ConnectionId,
    ParameterValue,
    ResolvedType,
    ResolvedSchema,
> = Result<
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
>;

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
    ) -> SemanticValidationResult<
        GraphRevision,
        NodeId,
        PortAddress,
        ConnectionId,
        ParameterValue,
        ResolvedType,
        ResolvedSchema,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ResourceKey, ResourceVersion, ResourceVersionSet};
    use uuid::Uuid;
    use yss_graph_document::{ConnectionId, GraphRevision, NodeId, PortAddress};
    use yss_graph_protocol::{SchemaExpr, TypeExpr, TypeId};
    use yss_graph_registry::RegistryFingerprint;

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
                yss_graph_protocol::ResolvedSchemaFact::new(
                    SchemaExpr::Input(PortKey::new("source_b").unwrap()),
                    [yss_graph_protocol::SchemaField {
                        name: yss_graph_protocol::SchemaColumnRef("name".into()),
                        scalar_type: yss_graph_protocol::RelationalScalarType::String,
                        lineage: None,
                    }],
                ),
            ),
            (
                first,
                yss_graph_protocol::ResolvedSchemaFact::new(
                    SchemaExpr::Input(PortKey::new("source_a").unwrap()),
                    [yss_graph_protocol::SchemaField {
                        name: yss_graph_protocol::SchemaColumnRef("amount".into()),
                        scalar_type: yss_graph_protocol::RelationalScalarType::Float64,
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
                    ResourceKey::new("resource.test"),
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
            yss_canonical_hash::hash_canonical("yssbi.analysis-snapshot.test.v1", &baseline,)
                .unwrap(),
            yss_canonical_hash::hash_canonical("yssbi.analysis-snapshot.test.v1", &reordered,)
                .unwrap()
        );
        assert_eq!(
            yss_canonical_hash::hash_canonical(
                "yssbi.validated-semantic-graph.test.v1",
                &baseline_semantic,
            )
            .unwrap(),
            yss_canonical_hash::hash_canonical(
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
            .scalar_type = yss_graph_protocol::RelationalScalarType::Int64;
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
            yss_canonical_hash::hash_canonical("yssbi.analysis-snapshot.test.v1", &baseline,)
                .unwrap(),
            yss_canonical_hash::hash_canonical("yssbi.analysis-snapshot.test.v1", &changed,)
                .unwrap()
        );
        assert_ne!(
            yss_canonical_hash::hash_canonical(
                "yssbi.validated-semantic-graph.test.v1",
                &baseline_semantic,
            )
            .unwrap(),
            yss_canonical_hash::hash_canonical(
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
