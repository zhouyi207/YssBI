use super::*;
use crate::finalization::{ReadyPinResult, ReadyResult, ResultObservationIntent};
use crate::plan::{PlanGraphId, PlanPortAddress, ResultCategory};
use crate::result::{
    ConnectionCacheState, GraphResultCacheState, ResultCacheState, ResultProvenance,
    ResultRetentionError,
};
use std::sync::Arc;
use yss_data_contract::TabularScalar;

fn output() -> PlanOutputRef {
    PlanOutputRef::new(
        PlanGraphId::from_existing("events/main.yssbi-event".into()),
        PlanPortAddress::from_existing("node:result".into()),
    )
}

fn named_output(port: &str) -> PlanOutputRef {
    PlanOutputRef::new(
        output().graph().clone(),
        PlanPortAddress::from_existing(port.into()),
    )
}

fn cache_inputs(hash: u8, nodes: &[(&str, u8, &[&str])]) -> GraphResultInputs {
    GraphResultInputs {
        semantic_input_hash: [hash; 32],
        definition_input_hash: [hash; 32],
        schema_observations: BTreeMap::new(),
        observers: BTreeMap::new(),
        outputs: nodes
            .iter()
            .map(|(port, fingerprint, sources)| {
                (
                    named_output(port),
                    OutputResultInputs {
                        fingerprint: [*fingerprint; 32],
                        bindings: if sources.is_empty() {
                            BTreeMap::new()
                        } else {
                            BTreeMap::from([(
                                PlanPortAddress::from_existing(format!("{port}:input").into()),
                                sources.iter().map(|port| named_output(port)).collect(),
                            )])
                        },
                        resources: BTreeMap::new(),
                        available: true,
                    },
                )
            })
            .collect(),
    }
}

fn cached_result(id: u64, run: RunId, output: PlanOutputRef) -> ReadyResult {
    let id = ResultId::from_existing(id);
    ReadyResult::from_scheduler(
        id,
        StoredResult::new(yss_node_kernel::RuntimeValue::Scalar(
            TabularScalar::Integer(id.get() as i64),
        )),
        ResultCategory::Value,
        ReadyPinResult::new(
            output,
            ResultProvenance::produced(
                crate::identity::ExecutionSessionId::new(Uuid::nil()),
                id,
                run,
                10,
            ),
        ),
    )
}

fn publish_cached_graph(store: &ResultStore, inputs: &GraphResultInputs) {
    let graph = output();
    let basis = store
        .capture_run_basis(graph.graph().as_str(), inputs.clone())
        .unwrap();
    let outputs = inputs.outputs.keys().cloned().collect::<Vec<_>>();
    let run = RunId::from_existing(1);
    assert!(store.begin_run(run, &outputs, Some(&basis), &BTreeMap::new()));
    assert!(
        store.publish(
            &outputs
                .into_iter()
                .enumerate()
                .map(|(index, output)| { cached_result(index as u64 + 1, run, output) })
                .collect::<Vec<_>>(),
            &[],
        )
    );
}

