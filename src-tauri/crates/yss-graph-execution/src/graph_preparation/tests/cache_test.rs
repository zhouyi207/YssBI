use super::*;
use crate::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_graph_analysis_contract::GraphAnalysisBasis;
use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};
use yss_node_registry::RegistryFingerprint;

fn components() -> GraphRuntimeComponents {
    let builtin = build_builtin_node_system().unwrap();
    GraphRuntimeComponents {
        registry: builtin.registry,
        catalog: builtin.catalog,
    }
}
fn empty_resource_catalog() -> ResourceCatalogSnapshot {
    ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    )
}
fn basis(runtime: &GraphRuntimeState) -> GraphAnalysisBasis {
    GraphAnalysisBasis {
        kernel_fingerprint: yss_node_kernel::KernelRegistry::default()
            .fingerprint()
            .as_bytes(),
        registry_fingerprint: RegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
        resource_versions: BTreeMap::new(),
        resource_observations: BTreeMap::new(),
    }
}

#[test]
fn execution_package_cache_tracks_semantics_and_requires_current_readiness() {
    let runtime =
        GraphRuntimeState::from_components(GraphRuntimeEpoch::from_existing(1), components())
            .unwrap();
    let execution = ExecutionRuntimeState::new(
        ExecutionSessionId::new(uuid::Uuid::nil()),
        RuntimeGeneration::INITIAL,
        Arc::new(yss_node_kernel::KernelRegistry::default()),
    );
    let plan_basis = PlanBasis::new(
        PlanProjectSessionId::from_existing("cache-session".into()),
        PlanRegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
        execution.kernels().fingerprint(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let graph =
        GraphResourcePath::new("events/Cache.yssbi-event").expect("test graph path is valid");
    let node_id = NodeId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type: "yssbi.constant.get"
                .parse()
                .expect("built-in node type is valid"),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::from([(
                "value".parse().expect("built-in parameter key is valid"),
                serde_json::json!(7),
            )]),
            user_label: None,
        },
    );
    set_constant(
        &mut document,
        node_id,
        yss_data_contract::DataType::Int64,
        yss_data_contract::DataValue::Int64(7),
    );
    let resources = empty_resource_catalog();
    let analysis_basis = basis(&runtime);

    let resolve =
        |document: &GraphDocument, resources: &ResourceCatalogSnapshot, supported: bool| {
            let analysis = runtime.resolve_graph_document(
                &graph,
                document,
                &analysis_basis,
                resources,
                &[],
                "en-US",
            );
            let semantics = analysis
                .semantic_snapshot()
                .clone()
                .with_execution_kernel_support(&|_| supported);
            analysis.with_semantic_snapshot(semantics)
        };
    let first = resolve(&document, &resources, true);
    let first_package = execution
        .prepare_graph_package(&graph, &first, plan_basis.clone())
        .unwrap();
    let unsupported = resolve(&document, &resources, false);
    assert_eq!(
        unsupported.semantic_snapshot().outcome(),
        &yss_graph_analysis::GraphResolutionOutcome::Incomplete
    );
    assert!(
        unsupported
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .any(|diagnostic| {
                diagnostic.blocking && diagnostic.code.as_str() == "graph.node.kernel_unavailable"
            })
    );
    assert!(
        execution
            .prepare_graph_package(&graph, &unsupported, plan_basis.clone())
            .is_err()
    );

    document.nodes.get_mut(&node_id).unwrap().position = NodePosition { x: 20.0, y: 40.0 };
    document.nodes.get_mut(&node_id).unwrap().user_label = Some("Renamed".into());
    let layout_only = resolve(&document, &resources, true);
    let layout_package = execution
        .prepare_graph_package(&graph, &layout_only, plan_basis.clone())
        .unwrap();
    assert!(Arc::ptr_eq(first_package.plan(), layout_package.plan()));
    assert_eq!(
        layout_only.semantic_input_hash(),
        first.semantic_input_hash()
    );
    assert!(!layout_only.semantic_snapshot().has_blocking_diagnostics());

    set_constant(
        &mut document,
        node_id,
        yss_data_contract::DataType::Int64,
        yss_data_contract::DataValue::Int64(8),
    );
    let semantic_change = resolve(&document, &resources, true);
    let changed_package = execution
        .prepare_graph_package(&graph, &semantic_change, plan_basis.clone())
        .unwrap();
    assert!(!Arc::ptr_eq(changed_package.plan(), first_package.plan()));
    assert_ne!(
        semantic_change.semantic_input_hash(),
        first.semantic_input_hash()
    );

    // A signature label change keeps ABI/hash identity but must refresh editor facts.
    let function = GraphResourcePath::new("functions/Labels.yssbi-function").unwrap();
    let resources = |label: &str| {
        ResourceCatalogSnapshot::new(
            BTreeMap::from([(
                function.clone(),
                yss_graph_resource_contract::FunctionCatalogEntry::new(
                    yss_graph_resource_contract::FunctionSignature::new(
                        vec![yss_graph_resource_contract::FunctionParameterContract::new(
                            yss_graph_document::FunctionParameterId::new("parameter"),
                            label,
                            yss_data_contract::DataType::Int64,
                        )],
                        None,
                    ),
                ),
            )]),
            BTreeMap::new(),
            yss_graph_resource_contract::ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    };
    let entry = document.nodes.get_mut(&node_id).unwrap();
    entry.node_type = "yssbi.project.function.entry".parse().unwrap();
    entry.parameters = ParameterValues::from([(
        "function".parse().unwrap(),
        serde_json::json!(function.as_str()),
    )]);
    let original = runtime.resolve_graph_document(
        &function,
        &document,
        &analysis_basis,
        &resources("Before"),
        &[],
        "en-US",
    );
    let original_package = execution
        .prepare_graph_package(&function, &original, plan_basis.clone())
        .unwrap();
    let renamed = runtime.resolve_graph_document(
        &function,
        &document,
        &analysis_basis,
        &resources("After"),
        &[],
        "en-US",
    );
    let renamed_package = execution
        .prepare_graph_package(&function, &renamed, plan_basis.clone())
        .unwrap();
    assert!(Arc::ptr_eq(original_package.plan(), renamed_package.plan()));
    assert_eq!(
        renamed.semantic_input_hash(),
        original.semantic_input_hash()
    );
    assert_eq!(
        renamed.semantic_snapshot().node(node_id).unwrap().ports[0]
            .label
            .as_ref(),
        "After"
    );
}
