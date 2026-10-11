use super::*;
use crate::graph::editing::GraphEditRequest;
use crate::graph::open::OpenGraphRequest;
use crate::graph::run::{RunDemand, RunGraphRequest, run_graph};
use yss_data_contract::{DataValue, TabularScalar, ValueType};
use yss_graph_document::{ConnectionId, DocumentConnection};
use yss_graph_editor::EditorGraphMutation;
use yss_graph_execution::plan::{NodeExecutionMode, PlanGraphId, PlanOutputRef, PlanPortAddress};
use yss_graph_execution::result::ResultCacheState;
use yss_node_kernel::RuntimeValue;

fn port(node: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node, key.parse().unwrap())
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

fn install(session: &StagedSession, graph: &GraphResourcePath, document: GraphDocument) {
    let overwrite = session
        .session
        .project()
        .capture_graph_overwrite_operation(
            session.session.project_instance_id(),
            graph,
            yss_project_identity::OperationId::new(),
        )
        .unwrap();
    session
        .session
        .project()
        .commit_graph_candidate(overwrite.into_authority(), Arc::new(document))
        .unwrap();
}

fn edit_request(session: &StagedSession, graph: &GraphResourcePath) -> GraphEditRequest {
    let instance = session.session.project_instance_id();
    GraphEditRequest {
        project_instance_id: instance.clone(),
        graph_path: graph.clone(),
        locale: "en-US".into(),
        operation_id: yss_project_identity::OperationId::new(),
        version: session
            .session
            .project()
            .read_graph_editing(instance, graph)
            .unwrap()
            .state
            .version,
    }
}

fn open(
    session: &StagedSession,
    graph: &GraphResourcePath,
) -> crate::graph::open::OpenGraphApplicationReceipt {
    session
        .application
        .open_graph(OpenGraphRequest::new(
            session.session.project_instance_id().clone(),
            graph.clone(),
            0,
            "en-US",
        ))
        .unwrap()
}

fn request(
    session: &StagedSession,
    graph: &GraphResourcePath,
    target: NodeId,
    mode: NodeExecutionMode,
) -> RunGraphRequest {
    let opened = open(session, graph);
    RunGraphRequest::new(
        session.session.project_instance_id().clone(),
        graph.clone(),
        opened.document().clone(),
        opened.projection().basis.semantic_input_hash,
    )
    .with_demand(RunDemand::Node {
        node_id: target,
        mode,
    })
}

fn output(graph: &GraphResourcePath, node: NodeId, key: &str) -> PlanOutputRef {
    PlanOutputRef::new(
        PlanGraphId::from_existing(graph.as_str().into()),
        PlanPortAddress::from_existing(port(node, key).to_string().into()),
    )
}

#[test]
fn inserted_reroutes_run_with_cached_inputs_and_follow_edits_and_undo() {
    let graph = GraphResourcePath::new("events/New Event.yssbi-event").unwrap();
    let session = staged_session(
        compatible_project(&graph),
        "reroute-scalar",
        GraphRuntimeTestControl::default(),
    );
    let source = NodeId::new();
    let mut document = compatible_draft(source);
    let viewer = node(&mut document, "yssbi.debug.view");
    connect(&mut document, port(source, "value"), port(viewer, "data"));
    install(&session, &graph, document);
    let mut routes = Vec::new();
    for _ in 0..2 {
        let before = open(&session, &graph);
        let connection_id = before
            .document()
            .connections
            .values()
            .find(|edge| edge.input == port(viewer, "data"))
            .unwrap()
            .id;
        let inserted = session
            .application
            .edit_graph(
                edit_request(&session, &graph),
                EditorGraphMutation::InsertReroute {
                    connection_id,
                    position: NodePosition { x: 100., y: 100. },
                },
            )
            .unwrap();
        routes.push(
            *inserted
                .update
                .document
                .nodes
                .keys()
                .find(|id| !before.document().nodes.contains_key(id))
                .unwrap(),
        );
    }
    let target = routes[1];
    run_graph(
        &session.application,
        request(&session, &graph, target, NodeExecutionMode::Dependencies),
    )
    .unwrap();
    let original = session
        .session
        .execution()
        .query_pin_result(&output(&graph, source, "value"))
        .unwrap();
    for route in &routes {
        let result = session
            .session
            .execution()
            .query_pin_result(&output(&graph, *route, "output"))
            .unwrap();
        assert_eq!(
            result.value().value(),
            &RuntimeValue::Scalar(TabularScalar::Integer(0))
        );
    }
    run_graph(
        &session.application,
        request(&session, &graph, target, NodeExecutionMode::CurrentInputs),
    )
    .unwrap();
    assert_eq!(
        session
            .session
            .execution()
            .query_pin_result(&output(&graph, source, "value"))
            .unwrap()
            .provenance()
            .result_id(),
        original.provenance().result_id()
    );
    let before = open(&session, &graph);
    let mut constant = before.document().constants.values().next().unwrap().clone();
    constant.data_value = DataValue::Integer(84);
    session
        .application
        .edit_graph(
            edit_request(&session, &graph),
            EditorGraphMutation::SetConstant {
                id: constant.id,
                constant: Some(constant),
            },
        )
        .unwrap();
    let changed = open(&session, &graph);
    for route in &routes {
        assert!(matches!(
            changed.result_state().outputs[&output(&graph, *route, "output")],
            ResultCacheState::Missing
        ));
    }
    assert!(matches!(
        run_graph(
            &session.application,
            request(&session, &graph, target, NodeExecutionMode::CurrentInputs)
        ),
        Err(
            crate::graph::run::ExecutionApplicationError::PreparedExecution(
                yss_graph_execution::error::ExecutePreparedError::Kernel(
                    yss_graph_execution::error::OperationExecutionError::InputResultUnavailable { .. }
                )
            )
        )
    ));
    session
        .application
        .change_graph_history(edit_request(&session, &graph), false)
        .unwrap();
    let restored = open(&session, &graph);
    for route in &routes {
        assert!(matches!(
            restored.result_state().outputs[&output(&graph, *route, "output")],
            ResultCacheState::Missing
        ));
    }
    session
        .application
        .change_graph_history(edit_request(&session, &graph), true)
        .unwrap();
    run_graph(
        &session.application,
        request(&session, &graph, viewer, NodeExecutionMode::Dependencies),
    )
    .unwrap();
    assert_eq!(
        session
            .session
            .execution()
            .query_pin_result(&output(&graph, target, "output"))
            .unwrap()
            .value()
            .value(),
        &RuntimeValue::Scalar(TabularScalar::Integer(84))
    );
}