#[test]
fn observed_input_branches_require_consumption_and_follow_the_shared_result_lifetime() {
    use crate::plan::PlanSourceIdentity;

    let store = ResultStore::new();
    let output = named_output("species");
    let graph = output.graph().as_str();
    let node = |name: &str| PlanNodeId::from_existing(name.into());
    let input = |name: &str| PlanPortAddress::from_existing(format!("{name}:data").into());
    let observed_inputs = |name: &str| OutputResultInputs {
        fingerprint: [9; 32],
        bindings: BTreeMap::from([(input(name), Box::new([output.clone()]) as Box<[_]>)]),
        resources: BTreeMap::new(),
        available: true,
    };
    let mut original = cache_inputs(1, &[("species", 1, &[])]);
    original.observers = ["b", "c", "d"]
        .into_iter()
        .map(|name| (node(name), observed_inputs(name)))
        .collect();
    let observations = ["b", "c", "d"]
        .into_iter()
        .map(|name| ResultObservationIntent {
            result_id: ResultId::from_existing(1),
            input_basis: Some(observed_inputs(name)),
            requester: PlanSourceIdentity::new(output.graph().clone(), Some(node(name)), None),
        })
        .collect::<Vec<_>>();
    let basis = store.capture_run_basis(graph, original.clone()).unwrap();
    let run = RunId::from_existing(1);
    assert!(store.begin_run(
        run,
        std::slice::from_ref(&output),
        Some(&basis),
        &BTreeMap::new()
    ));
    assert!(store.publish(&[cached_result(1, run, output.clone())], &observations[..2]));
    // A later View run may observe an existing current result without recomputing it.
    assert!(store.publish(&[], &observations[2..]));
    let states = store.query_cache_states(graph, &[1; 32]).unwrap();
    assert_eq!(states.connections.len(), 3);
    assert!(
        states
            .connections
            .iter()
            .all(|edge| edge.state == ConnectionCacheState::Valid)
    );
    assert_eq!(store.query_graph_results(graph, 10).len(), 1);

    let mut edited = original.clone();
    edited.semantic_input_hash = [2; 32];
    edited
        .observers
        .get_mut(&node("b"))
        .unwrap()
        .bindings
        .clear();
    edited.observers.get_mut(&node("b")).unwrap().available = false;
    edited.observers.insert(node("new"), observed_inputs("new"));
    store.observe_graph_inputs(graph, edited.clone());
    let states = store.query_cache_states(graph, &[2; 32]).unwrap();
    for edge in &states.connections {
        assert_eq!(
            edge.state,
            if edge.input == input("new") {
                ConnectionCacheState::New
            } else {
                ConnectionCacheState::Valid
            }
        );
    }
    assert!(store.query_pin_result(&output).is_some());
    edited.semantic_input_hash = [3; 32];
    edited.observers.get_mut(&node("c")).unwrap().fingerprint = [8; 32];
    store.observe_graph_inputs(graph, edited);
    assert!(!store.publish(&[], &observations[1..2]));
    let states = store.query_cache_states(graph, &[3; 32]).unwrap();
    assert_eq!(
        states
            .connections
            .iter()
            .find(|edge| edge.input == input("c"))
            .unwrap()
            .state,
        ConnectionCacheState::Stale
    );
    assert_eq!(
        states
            .connections
            .iter()
            .find(|edge| edge.input == input("d"))
            .unwrap()
            .state,
        ConnectionCacheState::Valid
    );

    store.observe_graph_inputs(graph, original.clone());
    assert!(
        store
            .query_cache_states(graph, &[1; 32])
            .unwrap()
            .connections
            .iter()
            .all(|edge| edge.state == ConnectionCacheState::Valid)
    );
    let basis = store.capture_run_basis(graph, original).unwrap();
    let run = RunId::from_existing(2);
    assert!(store.begin_run(
        run,
        std::slice::from_ref(&output),
        Some(&basis),
        &BTreeMap::new()
    ));
    assert!(store.get(ResultId::from_existing(1)).is_some());
    assert!(!store.publish(&[], &observations));
    assert!(store.publish(&[cached_result(2, run, output.clone())], &[]));
    assert!(store.get(ResultId::from_existing(1)).is_none());
    assert!(
        store
            .query_cache_states(graph, &[1; 32])
            .unwrap()
            .connections
            .iter()
            .all(|edge| edge.state == ConnectionCacheState::New)
    );
    store.invalidate_graph(graph);
    assert!(store.get(ResultId::from_existing(2)).is_none());
}

