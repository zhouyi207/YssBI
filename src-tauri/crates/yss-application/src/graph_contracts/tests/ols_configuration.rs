use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use yss_data_contract::DataType;
use yss_execution::{
    identity::{ExecutionSessionId, RuntimeGeneration},
    plan::{
        PlanCompilationBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
    },
    resource_preparation::{ResourceProviderFactory, RunResourceBindings},
    result::StoredResult,
    state::{ExecutionRuntimeState, RunExecutionControl},
    value::RuntimeValue,
};
use yss_graph_analysis_contract::CompilationBasis;
use yss_graph_document::{
    DocumentConnection, DocumentNode, GraphDocument, NodeId, NodePosition, ParameterValues,
    PortAddress,
};
use yss_graph_document_edit::apply_graph_document_patch;
use yss_graph_editor::{EditorGraphMutation, PortPlacement};
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};

#[test]
fn node_owned_ols_configuration_compiles_and_changes_computed_results() {
    let system = yss_graph_catalog::build_builtin_node_system().unwrap();
    let runtime = GraphRuntimeState::from_components(
        GraphRuntimeEpoch::from_existing(1),
        GraphRuntimeComponents {
            registry: system.registry.clone(),
            catalog: system.catalog,
        },
    );
    let graph = "events/ols.yssbi-event".parse().unwrap();
    let mut document = GraphDocument::default();
    let [response, predictor, fit, summary] = std::array::from_fn(|_| NodeId::new());
    for (id, node_type) in [
        (response, "yssbi.constant.get"),
        (predictor, "yssbi.constant.get"),
        (fit, "yssbi.statistics.ols.fit"),
        (summary, "yssbi.statistics.ols.summary"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: node_type.parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                user_label: None,
                parameters: ParameterValues::new(),
            },
        );
    }
    let series = [
        (response, "Response", vec![2., 4., 5., 4., 7., 8.]),
        (predictor, "Predictor", vec![1., 2., 3., 4., 5., 6.]),
    ];
    for (node, name, values) in &series {
        let id = yss_graph_document::ConstantId::new();
        let mut constant = yss_graph_document::GraphConstant {
            id,
            name: (*name).into(),
            data_type: DataType::DataSeries(Box::new(DataType::Float64)),
            data_value: yss_data_contract::DataValue::DataSeries(
                yss_data_contract::DataSeriesValue::with_element_type(
                    serde_json::json!({"value": values}).to_string(),
                    DataType::Float64,
                ),
            ),
            tabular: None,
            description: String::new(),
            tags: vec![],
        };
        yss_graph_document::normalize_constant_value(&mut constant).unwrap();
        document.constants.insert(id, constant);
        document
            .nodes
            .get_mut(node)
            .unwrap()
            .parameters
            .insert("constant".parse().unwrap(), id.to_string().into());
    }
    let port = |node, key: &str| PortAddress::declared(node, key.parse().unwrap());
    let apply = |document: &mut GraphDocument, mutation: EditorGraphMutation| {
        let patch = mutation
            .into_patch(&graph, document, &system.registry)
            .unwrap();
        apply_graph_document_patch(document, &patch).unwrap();
    };
    for target in [fit, summary] {
        apply(
            &mut document,
            EditorGraphMutation::AddPortInstance {
                node_id: target,
                template_key: "predictors".parse().unwrap(),
                placement: PortPlacement::Append,
            },
        );
        let input = document
            .port_bindings
            .keys()
            .find(|address| address.node_id == target)
            .unwrap()
            .clone();
        // Graph constants supply both the declared series type and its values.
        for (source, input) in [(response, port(target, "response")), (predictor, input)] {
            let source = if target == summary {
                let mut node = document.nodes[&source].clone();
                node.id = NodeId::new();
                let id = node.id;
                document.nodes.insert(id, node);
                id
            } else {
                source
            };
            let output = port(source, "value");
            let id = yss_graph_document::ConnectionId::new();
            document.connections.insert(
                id,
                DocumentConnection {
                    id,
                    output,
                    input,
                    order: None,
                },
            );
        }
    }
    let catalog = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([1; 32]),
    );
    let basis = CompilationBasis {
        registry_fingerprint: yss_graph_registry::RegistryFingerprint::from_bytes(
            runtime.registry_fingerprint(),
        ),
        resource_versions: BTreeMap::new(),
        resource_observations: BTreeMap::new(),
    };
    let execute = |document: &GraphDocument| {
        let compiled = runtime
            .compile_draft(document, graph.clone(), &catalog, &basis)
            .unwrap();
        let projection = crate::editor_projection::build_editor_projection(
            crate::editor_projection::EditorProjectionInput {
                graph_path: &graph,
                document,
                analysis: compiled.analysis(),
                registry_fingerprint: runtime.registry_fingerprint(),
            },
        )
        .unwrap();
        let node = projection
            .nodes
            .iter()
            .find(|node| node.node_id == fit)
            .unwrap();
        assert!(
            node.ports
                .iter()
                .all(|input| input.address != port(fit, "configuration"))
        );
        let parameter = node
            .parameters
            .iter()
            .find(|parameter| parameter.key.as_str() == "configuration")
            .unwrap();
        let Some(crate::editor_projection::EditorParameterConfiguration::Configuration { fields }) =
            &parameter.configuration
        else {
            panic!("configuration must be projected as a node parameter");
        };
        let selected = fields
            .iter()
            .find(|field| field.key.as_str() == "covariance")
            .unwrap();
        assert!(
            matches!(&selected.configuration, Some(crate::editor_projection::EditorParameterConfiguration::SelectOptions { options }) if options.iter().any(|option| option.as_ref() == "HC1"))
        );
        let expected_fields = match selected
            .value
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .unwrap()
        {
            "HAC" => vec!["constant", "covariance", "kernel", "bandwidth"],
            "fixed scale" => vec!["constant", "covariance", "scale"],
            "newey" => vec!["constant", "covariance", "lag"],
            _ => vec!["constant", "covariance"],
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.key.as_str())
                .collect::<Vec<_>>(),
            expected_fields
        );
        let artifact_id = compiled.artifact_id().unwrap_or_else(|| {
            panic!(
                "{:#?}",
                compiled.analysis().semantic_snapshot().diagnostics()
            )
        });
        let package = runtime
            .compiled_draft(&graph, artifact_id)
            .unwrap()
            .package()
            .clone();
        let execution_basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes(runtime.registry_fingerprint()),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let package =
            crate::graph_contracts::execution_package_from_graph(package, execution_basis).unwrap();
        let execution = ExecutionRuntimeState::from_composition(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            RuntimeGeneration::INITIAL,
            Arc::new(yss_sci_runtime::SciRuntimeBackend::new()),
        );
        let plan = execution
            .prepare_compiled_package(package, RuntimeGeneration::INITIAL)
            .unwrap();
        let run = execution
            .execute_prepared_handoff(
                &plan,
                RunResourceBindings::new(
                    PlanProjectSessionId::from_existing("session".into()),
                    [],
                    [],
                ),
                &ResourceProviderFactory::new("session".into()),
                &RunExecutionControl::with_cancellation(
                    Arc::new(AtomicBool::new(false)),
                    Instant::now() + Duration::from_secs(30),
                ),
                &PlanExecutionDemand::Default,
                |_| {},
            )
            .unwrap();
        let results: BTreeMap<_, _> = run
            .handoff()
            .results()
            .iter()
            .map(|result| {
                (
                    result.output().port().as_str().to_owned(),
                    result.value().value().clone(),
                )
            })
            .collect();
        execution.finalize_run_success(run.run_id()).unwrap();
        results
    };
    let default_results = execute(&document);
    for target in [fit, summary] {
        apply(
            &mut document,
            EditorGraphMutation::SetConfiguration {
                node_id: target,
                key: "configuration".parse().unwrap(),
                values: [
                    ("constant".parse().unwrap(), false.into()),
                    ("covariance".parse().unwrap(), "HC1".into()),
                ]
                .into(),
            },
        );
    }
    let local = execute(&document);
    let model_key = port(fit, "model").to_string();
    let report_key = port(summary, "report").to_string();
    let StoredResult::Runtime(RuntimeValue::Record(model)) = &local[&model_key] else {
        panic!("model must be a record");
    };
    assert_eq!(model["constant"], RuntimeValue::Bool(false));
    let RuntimeValue::List(coefficients) = &model["coefficients"] else {
        panic!("coefficients must be a list");
    };
    assert_eq!(coefficients.len(), 1);
    let RuntimeValue::Decimal(coefficient) = coefficients[0] else {
        panic!("coefficient must be numeric");
    };
    assert!((coefficient - 124.0 / 91.0).abs() < 1e-10);
    assert_ne!(default_results[&model_key], local[&model_key]);
    assert_ne!(default_results[&report_key], local[&report_key]);
    let saved = serde_json::from_slice(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(local, execute(&saved));
    // Isolate covariance from the intercept setting: coefficients stay fixed,
    // while the uncertainty reported by the real scientific backend changes.
    for target in [fit, summary] {
        apply(
            &mut document,
            EditorGraphMutation::SetConfiguration {
                node_id: target,
                key: "configuration".parse().unwrap(),
                values: [
                    ("constant".parse().unwrap(), false.into()),
                    ("covariance".parse().unwrap(), "fixed scale".into()),
                    ("scale".parse().unwrap(), 2.into()),
                ]
                .into(),
            },
        );
    }
    let fixed_scale = execute(&document);
    assert_eq!(fixed_scale[&model_key], local[&model_key]);
    assert_ne!(fixed_scale[&report_key], local[&report_key]);
    for target in [fit, summary] {
        document
            .nodes
            .get_mut(&target)
            .unwrap()
            .parameters
            .get_mut(
                &"configuration"
                    .parse::<yss_graph_protocol::ParameterKey>()
                    .unwrap(),
            )
            .unwrap()["scale"] = "2".into();
    }
    assert_eq!(execute(&document)[&report_key], fixed_scale[&report_key]);
    for covariance in ["HAC", "newey"] {
        for target in [fit, summary] {
            apply(
                &mut document,
                EditorGraphMutation::SetConfiguration {
                    node_id: target,
                    key: "configuration".parse().unwrap(),
                    values: [("covariance".parse().unwrap(), covariance.into())].into(),
                },
            );
        }
        assert_eq!(execute(&document)[&model_key], local[&model_key]);
    }
    document
        .nodes
        .get_mut(&fit)
        .unwrap()
        .parameters
        .insert("configuration".parse().unwrap(), serde_json::json!({}));
    assert!(
        runtime
            .compile_draft(&document, graph, &catalog, &basis)
            .unwrap()
            .artifact_id()
            .is_none()
    );
}
