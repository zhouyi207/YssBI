use super::*;
use crate::{GraphSemanticCache, resolve_graph_semantics, resolve_graph_semantics_with_cache};
use yss_graph_document::{ConnectionId, DocumentConnection, DocumentNode, NodePosition};
use yss_graph_resource_contract::{ColumnSchema, DataSchema, ResourceCatalogFingerprint};

#[test]
fn drop_nodes_resolve_remaining_fields_and_refresh_after_upstream_changes() {
    use yss_data_contract::SemanticType as S;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resource = |extra: bool| {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("data"),
                DataSchema {
                    columns: [
                        ("amount", S::Numeric),
                        ("unused", S::Text),
                        ("id", S::Identifier),
                    ]
                    .into_iter()
                    .chain(extra.then_some(("new", S::Text)))
                    .map(|(name, kind)| ColumnSchema {
                        name: name.into(),
                        data_type: ValueType::Scalar(kind),
                        physical_type: None,
                        semantic: None,
                    })
                    .collect(),
                },
            )]),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    };
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("data"))],
    );
    let columns = node(
        &mut document,
        "yssbi.dataframe.drop.columns",
        &[("columns", serde_json::json!(["unused"]))],
    );
    let rows = node(
        &mut document,
        "yssbi.dataframe.drop.rows",
        &[(
            "predicate",
            serde_json::json!({"column":"amount","operator":"lessThan","value":{"type":"integer","value":"0"}}),
        )],
    );
    connect(
        &mut document,
        port(source, "dataframe"),
        port(columns, "source"),
    );
    connect(&mut document, port(columns, "result"), port(rows, "source"));
    let na_rows = node(&mut document, "yssbi.dataframe.dropna.rows", &[]);
    let na_columns = node(&mut document, "yssbi.dataframe.dropna.columns", &[]);
    let after_na = node(&mut document, "yssbi.dataframe.dropna.rows", &[]);
    connect(&mut document, port(rows, "result"), port(na_rows, "source"));
    connect(
        &mut document,
        port(na_rows, "result"),
        port(na_columns, "source"),
    );
    connect(
        &mut document,
        port(na_columns, "result"),
        port(after_na, "source"),
    );
    let mut cache = GraphSemanticCache::default();
    for extra in [false, true] {
        let snapshot =
            assert_matches_full(&document, &builtin.registry, &resource(extra), &mut cache);
        for id in [na_rows, na_columns] {
            assert!(matches!(
                &snapshot.node(id).unwrap().parameters[0].configuration,
                Some(crate::GraphParameterConfigurationFact::ProjectColumns {
                    allow_empty: true, available: true, options, value, ..
                }) if options.len() == if extra { 3 } else { 2 } && value.is_empty()
            ));
        }
        assert!(matches!(
            &snapshot.node(after_na).unwrap().parameters[0].configuration,
            Some(crate::GraphParameterConfigurationFact::ProjectColumns {
                allow_empty: true, available: false, unavailable_reason: Some(reason), ..
            }) if reason.as_ref() == "editors.dataframe.deferred_columns"
        ));
        assert!(matches!(
            &snapshot.node(columns).unwrap().parameters[0].configuration,
            Some(crate::GraphParameterConfigurationFact::ProjectColumns { available: true, options, .. })
                if options.len() == if extra { 4 } else { 3 }
        ));
        assert!(matches!(
            &snapshot.node(rows).unwrap().parameters[0].configuration,
            Some(crate::GraphParameterConfigurationFact::FilterPredicate { available: true, columns, .. })
                if columns.len() == if extra { 3 } else { 2 }
        ));
        let source_fields = snapshot
            .node(source)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(source, "dataframe"))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields
            .clone();
        let expected: Vec<_> = source_fields
            .into_iter()
            .filter(|f| f.name.0.as_ref() != "unused")
            .collect();
        assert!(
            !snapshot.has_blocking_diagnostics(),
            "{:?}",
            snapshot.diagnostics()
        );
        for id in [na_columns, after_na] {
            let result = snapshot
                .node(id)
                .unwrap()
                .ports
                .iter()
                .find(|p| p.address == port(id, "result"))
                .unwrap();
            assert!(matches!(result.schema_state, GraphSchemaState::Deferred));
            assert!(result.schema_state.exact().is_none());
        }
        for id in [columns, rows, na_rows] {
            let result = snapshot
                .node(id)
                .unwrap()
                .ports
                .iter()
                .find(|p| p.address == port(id, "result"))
                .unwrap();
            assert_eq!(result.schema_state.exact().unwrap().fields, expected);
        }
    }
    // A deferred frame is executable, but cannot promise a statically selected column.
    let mut dependent = document.clone();
    let select = node(
        &mut dependent,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("amount"))],
    );
    connect(
        &mut dependent,
        port(na_columns, "result"),
        port(select, "dataframe"),
    );
    let blocked = resolve_graph_semantics(&dependent, &builtin.registry, &resource(false));
    assert!(blocked.has_blocking_diagnostics());
    for (selection, issue) in [
        (
            serde_json::json!(["absent"]),
            GraphSchemaIssue::MissingColumn,
        ),
        (
            serde_json::json!(["amount", "unused", "id"]),
            GraphSchemaIssue::InvalidParameter,
        ),
    ] {
        document
            .nodes
            .get_mut(&columns)
            .unwrap()
            .parameters
            .insert("columns".parse().unwrap(), selection);
        let snapshot =
            assert_matches_full(&document, &builtin.registry, &resource(false), &mut cache);
        let result = snapshot
            .node(columns)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(columns, "result"))
            .unwrap();
        assert_eq!(result.schema_state.issue(), Some(issue));
    }
}