#[test]
fn reroutes_forward_deferred_schema_and_share_one_materialized_relation() {
    let graph = GraphResourcePath::new("events/New Event.yssbi-event").unwrap();
    let session = staged_session(
        compatible_project(&graph),
        "reroute-schema",
        GraphRuntimeTestControl::default(),
    );
    let source = NodeId::new();
    let mut document = compatible_draft(source);
    set_constant(
        &mut document,
        source,
        ValueType::DataFrame,
        DataValue::String(r#"{"kept":[1,2],"empty":[null,null]}"#.into()),
    );
    yss_graph_document::normalize_constant_value(document.constants.values_mut().next().unwrap())
        .unwrap();
    let drop_columns = node(&mut document, "yssbi.dataframe.dropna.columns");
    let first = node(&mut document, "yssbi.core.reroute");
    let second = node(&mut document, "yssbi.core.reroute");
    let decompose = node(&mut document, "yssbi.dataframe.decompose");
    for (output, input) in [
        (port(source, "value"), port(drop_columns, "source")),
        (port(drop_columns, "result"), port(first, "input")),
        (port(first, "output"), port(second, "input")),
        (port(second, "output"), port(decompose, "dataframe")),
    ] {
        connect(&mut document, output, input);
    }
    install(&session, &graph, document);
    let before = open(&session, &graph);
    run_graph(
        &session.application,
        request(&session, &graph, second, NodeExecutionMode::Dependencies),
    )
    .unwrap();
    let after = open(&session, &graph);
    assert_eq!(
        before.editing(),
        after.editing(),
        "running does not edit or save the graph"
    );
    let producer = session
        .session
        .execution()
        .query_pin_result(&output(&graph, drop_columns, "result"))
        .unwrap();
    for route in [first, second] {
        let result = session
            .session
            .execution()
            .query_pin_result(&output(&graph, route, "output"))
            .unwrap();
        assert!(result.value().is_evaluated());
        assert_eq!(
            result.value().value(),
            producer.value().value(),
            "transparent routing shares the same materialized relation handle"
        );
        assert!(matches!(
            after.result_state().outputs[&output(&graph, route, "output")],
            ResultCacheState::Valid { .. }
        ));
    }
    let columns = |opened: &crate::graph::open::OpenGraphApplicationReceipt| {
        opened
            .projection()
            .nodes
            .iter()
            .find(|node| node.node_id == decompose)
            .unwrap()
            .ports
            .iter()
            .filter(|port| {
                port.direction == yss_node_protocol::PortDirection::Output && !port.orphan
            })
            .map(|port| port.display.label.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(columns(&after), ["kept"]);
    run_graph(
        &session.application,
        request(&session, &graph, second, NodeExecutionMode::CurrentInputs),
    )
    .unwrap();
    assert_eq!(
        session
            .session
            .execution()
            .query_pin_result(&output(&graph, second, "output"))
            .unwrap()
            .value()
            .value(),
        producer.value().value()
    );
    let mut constant = after.document().constants.values().next().unwrap().clone();
    constant.data_value = DataValue::String(r#"{"kept":[null,null],"empty":[4,5]}"#.into());
    constant.tabular = None;
    session
        .application
        .edit_graph(
            edit_request(&session, &graph),
            EditorGraphMutation::SetConstant {
                id: constant.id,
                constant: Some(constant),
            },
        )
        .unwrap();
    assert!(
        columns(&open(&session, &graph)).is_empty(),
        "changed inputs withdraw the routed observation"
    );
    run_graph(
        &session.application,
        request(&session, &graph, decompose, NodeExecutionMode::Dependencies),
    )
    .unwrap();
    let refreshed = open(&session, &graph);
    assert_eq!(columns(&refreshed), ["empty"]);
}