#[test]
fn edits_revalidate_only_dependent_caches_and_undo_cannot_resurrect_deleted_outputs() {
    let store = ResultStore::new();
    let original = cache_inputs(
        1,
        &[
            ("a", 1, &[]),
            ("b", 2, &["a"]),
            ("c", 3, &["b"]),
            ("d", 4, &["a"]),
        ],
    );
    publish_cached_graph(&store, &original);
    let b = store.query_pin_result(&named_output("b")).unwrap();
    let weak = Arc::downgrade(b.value());
    let b_id = b.provenance().result_id();
    drop(b);
    let graph = output();
    let graph = graph.graph().as_str();
    let edited = cache_inputs(
        2,
        &[
            ("a", 1, &[]),
            ("b", 8, &[]),
            ("c", 3, &["b"]),
            ("d", 4, &["a"]),
            ("e", 5, &["a"]),
        ],
    );
    store.observe_graph_inputs(graph, edited.clone());
    let states = store.query_cache_states(graph, &[2; 32]).unwrap();
    for port in ["a", "d"] {
        assert!(matches!(
            states.outputs[&named_output(port)],
            ResultCacheState::Valid { .. }
        ));
        assert!(store.query_pin_result(&named_output(port)).is_some());
    }
    for port in ["b", "c"] {
        let ResultCacheState::Stale { result_id } = states.outputs[&named_output(port)] else {
            panic!("edited output must retain a stale result identity");
        };
        assert_eq!(store.get(result_id).unwrap().output(), &named_output(port));
        if port == "b" {
            assert_eq!(result_id, b_id);
        }
        assert!(store.query_pin_result(&named_output(port)).is_none());
    }
    assert_eq!(
        states.outputs[&named_output("e")],
        ResultCacheState::Missing
    );
    let connection = |state: &GraphResultCacheState, source: &str, input: &str| {
        state
            .connections
            .iter()
            .find(|connection| {
                connection.output == named_output(source) && connection.input.as_str() == input
            })
            .unwrap()
            .state
    };
    assert_eq!(
        connection(&states, "a", "d:input"),
        ConnectionCacheState::Valid
    );
    assert_eq!(
        connection(&states, "b", "c:input"),
        ConnectionCacheState::Stale
    );
    assert_eq!(
        connection(&states, "a", "e:input"),
        ConnectionCacheState::New
    );
    let rewired = cache_inputs(
        4,
        &[
            ("a", 1, &[]),
            ("b", 8, &["d"]),
            ("c", 3, &["b"]),
            ("d", 4, &["a"]),
        ],
    );
    store.observe_graph_inputs(graph, rewired);
    let states = store.query_cache_states(graph, &[4; 32]).unwrap();
    assert_eq!(
        connection(&states, "d", "b:input"),
        ConnectionCacheState::New
    );
    assert_eq!(
        connection(&states, "b", "c:input"),
        ConnectionCacheState::Stale
    );
    assert!(store.get(b_id).is_some());
    assert!(store.query_cache_states(graph, &[1; 32]).is_none());
    store.observe_graph_inputs(graph, original.clone());
    let states = store.query_cache_states(graph, &[1; 32]).unwrap();
    assert!(
        states
            .connections
            .iter()
            .all(|connection| connection.state == ConnectionCacheState::Valid)
    );
    assert_eq!(
        store
            .query_pin_result(&named_output("b"))
            .unwrap()
            .provenance()
            .result_id(),
        b_id
    );
    assert!(store.query_pin_result(&named_output("c")).is_some());
    store.observe_graph_inputs(graph, edited);
    assert!(store.query_pin_result(&named_output("c")).is_none());
    let mut deleted = original.clone();
    deleted.semantic_input_hash = [3; 32];
    deleted.outputs.remove(&named_output("b"));
    store.observe_graph_inputs(graph, deleted);
    assert!(weak.upgrade().is_none());
    store.observe_graph_inputs(graph, original);
    assert!(store.query_pin_result(&named_output("b")).is_none());
    assert!(store.query_pin_result(&named_output("c")).is_none());
    assert!(store.query_pin_result(&named_output("d")).is_some());
}

