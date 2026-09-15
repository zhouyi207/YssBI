use super::*;
use crate::{GraphSemanticCache, resolve_graph_semantics, resolve_graph_semantics_with_cache};
use yss_graph_document::{ConnectionId, DocumentConnection, DocumentNode, NodePosition};
use yss_graph_resource_contract::{ColumnSchema, DataSchema, ResourceCatalogFingerprint};

fn node(
    document: &mut GraphDocument,
    kind: &str,
    parameters: &[(&str, serde_json::Value)],
) -> NodeId {
    let id = NodeId::new();
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: kind.parse().unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: parameters
                .iter()
                .map(|(key, value)| (key.parse().unwrap(), value.clone()))
                .collect(),
            user_label: None,
        },
    );
    id
}

fn port(node: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node, key.parse().unwrap())
}

fn connect(document: &mut GraphDocument, output: PortAddress, input: PortAddress) -> ConnectionId {
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output,
            input,
            order: None,
        },
    );
    id
}

fn catalog(a: Option<DataType>) -> ResourceCatalogSnapshot {
    let mut databases = BTreeMap::from([(
        GraphResourceId::new("databases/b"),
        DataSchema {
            columns: vec![ColumnSchema {
                name: "amount".into(),
                data_type: DataType::Int64,
            }],
        },
    )]);
    if let Some(data_type) = a {
        databases.insert(
            GraphResourceId::new("databases/a"),
            DataSchema {
                columns: vec![ColumnSchema {
                    name: "amount".into(),
                    data_type,
                }],
            },
        );
    }
    ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        databases,
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    )
}

fn branch(document: &mut GraphDocument, database: &str) -> [NodeId; 4] {
    let source = node(
        document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!(database))],
    );
    let limit = node(
        document,
        "yssbi.dataframe.limit",
        &[("rows", serde_json::json!(10))],
    );
    let project = node(
        document,
        "yssbi.dataframe.project",
        &[("columns", serde_json::json!(["amount"]))],
    );
    let view = node(document, "yssbi.core.reroute", &[]);
    connect(document, port(source, "dataframe"), port(limit, "source"));
    connect(document, port(limit, "result"), port(project, "source"));
    connect(document, port(project, "result"), port(view, "input"));
    [source, limit, project, view]
}

fn assert_matches_full(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    cache: &mut GraphSemanticCache,
) -> crate::GraphSemanticSnapshot {
    let incremental_resources = resources.tracked();
    let incremental =
        resolve_graph_semantics_with_cache(document, registry, &incremental_resources, cache);
    let full_resources = resources.tracked();
    let full = resolve_graph_semantics(document, registry, &full_resources);
    assert_eq!(incremental, full);
    assert_eq!(
        incremental_resources.dependencies(),
        full_resources.dependencies()
    );
    incremental
}

#[test]
fn schema_cache_stops_invalidation_when_upstream_schema_is_unchanged() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let a = branch(&mut document, "databases/a");
    branch(&mut document, "databases/b");
    let mut cache = GraphSemanticCache::default();
    let resources = catalog(Some(DataType::Int64));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    document
        .nodes
        .get_mut(&a[1])
        .unwrap()
        .parameters
        .insert("rows".parse().unwrap(), serde_json::json!(20));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 7);
    assert_matches_full(
        &document,
        &registry,
        &catalog(Some(DataType::String)),
        &mut cache,
    );
    assert_eq!(
        cache.schemas.reused_outputs, 4,
        "only the other branch retains its Schema"
    );
}

#[test]
fn schema_cache_preserves_absent_reads_and_recovers_from_missing_resources() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    branch(&mut document, "databases/a");
    let mut cache = GraphSemanticCache::default();
    assert_matches_full(&document, &registry, &catalog(None), &mut cache);
    assert_matches_full(&document, &registry, &catalog(None), &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 4);
    let recovered = assert_matches_full(
        &document,
        &registry,
        &catalog(Some(DataType::Int64)),
        &mut cache,
    );
    assert_eq!(cache.schemas.reused_outputs, 0);
    assert!(recovered.ready().is_some(), "{:?}", recovered.diagnostics());
    assert_matches_full(&document, &registry, &catalog(None), &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 0);
}

#[test]
fn schema_cache_tracks_rewiring_cycles_and_deleted_outputs() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let a = branch(&mut document, "databases/a");
    let b = branch(&mut document, "databases/b");
    let resources = catalog(Some(DataType::Int64));
    let mut cache = GraphSemanticCache::default();
    assert_matches_full(&document, &registry, &resources, &mut cache);
    let edge = document
        .connections
        .values()
        .find(|edge| edge.input == port(a[1], "source"))
        .unwrap()
        .id;
    document.connections.get_mut(&edge).unwrap().output = port(b[0], "dataframe");
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 5);
    document.connections.get_mut(&edge).unwrap().output = port(a[3], "output");
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(cache.schemas.outputs.is_empty());
    document.connections.get_mut(&edge).unwrap().output = port(a[0], "dataframe");
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 0);
    document.nodes.remove(&a[3]);
    document
        .connections
        .retain(|_, edge| edge.input.node_id != a[3] && edge.output.node_id != a[3]);
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(
        cache
            .schemas
            .outputs
            .keys()
            .all(|address| address.node_id != a[3])
    );
}

#[test]
fn schema_cache_rechecks_tabular_constant_cells_without_recomputing_unchanged_downstream_schema() {
    use yss_data_contract::DataValue;
    use yss_graph_document::{ConstantId, GraphConstant, normalize_constant_value};
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let id = ConstantId::new();
    let definition = |json: &str| {
        let mut constant = GraphConstant {
            id,
            name: "Table".into(),
            data_type: DataType::DataFrame,
            data_value: DataValue::DataFrame(json.into()),
            tabular: None,
            description: String::new(),
            tags: vec![],
        };
        normalize_constant_value(&mut constant).unwrap();
        constant
    };
    document
        .constants
        .insert(id, definition(r#"{"amount":[1]}"#));
    let source = node(
        &mut document,
        "yssbi.constant.get",
        &[("constant", serde_json::json!(id.to_string()))],
    );
    let consumer = node(
        &mut document,
        "yssbi.dataframe.project",
        &[("columns", serde_json::json!(["amount"]))],
    );
    connect(
        &mut document,
        port(source, "value"),
        port(consumer, "source"),
    );
    let mut cache = GraphSemanticCache::default();
    let resources = catalog(None);
    assert_matches_full(&document, &registry, &resources, &mut cache);
    document
        .constants
        .insert(id, definition(r#"{"amount":[2]}"#));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 1);
    document
        .constants
        .insert(id, definition(r#"{"amount":["label"]}"#));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 0);
}
