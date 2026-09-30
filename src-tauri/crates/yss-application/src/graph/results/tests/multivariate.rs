use crate::session::ApplicationState;
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicPortBinding, GraphDocument,
    GraphResourcePath, NodeId, NodePosition, OrderKey, ParameterValues, PortAddress,
    PortInstanceId,
};
use yss_graph_execution::{
    plan::{PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint},
    resource_preparation::RunResourceBindings,
    state::RunExecutionControl,
};
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};

fn node(document: &mut GraphDocument, kind: &str) -> NodeId {
    let id = NodeId::new();
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: kind.parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    id
}
fn connect(document: &mut GraphDocument, output: PortAddress, input: PortAddress) {
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
}
fn frame(document: &mut GraphDocument, data: serde_json::Value) -> NodeId {
    let source = node(document, "yssbi.constant.get");
    let id = yss_graph_document::ConstantId::new();
    let mut constant = yss_graph_document::GraphConstant {
        id,
        name: id.to_string(),
        data_type: ValueType::DataFrame,
        data_value: DataValue::String(data.to_string().into()),
        tabular: None,
        description: String::new(),
        tags: vec![],
    };
    yss_graph_document::normalize_constant_value(&mut constant).unwrap();
    document.constants.insert(id, constant);
    document.nodes.get_mut(&source).unwrap().parameters.insert(
        "constant".parse().unwrap(),
        serde_json::json!(id.to_string()),
    );
    source
}
fn select(document: &mut GraphDocument, source: NodeId, output: &str, column: &str) -> NodeId {
    let id = node(document, "yssbi.dataframe.series.select");
    document
        .nodes
        .get_mut(&id)
        .unwrap()
        .parameters
        .insert("column".parse().unwrap(), serde_json::json!(column));
    connect(
        document,
        PortAddress::declared(source, output.parse().unwrap()),
        PortAddress::declared(id, "dataframe".parse().unwrap()),
    );
    id
}
fn group(document: &mut GraphDocument, id: NodeId, key: &str, sources: &[NodeId]) {
    for (index, &source) in sources.iter().enumerate() {
        let address = PortAddress::instance(id, key.parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(index.to_string()),
            },
        );
        connect(
            document,
            PortAddress::declared(source, "series".parse().unwrap()),
            address,
        );
    }
}
fn execute(
    app: &ApplicationState,
    document: &GraphDocument,
) -> (
    yss_graph_execution::plan::ExecutionPlanPackage,
    Vec<(
        yss_graph_execution::plan::PlanOutputRef,
        yss_graph_execution::result::ResultReference,
    )>,
) {
    let captured = app.capture_session().unwrap();
    let runtime = captured.execution();
    let graph = GraphResourcePath::new("events/multivariate.yssbi-event").unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let analysis = captured.graph().resolve_graph_document(
        &graph,
        document,
        &yss_graph_analysis_contract::GraphAnalysisBasis {
            registry_fingerprint: yss_node_registry::RegistryFingerprint::from_bytes(
                captured.graph().registry_fingerprint(),
            ),
            kernel_fingerprint: runtime.kernels().fingerprint().as_bytes(),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        },
        &resources,
        &[],
        "en-US",
    );
    assert!(
        !analysis.semantic_snapshot().has_blocking_diagnostics(),
        "{:?}",
        analysis.semantic_snapshot().diagnostics()
    );
    let session =
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into());
    let package = runtime
        .prepare_graph_package(
            &graph,
            &analysis,
            PlanBasis::new(
                session.clone(),
                PlanRegistryFingerprint::from_bytes(captured.graph().registry_fingerprint()),
                runtime.kernels().fingerprint(),
                BTreeMap::new(),
                BTreeMap::new(),
            ),
        )
        .unwrap();
    let prepared = runtime
        .prepare_package(package.clone(), runtime.generation())
        .unwrap();
    let execution = runtime
        .execute_prepared_handoff(
            &prepared,
            RunResourceBindings::new(session, [], []),
            captured.resource_provider_factory(),
            &RunExecutionControl::with_cancellation(
                Arc::new(AtomicBool::new(false)),
                Instant::now() + Duration::from_secs(30),
            ),
            yss_graph_execution::state::ExecutionResultRequest {
                demand: &PlanExecutionDemand::Default,
                basis: None,
            },
            |_| {},
        )
        .unwrap();
    assert!(runtime.publish_committed_results(execution.handoff()));
    runtime.finalize_run_success(execution.run_id()).unwrap();
    let references = execution
        .handoff()
        .results()
        .iter()
        .map(|entry| {
            (
                entry.output().clone(),
                yss_graph_execution::result::ResultReference {
                    execution_session_id: captured.execution_session_id(),
                    result_id: entry.result_id(),
                },
            )
        })
        .collect();
    (package, references)
}