#[test]
fn rerun_versions_and_edit_epochs_prevent_obsolete_cache_or_run_restoration() {
    let store = ResultStore::new();
    let original = cache_inputs(1, &[("a", 1, &[]), ("b", 2, &["a"])]);
    publish_cached_graph(&store, &original);
    let graph_output = output();
    let graph = graph_output.graph().as_str();
    let obsolete = store.capture_run_basis(graph, original.clone()).unwrap();
    let edited = cache_inputs(2, &[("a", 1, &[]), ("b", 3, &[])]);
    let before = store
        .query_cache_states(graph, &original.semantic_input_hash)
        .unwrap()
        .revision;
    let edited_state = store.observe_graph_inputs(graph, edited.clone());
    let restored_state = store.observe_graph_inputs(graph, original.clone());
    assert!(before < edited_state.revision && edited_state.revision < restored_state.revision);
    assert_eq!(
        store.observe_graph_inputs(graph, original.clone()).revision,
        restored_state.revision
    );
    let outputs = [named_output("a")];
    assert!(!store.begin_run(
        RunId::from_existing(2),
        &outputs,
        Some(&obsolete),
        &BTreeMap::new()
    ));
    let basis = store.capture_run_basis(graph, original.clone()).unwrap();
    let run = RunId::from_existing(3);
    assert!(store.begin_run(run, &outputs, Some(&basis), &BTreeMap::new()));
    let started_revision = store
        .query_cache_states(graph, &original.semantic_input_hash)
        .unwrap()
        .revision;
    assert!(started_revision > restored_state.revision);
    assert!(store.query_pin_result(&named_output("b")).is_none());
    assert!(store.publish(&[cached_result(3, run, named_output("a"))], &[]));
    assert!(
        store
            .query_cache_states(graph, &original.semantic_input_hash)
            .unwrap()
            .revision
            > started_revision
    );
    assert!(store.query_pin_result(&named_output("a")).is_some());
    assert!(store.query_pin_result(&named_output("b")).is_none());
    store.observe_graph_inputs(graph, edited.clone());
    store.observe_graph_inputs(graph, original.clone());
    assert!(store.query_pin_result(&named_output("b")).is_none());
    let run = RunId::from_existing(4);
    let basis = store.capture_run_basis(graph, original.clone()).unwrap();
    assert!(store.begin_run(run, &outputs, Some(&basis), &BTreeMap::new()));
    store.observe_graph_inputs(graph, edited);
    store.observe_graph_inputs(graph, original.clone());
    assert!(!store.publish(&[cached_result(4, run, named_output("a"))], &[]));
    assert!(store.query_pin_result(&named_output("a")).is_none());
    let resources = ResultStore::new();
    let mut original = original;
    original
        .outputs
        .get_mut(&named_output("a"))
        .unwrap()
        .resources
        .insert("database:source".into(), Some([1; 32]));
    publish_cached_graph(&resources, &original);
    let old_admission = resources
        .capture_run_basis(graph, original.clone())
        .unwrap();
    resources
        .observe_resource_versions(&BTreeMap::from([("database:source".into(), Some([7; 32]))]));
    assert!(resources.query_pin_result(&named_output("a")).is_none());
    assert!(resources.query_pin_result(&named_output("b")).is_none());
    assert!(!resources.begin_run(
        RunId::from_existing(2),
        &outputs,
        Some(&old_admission),
        &BTreeMap::new()
    ));
    // Undo restores graph semantics, while the independent data revision stays current.
    original
        .outputs
        .get_mut(&named_output("a"))
        .unwrap()
        .resources
        .insert("database:source".into(), Some([7; 32]));
    resources.observe_graph_inputs(graph, original);
    assert!(resources.query_pin_result(&named_output("b")).is_none());
    assert!(resources.get(ResultId::from_existing(1)).is_some());

    let feedback = ResultStore::new();
    let initial = cache_inputs(1, &[("a", 1, &[])]);
    publish_cached_graph(&feedback, &initial);
    let first = feedback
        .query_pin_result(&named_output("a"))
        .unwrap()
        .provenance()
        .result_id();
    let mut observed = initial.clone();
    observed.semantic_input_hash = [9; 32];
    observed
        .schema_observations
        .insert(named_output("a"), first);
    let basis = feedback.capture_run_basis(graph, observed.clone()).unwrap();
    let run = RunId::from_existing(2);
    assert!(feedback.begin_run(run, &outputs, Some(&basis), &BTreeMap::new()));
    assert!(feedback.query_pin_result(&named_output("a")).is_none());
    assert_eq!(
        feedback
            .matching_schema_results(graph, &observed)
            .get(&named_output("a")),
        Some(&first)
    );
    feedback.observe_graph_inputs(graph, initial.clone());
    assert!(
        feedback.publish(&[cached_result(20, run, named_output("a"))], &[]),
        "Schema feedback cannot revoke a producer whose inputs did not change"
    );
    assert!(
        feedback.capture_run_basis(graph, observed).is_none(),
        "a replaced observation cannot authorize a new run"
    );
}

fn result(id: u64, run: RunId) -> ReadyResult {
    let id = ResultId::from_existing(id);
    ReadyResult::from_scheduler(
        id,
        StoredResult::new(yss_node_kernel::RuntimeValue::float64(id.get() as f64).unwrap()),
        ResultCategory::Value,
        ReadyPinResult::new(
            output(),
            ResultProvenance::produced(
                crate::identity::ExecutionSessionId::new(Uuid::nil()),
                id,
                run,
                10,
            ),
        ),
    )
}

