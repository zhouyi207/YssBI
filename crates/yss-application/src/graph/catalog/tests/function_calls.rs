use super::*;
mod groups;
use crate::graph::editing::GraphEditRequest;
use crate::graph::run::{RunApplicationEventKind, RunGraphRequest, run_graph_with_sink};
use yss_data_contract::{DataValue, TabularScalar, ValueType};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, LastKnownPortMetadata, OrderKey, PortInstanceId,
};
use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};

fn node(document: &mut GraphDocument, kind: &str, parameters: ParameterValues) -> NodeId {
    let id = NodeId::new();
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: kind.parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters,
            user_label: None,
        },
    );
    id
}

fn port(node: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node, key.parse().unwrap())
}

fn member(
    document: &mut GraphDocument,
    node: NodeId,
    template: &str,
    function: &GraphResourcePath,
    name: &str,
) -> PortAddress {
    let address = PortAddress::instance(node, template.parse().unwrap(), PortInstanceId::new());
    document.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: function.clone(),
                parameter: FunctionParameterId::new(name),
            },
            order: OrderKey::new("0"),
            last_known: LastKnownPortMetadata::default(),
        },
    );
    address
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

fn reference(key: &str, path: &GraphResourcePath) -> ParameterValues {
    [(key.parse().unwrap(), serde_json::json!(path.as_str()))].into()
}

fn function(
    path: &GraphResourcePath,
    callee: Option<&GraphResourcePath>,
) -> (GraphResourceDocument, NodeId) {
    let mut resource =
        GraphResourceDocument::new(path.display_name(), GraphResourceKind::FunctionGraph);
    let signature = &mut resource.function.as_mut().unwrap().signature;
    signature.parameters = vec![yss_project_history::FunctionParameter {
        id: FunctionParameterId::new("frame"),
        name: "Data".into(),
        type_name: "DataFrame".into(),
    }];
    signature.return_type = Some("DataFrame".into());
    let body = Arc::make_mut(&mut resource.document);
    let entry = node(
        body,
        "yssbi.project.function.entry",
        reference("function", path),
    );
    let exit = node(
        body,
        "yssbi.project.function.return",
        reference("function", path),
    );
    let arg = member(body, entry, "parameters", path, "frame");
    let returned = member(body, exit, "results", path, "return");
    let operation = if let Some(callee) = callee {
        let call = node(
            body,
            "yssbi.project.function.call",
            reference("target", callee),
        );
        let input = member(body, call, "arguments", callee, "frame");
        let output = member(body, call, "results", callee, "return");
        connect(body, arg, input);
        connect(body, output, returned);
        call
    } else {
        let drop_columns = node(
            body,
            "yssbi.dataframe.dropna.columns",
            ParameterValues::new(),
        );
        let project = node(
            body,
            "yssbi.dataframe.project",
            [("columns".parse().unwrap(), serde_json::json!(["sales"]))].into(),
        );
        connect(body, arg, port(drop_columns, "source"));
        connect(body, port(drop_columns, "result"), port(project, "source"));
        connect(body, port(project, "result"), returned);
        project
    };
    (resource, operation)
}

