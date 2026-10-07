use super::*;
use crate::graph::editing::{GraphActivity, GraphEditRequest};
use crate::graph::run::{RunDemand, RunGraphRequest, run_graph};
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{ConnectionId, DocumentConnection};
use yss_graph_editor::EditorGraphMutation;
use yss_graph_execution::plan::NodeExecutionMode;
use yss_graph_execution::result::ResultCacheState;

#[test]
fn evaluated_schema_updates_decompose_without_invalidating_its_producer() {
    let graph = GraphResourcePath::new("events/New Event.yssbi-event").unwrap();
    let session = staged_session(
        compatible_project(&graph),
        "schema-feedback",
        GraphRuntimeTestControl::default(),
    );
    let app = &session.application;
    let captured = &session.session;
    let instance = captured.project_instance_id().clone();
    let source = NodeId::new();
    let drop_columns = NodeId::new();
    let decompose = NodeId::new();
    let viewer = NodeId::new();
    let mut document = compatible_draft(source);
    set_constant(
        &mut document,
        source,
        ValueType::DataFrame,
        DataValue::String(r#"{"kept":[1,2],"maybe":[null,null]}"#.into()),
    );
    yss_graph_document::normalize_constant_value(document.constants.values_mut().next().unwrap())
        .unwrap();
    for (id, kind) in [
        (drop_columns, "yssbi.dataframe.dropna.columns"),
        (decompose, "yssbi.dataframe.decompose"),
        (viewer, "yssbi.debug.view"),
    ] {
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
    }
    let port = |id, key: &str| PortAddress::declared(id, key.parse().unwrap());
    for (output, input) in [
        (port(source, "value"), port(drop_columns, "source")),
        (port(drop_columns, "result"), port(decompose, "dataframe")),
    ] {
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
    let overwrite = captured
        .project()
        .capture_graph_overwrite_operation(
            &instance,
            &graph,
            yss_project_identity::OperationId::new(),
        )
        .unwrap();
    captured
        .project()
        .commit_graph_candidate(overwrite.into_authority(), Arc::new(document))
        .unwrap();
    let edit_request = || GraphEditRequest {
        project_instance_id: instance.clone(),
        graph_path: graph.clone(),
        locale: "en-US".into(),
        operation_id: yss_project_identity::OperationId::new(),
        version: captured
            .project()
            .read_graph_editing(&instance, &graph)
            .unwrap()
            .state
            .version,
    };
    let resolve = || {
        app.open_graph(crate::graph::open::OpenGraphRequest::new(
            instance.clone(),
            graph.clone(),
            0,
            "en-US",
        ))
        .unwrap()
    };
    let output_columns = |projection: &yss_graph_editor::projection::EditorProjectionModel| {
        projection
            .nodes
            .iter()
            .find(|node| node.node_id == decompose)
            .unwrap()
            .ports
            .iter()
            .filter(|port| {
                port.direction == yss_node_protocol::PortDirection::Output && !port.orphan
            })
            .map(|port| (port.display.label.to_string(), port.address.clone()))
            .collect::<Vec<_>>()
    };
    let before = resolve();
    assert!(output_columns(before.projection()).is_empty());
    let changes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&changes);
    let _subscription = app
        .subscribe_graph_activity(
            &instance,
            Arc::new(move |event| {
                if matches!(event, GraphActivity::Changed { .. }) {
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }),
        )
        .unwrap();
    let run_drop = |document, projection: &yss_graph_editor::projection::EditorProjectionModel| {
        run_graph(
            app,
            RunGraphRequest::new(
                instance.clone(),
                graph.clone(),
                document,
                projection.basis.semantic_input_hash,
            )
            .with_demand(RunDemand::Node {
                node_id: drop_columns,
                mode: NodeExecutionMode::Dependencies,
            }),
        )
        .unwrap();
    };
    run_drop(before.document().clone(), before.projection());
    assert_eq!(
        changes.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "run requests projection refresh without editing the document"
    );
    let evaluated = resolve();
    assert_eq!(before.editing(), evaluated.editing());
    let columns = output_columns(evaluated.projection());
    assert_eq!(
        columns
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["kept"]
    );
    let result = evaluated
        .result_state()
        .outputs
        .iter()
        .find(|(output, _)| output.port().as_str() == port(drop_columns, "result").to_string())
        .unwrap()
        .1;
    assert!(
        matches!(result, ResultCacheState::Valid { .. }),
        "Schema feedback must retain a valid producer result"
    );
    run_drop(evaluated.document().clone(), evaluated.projection());
    let rerun = resolve();
    assert_eq!(
        output_columns(rerun.projection()),
        columns,
        "field identity survives a new result ID"
    );
    let connected = app
        .edit_graph(
            edit_request(),
            EditorGraphMutation::Connect {
                output: columns[0].1.clone(),
                input: port(viewer, "data"),
                order: None,
            },
        )
        .unwrap();
    let mut changed_constant = connected
        .update
        .document
        .constants
        .values()
        .next()
        .unwrap()
        .clone();
    // A saved graph may contain a derived-port binding but no execution results.
    // Both replay modes must discover that port and continue in the same run.
    for demand in [
        RunDemand::Node {
            node_id: viewer,
            mode: NodeExecutionMode::Dependencies,
        },
        RunDemand::Default,
    ] {
        let cold = staged_session(
            compatible_project(&graph),
            "schema-cold-replay",
            GraphRuntimeTestControl::default(),
        );
        let cold_instance = cold.session.project_instance_id().clone();
        let overwrite = cold
            .session
            .project()
            .capture_graph_overwrite_operation(
                &cold_instance,
                &graph,
                yss_project_identity::OperationId::new(),
            )
            .unwrap();
        cold.session
            .project()
            .commit_graph_candidate(
                overwrite.into_authority(),
                Arc::new(connected.update.document.clone()),
            )
            .unwrap();
        let cold_resolve = || {
            cold.application
                .open_graph(crate::graph::open::OpenGraphRequest::new(
                    cold_instance.clone(),
                    graph.clone(),
                    0,
                    "en-US",
                ))
                .unwrap()
        };
        let initial = cold_resolve();
        assert!(output_columns(initial.projection()).is_empty());
        let mut events = Vec::new();
        let replay = crate::graph::run::run_graph_with_sink(
            &cold.application,
            RunGraphRequest::new(
                cold_instance.clone(),
                graph.clone(),
                initial.document().clone(),
                initial.projection().basis.semantic_input_hash,
            )
            .with_demand(demand.clone()),
            |event| {
                events.push(event);
                true
            },
        )
        .expect("cold replay evaluates the schema boundary before its consumers");
        assert!(
            events
                .iter()
                .all(|event| event.identity() == &replay.identity)
        );
        let starts = events
            .iter()
            .filter_map(|event| match event.kind() {
                crate::graph::run::RunApplicationEventKind::RunStarted { outputs } => Some(outputs),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(starts.len(), 2, "one boundary and one continuation");
        assert!(
            starts[0].iter().all(|output| starts[1].contains(output)),
            "progress is cumulative for event recovery"
        );
        let producer = replay
            .results
            .iter()
            .find(|result| {
                result.output.port().as_str() == port(drop_columns, "result").to_string()
            })
            .unwrap();
        assert_eq!(
            cold.session
                .execution()
                .query_pin_result(&producer.output)
                .unwrap()
                .provenance()
                .result_id(),
            producer.result_id
        );
        assert!(
            replay
                .results
                .iter()
                .any(|result| result.output.port().as_str() == columns[0].1.to_string())
        );
        let after = cold_resolve();
        assert_eq!(output_columns(after.projection()), columns);
        assert_eq!(after.editing(), initial.editing());
        assert_eq!(
            cold.session
                .execution()
                .runs()
                .state(replay.identity.run_id()),
            Some(yss_graph_execution::run_registry::RunState::Succeeded)
        );
        if matches!(demand, RunDemand::Default) {
            let mut starts = 0;
            crate::graph::run::run_graph_with_sink(
                &cold.application,
                RunGraphRequest::new(
                    cold_instance,
                    graph.clone(),
                    after.document().clone(),
                    after.projection().basis.semantic_input_hash,
                ),
                |event| {
                    if matches!(
                        event.kind(),
                        crate::graph::run::RunApplicationEventKind::RunStarted { .. }
                    ) {
                        starts += 1;
                    }
                    true
                },
            )
            .expect("full reruns refresh observed schemas before downstream execution");
            assert_eq!(starts, 2);
        }
    }
    run_graph(
        app,
        RunGraphRequest::new(
            instance.clone(),
            graph.clone(),
            connected.update.document.clone(),
            connected
                .update
                .projection_replacement
                .projection
                .basis
                .semantic_input_hash,
        )
        .with_demand(RunDemand::Node {
            node_id: viewer,
            mode: NodeExecutionMode::Dependencies,
        }),
    )
    .unwrap();
    assert!(
        resolve()
            .result_state()
            .outputs
            .iter()
            .any(
                |(output, state)| output.port().as_str() == columns[0].1.to_string()
                    && matches!(state, ResultCacheState::Valid { .. })
            ),
        "the newly observed column can be executed and retained"
    );
    changed_constant.data_value = DataValue::String(r#"{"kept":[null,null],"maybe":[4,5]}"#.into());
    changed_constant.tabular = None;
    let changed = app
        .edit_graph(
            edit_request(),
            EditorGraphMutation::SetConstant {
                id: changed_constant.id,
                constant: Some(changed_constant),
            },
        )
        .unwrap();
    assert!(
        output_columns(&changed.update.projection_replacement.projection).is_empty(),
        "old columns are withdrawn when producer inputs change"
    );
    let restored = app.change_graph_history(edit_request(), false).unwrap();
    assert_eq!(
        output_columns(&restored.update.projection_replacement.projection),
        columns,
        "undo may reuse a retained result with matching inputs"
    );
    let changed = app.change_graph_history(edit_request(), true).unwrap();
    run_drop(
        changed.update.document.clone(),
        &changed.update.projection_replacement.projection,
    );
    let updated = resolve();
    assert_eq!(
        output_columns(updated.projection())
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["maybe"]
    );
    let retained = updated
        .projection()
        .nodes
        .iter()
        .find(|node| node.node_id == decompose)
        .unwrap();
    assert!(
        retained
            .ports
            .iter()
            .any(|port| port.address == columns[0].1 && port.orphan),
        "removed connected fields remain repairable"
    );
    assert_eq!(updated.document().connections.len(), 3);
    let fresh = staged_session(
        compatible_project(&graph),
        "schema-fresh-session",
        GraphRuntimeTestControl::default(),
    );
    let reopened = fresh
        .application
        .resolve_graph_document(
            fresh.session.project_instance_id().clone(),
            graph.clone(),
            updated.document().clone(),
            "en-US".into(),
        )
        .unwrap();
    assert!(
        output_columns(&reopened).is_empty(),
        "persisted derived-port metadata is not an evaluated result"
    );
    let instance = fresh.session.project_instance_id().clone();
    let overwrite = fresh
        .session
        .project()
        .capture_graph_overwrite_operation(
            &instance,
            &graph,
            yss_project_identity::OperationId::new(),
        )
        .unwrap();
    fresh
        .session
        .project()
        .commit_graph_candidate(
            overwrite.into_authority(),
            Arc::new(updated.document().clone()),
        )
        .unwrap();
    let mut events = Vec::new();
    let error = crate::graph::run::run_graph_with_sink(
        &fresh.application,
        RunGraphRequest::new(
            instance,
            graph.clone(),
            updated.document().clone(),
            reopened.basis.semantic_input_hash,
        ),
        |event| {
            events.push(event);
            true
        },
    )
    .expect_err("a field absent from the actual result remains a blocking error");
    assert!(matches!(
        error,
        crate::graph::run::ExecutionApplicationError::GraphNotReady
    ));
    assert!(
        matches!(events.last().unwrap().kind(), crate::graph::run::RunApplicationEventKind::RunErrored { failure }
        if failure.phase == yss_graph_execution::error::RunPhase::PlanValidation)
    );
    let run = events[0].identity().run_id();
    assert_eq!(
        fresh.session.execution().runs().state(run),
        Some(yss_graph_execution::run_registry::RunState::Failed)
    );
    let output = yss_graph_execution::plan::PlanOutputRef::new(
        yss_graph_execution::plan::PlanGraphId::from_existing(graph.as_str().into()),
        yss_graph_execution::plan::PlanPortAddress::from_existing(
            port(drop_columns, "result").to_string().into(),
        ),
    );
    assert_eq!(
        fresh
            .session
            .execution()
            .query_pin_result(&output)
            .unwrap()
            .provenance()
            .run_id(),
        run,
        "a successfully completed boundary remains inspectable when a later stage fails"
    );
}
