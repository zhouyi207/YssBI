use super::*;
use crate::node_system::catalog::{BuiltinCatalog, LocalizedCatalog, build_builtin_node_system};
use crate::node_system::document::{DocumentConnection, DocumentNode, GraphDocument};
use crate::node_system::protocol::{NodeTypeId, PortKey, Value};
use crate::node_system::registry::{
    NodeRegistry, canonical_semantic_protocol_snapshot, i18n_inventory,
};
use crate::node_system::runtime::{CancellationToken, RunError, RunExecutor, RuntimeValue};

fn node_type(name: &str) -> NodeTypeId {
    NodeTypeId::new(format!("yssbi.testing.{name}")).unwrap()
}

fn port(name: &str) -> PortKey {
    PortKey::new(name).unwrap()
}

fn arithmetic_provider() -> TestProvider {
    let mut builder = TestProviderBuilder::new();
    builder
        .constant(node_type("two"), port("value"), Value::Integer(2))
        .constant(node_type("three"), port("value"), Value::Integer(3))
        .add(node_type("add"), port("left"), port("right"), port("sum"));
    builder.build()
}

fn arithmetic_graph() -> (GraphDocument, TestNode) {
    let mut builder = TestGraphBuilder::new();
    let two = builder.add_node(node_type("two"));
    let three = builder.add_node(node_type("three"));
    let add = builder.add_node(node_type("add"));
    builder
        .connect(&two, &port("value"), &add, &port("left"))
        .connect(&three, &port("value"), &add, &port("right"));
    (builder.build(), add)
}

#[test]
fn protocol_to_runtime_executes_constants_and_addition() {
    let provider = arithmetic_provider();
    let (document, add) = arithmetic_graph();
    let compiled = provider.compile(&document);
    let analysis_snapshot = canonical_analysis(&compiled.analysis);
    assert!(analysis_snapshot.contains("yssbi.testing.add"));

    let mut plan = compile_assertions(compiled)
        .has_plan()
        .has_no_diagnostics()
        .into_plan();
    provider.expose_result(&mut plan, &add, &port("sum"), "sum");
    plan.resources = vec![tracked_requirement("testing.arithmetic")].into_boxed_slice();
    assert!(plan_debug_snapshot(&plan).contains("testing.kernel.yssbi.testing.add"));

    let resources = ResourceLeakTracker::new();
    let result = RunExecutor::new(provider.kernels(), &resources, &NoFunctionPlans)
        .run(&plan, CancellationToken::new());
    run_assertions(result)
        .succeeds()
        .has_value("sum", &Value::Integer(5));
    resources.assert_no_leaks();

    let records = provider.recorder().records();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records.last().unwrap().inputs,
        [
            RuntimeValue::Scalar(Value::Integer(2)),
            RuntimeValue::Scalar(Value::Integer(3)),
        ]
    );
}

#[test]
fn blocking_diagnostic_never_produces_a_plan() {
    let provider = arithmetic_provider();
    let mut graph = TestGraphBuilder::new();
    graph.add_node(node_type("missing"));

    compile_assertions(provider.compile(&graph.build()))
        .has_no_plan()
        .has_diagnostic("compiler.node.unknown");
}

struct LocaleState {
    document: GraphDocument,
    registry: NodeRegistry,
    catalog: BuiltinCatalog,
    localized: Option<LocalizedCatalog>,
}

#[test]
fn semantic_protocol_and_i18n_exporters_are_canonical() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let semantic = canonical_semantic_protocol_snapshot(&registry).unwrap();
    let semantic: serde_json::Value = serde_json::from_str(&semantic).unwrap();
    let node_ids = semantic["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["nodeTypeId"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(node_ids.windows(2).all(|pair| pair[0] < pair[1]));

    let inventory = i18n_inventory(&registry).unwrap();
    let keys: Vec<String> = serde_json::from_str(&inventory).unwrap();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(keys.iter().any(|key| key == "nodes.yssbi.logic.not.title"));

    assert!(
        !canonical_semantic_protocol_snapshot(&registry)
            .unwrap()
            .contains("Boolean Constant")
    );
    assert!(
        !canonical_semantic_protocol_snapshot(&registry)
            .unwrap()
            .contains("布尔常量")
    );
    assert!(!inventory.contains("Boolean Constant"));
    assert!(!inventory.contains("布尔常量"));
}