#[test]
fn publication_reuses_the_prepared_result_and_immutable_nested_buffers() {
    use yss_node_kernel::RuntimeValue;
    let numbers: Arc<[_]> = (0..4096)
        .map(|value| RuntimeValue::Scalar(TabularScalar::Integer(value)))
        .collect();
    let fields = Arc::new(BTreeMap::from([(
        "values".into(),
        RuntimeValue::List(numbers.clone()),
    )]));
    let original = RuntimeValue::Record(fields.clone());
    let RuntimeValue::Record(copy) = original.clone() else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(&fields, &copy));
    let run = RunId::from_existing(1);
    let ready = ReadyResult::from_scheduler(
        ResultId::from_existing(1),
        StoredResult::new(original),
        ResultCategory::Value,
        ReadyPinResult::new(output(), result(1, run).pin().provenance().clone()),
    );
    let store = ResultStore::new();
    store.begin_run(run, &[output()], None, &BTreeMap::new());
    assert!(store.publish(std::slice::from_ref(&ready), &[]));
    let snapshot = store.get(ready.result_id()).unwrap();
    assert!(Arc::ptr_eq(ready.value(), snapshot.value()));
    let RuntimeValue::Record(stored) = snapshot.value().value() else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(&fields, stored));
    let RuntimeValue::List(stored) = &stored["values"] else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(&numbers, stored));
}

#[test]
fn retained_snapshots_survive_output_changes_until_the_last_lease_is_released() {
    let store = ResultStore::new();
    let first = RunId::from_existing(1);
    store.begin_run(first, &[output()], None, &BTreeMap::new());
    assert!(store.publish(&[result(1, first)], &[]));
    let id = ResultId::from_existing(1);
    let weak = Arc::downgrade(store.get(id).unwrap().value());
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    store.retain(id, a, "main", None).unwrap();
    store.retain(id, a, "main", None).unwrap();
    store.retain(id, b, "report-window", None).unwrap();
    assert_eq!(store.registry.read().unwrap().values[&id].leases.len(), 2);
    store.invalidate_graph("events/main.yssbi-event");
    assert!(store.query_pin_result(&output()).is_none());
    assert!(
        store
            .query_graph_results("events/main.yssbi-event", 10)
            .is_empty()
    );
    assert!(store.get(id).is_some());
    let next = RunId::from_existing(2);
    store.begin_run(next, &[output()], None, &BTreeMap::new());
    assert!(store.publish(&[result(2, next)], &[]));
    assert_eq!(
        store
            .query_graph_results("events/main.yssbi-event", 10)
            .len(),
        1
    );
    assert_eq!(
        store
            .query_pin_result(&output())
            .unwrap()
            .provenance()
            .result_id(),
        ResultId::from_existing(2)
    );
    store.release(a, "main").unwrap();
    store.release(a, "main").unwrap();
    assert!(store.get(id).is_some());
    assert!(matches!(
        store.release(b, "main"),
        Err(ResultRetentionError::WrongOwner)
    ));
    store.release(b, "report-window").unwrap();
    assert!(store.get(id).is_none());
    assert!(weak.upgrade().is_none());
    assert!(store.get(ResultId::from_existing(2)).is_some());
}

#[test]
fn window_handoffs_and_owner_reconciliation_do_not_leak_or_drop_claimed_results() {
    let store = ResultStore::new();
    let run = RunId::from_existing(1);
    store.begin_run(run, &[output()], None, &BTreeMap::new());
    store.publish(&[result(1, run)], &[]);
    let id = ResultId::from_existing(1);
    let lease = Uuid::new_v4();
    store.retain(id, lease, "main", Some("plot")).unwrap();
    store.invalidate_graph("events/main.yssbi-event");
    store.reconcile("main", &BTreeSet::new());
    assert!(store.get(id).is_some());
    assert!(matches!(
        store.claim(lease, "other"),
        Err(ResultRetentionError::WrongOwner)
    ));
    store.claim(lease, "plot").unwrap();
    store.claim(lease, "plot").unwrap();
    store.close_owner("main");
    assert!(store.get(id).is_some());
    assert!(matches!(
        store.retain(id, Uuid::new_v4(), "main", None),
        Err(ResultRetentionError::OwnerClosed)
    ));
    store.reconcile("plot", &BTreeSet::from([lease]));
    assert!(store.get(id).is_some());
    store.reconcile("plot", &BTreeSet::new());
    assert!(store.get(id).is_none());
    for closed in ["main", "plot"] {
        let store = ResultStore::new();
        let run = RunId::from_existing(2);
        store.begin_run(run, &[output()], None, &BTreeMap::new());
        store.publish(&[result(2, run)], &[]);
        let id = ResultId::from_existing(2);
        store
            .retain(id, Uuid::new_v4(), "main", Some("plot"))
            .unwrap();
        store.invalidate_graph("events/main.yssbi-event");
        store.close_owner(closed);
        assert!(store.get(id).is_none());
        assert!(store.registry.read().unwrap().leases.is_empty());
        assert!(store.registry.read().unwrap().owner_leases.is_empty());
    }
}

