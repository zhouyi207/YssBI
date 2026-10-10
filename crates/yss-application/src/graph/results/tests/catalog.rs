use super::*;
use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot, NodeComponents};
use yss_graph_execution::result::ResultValidity;

#[test]
fn graph_result_query_keeps_graph_scope_and_current_output_ownership() {
    let nodes = NodeComponents::builtins().unwrap();
    let candidate = crate::session::build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        Arc::new(yss_project::ProjectState::new()),
        [],
        &nodes,
    )
    .unwrap();
    let app = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(nodes)));
    app.install_candidate(candidate).unwrap();
    let captured = app.capture_session().unwrap();
    let graph = GraphResourcePath::new("events/results.yssbi-event").unwrap();
    let output = PlanOutputRef::new(
        PlanGraphId::from_existing(graph.as_str().into()),
        PlanPortAddress::from_existing("value".into()),
    );
    let value = || StoredResult::new(RuntimeValue::Scalar(TabularScalar::Integer(7)));
    let old = captured
        .execution()
        .publish_fixture_result(output.clone(), value());
    let reference = ResultReference {
        execution_session_id: captured.execution_session_id(),
        result_id: old,
    };
    let (lease, _) = app.retain_owned_result(reference).unwrap();
    let current = captured
        .execution()
        .publish_fixture_result(output.clone(), value());
    captured.execution().publish_fixture_result(
        PlanOutputRef::new(
            PlanGraphId::from_existing("events/other.yssbi-event".into()),
            PlanPortAddress::from_existing("value".into()),
        ),
        value(),
    );

    let results = app.query_graph_results(&graph).unwrap();
    assert_eq!(results.len(), 1);
    let entry = &results[0];
    assert_eq!(entry.validity, ResultValidity::CurrentValid);
    assert_eq!(entry.result.output(), &output);
    assert_eq!(entry.result.provenance().result_id(), current);
    assert_eq!(entry.result.provenance().run_id().get(), current.get());
    assert!(Arc::ptr_eq(
        entry.result.value(),
        app.query_result(entry.result.provenance().reference())
            .unwrap()
            .unwrap()
            .value()
    ));
    assert!(app.query_result(reference).unwrap().is_some());

    captured
        .execution()
        .invalidate_graph_results(graph.as_str());
    assert!(app.query_graph_results(&graph).unwrap().is_empty());
    assert!(app.query_result(reference).unwrap().is_some());
    drop(lease);
    assert!(app.query_result(reference).unwrap().is_none());
}
