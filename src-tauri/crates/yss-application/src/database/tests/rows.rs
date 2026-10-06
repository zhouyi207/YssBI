use super::*;
use serde_json::json;
use std::collections::BTreeMap;
use yss_database_runtime::session_api::DatabaseCellUpdate;

#[test]
fn row_batches_return_committed_ids_and_keep_one_history_unit_and_revision_gate() {
    let directory = Directory::new();
    let root = directory.0.join("project");
    let project = Arc::new(ProjectState::new());
    let created = project
        .create_project_transaction("Row batches", &root, OperationId::new())
        .unwrap();
    project
        .activate_project_from_path(&created.metadata_path)
        .unwrap();
    let app = application(project);
    let instance = app.capture_session().unwrap().project_instance_id().clone();
    let csv = directory.0.join("rows.csv");
    std::fs::write(&csv, b"value\n10\n20\n30\n").unwrap();
    let id = app
        .load_database_for_application(
            instance.clone(),
            OperationId::new(),
            DatabaseImportSource::Csv {
                path: csv.to_string_lossy().into(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: Some(10),
            },
            None,
        )
        .unwrap()
        .data
        .id;
    let apply = |mutation, expected| {
        app.mutate_database_for_application(
            instance.clone(),
            id.clone(),
            expected,
            OperationId::new(),
            mutation,
        )
    };
    let initial = revision(&app, &id);
    let row = |value| BTreeMap::from([("value".into(), serde_json::from_value(value).unwrap())]);
    let bad = apply(
        DatabaseMutation::InsertRows {
            rows: vec![row(json!(11)), row(json!("invalid-number"))],
            before_row_id: Some(1),
        },
        initial,
    );
    assert!(bad.is_err());
    assert_eq!(revision(&app, &id), initial);
    assert_eq!(rows(&app, &id).row_ids, [0, 1, 2]);
    let inserted = apply(
        DatabaseMutation::InsertRows {
            rows: vec![row(json!(11)), row(json!(12))],
            before_row_id: Some(1),
        },
        initial,
    )
    .unwrap();
    assert_eq!(inserted.data.inserted_row_ids, [3, 4]);
    assert_eq!(inserted.data.edit_state.undo_count, 1);
    let values = || serde_json::to_value(rows(&app, &id).rows.columns()[0].values()).unwrap();
    assert_eq!(values(), json!([10, 11, 12, 20, 30]));
    assert_eq!(rows(&app, &id).row_ids, [0, 3, 4, 1, 2]);
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(values(), json!([10, 20, 30]));
    edit(&app, &id, DatabaseMutation::Redo);
    assert_eq!(rows(&app, &id).row_ids, [0, 3, 4, 1, 2]);
    let change = |row_id, value| DatabaseCellUpdate {
        row_id,
        column: "value".into(),
        value: serde_json::from_value(value).unwrap(),
    };
    assert!(
        apply(
            DatabaseMutation::UpdateCells {
                updates: vec![change(3, json!(999))]
            },
            initial
        )
        .is_err()
    );
    let before_cells = revision(&app, &id);
    assert!(
        apply(
            DatabaseMutation::UpdateCells {
                updates: vec![change(3, json!(13)), change(1, json!("invalid-number"))],
            },
            before_cells
        )
        .is_err()
    );
    assert_eq!(revision(&app, &id), before_cells);
    assert_eq!(values(), json!([10, 11, 12, 20, 30]));
    let updated = apply(
        DatabaseMutation::UpdateCells {
            updates: vec![change(3, json!(13)), change(1, json!(21))],
        },
        before_cells,
    )
    .unwrap();
    assert!(updated.data.inserted_row_ids.is_empty());
    assert_eq!(updated.data.edit_state.undo_count, 2);
    assert_eq!(values(), json!([10, 13, 12, 21, 30]));
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(values(), json!([10, 11, 12, 20, 30]));
    edit(&app, &id, DatabaseMutation::Redo);
    assert_eq!(values(), json!([10, 13, 12, 21, 30]));
    let before_delete = revision(&app, &id);
    assert!(
        apply(
            DatabaseMutation::DeleteRowIds {
                row_ids: vec![3, 99]
            },
            before_delete
        )
        .is_err()
    );
    assert_eq!(revision(&app, &id), before_delete);
    let deleted = apply(
        DatabaseMutation::DeleteRowIds {
            row_ids: vec![3, 1],
        },
        before_delete,
    )
    .unwrap();
    assert_eq!(deleted.data.edit_state.undo_count, 3);
    assert_eq!(rows(&app, &id).row_ids, [0, 4, 2]);
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(values(), json!([10, 13, 12, 21, 30]));
    app.save_database_for_application(
        instance,
        id.clone(),
        revision(&app, &id),
        OperationId::new(),
    )
    .unwrap();
    assert_eq!(values(), json!([10, 13, 12, 21, 30]));
    assert_eq!(
        app.query_database_edit_state_for_application(
            app.capture_session().unwrap().project_instance_id().clone(),
            id.clone(),
            revision(&app, &id),
        )
        .unwrap()
        .undo_count,
        0
    );
}
