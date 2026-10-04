use super::*;
use crate::graph::results::report::{ReportQueryError, ResultTablePart};
use crate::session::ApplicationState;
use std::{
    collections::BTreeMap,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{
    ConnectionId, ConstantId, DocumentConnection, DocumentNode, DynamicPortBinding, GraphConstant,
    GraphDocument, GraphResourcePath, NodeId, NodePosition, OrderKey, PortAddress, PortInstanceId,
};
use yss_graph_execution::{
    plan::{PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint},
    resource_preparation::RunResourceBindings,
    result::ResultReference,
    state::{ExecutionResultRequest, RunExecutionControl},
};
use yss_graph_resource_contract::ResourceCatalogSnapshot;

fn poisson(rows: usize) -> (ApplicationState, ResultReference) {
    let mut document = GraphDocument::default();
    let [source, response, predictor, model] = std::array::from_fn(|_| NodeId::new());
    for (id, kind, parameters) in [
        (source, "yssbi.constant.get", serde_json::json!({})),
        (
            response,
            "yssbi.dataframe.series.select",
            serde_json::json!({"column": "y"}),
        ),
        (
            predictor,
            "yssbi.dataframe.series.select",
            serde_json::json!({"column": "x"}),
        ),
        (
            model,
            "yssbi.statistics.regression.poisson",
            serde_json::json!({}),
        ),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: serde_json::from_value(parameters).unwrap(),
                user_label: None,
            },
        );
    }
    let id = ConstantId::new();
    let mut constant = GraphConstant {
        id,
        name: "data".into(),
        data_type: ValueType::DataFrame,
        data_value: DataValue::String(
            serde_json::json!({
                "y": (0..rows).map(|i| [1, 2, 3, 2, 4, 6][i % 6]).collect::<Vec<_>>(),
                "x": (0..rows).map(|i| i % 6 / 3).collect::<Vec<_>>(),
            })
            .to_string()
            .into(),
        ),
        tabular: None,
        description: String::new(),
        tags: vec![],
    };
    yss_graph_document::normalize_constant_value(&mut constant).unwrap();
    document.constants.insert(id, constant);
    document
        .nodes
        .get_mut(&source)
        .unwrap()
        .parameters
        .insert("constant".parse().unwrap(), id.to_string().into());
    let input = PortAddress::instance(model, "x".parse().unwrap(), PortInstanceId::new());
    document.port_bindings.insert(
        input.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey::new("0"),
        },
    );
    for (node, port, target) in [
        (
            source,
            "value",
            PortAddress::declared(response, "dataframe".parse().unwrap()),
        ),
        (
            source,
            "value",
            PortAddress::declared(predictor, "dataframe".parse().unwrap()),
        ),
        (
            response,
            "series",
            PortAddress::declared(model, "y".parse().unwrap()),
        ),
        (predictor, "series", input),
    ] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(node, port.parse().unwrap()),
                input: target,
                order: None,
            },
        );
    }
    execute_report(
        &document,
        "events/poisson.yssbi-event",
        "yssbi.statistics.regression.poisson",
    )
}

fn execute_report(
    document: &GraphDocument,
    graph: &str,
    node_type: &str,
) -> (ApplicationState, ResultReference) {
    let app = ApplicationState::initialize().unwrap();
    let captured = app.capture_session().unwrap();
    let runtime = captured.execution();
    let graph = GraphResourcePath::new(graph).unwrap();
    let resources = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
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
            &yss_graph_execution::graph_preparation::GraphExecutionScope::all(
                analysis.semantic_snapshot(),
            ),
        )
        .unwrap();
    let prepared = runtime
        .prepare_package(package.clone(), runtime.generation())
        .unwrap();
    let run = runtime
        .execute_prepared_handoff(
            &prepared,
            RunResourceBindings::new(session, [], []),
            captured.resource_provider_factory(),
            &RunExecutionControl::with_cancellation(
                Arc::new(AtomicBool::new(false)),
                Instant::now() + Duration::from_secs(30),
            ),
            ExecutionResultRequest {
                demand: &PlanExecutionDemand::Default,
                basis: None,
            },
            |_| {},
        )
        .unwrap();
    assert!(runtime.publish_committed_results(run.handoff()));
    runtime.finalize_run_success(run.run_id()).unwrap();
    let output = package
        .plan()
        .operations()
        .iter()
        .find(|operation| operation.node_type().as_str() == node_type)
        .unwrap()
        .outputs()[0]
        .output();
    let entry = run
        .handoff()
        .results()
        .iter()
        .find(|entry| entry.output() == output)
        .unwrap();
    let reference = ResultReference {
        execution_session_id: captured.execution_session_id(),
        result_id: entry.result_id(),
    };
    (app, reference)
}