#[test]
fn nested_dataframe_calls_bind_private_frames_and_report_inner_schema_failures() {
    let graph = GraphResourcePath::new("events/Calls.yssbi-event").unwrap();
    let inner = GraphResourcePath::new("functions/Select.yssbi-function").unwrap();
    let wrapper = GraphResourcePath::new("functions/Wrapper.yssbi-function").unwrap();
    let (inner_resource, project_node) = function(&inner, None);
    let (wrapper_resource, _) = function(&wrapper, Some(&inner));
    let saved_definition = inner_resource.document.clone();
    let mut data = compatible_project(&graph);
    data.graphs.insert(inner.clone(), inner_resource);
    data.graphs.insert(wrapper.clone(), wrapper_resource);
    let root = Arc::make_mut(&mut data.graphs.get_mut(&graph).unwrap().document);
    let mut outputs = Vec::new();
    for literal in [
        r#"{"sales":[1,2],"empty":[null,null]}"#,
        r#"{"sales":["a","b"],"empty":[null,null]}"#,
    ] {
        let source = node(root, "yssbi.constant.get", ParameterValues::new());
        set_constant(
            root,
            source,
            ValueType::DataFrame,
            DataValue::String(literal.into()),
        );
        yss_graph_document::normalize_constant_value(
            root.constants
                .get_mut(&yss_graph_document::ConstantId::from_uuid(source.as_uuid()))
                .unwrap(),
        )
        .unwrap();
        let call = node(
            root,
            "yssbi.project.function.call",
            reference("target", &wrapper),
        );
        let input = member(root, call, "arguments", &wrapper, "frame");
        let output = member(root, call, "results", &wrapper, "return");
        let decompose = node(root, "yssbi.dataframe.decompose", ParameterValues::new());
        connect(root, port(source, "value"), input);
        connect(root, output.clone(), port(decompose, "dataframe"));
        outputs.push((output, decompose));
    }
    let saved_root = root.clone();
    let session = staged_session(data, "function-frames", GraphRuntimeTestControl::default());
    let app = &session.application;
    let captured = &session.session;
    let instance = captured.project_instance_id().clone();
    let resolve = || {
        app.open_graph(crate::graph::open::OpenGraphRequest::new(
            instance.clone(),
            graph.clone(),
            0,
            "en-US",
        ))
        .unwrap()
    };
    let before = resolve();
    let mut events = Vec::new();
    let receipt = run_graph_with_sink(
        app,
        RunGraphRequest::new(
            instance.clone(),
            graph.clone(),
            before.document().clone(),
            before.projection().basis.semantic_input_hash,
        ),
        |event| {
            events.push(event);
            true
        },
    )
    .unwrap();
    assert!(matches!(
        events.last().unwrap().kind(),
        RunApplicationEventKind::RunCompleted
    ));
    assert!(
        events
            .iter()
            .all(|event| event.identity().run_id() == receipt.identity.run_id())
    );
    let after = resolve();
    assert_eq!(after.document().as_ref(), &saved_root);
    assert_eq!(
        captured
            .project()
            .read_resident_graph(&inner)
            .unwrap()
            .unwrap()
            .document,
        saved_definition
    );
    let control = yss_relational_contract::RelationControl {
        cancellation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
        max_input_bytes: 1024 * 1024,
    };
    for ((output, decompose), expected) in outputs.iter().zip([
        vec![TabularScalar::Unsigned(1), TabularScalar::Unsigned(2)],
        vec![
            TabularScalar::String("a".into()),
            TabularScalar::String("b".into()),
        ],
    ]) {
        let reference = PlanOutputRef::new(
            PlanGraphId::from_existing(graph.as_str().into()),
            PlanPortAddress::from_existing(output.to_string().into()),
        );
        let value = captured
            .execution()
            .query_pin_result(&reference)
            .expect("call result stays valid after schema feedback");
        let yss_node_kernel::RuntimeValue::Relation(relation) = value.value().value() else {
            panic!("expected dataframe");
        };
        let page = relation.page(0, 10, &control).unwrap();
        assert_eq!(page.data.columns().len(), 1);
        assert_eq!(page.data.columns()[0].name().as_str(), "sales");
        assert_eq!(page.data.columns()[0].values(), expected);
        let columns = after
            .projection()
            .nodes
            .iter()
            .find(|node| node.node_id == *decompose)
            .unwrap()
            .ports
            .iter()
            .filter(|port| {
                port.direction == yss_node_protocol::PortDirection::Output && !port.orphan
            })
            .collect::<Vec<_>>();
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].display.label.as_ref(), "sales");
    }
    assert!(
        captured
            .execution()
            .result_schema_candidates(inner.as_str())
            .is_empty(),
        "function intermediates must not leak into global results"
    );

    let state = captured
        .project()
        .read_graph_editing(&instance, &inner)
        .unwrap();
    app.edit_graph(
        GraphEditRequest {
            project_instance_id: instance.clone(),
            graph_path: inner.clone(),
            version: state.state.version,
            locale: "en-US".into(),
            operation_id: yss_project_identity::OperationId::new(),
        },
        yss_graph_editor::EditorGraphMutation::SetParameters {
            node_id: project_node,
            parameters: [("columns".parse().unwrap(), serde_json::json!(["missing"]))].into(),
        },
    )
    .unwrap();
    let changed = resolve();
    assert!(
        changed
            .result_state()
            .outputs
            .iter()
            .filter(|(output, _)| outputs
                .iter()
                .any(|(port, _)| output.port().as_str() == port.to_string()))
            .all(|(_, state)| matches!(
                state,
                yss_graph_execution::result::ResultCacheState::Stale { .. }
            ))
    );
    events.clear();
    run_graph_with_sink(
        app,
        RunGraphRequest::new(
            instance,
            graph,
            changed.document().clone(),
            changed.projection().basis.semantic_input_hash,
        ),
        |event| {
            events.push(event);
            true
        },
    )
    .unwrap_err();
    let RunApplicationEventKind::RunErrored { failure } = events.last().unwrap().kind() else {
        panic!("expected terminal error");
    };
    let location = failure.source.as_ref().unwrap();
    assert_eq!(location.graph().as_str(), inner.as_str());
    assert_eq!(location.node().unwrap().as_str(), project_node.to_string());
    assert_eq!(
        failure.phase,
        yss_graph_execution::error::RunPhase::PlanValidation
    );
}