#[test]
fn composed_schemas_track_input_order_join_keys_and_mixed_series() {
    use yss_data_contract::SemanticType as S;
    use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([
            (
                GraphResourceId::new("databases/left"),
                DataSchema {
                    columns: [("left_id", S::Numeric), ("label", S::Text)]
                        .map(|(name, kind)| ColumnSchema {
                            name: name.into(),
                            data_type: ValueType::Scalar(kind),
                            physical_type: None,
                            semantic: None,
                        })
                        .into(),
                },
            ),
            (
                GraphResourceId::new("databases/right"),
                DataSchema {
                    columns: [("right_id", S::Numeric), ("label", S::Text)]
                        .map(|(name, kind)| ColumnSchema {
                            name: name.into(),
                            data_type: ValueType::Scalar(kind),
                            physical_type: None,
                            semantic: None,
                        })
                        .into(),
                },
            ),
        ]),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let mut document = GraphDocument::default();
    let left = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/left"))],
    );
    let right = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/right"))],
    );
    let join = node(
        &mut document,
        "yssbi.dataframe.join",
        &[
            ("left_keys", serde_json::json!(["left_id"])),
            ("right_keys", serde_json::json!(["right_id"])),
        ],
    );
    connect(&mut document, port(left, "dataframe"), port(join, "left"));
    connect(&mut document, port(right, "dataframe"), port(join, "right"));
    let concat = node(&mut document, "yssbi.dataframe.concat.rows", &[]);
    let mut dynamic = Vec::new();
    for (index, source) in [left, right].into_iter().enumerate() {
        let input = PortAddress::instance(concat, "frames".parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            input.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(index.to_string()),
            },
        );
        connect(&mut document, port(source, "dataframe"), input.clone());
        dynamic.push(input);
    }
    let mut cache = GraphSemanticCache::default();
    let first = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    let fields = |snapshot: &crate::GraphSemanticSnapshot, id| {
        snapshot
            .node(id)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.direction == yss_node_protocol::PortDirection::Output)
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields
            .iter()
            .map(|f| f.name.0.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        fields(&first, join),
        ["left_id", "label", "right_id", "label_right"]
    );
    assert_eq!(fields(&first, concat), ["left_id", "label", "right_id"]);
    let right_keys = first
        .node(join)
        .unwrap()
        .parameters
        .iter()
        .find(|p| p.key.as_str() == "right_keys")
        .unwrap();
    assert!(
        matches!(&right_keys.configuration, Some(crate::GraphParameterConfigurationFact::ProjectColumns { options, .. }) if options[0].name.as_ref() == "right_id")
    );
    document.port_bindings.insert(
        dynamic[0].clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey::new("z"),
        },
    );
    let second = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert_eq!(fields(&second, concat), ["right_id", "label", "left_id"]);
    let number = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("left_id"))],
    );
    let text = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("label"))],
    );
    for target in [number, text] {
        connect(
            &mut document,
            port(left, "dataframe"),
            port(target, "dataframe"),
        );
    }
    let assemble = node(&mut document, "yssbi.dataframe.combine", &[]);
    for (index, source) in [number, text].into_iter().enumerate() {
        let input =
            PortAddress::instance(assemble, "series".parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            input.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(index.to_string()),
            },
        );
        connect(&mut document, port(source, "series"), input);
    }
    let third = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert_eq!(fields(&third, assemble), ["left_id", "label"]);
    document
        .nodes
        .get_mut(&join)
        .unwrap()
        .parameters
        .insert("right_keys".parse().unwrap(), serde_json::json!(["absent"]));
    let invalid = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(
        invalid
            .node(join)
            .unwrap()
            .ports
            .iter()
            .any(|p| p.direction == yss_node_protocol::PortDirection::Output
                && p.schema_state.exact().is_none())
    );
}

