use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_data_contract::TabularScalar;
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath, NodeId,
    NodePosition, ParameterValues, PortAddress,
};
use yss_graph_execution::plan::{
    PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_graph_execution::resource_preparation::RunResourceBindings;
use yss_graph_execution::state::RunExecutionControl;
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_kernel::RuntimeValue;

#[test]
fn anova_nodes_execute_graph_defaults_with_mixed_factor_labels_and_interactions() {
    use yss_data_contract::{DataValue, ValueType};
    use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};
    let app = crate::session::ApplicationState::initialize().unwrap();
    let mut document = GraphDocument::default();
    let source = NodeId::new();
    document.nodes.insert(
        source,
        DocumentNode {
            id: source,
            node_type: "yssbi.constant.get".parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    let mut a = Vec::new();
    let mut b = Vec::new();
    let mut c = Vec::new();
    let mut subjects = Vec::new();
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut z = Vec::new();
    for i in 0..32 {
        let av = i / 4 % 2;
        let bv = i / 2 % 2;
        let cv = i % 2;
        let noise = ((i * 7 % 19) as f64 - 9.0) * 0.07;
        a.push(if av == 0 { "control" } else { "treatment" });
        b.push(bv);
        c.push(9_007_199_254_740_992i64 + cv as i64);
        subjects.push(format!("subject{}", i / 8));
        x.push((i * 7 % 29) as f64);
        let response = 1.0
            + av as f64 * 2.0
            + bv as f64 * 0.7
            + cv as f64 * 0.4
            + (av * bv * cv) as f64 * 1.3
            + noise;
        y.push(response);
        z.push(response * 0.3 + noise * noise + (i % 5) as f64 * 0.05);
    }
    set_constant(
        &mut document,
        source,
        ValueType::DataFrame,
        DataValue::String(
            serde_json::json!({"a":a,"b":b,"c":c,"subjects":subjects,"x":x,"y":y,"z":z})
                .to_string()
                .into(),
        ),
    );
    for constant in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(constant).unwrap();
    }
    let mut selectors = BTreeMap::new();
    for column in ["a", "b", "c", "subjects", "x", "y", "z"] {
        let id = NodeId::new();
        selectors.insert(column, id);
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: "yssbi.dataframe.series.select".parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: serde_json::from_value(serde_json::json!({"column":column})).unwrap(),
                user_label: None,
            },
        );
        let connection = ConnectionId::new();
        document.connections.insert(
            connection,
            DocumentConnection {
                id: connection,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input: PortAddress::declared(id, "dataframe".parse().unwrap()),
                order: None,
            },
        );
    }
    for method in [
        "one_way",
        "two_way",
        "three_way",
        "factorial",
        "ancova",
        "manova",
        "repeated_measures",
    ] {
        let node = NodeId::new();
        let kind = format!("yssbi.statistics.anova.{method}");
        document.nodes.insert(
            node,
            DocumentNode {
                id: node,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let mut fixed = if method == "manova" {
            vec![]
        } else {
            vec![("response", "y")]
        };
        if method == "repeated_measures" {
            fixed.push(("subjects", "subjects"));
        }
        for (input, column) in fixed {
            let connection = ConnectionId::new();
            document.connections.insert(
                connection,
                DocumentConnection {
                    id: connection,
                    output: PortAddress::declared(selectors[column], "series".parse().unwrap()),
                    input: PortAddress::declared(node, input.parse().unwrap()),
                    order: None,
                },
            );
        }
        let factor_count = match method {
            "one_way" => 1,
            "two_way" => 2,
            _ => 3,
        };
        let mut groups = vec![("factors", ["a", "b", "c"][..factor_count].to_vec())];
        if method == "manova" {
            groups.push(("responses", vec!["y", "z"]));
        }
        if method == "ancova" {
            groups.push(("covariates", vec!["x"]));
        }
        for (input, columns) in groups {
            for (index, column) in columns.into_iter().enumerate() {
                let address =
                    PortAddress::instance(node, input.parse().unwrap(), PortInstanceId::new());
                document.port_bindings.insert(
                    address.clone(),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey::new(index.to_string()),
                    },
                );
                let connection = ConnectionId::new();
                document.connections.insert(
                    connection,
                    DocumentConnection {
                        id: connection,
                        output: PortAddress::declared(selectors[column], "series".parse().unwrap()),
                        input: address,
                        order: None,
                    },
                );
            }
        }
    }
    let captured = app.capture_session().unwrap();
    let runtime = captured.execution();
    let graph = GraphResourcePath::new("events/variance.yssbi-event").unwrap();
    let resources = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
    let analysis = captured.graph().resolve_graph_document(
        &graph,
        &document,
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
    for method in [
        "one_way",
        "two_way",
        "three_way",
        "factorial",
        "ancova",
        "manova",
        "repeated_measures",
    ] {
        let kind = format!("yssbi.statistics.anova.{method}");
        let output = package
            .plan()
            .operations()
            .iter()
            .find(|operation| operation.node_type().as_str() == kind)
            .unwrap()
            .outputs()[0]
            .output();
        let entry = execution
            .handoff()
            .results()
            .iter()
            .find(|entry| entry.output() == output)
            .unwrap();
        let reference = yss_graph_execution::result::ResultReference {
            execution_session_id: captured.execution_session_id(),
            result_id: entry.result_id(),
        };
        let crate::graph::results::ResultValueProjection::Value(RuntimeValue::Record(result)) =
            app.query_result_projection(reference).unwrap().unwrap()
        else {
            panic!("{method} report");
        };
        assert_eq!(
            result["observations"],
            RuntimeValue::Scalar(TabularScalar::Integer(32))
        );
        let wire = crate::result_encoding::query_result_json(&app, reference)
            .unwrap()
            .unwrap();
        let factors = app
            .query_result_table(
                reference,
                wire["factors"]["part"].as_str().unwrap().parse().unwrap(),
                0,
                10,
            )
            .unwrap();
        let first = crate::result_encoding::runtime_value_to_json(&factors.values[0]).unwrap();
        let levels = app
            .query_result_table(
                reference,
                first[0]["levels"]["part"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap(),
                0,
                10,
            )
            .unwrap();
        let levels = levels
            .values
            .iter()
            .map(crate::result_encoding::runtime_value_to_json)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            levels,
            vec![
                serde_json::json!(["control"]),
                serde_json::json!(["treatment"])
            ]
        );
        assert_eq!(wire["table"]["kind"], "tableRef");
        if method != "repeated_measures" {
            assert_eq!(wire["options"]["sums_of_squares"], "type_iii");
        }
        let table = app
            .query_result_table(
                reference,
                wire["table"]["part"].as_str().unwrap().parse().unwrap(),
                0,
                100,
            )
            .unwrap();
        assert_eq!(
            table.values.len(),
            match method {
                "one_way" => 1,
                "two_way" => 3,
                "ancova" => 8,
                _ => 7,
            }
        );
    }
}

fn set_constant(
    document: &mut GraphDocument,
    node: NodeId,
    data_type: yss_data_contract::ValueType,
    data_value: yss_data_contract::DataValue,
) {
    let id = yss_graph_document::ConstantId::from_uuid(node.as_uuid());
    document.constants.insert(
        id,
        yss_graph_document::GraphConstant {
            id,
            name: id.to_string(),
            data_type,
            data_value,
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    document.nodes.get_mut(&node).unwrap().parameters = ParameterValues::from([(
        "constant".parse().unwrap(),
        serde_json::json!(id.to_string()),
    )]);
}