#[test]
fn poisson_report_reads_bounded_json_and_complete_array_pages_with_a_lease() {
    let (app, reference) = poisson(53_940);
    let value = crate::result_encoding::query_result_json(&app, reference)
        .unwrap()
        .unwrap();
    assert!(
        serde_json::to_vec(&value).unwrap().len()
            < crate::result_encoding::MAX_INLINE_RESULT_JSON_BYTES
    );
    assert_eq!(value["observations"], 53_940);
    assert_eq!(value["coefficients"]["rowCount"], 2);
    assert_eq!(value["coefficients"]["kind"], "tableRef");
    fn contains_array(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Array(_) => true,
            serde_json::Value::Object(fields) => fields.values().any(contains_array),
            _ => false,
        }
    }
    assert!(
        !contains_array(&value),
        "report overview must not inline data arrays"
    );
    let coefficients = app
        .query_result_table(
            reference,
            value["coefficients"]["part"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            0,
            100,
        )
        .unwrap();
    assert_eq!(coefficients.values.len(), 2);
    for row in &coefficients.values {
        let wire = crate::result_encoding::runtime_value_to_json(row).unwrap();
        assert!((wire[0]["estimate"].as_f64().unwrap() - 2f64.ln()).abs() < 1e-6);
        assert_eq!(wire[0]["confidence_interval"]["kind"], "tableRef");
    }
    for key in ["fitted", "residuals"] {
        assert_eq!(value[key]["kind"], "tableRef");
        assert_eq!(value[key]["rowCount"], 53_940);
    }
    let lease = uuid::Uuid::new_v4();
    app.retain_result(reference, lease, "report", None).unwrap();
    app.capture_session()
        .unwrap()
        .execution()
        .invalidate_graph_results("events/poisson.yssbi-event");
    let part = value["fitted"]["part"].as_str().unwrap();
    let first = app
        .query_result_table(reference, part.parse().unwrap(), 0, 100)
        .unwrap();
    assert_eq!(first.values.len(), 100);
    assert!(first.has_more);
    let last = app
        .query_result_table(reference, part.parse().unwrap(), 53_930, 100)
        .unwrap();
    assert_eq!(last.values.len(), 10);
    assert_eq!(last.total_count, Some(53_940));
    assert!(!last.has_more);
    for (index, row) in last.values.iter().enumerate() {
        let RuntimeValue::List(cells) = row else {
            panic!("row");
        };
        let RuntimeValue::Scalar(TabularScalar::Float64(fitted)) = cells[0] else {
            panic!("number");
        };
        let expected = if (53_930 + index) % 6 < 3 { 2.0 } else { 4.0 };
        assert!((fitted.as_f64() - expected).abs() < 1e-6);
    }
    let mut foreign = reference;
    foreign.execution_session_id =
        yss_graph_execution::identity::ExecutionSessionId::new(uuid::Uuid::new_v4());
    assert!(matches!(
        app.query_result_table(foreign, part.parse().unwrap(), 0, 1),
        Err(ReportQueryError::Stale)
    ));
    app.release_result_lease(lease, "report").unwrap();
    assert!(matches!(
        app.query_result_table(reference, part.parse().unwrap(), 0, 1),
        Err(ReportQueryError::Unavailable)
    ));
}

