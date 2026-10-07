use super::*;
use crate::graph::run::RunDemand;
use yss_graph_execution::plan::NodeExecutionMode;
use yss_graph_execution::result::ResultCacheState;

#[test]
fn group_functions_support_manual_steps_schema_feedback_and_located_failures() {
    let graph = GraphResourcePath::new("events/Groups.yssbi-event").unwrap();
    let inner = GraphResourcePath::new("functions/Select.yssbi-function").unwrap();
    let wrapper = GraphResourcePath::new("functions/Wrapper.yssbi-function").unwrap();
    let (inner_resource, project_node) = function(&inner, None);
    let (wrapper_resource, _) = function(&wrapper, Some(&inner));
    let saved_definition = inner_resource.document.clone();
    let mut data = compatible_project(&graph);
    data.graphs.insert(inner.clone(), inner_resource);
    data.graphs.insert(wrapper.clone(), wrapper_resource);
    let root = &mut data.graphs.get_mut(&graph).unwrap().document;
    let source = node(root, "yssbi.constant.get", ParameterValues::new());
    set_constant(
        root,
        source,
        ValueType::DataFrame,
        DataValue::String(
            r#"{"sector":["a","b","a","b"],"sales":[1,10,3,20],"empty":[null,null,null,null]}"#
                .into(),
        ),
    );
    yss_graph_document::normalize_constant_value(
        root.constants
            .get_mut(&yss_graph_document::ConstantId::from_uuid(source.as_uuid()))
            .unwrap(),
    )
    .unwrap();
    let grouped = node(
        root,
        "yssbi.dataframe.groupby.groups",
        [("keys".parse().unwrap(), serde_json::json!(["sector"]))].into(),
    );
    let mut parameters = reference("target", &wrapper);
    parameters.insert("key_prefix".parse().unwrap(), serde_json::json!("key."));
    let apply = node(root, "yssbi.dataframe.groupby.apply", parameters);
    let transform = node(
        root,
        "yssbi.dataframe.groupby.transform",
        reference("target", &wrapper),
    );
    let decompose = node(root, "yssbi.dataframe.decompose", ParameterValues::new());
    connect(root, port(source, "value"), port(grouped, "source"));
    connect(root, port(grouped, "groups"), port(apply, "groups"));
    connect(root, port(grouped, "groups"), port(transform, "groups"));
    connect(
        root,
        port(transform, "result"),
        port(decompose, "dataframe"),
    );
    let saved_root = root.clone();
    let session = staged_session(data, "group-functions", GraphRuntimeTestControl::default());
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
    let run = |node_id, mode| {
        let before = resolve();
        let mut events = Vec::new();
        let receipt = run_graph_with_sink(
            app,
            RunGraphRequest::new(
                instance.clone(),
                graph.clone(),
                before.document().clone(),
                before.projection().basis.semantic_input_hash,
            )
            .with_demand(RunDemand::Node { node_id, mode }),
            |event| {
                events.push(event);
                true
            },
        );
        (receipt, events)
    };
    let output_ref = |node, key| {
        PlanOutputRef::new(
            PlanGraphId::from_existing(graph.as_str().into()),
            PlanPortAddress::from_existing(port(node, key).to_string().into()),
        )
    };
    let (receipt, events) = run(grouped, NodeExecutionMode::Dependencies);
    receipt.unwrap();
    assert!(matches!(
        events.last().unwrap().kind(),
        RunApplicationEventKind::RunCompleted
    ));
    let grouped_output = output_ref(grouped, "groups");
    let retained = captured
        .execution()
        .query_pin_result(&grouped_output)
        .unwrap();
    assert!(matches!(
        retained.value().value(),
        yss_node_kernel::RuntimeValue::Grouped(_)
    ));
    let json = crate::result_encoding::query_result_json(app, retained.provenance().reference())
        .unwrap()
        .unwrap();
    assert_eq!(json["kind"], "groupedDataFrame");
    assert_eq!(json["keys"], serde_json::json!(["sector"]));
    let control = yss_relational_contract::RelationControl {
        cancellation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
        max_input_bytes: 1024 * 1024,
    };
    for (node, expected) in [(apply, [1, 3, 10, 20]), (transform, [1, 10, 3, 20])] {
        let (receipt, events) = run(node, NodeExecutionMode::CurrentInputs);
        let receipt = receipt.unwrap();
        assert!(
            events
                .iter()
                .all(|e| e.identity().run_id() == receipt.identity.run_id())
        );
        let result = captured
            .execution()
            .query_pin_result(&output_ref(node, "result"))
            .unwrap();
        let yss_node_kernel::RuntimeValue::Relation(relation) = result.value().value() else {
            panic!("expected table");
        };
        let page = relation.page(0, 10, &control).unwrap();
        let sales = page
            .data
            .columns()
            .iter()
            .find(|col| col.name().as_str() == "sales")
            .unwrap();
        assert_eq!(sales.values(), expected.map(TabularScalar::Unsigned));
        if node == apply {
            assert_eq!(page.data.columns()[0].name().as_str(), "key.sector");
            assert_eq!(
                page.data.columns()[0].values(),
                &["a", "a", "b", "b"].map(|s| TabularScalar::String(s.into()))
            );
        } else {
            assert_eq!(page.data.columns().len(), 1);
        }
    }
    assert_eq!(
        captured
            .execution()
            .query_pin_result(&grouped_output)
            .unwrap()
            .provenance()
            .result_id(),
        retained.provenance().result_id()
    );
    let after = resolve();
    assert_eq!(after.document(), &saved_root);
    let columns = &after
        .projection()
        .nodes
        .iter()
        .find(|node| node.node_id == decompose)
        .unwrap()
        .ports;
    assert!(
        columns
            .iter()
            .any(|p| p.direction == yss_node_protocol::PortDirection::Output
                && !p.orphan
                && p.display.label.as_ref() == "sales")
    );
    assert_eq!(
        captured
            .project()
            .read_resident_graph(&inner)
            .unwrap()
            .unwrap()
            .document,
        saved_definition
    );
    assert!(
        captured
            .execution()
            .result_schema_candidates(inner.as_str())
            .is_empty()
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
    for node in [apply, transform] {
        assert!(matches!(
            changed
                .result_state()
                .outputs
                .get(&output_ref(node, "result")),
            Some(ResultCacheState::Stale { .. })
        ));
    }
    let (receipt, events) = run(apply, NodeExecutionMode::CurrentInputs);
    receipt.unwrap_err();
    let RunApplicationEventKind::RunErrored { failure } = events.last().unwrap().kind() else {
        panic!("expected group failure");
    };
    assert_eq!(failure.groups.len(), 1);
    assert_eq!(failure.groups[0].ordinal, Some(1));
    assert_eq!(failure.groups[0].function.as_str(), wrapper.as_str());
    assert_eq!(
        failure.groups[0].caller.node().unwrap().as_str(),
        apply.to_string()
    );
    let location = failure.source.as_ref().unwrap();
    assert_eq!(location.graph().as_str(), inner.as_str());
    assert_eq!(location.node().unwrap().as_str(), project_node.to_string());
}
