use super::*;
use yss_data_contract::{DataType, DataValue};
use yss_graph_document::{ConstantId, DocumentNode, GraphConstant, NodePosition};
use yss_graph_editor::NodePositionMutation;
use yss_graph_editor::projection::{EditorProjectionInput, build_editor_projection};
use yss_graph_resource_contract::{
    ColumnSchema, DataSchema, FunctionCatalogEntry, FunctionParameterContract, FunctionSignature,
    GraphResourceId, ResourceCatalogFingerprint,
};

fn runtime() -> GraphRuntimeState {
    GraphRuntimeState::from_components(
        GraphRuntimeEpoch::from_existing(1),
        super::tests::components(),
    )
    .unwrap()
}

fn graph() -> GraphResourcePath {
    GraphResourcePath::new("events/Cache.yssbi-event").unwrap()
}

fn resources() -> ResourceCatalogSnapshot {
    ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    )
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

fn constant_document() -> (GraphDocument, NodeId, ConstantId) {
    let mut document = GraphDocument::default();
    let id = ConstantId::new();
    document.constants.insert(
        id,
        GraphConstant {
            id,
            name: "Threshold".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    let node = node(
        &mut document,
        "yssbi.constant.get",
        &[("constant", serde_json::json!(id.to_string()))],
    );
    (document, node, id)
}

fn resolve(
    runtime: &GraphRuntimeState,
    document: &GraphDocument,
    resources: &ResourceCatalogSnapshot,
) -> GraphAnalysis {
    runtime.resolve_graph_document(
        &graph(),
        document,
        &super::tests::basis(runtime),
        resources,
        &[],
        "en-US",
    )
}

fn assert_reused(before: &GraphAnalysis, after: &GraphAnalysis, node: NodeId, expected: bool) {
    assert_eq!(
        Arc::ptr_eq(
            before
                .semantic_snapshot()
                .node(node)
                .unwrap()
                .constant
                .as_ref()
                .unwrap(),
            after
                .semantic_snapshot()
                .node(node)
                .unwrap()
                .constant
                .as_ref()
                .unwrap(),
        ),
        expected
    );
}

#[test]
fn layout_reuses_semantics_and_projects_current_display_but_constant_metadata_refreshes() {
    let runtime = runtime();
    let (mut document, node, id) = constant_document();
    let initial = resolve(&runtime, &document, &resources());
    document.nodes.get_mut(&node).unwrap().position.x = 240.0;
    document.nodes.get_mut(&node).unwrap().user_label = Some("New label".into());
    let moved = resolve(&runtime, &document, &resources());
    assert_reused(&initial, &moved, node, true);
    let projection = build_editor_projection(EditorProjectionInput {
        graph_path: &graph(),
        document: &document,
        analysis: &moved,
        registry_fingerprint: runtime.registry_fingerprint(),
    })
    .unwrap();
    assert_eq!(projection.nodes[0].position.x, 240.0);
    assert_eq!(
        projection.nodes[0].display.user_label.as_deref(),
        Some("New label")
    );
    document.constants.get_mut(&id).unwrap().name = "Renamed".into();
    let renamed = resolve(&runtime, &document, &resources());
    assert_reused(&moved, &renamed, node, false);
    assert_eq!(renamed.semantic_input_hash(), moved.semantic_input_hash());
    assert_eq!(
        renamed
            .semantic_snapshot()
            .node(node)
            .unwrap()
            .instance_title
            .as_deref(),
        Some("Renamed")
    );
    document.constants.get_mut(&id).unwrap().data_value = DataValue::Int64(2);
    let edited = resolve(&runtime, &document, &resources());
    assert_ne!(edited.semantic_input_hash(), renamed.semantic_input_hash());
    assert_eq!(edited, resolve(&self::runtime(), &document, &resources()));
}

#[test]
fn snapshot_rechecks_used_and_absent_resources_without_invalidating_unread_catalog_entries() {
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/used"))],
    );
    let catalog = |entries: &[(&str, DataType)]| {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            entries
                .iter()
                .map(|(name, data_type)| {
                    (
                        GraphResourceId::new(*name),
                        DataSchema {
                            columns: vec![ColumnSchema {
                                name: "amount".into(),
                                data_type: data_type.clone(),
                            }],
                        },
                    )
                })
                .collect(),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    };
    let missing = resolve(&runtime, &document, &catalog(&[]));
    let unrelated = resolve(
        &runtime,
        &document,
        &catalog(&[("databases/unread", DataType::Int64)]),
    );
    assert_reused(&missing, &unrelated, constant, true);
    let present = resolve(
        &runtime,
        &document,
        &catalog(&[("databases/used", DataType::Int64)]),
    );
    assert_reused(&unrelated, &present, constant, false);
    let changed_catalog = catalog(&[("databases/used", DataType::String)]);
    let changed = resolve(&runtime, &document, &changed_catalog);
    assert_reused(&present, &changed, constant, false);
    assert_eq!(
        changed,
        resolve(&self::runtime(), &document, &changed_catalog)
    );
    assert_eq!(missing, resolve(&runtime, &document, &catalog(&[])));
}

#[test]
fn function_labels_and_bodies_refresh_even_when_execution_identity_is_unchanged() {
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    let path = GraphResourcePath::new("functions/Used.yssbi-function").unwrap();
    let call = node(
        &mut document,
        "yssbi.project.function.call",
        &[("target", serde_json::json!(path.as_str()))],
    );
    let catalog = |label: &str| {
        ResourceCatalogSnapshot::new(
            BTreeMap::from([(
                path.clone(),
                FunctionCatalogEntry::new(FunctionSignature::new(
                    vec![FunctionParameterContract::new(
                        yss_graph_document::FunctionParameterId::new("arg"),
                        label,
                        DataType::Int64,
                    )],
                    None,
                )),
            )]),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    };
    let initial = resolve(&runtime, &document, &catalog("Before"));
    let renamed = resolve(&runtime, &document, &catalog("After"));
    assert_reused(&initial, &renamed, constant, false);
    assert_eq!(initial.semantic_input_hash(), renamed.semantic_input_hash());
    assert_eq!(
        renamed.semantic_snapshot().node(call).unwrap().ports[0]
            .label
            .as_ref(),
        "After"
    );
    let (body, body_node, id) = constant_document();
    let with_body = catalog("After").with_function_document(&path, body.clone());
    let first_body = resolve(&runtime, &document, &with_body);
    let mut edited_body = body;
    edited_body.constants.get_mut(&id).unwrap().name = "Body constant".into();
    let edited_catalog = catalog("After").with_function_document(&path, edited_body.clone());
    let new_body = resolve(&runtime, &document, &edited_catalog);
    assert_reused(&first_body, &new_body, constant, false);
    assert_eq!(
        first_body.semantic_input_hash(),
        new_body.semantic_input_hash()
    );
    assert_eq!(
        new_body,
        resolve(&self::runtime(), &document, &edited_catalog)
    );
    edited_body.nodes.get_mut(&body_node).unwrap().position.x = 50.0;
    let moved_catalog = catalog("After").with_function_document(&path, edited_body);
    assert_reused(
        &new_body,
        &resolve(&runtime, &document, &moved_catalog),
        constant,
        true,
    );
}

#[test]
fn kernel_capability_identity_invalidates_cached_analysis() {
    let runtime = runtime();
    let (document, node, _) = constant_document();
    let initial = resolve(&runtime, &document, &resources());
    let mut basis = super::tests::basis(&runtime);
    basis.kernel_fingerprint = [9; 32];
    let changed =
        runtime.resolve_graph_document(&graph(), &document, &basis, &resources(), &[], "en-US");
    assert_reused(&initial, &changed, node, false);
    assert_ne!(initial.semantic_input_hash(), changed.semantic_input_hash());
}

#[test]
fn snapshot_refreshes_diagnostic_connection_ids_excluded_from_execution_identity() {
    use yss_graph_analysis::GraphDiagnosticLocation;
    use yss_graph_document::{ConnectionId, DocumentConnection};
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    let bad = ConnectionId::new();
    let source = PortAddress::declared(constant, "value".parse().unwrap());
    document.connections.insert(
        bad,
        DocumentConnection {
            id: bad,
            input: source.clone(),
            output: source,
            order: None,
        },
    );
    let initial = resolve(&runtime, &document, &resources());
    let replacement = ConnectionId::new();
    let mut connection = document.connections.remove(&bad).unwrap();
    connection.id = replacement;
    document.connections.insert(replacement, connection);
    let replaced = resolve(&runtime, &document, &resources());
    assert_eq!(
        initial.semantic_input_hash(),
        replaced.semantic_input_hash()
    );
    assert_reused(&initial, &replaced, constant, false);
    assert!(
        replaced
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .any(|diagnostic| {
                diagnostic.primary == GraphDiagnosticLocation::Connection(replacement)
            })
    );
    assert!(
        replaced
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .all(|diagnostic| { diagnostic.primary != GraphDiagnosticLocation::Connection(bad) })
    );
    assert_eq!(replaced, resolve(&self::runtime(), &document, &resources()));
}

#[test]
fn snapshot_rechecks_transitive_function_bodies() {
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    let a = GraphResourcePath::new("functions/A.yssbi-function").unwrap();
    let b = GraphResourcePath::new("functions/B.yssbi-function").unwrap();
    node(
        &mut document,
        "yssbi.project.function.call",
        &[("target", serde_json::json!(a.as_str()))],
    );
    let mut a_body = GraphDocument::default();
    node(
        &mut a_body,
        "yssbi.project.function.entry",
        &[("function", serde_json::json!(a.as_str()))],
    );
    node(
        &mut a_body,
        "yssbi.project.function.call",
        &[("target", serde_json::json!(b.as_str()))],
    );
    let (mut b_body, _, id) = constant_document();
    node(
        &mut b_body,
        "yssbi.project.function.entry",
        &[("function", serde_json::json!(b.as_str()))],
    );
    let catalog = ResourceCatalogSnapshot::new(
        [a.clone(), b.clone()]
            .into_iter()
            .map(|path| {
                (
                    path,
                    FunctionCatalogEntry::new(FunctionSignature::new(vec![], None)),
                )
            })
            .collect(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    )
    .with_function_document(&a, a_body);
    let missing = resolve(&runtime, &document, &catalog);
    let present = resolve(
        &runtime,
        &document,
        &catalog.clone().with_function_document(&b, b_body.clone()),
    );
    assert_reused(&missing, &present, constant, false);
    b_body.constants.get_mut(&id).unwrap().data_value = DataValue::Int64(7);
    let changed_catalog = catalog.with_function_document(&b, b_body);
    let changed = resolve(&runtime, &document, &changed_catalog);
    assert_reused(&present, &changed, constant, false);
    assert_ne!(present.semantic_input_hash(), changed.semantic_input_hash());
    assert_eq!(
        changed,
        resolve(&self::runtime(), &document, &changed_catalog)
    );
}

#[test]
fn document_only_mutations_do_not_request_semantics() {
    let runtime = runtime();
    let (document, node, _) = constant_document();
    let patch = runtime
        .plan_editor_mutation(
            &graph(),
            &document,
            EditorGraphMutation::MoveNodes {
                positions: vec![NodePositionMutation {
                    node_id: node,
                    position: NodePosition { x: 20.0, y: 0.0 },
                }],
            },
            &CatalogMutationValidationSnapshot {
                resources: BTreeMap::new(),
            },
            || panic!("move must not resolve"),
        )
        .unwrap();
    let mut moved = document;
    apply_graph_document_patch(&mut moved, &patch).unwrap();
    assert_eq!(moved.nodes[&node].position.x, 20.0);
}

#[test]
fn resolution_cache_evicts_old_graphs_without_changing_their_results() {
    let runtime = runtime();
    let (document, node, _) = constant_document();
    let initial = resolve(&runtime, &document, &resources());
    for index in 0..20 {
        runtime.resolve_graph_document(
            &GraphResourcePath::new(format!("events/Other{index}.yssbi-event")).unwrap(),
            &document,
            &super::tests::basis(&runtime),
            &resources(),
            &[],
            "en-US",
        );
    }
    let revisited = resolve(&runtime, &document, &resources());
    assert_reused(&initial, &revisited, node, false);
    assert_eq!(initial, revisited);
}

#[test]
#[ignore = "manual timing probe; run with --ignored --nocapture"]
fn benchmark_repeated_resolution_and_projection() {
    use std::hint::black_box;
    use std::time::Instant;
    use yss_graph_document::{ConnectionId, DocumentConnection};
    let runtime = runtime();
    let basis = super::tests::basis(&runtime);
    let graph = graph();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new("databases/bench"),
            DataSchema {
                columns: (0..12)
                    .map(|index| ColumnSchema {
                        name: format!("column{index}"),
                        data_type: DataType::Int64,
                    })
                    .collect(),
            },
        )]),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    for (count, shape) in [100, 1000, 5000]
        .into_iter()
        .flat_map(|count| ["scalar", "dataframe"].map(|shape| (count, shape)))
    {
        let mut document = GraphDocument::default();
        let mut previous = None;
        for index in 0..count {
            if shape == "scalar" {
                node(&mut document, "yssbi.logic.not", &[]);
            } else if index % 10 == 0 {
                let source = node(
                    &mut document,
                    "yssbi.dataframe.source.get",
                    &[("dataframe", serde_json::json!("databases/bench"))],
                );
                previous = Some(PortAddress::declared(source, "dataframe".parse().unwrap()));
            } else {
                let consumer = node(
                    &mut document,
                    "yssbi.dataframe.limit",
                    &[("rows", serde_json::json!(10))],
                );
                let id = ConnectionId::new();
                document.connections.insert(
                    id,
                    DocumentConnection {
                        id,
                        input: PortAddress::declared(consumer, "source".parse().unwrap()),
                        output: previous.take().unwrap(),
                        order: None,
                    },
                );
                previous = Some(PortAddress::declared(consumer, "result".parse().unwrap()));
            }
        }
        // Warm type/Schema caches in both modes. Discard only the whole
        // snapshot to measure the additional benefit of snapshot reuse.
        runtime.resolve_graph_document(&graph, &document, &basis, &resources, &[], "en-US");
        for reuse in [false, true] {
            let samples = if cfg!(debug_assertions) && count == 5000 {
                4
            } else {
                20
            };
            let mut resolve_time = std::time::Duration::ZERO;
            let mut projection_time = std::time::Duration::ZERO;
            for _ in 0..samples {
                if !reuse {
                    let mut caches = runtime.semantic_caches.lock().unwrap();
                    let mut cache = caches.take(&graph);
                    cache.analysis = None;
                    let retired = caches.put(graph.clone(), cache);
                    drop(caches);
                    drop(retired);
                }
                let started = Instant::now();
                let analysis = runtime.resolve_graph_document(
                    &graph,
                    &document,
                    &basis,
                    &resources,
                    &[],
                    "en-US",
                );
                resolve_time += started.elapsed();
                let started = Instant::now();
                black_box(
                    build_editor_projection(EditorProjectionInput {
                        graph_path: &graph,
                        document: &document,
                        analysis: &analysis,
                        registry_fingerprint: runtime.registry_fingerprint(),
                    })
                    .unwrap(),
                );
                projection_time += started.elapsed();
            }
            println!(
                "shape={shape} nodes={count} snapshot_reuse={reuse} samples={samples} resolve_mean_ms={:.3} projection_mean_ms={:.3}",
                resolve_time.as_secs_f64() * 1000.0 / f64::from(samples),
                projection_time.as_secs_f64() * 1000.0 / f64::from(samples)
            );
        }
    }
}