#[test]
fn multivariate_nodes_execute_publish_summaries_and_page_connectable_scores() {
    let app = ApplicationState::initialize().unwrap();
    let mut document = GraphDocument::default();
    let n = 64;
    let loadings = [
        [0.8, 0.1],
        [0.7, 0.2],
        [0.75, 0.05],
        [0.1, 0.8],
        [0.2, 0.7],
        [0.05, 0.75],
    ];
    let basis = |frequency: usize, row: usize| {
        ((n - 1) as f64 * 2.0 / n as f64).sqrt()
            * (2.0 * std::f64::consts::PI * frequency as f64 * row as f64 / n as f64).sin()
    };
    let mut values = serde_json::Map::new();
    for (j, l) in loadings.iter().enumerate() {
        values.insert(
            format!("v{}", j + 1),
            serde_json::json!(
                (0..n)
                    .map(|i| l[0] * basis(1, i)
                        + l[1] * basis(2, i)
                        + (1.0_f64 - l[0] * l[0] - l[1] * l[1]).sqrt() * basis(j + 3, i))
                    .collect::<Vec<_>>()
            ),
        );
    }
    values.insert(
        "class".into(),
        serde_json::json!(
            (0..n)
                .map(|i| if basis(1, i) >= 0.0 { "A" } else { "B" })
                .collect::<Vec<_>>()
        ),
    );
    let source = frame(&mut document, serde_json::Value::Object(values));
    let variables = (1..=6)
        .map(|j| select(&mut document, source, "value", &format!("v{j}")))
        .collect::<Vec<_>>();
    let classes = select(&mut document, source, "value", "class");
    let table = frame(
        &mut document,
        serde_json::json!({"c1":[12,3,5],"c2":[4,15,8],"c3":[7,6,11]}),
    );
    let counts = (1..=3)
        .map(|j| select(&mut document, table, "value", &format!("c{j}")))
        .collect::<Vec<_>>();
    let mut targets = Vec::new();
    let mut canonical = None;
    for method in [
        "pca",
        "exploratory_factor",
        "correspondence",
        "discriminant",
        "rda",
        "mds",
        "canonical",
    ] {
        let kind = if method == "canonical" {
            "yssbi.statistics.association.canonical".to_owned()
        } else {
            format!("yssbi.statistics.multivariate.{method}")
        };
        let id = node(&mut document, &kind);
        targets.push((method, id));
        match method {
            "correspondence" => group(&mut document, id, "columns", &counts),
            "canonical" => {
                group(&mut document, id, "x", &variables[..2]);
                group(&mut document, id, "y", &variables[3..5]);
                canonical = Some(id);
            }
            "rda" => {
                group(&mut document, id, "responses", &variables[3..5]);
                group(&mut document, id, "constraints", &variables[..2]);
            }
            "discriminant" => {
                connect(
                    &mut document,
                    PortAddress::declared(classes, "series".parse().unwrap()),
                    PortAddress::declared(id, "groups".parse().unwrap()),
                );
                group(&mut document, id, "variables", &variables[..2]);
            }
            _ => group(
                &mut document,
                id,
                "variables",
                if method == "exploratory_factor" {
                    &variables
                } else {
                    &variables[..3]
                },
            ),
        }
    }
    let canonical = canonical.unwrap();
    let sx = select(&mut document, canonical, "scores", "x_axis1");
    let sy = select(&mut document, canonical, "scores", "y_axis1");
    let pearson = node(&mut document, "yssbi.statistics.association.pearson");
    connect(
        &mut document,
        PortAddress::declared(sx, "series".parse().unwrap()),
        PortAddress::declared(pearson, "x".parse().unwrap()),
    );
    connect(
        &mut document,
        PortAddress::declared(sy, "series".parse().unwrap()),
        PortAddress::declared(pearson, "y".parse().unwrap()),
    );
    let (package, references) = execute(&app, &document);
    let reference = |id: NodeId, index: usize| {
        let operation = package
            .plan()
            .operations()
            .iter()
            .find(|op| op.node_type().as_str() == document.nodes[&id].node_type.as_str())
            .unwrap();
        let output = operation.outputs()[index].output();
        references.iter().find(|(key, _)| key == output).unwrap().1
    };
    let mut correlation = 0.0;
    for (method, id) in targets {
        let report = crate::result_encoding::query_result_json(&app, reference(id, 0))
            .unwrap()
            .unwrap();
        assert!(report.is_object(), "{method}");
        let page = app
            .query_result_page(reference(id, 1), 0, 3)
            .unwrap()
            .unwrap();
        assert_eq!(page.values.len(), 3, "{method}");
        assert_eq!(
            page.columns.len(),
            match method {
                "pca" | "mds" | "canonical" => 2,
                _ => 1,
            }
        );
        if method == "correspondence" {
            assert_eq!(
                app.query_result_page(reference(id, 2), 0, 3)
                    .unwrap()
                    .unwrap()
                    .values
                    .len(),
                3
            );
        }
        if method == "canonical" {
            correlation = report["correlations"][0].as_f64().unwrap();
            assert_eq!(page.columns[0].name.as_ref(), "x_axis1");
            assert_eq!(page.columns[1].name.as_ref(), "y_axis1");
        }
    }
    let pearson_report = crate::result_encoding::query_result_json(&app, reference(pearson, 0))
        .unwrap()
        .unwrap();
    assert!((pearson_report["coefficient"].as_f64().unwrap() - correlation).abs() < 1e-9);
}