#[test]
fn rerun_preserves_previous_success_until_replacement_and_rejects_obsolete_publication() {
    let store = ResultStore::new();
    let first = RunId::from_existing(1);
    let second = RunId::from_existing(2);
    store.begin_run(first, &[output()], None, &BTreeMap::new());
    assert!(store.publish(&[result(1, first)], &[]));
    let previous = Arc::downgrade(store.get(ResultId::from_existing(1)).unwrap().value());
    store.begin_run(second, &[output()], None, &BTreeMap::new());
    assert!(previous.upgrade().is_some());
    assert!(store.get(ResultId::from_existing(1)).is_some());
    assert!(store.query_pin_result(&output()).is_none());
    assert!(!store.publish(&[result(2, first)], &[]));
    assert!(store.publish(&[result(3, second)], &[]));
    assert!(previous.upgrade().is_none());
    assert!(!store.publish(&[result(3, second)], &[]));
    assert!(store.get(ResultId::from_existing(1)).is_none());
    assert_eq!(
        store
            .query_pin_result(&output())
            .unwrap()
            .provenance()
            .result_id(),
        ResultId::from_existing(3)
    );
    store.invalidate_graph("events/main.yssbi-event");
    assert!(store.query_pin_result(&output()).is_none());
    assert!(!store.publish(&[result(4, second)], &[]));
}
#[test]
fn superseded_batch_does_not_publish_any_output_or_remove_unrelated_results() {
    let store = ResultStore::new();
    let first = RunId::from_existing(1);
    let second = RunId::from_existing(2);
    let third = RunId::from_existing(3);
    let other = PlanOutputRef::new(
        output().graph().clone(),
        PlanPortAddress::from_existing("other:value".into()),
    );
    let other_result = |id, run| {
        ReadyResult::from_scheduler(
            ResultId::from_existing(id),
            StoredResult::new(yss_node_kernel::RuntimeValue::float64(1.0).unwrap()),
            ResultCategory::Value,
            ReadyPinResult::new(
                other.clone(),
                ResultProvenance::produced(
                    crate::identity::ExecutionSessionId::new(Uuid::nil()),
                    ResultId::from_existing(id),
                    run,
                    10,
                ),
            ),
        )
    };
    assert!(store.begin_run(first, &[output(), other.clone()], None, &BTreeMap::new()));
    assert!(store.publish(&[result(1, first), other_result(2, first)], &[]));
    assert!(store.begin_run(second, &[output()], None, &BTreeMap::new()));
    assert_eq!(
        store
            .query_pin_result(&other)
            .unwrap()
            .provenance()
            .result_id(),
        ResultId::from_existing(2)
    );
    assert!(store.begin_run(second, std::slice::from_ref(&other), None, &BTreeMap::new()));
    assert!(store.begin_run(third, &[output()], None, &BTreeMap::new()));
    assert!(!store.publish(&[result(3, second), other_result(4, second)], &[]));
    assert!(store.query_pin_result(&other).is_none());
    assert!(store.publish(&[result(5, third)], &[]));
    assert!(!store.begin_run(second, &[output()], None, &BTreeMap::new()));
    assert!(store.get(ResultId::from_existing(5)).is_some());
    store.observe_graph_inputs(
        output().graph().as_str(),
        cache_inputs(1, &[("node:result", 1, &[])]),
    );
    store.observe_graph_inputs(
        output().graph().as_str(),
        cache_inputs(1, &[("node:result", 1, &[])]),
    );
    assert!(store.get(ResultId::from_existing(5)).is_some());
    store.observe_graph_inputs(
        output().graph().as_str(),
        cache_inputs(2, &[("node:result", 2, &[])]),
    );
    assert!(store.query_pin_result(&output()).is_none());
    assert!(!store.publish(&[result(6, third)], &[]));
}