#[test]
fn dataframe_decomposition_uses_all_seven_semantics_and_tracks_metadata_changes() {
    use yss_data_contract::{ColumnSemantic, SemanticType};
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/typed"))],
    );
    let decompose = node(&mut document, "yssbi.dataframe.decompose", &[]);
    connect(
        &mut document,
        port(source, "dataframe"),
        port(decompose, "dataframe"),
    );
    let columns = SemanticType::ALL
        .into_iter()
        .map(|semantic| ColumnSchema {
            name: semantic.as_str().into(),
            data_type: ValueType::Scalar(semantic),
            physical_type: Some(
                match semantic {
                    SemanticType::Text => "Utf8",
                    SemanticType::Datetime => "Date",
                    _ => "Int64",
                }
                .into(),
            ),
            semantic: Some(ColumnSemantic::new(semantic)),
        })
        .collect::<Vec<_>>();
    let resources = |columns| {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("databases/typed"),
                DataSchema { columns },
            )]),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    };
    let first = resources(columns.clone());
    let mut cache = GraphSemanticCache::default();
    let snapshot = assert_matches_full(&document, &builtin.registry, &first, &mut cache);
    let outputs = snapshot
        .node(decompose)
        .unwrap()
        .ports
        .iter()
        .filter(|port| port.direction == yss_node_protocol::PortDirection::Output)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 7);
    for (port, semantic) in outputs.into_iter().zip(SemanticType::ALL) {
        assert_eq!(
            port.type_state.exact(),
            Some(&yss_node_protocol::ResolvedType::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: Box::new([yss_node_protocol::ResolvedType::Nominal(
                    semantic.type_id().parse().unwrap()
                )]),
            })
        );
    }
    let observed = first.tracked();
    observed
        .database_schema(&GraphResourceId::new("databases/typed"))
        .unwrap();
    let dependencies = observed.dependencies();
    let mut changed = columns;
    changed[0].physical_type = Some("Float64".into());
    changed[0].semantic.as_mut().unwrap().numeric = Some(yss_data_contract::NumericConstraints {
        integer: true,
        minimum: None,
        maximum: None,
    });
    let second = resources(changed);
    assert!(!second.matches_dependencies(&dependencies));
    assert_matches_full(&document, &builtin.registry, &second, &mut cache);
}

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

fn catalog(a: Option<ValueType>) -> ResourceCatalogSnapshot {
    let mut databases = BTreeMap::from([(
        GraphResourceId::new("databases/b"),
        DataSchema {
            columns: vec![ColumnSchema {
                semantic: None,
                physical_type: None,
                name: "amount".into(),
                data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            }],
        },
    )]);
    if let Some(data_type) = a {
        databases.insert(
            GraphResourceId::new("databases/a"),
            DataSchema {
                columns: vec![ColumnSchema {
                    semantic: None,
                    physical_type: None,
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
    let resources = catalog(Some(ValueType::Scalar(
        yss_data_contract::SemanticType::Numeric,
    )));
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
        &catalog(Some(ValueType::Scalar(
            yss_data_contract::SemanticType::Text,
        ))),
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
        &catalog(Some(ValueType::Scalar(
            yss_data_contract::SemanticType::Numeric,
        ))),
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
    let resources = catalog(Some(ValueType::Scalar(
        yss_data_contract::SemanticType::Numeric,
    )));
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
            data_type: ValueType::DataFrame,
            data_value: DataValue::String(json.into()),
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