#[test]
fn discriminant_new_rows_from_another_domain_preserve_wide_class_labels() {
    let app = ApplicationState::initialize().unwrap();
    let mut document = GraphDocument::default();
    let a = 9_007_199_254_740_992i64;
    let b = a + 1;
    let training = frame(
        &mut document,
        serde_json::json!({"x":[0.0,0.8,1.4,-0.3,4.0,4.6,5.1,3.7],"y":[1.0,1.3,0.2,-0.7,4.0,5.3,3.8,4.7],"class":[a,a,a,a,b,b,b,b]}),
    );
    let new = frame(
        &mut document,
        serde_json::json!({"x":[1.0,4.5,2.8],"y":[1.0,4.5,1.7]}),
    );
    let tx = select(&mut document, training, "value", "x");
    let ty = select(&mut document, training, "value", "y");
    let labels = select(&mut document, training, "value", "class");
    let nx = select(&mut document, new, "value", "x");
    let ny = select(&mut document, new, "value", "y");
    let id = node(&mut document, "yssbi.statistics.multivariate.discriminant");
    connect(
        &mut document,
        PortAddress::declared(labels, "series".parse().unwrap()),
        PortAddress::declared(id, "groups".parse().unwrap()),
    );
    group(&mut document, id, "variables", &[tx, ty]);
    group(&mut document, id, "new_variables", &[nx, ny]);
    let (package, references) = execute(&app, &document);
    let op = package
        .plan()
        .operations()
        .iter()
        .find(|op| op.node_type().as_str() == "yssbi.statistics.multivariate.discriminant")
        .unwrap();
    let reference = references
        .iter()
        .find(|(output, _)| output == op.outputs()[1].output())
        .unwrap()
        .1;
    let page = app.query_result_page(reference, 0, 3).unwrap().unwrap();
    assert_eq!(page.values.len(), 3);
    let data = page
        .values
        .iter()
        .map(crate::result_encoding::runtime_value_to_json)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        data,
        vec![
            serde_json::json!([a.to_string()]),
            serde_json::json!([b.to_string()]),
            serde_json::json!([a.to_string()])
        ]
    );
}