#[test]
fn nested_array_parts_preserve_paths_values_and_page_limits() {
    let small = RuntimeValue::Record(Arc::new(
        [
            (
                "values".into(),
                RuntimeValue::List(Arc::from([TabularScalar::Integer(7).into()])),
            ),
            ("empty".into(), RuntimeValue::List(Arc::from([]))),
        ]
        .into(),
    ));
    let overview =
        crate::result_encoding::runtime_value_to_json(&project(&small).unwrap()).unwrap();
    assert_eq!(
        overview["values"],
        serde_json::json!({"kind":"tableRef", "part":"structured:/values", "rowCount":1})
    );
    assert_eq!(overview["empty"]["rowCount"], 0);
    assert_eq!(overview["empty"]["kind"], "tableRef");
    let column = RuntimeValue::List(
        (0..150)
            .map(|i| TabularScalar::Unsigned(u64::MAX - i).into())
            .collect(),
    );
    let nested = RuntimeValue::Record(Arc::new([("a/~".into(), column)].into()));
    let root = RuntimeValue::Record(Arc::new(
        [(
            "stages".into(),
            RuntimeValue::List(vec![nested; 120].into()),
        )]
        .into(),
    ));
    let overview = crate::result_encoding::runtime_value_to_json(&project(&root).unwrap()).unwrap();
    assert_eq!(overview["stages"]["part"], "structured:/stages");
    let stage = page(&root, "/stages", 119, 10).unwrap();
    assert_eq!(stage.values.len(), 1);
    let wire = crate::result_encoding::runtime_value_to_json(&stage.values[0]).unwrap();
    assert_eq!(wire[0]["a/~"]["part"], "structured:/stages/119/a~1~0");
    let tail = page(&root, "/stages/119/a~1~0", 149, 100).unwrap();
    let wire = crate::result_encoding::runtime_value_to_json(&tail.values[0]).unwrap();
    assert_eq!(wire[0], (u64::MAX - 149).to_string());
    assert!(!tail.has_more);
    for path in [
        "/stages/01/a~1~0",
        "/stages/120/a~1~0",
        "/stages/0/a~2",
        "/missing",
    ] {
        assert!(page(&root, path, 0, 1).is_err(), "{path}");
    }
    assert!(page(&root, "/stages", 0, MAX_RESULT_PAGE_ROWS + 1).is_err());
    assert!(page(&root, "/stages", usize::MAX, 1).is_err());
    assert!("structured:/bad~2".parse::<ResultTablePart>().is_err());
}

