use super::*;

#[test]
fn result_value_dto_serializes_protocol_values_as_plain_json() {
    use crate::node_system::protocol::{CanonicalDecimal, Value};
    use std::collections::BTreeMap;

    let report = Value::Object(BTreeMap::from([
        ("title".into(), Value::String("OLS Summary".into())),
        (
            "model_basic_info".into(),
            Value::Object(BTreeMap::from([(
                "r_squared".into(),
                Value::Decimal(CanonicalDecimal::new("0.875").unwrap()),
            )])),
        ),
        ("coefficients".into(), Value::List(vec![Value::Integer(1)])),
    ]));

    let dto = ResultValueDto::Sequence(Box::new([result_value_to_json(&report).unwrap()]));

    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        serde_json::json!({
            "kind": "sequence",
            "value": [{
                "title": "OLS Summary",
                "model_basic_info": { "r_squared": 0.875 },
                "coefficients": [1],
            }],
        }),
    );
}

#[test]
fn stale_result_id_cannot_alias_replacement_project() {
    let old_root = std::env::temp_dir().join(format!(
        "yssbi-stale-result-source-command-old-{}",
        uuid::Uuid::new_v4()
    ));
    let replacement_root = std::env::temp_dir().join(format!(
        "yssbi-stale-result-source-command-replacement-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&old_root).unwrap();
    std::fs::create_dir_all(&replacement_root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(old_root.to_string_lossy().into_owned(), ProjectData::new());
    let old_results = state.project_store.read().unwrap().results.clone();
    let old_result_id = insert_ready_result(
        &old_results,
        RunId::new(1),
        ActivationId::next().unwrap(),
        test_output("events/test.yssbi-event"),
    );

    state.activate_project_fixture(
        replacement_root.to_string_lossy().into_owned(),
        ProjectData::new(),
    );
    let replacement_results = state.project_store.read().unwrap().results.clone();
    let replacement_result_id = insert_ready_result(
        &replacement_results,
        RunId::new(2),
        ActivationId::next().unwrap(),
        test_output("events/test.yssbi-event"),
    );

    assert_ne!(old_result_id, replacement_result_id);
    assert!(
        get_result_descriptor_from_state(&state, &old_result_id.get().to_string())
            .unwrap()
            .is_none()
    );
    assert!(
        get_result_descriptor_from_state(&state, &replacement_result_id.get().to_string())
            .unwrap()
            .is_some()
    );

    let _ = std::fs::remove_dir_all(old_root);
    let _ = std::fs::remove_dir_all(replacement_root);
}

#[test]
fn pin_history_command_returns_latest_failure_not_latest_success() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-result-history-command-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let output = test_output("events/test.yssbi-event");
    let results = state.project_store.read().unwrap().results.clone();
    insert_ready_result(
        &results,
        RunId::new(1),
        ActivationId::next().unwrap(),
        output.clone(),
    );
    let failed_activation = ActivationId::next().unwrap();
    let failed = results
        .create_pending_group(
            ActivationProvenance {
                run_id: RunId::new(2),
                activation_id: failed_activation,
                graph_path: output.graph_path.clone(),
                graph_revision: crate::graph_document::GraphRevision::new(1),
                node_id: output.port.node_id,
                created_at_ms: 2,
                usage: ResultUsage::Produced,
            },
            &[PendingOutputDescriptor {
                value: crate::node_system::plan::ValueRef::new(2),
                output: Some(output.clone()),
                presentation: crate::node_system::plan::ResultPresentation::Inspector,
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
            }],
        )
        .unwrap();
    results
        .fail_group(
            &failed,
            std::sync::Arc::new(crate::node_system::runtime::ResultFailure::new("failed")),
        )
        .unwrap();
    let output_address = PortAddressDto::from(output.port);

    let history =
        get_pin_result_history_from_state(&state, "events/test.yssbi-event", output_address)
            .unwrap();
    assert_eq!(
        history.last().unwrap().state_kind(),
        ResultStateKindDto::Failed
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn result_queries_are_descriptor_first_and_require_paging_for_collections() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-result-query-contract-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let results = state.project_store.read().unwrap().results.clone();
    let output = test_output("events/test.yssbi-event");
    let activation = ActivationId::next().unwrap();
    let group = results
        .create_pending_group(
            ActivationProvenance {
                run_id: RunId::new(3),
                activation_id: activation,
                graph_path: output.graph_path.clone(),
                graph_revision: crate::graph_document::GraphRevision::new(1),
                node_id: output.port.node_id,
                created_at_ms: 3,
                usage: ResultUsage::Produced,
            },
            &[PendingOutputDescriptor {
                value: crate::node_system::plan::ValueRef::new(3),
                output: Some(output),
                presentation: crate::node_system::plan::ResultPresentation::Inspector,
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
            }],
        )
        .unwrap();
    let result_id = group.output_result_ids[0].get().to_string();

    assert!(
        get_result_descriptor_from_state(&state, &result_id)
            .unwrap()
            .is_some()
    );
    let value_error = get_result_value_from_state(&state, &result_id).unwrap_err();
    assert_eq!(value_error.code(), "result_not_ready");
    let page_error = get_result_page_from_state(&state, &result_id, 0, 2).unwrap_err();
    assert_eq!(page_error.code(), "result_not_ready");
    assert!(
        get_result_value_from_state(&state, "999999999")
            .unwrap()
            .is_none()
    );

    results
        .complete_group(
            &group,
            vec![StoredValue::sequence(
                (0..5)
                    .map(crate::node_system::protocol::Value::Integer)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )]
            .into_boxed_slice(),
        )
        .unwrap();
    let paging_error = get_result_value_from_state(&state, &result_id).unwrap_err();
    assert_eq!(paging_error.code(), "result_requires_paging");
    let page = get_result_page_from_state(&state, &result_id, 1, 2)
        .unwrap()
        .unwrap();
    let page = serde_json::to_value(page).unwrap();
    assert_eq!(page["actualCount"], 2);
    assert_eq!(page["requestedLimit"], 2);
    assert_eq!(page["hasMore"], true);
    assert_eq!(page["nextOffset"], 3);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn oversized_scalar_requires_paging_and_is_retrievable_as_one_page() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-oversized-scalar-query-contract-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let results = state.project_store.read().unwrap().results.clone();
    let output = test_output("events/test.yssbi-event");
    let activation_id = ActivationId::next().unwrap();
    let group = results
        .create_pending_group(
            ActivationProvenance {
                run_id: RunId::new(5),
                activation_id,
                graph_path: output.graph_path.clone(),
                graph_revision: crate::graph_document::GraphRevision::new(1),
                node_id: output.port.node_id,
                created_at_ms: 5,
                usage: ResultUsage::Produced,
            },
            &[PendingOutputDescriptor {
                value: crate::node_system::plan::ValueRef::new(5),
                output: Some(output),
                presentation: crate::node_system::plan::ResultPresentation::Inspector,
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
            }],
        )
        .unwrap();
    let result_id = group.output_result_ids[0].get().to_string();
    let scalar = "x".repeat(MAX_INLINE_RESULT_JSON_BYTES + 1);
    results
        .complete_group(
            &group,
            vec![StoredValue::scalar(
                crate::node_system::protocol::Value::String(scalar.clone().into()),
            )]
            .into_boxed_slice(),
        )
        .unwrap();

    let error = get_result_value_from_state(&state, &result_id).unwrap_err();
    let page = get_result_page_from_state(&state, &result_id, 0, 1)
        .unwrap()
        .unwrap();
    let page = serde_json::to_value(page).unwrap();

    assert_eq!(error.code(), "result_requires_paging");
    assert_eq!(error.details().unwrap()["valueKind"], "scalar");
    assert_eq!(page["valueKind"], "scalar");
    assert_eq!(page["values"], serde_json::json!([scalar]));
    assert_eq!(page["hasMore"], false);
    let _ = std::fs::remove_dir_all(root);
}