#[test]
fn localized_text_does_not_change_semantic_snapshot_or_fingerprint() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let fingerprint = registry.fingerprint().clone();
    let semantic = canonical_semantic_protocol_snapshot(&registry).unwrap();
    let inventory = i18n_inventory(&registry).unwrap();

    let english = catalog.localize(&registry, "en-US");
    let chinese = catalog.localize(&registry, "zh-CN");
    assert_ne!(english.items[0].title, chinese.items[0].title);
    assert_eq!(registry.fingerprint(), &fingerprint);
    assert_eq!(
        canonical_semantic_protocol_snapshot(&registry).unwrap(),
        semantic
    );
    assert_eq!(i18n_inventory(&registry).unwrap(), inventory);
}

#[test]
fn changing_language_does_not_change_the_document() {
    let builtin = build_builtin_node_system().unwrap();
    let mut state = LocaleState {
        document: arithmetic_graph().0,
        registry: std::sync::Arc::unwrap_or_clone(builtin.registry),
        catalog: std::sync::Arc::unwrap_or_clone(builtin.catalog),
        localized: None,
    };

    assert_locale_invariance(
        &mut state,
        &["en-US", "zh-CN", "en_US", "unknown"],
        |state| canonical_document(&state.document),
        |state, locale| {
            state.localized = Some(state.catalog.localize(&state.registry, locale));
        },
    );
    assert_eq!(state.localized.unwrap().locale.as_ref(), "unknown");
}

#[derive(Clone)]
enum GraphEntry {
    Node(DocumentNode),
    Connection(DocumentConnection),
}

#[test]
fn randomized_btree_insertion_order_is_semantically_equivalent() {
    let provider = arithmetic_provider();
    let (graph, _) = arithmetic_graph();
    let entries = graph
        .nodes
        .values()
        .cloned()
        .map(GraphEntry::Node)
        .chain(
            graph
                .connections
                .values()
                .cloned()
                .map(GraphEntry::Connection),
        )
        .collect::<Vec<_>>();

    assert_random_insertion_order_determinism(&entries, 0x5eed, 32, |order| {
        let mut rebuilt = GraphDocument::default();
        for entry in order {
            match entry {
                GraphEntry::Node(node) => {
                    rebuilt.nodes.insert(node.id, node.clone());
                }
                GraphEntry::Connection(connection) => {
                    rebuilt
                        .connections
                        .insert(connection.id, connection.clone());
                }
            }
        }
        let compiled = provider.compile(&rebuilt);
        let plan = compiled.plan.as_ref().expect("permutation must compile");
        format!(
            "{}\n{}\n{}",
            canonical_document(&rebuilt),
            canonical_analysis(&compiled.analysis),
            plan_debug_snapshot(plan)
        )
    });
}

#[test]
fn successful_failed_and_cancelled_runs_release_every_resource() {
    let mut builder = TestProviderBuilder::new();
    builder
        .constant(node_type("ok"), port("value"), Value::Integer(1))
        .failing(node_type("fail"), port("value"), "injected failure")
        .cancelling(node_type("cancel"), port("value"));
    let provider = builder.build();

    for (kind, expected) in [
        ("ok", "success"),
        ("fail", "failure"),
        ("cancel", "cancelled"),
    ] {
        let mut graph = TestGraphBuilder::new();
        graph.add_node(node_type(kind));
        let mut plan = compile_assertions(provider.compile(&graph.build())).into_plan();
        plan.resources = vec![tracked_requirement(&format!("testing.{kind}"))].into_boxed_slice();
        let resources = ResourceLeakTracker::new();
        let result = RunExecutor::new(provider.kernels(), &resources, &NoFunctionPlans)
            .run(&plan, CancellationToken::new());

        match expected {
            "success" => {
                run_assertions(result).succeeds();
            }
            "failure" => {
                run_assertions(result).fails().error_matches(|error| {
                    matches!(error, RunError::KernelFailed { message, .. } if message.as_ref() == "injected failure")
                });
            }
            "cancelled" => {
                run_assertions(result).fails().is_cancelled();
            }
            _ => unreachable!(),
        }
        assert_eq!(resources.acquired(), 1);
        assert_eq!(resources.released(), 1);
        resources.assert_no_leaks();
    }
}

#[test]
fn partial_resource_acquisition_failure_releases_prior_leases() {
    let provider = arithmetic_provider();
    let mut graph = TestGraphBuilder::new();
    graph.add_node(node_type("two"));
    let mut plan = compile_assertions(provider.compile(&graph.build())).into_plan();
    plan.resources = vec![
        tracked_requirement("testing.one"),
        tracked_requirement("testing.two"),
    ]
    .into_boxed_slice();
    let resources = ResourceLeakTracker::failing_at(2);

    run_assertions(
        RunExecutor::new(provider.kernels(), &resources, &NoFunctionPlans)
            .run(&plan, CancellationToken::new()),
    )
    .fails()
    .error_matches(|error| matches!(error, RunError::ResourceAcquire { .. }));
    resources.assert_no_leaks();
}