#[test]
fn description_result_returns_complete_numeric_and_categorical_json() {
    let mut document = GraphDocument::default();
    let [source, describe] = std::array::from_fn(|_| NodeId::new());
    for (id, kind) in [
        (source, "yssbi.constant.get"),
        (describe, "yssbi.statistics.describe"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: Default::default(),
                user_label: None,
            },
        );
    }
    let id = ConstantId::new();
    let mut constant = GraphConstant {
        id,
        name: "data".into(),
        data_type: ValueType::DataFrame,
        data_value: DataValue::String(
            serde_json::json!({
                " amount/~ ": [1, 3, null],
                "b": [4, 8, 12],
                "c": [9, null, null],
                "text": ["a", "b", "c"],
            })
            .to_string()
            .into(),
        ),
        tabular: None,
        description: String::new(),
        tags: vec![],
    };
    yss_graph_document::normalize_constant_value(&mut constant).unwrap();
    document.constants.insert(id, constant);
    document
        .nodes
        .get_mut(&source)
        .unwrap()
        .parameters
        .insert("constant".parse().unwrap(), id.to_string().into());
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(source, "value".parse().unwrap()),
            input: PortAddress::declared(describe, "source".parse().unwrap()),
            order: None,
        },
    );
    let (app, reference) = execute_report(
        &document,
        "events/describe.yssbi-event",
        "yssbi.statistics.describe",
    );
    let snapshot = app.query_result(reference).unwrap().unwrap();
    assert!(supports(&snapshot));
    assert!(matches!(snapshot.value().value(), RuntimeValue::Record(_)));
    let overview = crate::result_encoding::query_result_json(&app, reference)
        .unwrap()
        .unwrap();
    let columns = overview["columns"].as_object().unwrap();
    assert_eq!(columns.len(), 3);
    let numeric = &columns[" amount/~ "];
    assert_eq!(numeric["semantic"], "Numeric");
    assert_eq!(numeric["position"], 1);
    assert_eq!(numeric["count"], 2);
    assert_eq!(numeric["missing"], 1);
    assert_eq!(numeric["mean"], 2.0);
    assert_eq!(numeric["q25"], 1.5);
    assert!((numeric["std"].as_f64().unwrap() - 2f64.sqrt()).abs() < 1e-12);
    assert_eq!(numeric.as_object().unwrap().len(), 11);
    assert!(numeric.get("mode").is_none());
    assert!(numeric.get("categories").is_none());
    assert_eq!(columns["b"]["position"], 2);
    assert_eq!(columns["c"]["position"], 3);
    assert!(columns["c"]["std"].is_null());
    assert_eq!(columns["c"]["missing"], 2);

    let constant = document.constants.values_mut().next().unwrap();
    constant.data_type = ValueType::DataSeries(Box::new(ValueType::Scalar(
        yss_data_contract::SemanticType::Categorical,
    )));
    constant.data_value = DataValue::String(r#"{"value":[2,1,2,null]}"#.into());
    constant.tabular = None;
    yss_graph_document::normalize_constant_value(constant).unwrap();
    let (app, reference) = execute_report(
        &document,
        "events/describe.yssbi-event",
        "yssbi.statistics.describe",
    );
    let result = crate::result_encoding::query_result_json(&app, reference)
        .unwrap()
        .unwrap();
    assert_eq!(result["columns"].as_object().unwrap().len(), 1);
    let categorical = &result["columns"]["value"];
    assert_eq!(categorical["position"], 1);
    assert_eq!(categorical["semantic"], "Categorical");
    assert_eq!(categorical["count"], 3);
    assert_eq!(categorical["missing"], 1);
    assert_eq!(categorical["unique"], 2);
    assert_eq!(categorical.as_object().unwrap().len(), 6);
    for key in ["mode", "mode_count", "mode_proportion"] {
        assert!(categorical.get(key).is_none());
    }
    assert!(categorical.get("mean").is_none());
    assert_eq!(
        categorical["categories"],
        serde_json::json!({
            "1": {"value": 1, "label": "1", "frequency": 1, "proportion": 1. / 3.},
            "2": {"value": 2, "label": "2", "frequency": 2, "proportion": 2. / 3.}
        })
    );

    // Empty input retains only categorical fields; no null row is invented as a category.
    let constant = document.constants.values_mut().next().unwrap();
    constant.data_value = DataValue::String(r#"{"value":[null,null]}"#.into());
    constant.tabular = None;
    yss_graph_document::normalize_constant_value(constant).unwrap();
    let (app, reference) = execute_report(
        &document,
        "events/describe.yssbi-event",
        "yssbi.statistics.describe",
    );
    let result = crate::result_encoding::query_result_json(&app, reference)
        .unwrap()
        .unwrap();
    assert_eq!(
        result["columns"]["value"],
        serde_json::json!({
            "position": 1, "semantic": "Categorical", "count": 0, "missing": 2,
            "unique": 0, "categories": {}
        })
    );

    // The inline distribution is complete beyond the normal 100-row preview page.
    let constant = document.constants.values_mut().next().unwrap();
    constant.data_value = DataValue::String(
        serde_json::json!({"value": (0..205).collect::<Vec<_>>()})
            .to_string()
            .into(),
    );
    constant.tabular = None;
    yss_graph_document::normalize_constant_value(constant).unwrap();
    let (app, reference) = execute_report(
        &document,
        "events/describe.yssbi-event",
        "yssbi.statistics.describe",
    );
    let result = crate::result_encoding::query_result_json(&app, reference)
        .unwrap()
        .unwrap();
    let categories = &result["columns"]["value"]["categories"];
    assert_eq!(categories.as_object().unwrap().len(), 205);
    assert_eq!(
        categories["205"],
        serde_json::json!({
            "value": 204, "label": "204", "frequency": 1, "proportion": 1. / 205.
        })
    );
}
