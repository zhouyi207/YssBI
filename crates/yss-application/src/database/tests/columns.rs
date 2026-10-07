use super::*;
use serde_json::json;

#[test]
fn column_batches_commit_one_history_unit_and_reject_late_failures_before_publication() {
    let directory = Directory::new();
    let project = Arc::new(ProjectState::new());
    let root = directory.0.join("project");
    let created = project
        .create_project_transaction("Columns", &root, OperationId::new())
        .unwrap();
    project
        .activate_project_from_path(&created.metadata_path)
        .unwrap();
    let app = application(project);
    let instance = app.capture_session().unwrap().project_instance_id().clone();
    let csv = directory.0.join("columns.csv");
    std::fs::write(&csv, b"a,b\n10,1.5\n20,2.5\n").unwrap();
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
    let apply = |operation| {
        app.mutate_database_for_application(
            instance.clone(),
            id.clone(),
            revision(&app, &id),
            OperationId::new(),
            operation,
        )
    };
    let snapshot = || {
        yss_database_store::DatasetStore::open(&root)
            .unwrap()
            .snapshot(&database_id(&id))
            .unwrap()
    };
    let initial = snapshot();
    let before = revision(&app, &id);
    assert!(
        apply(DatabaseMutation::CreateColumns {
            columns: vec![("x".into(), "Int8".into()), ("y".into(), "invalid".into())]
        })
        .is_err()
    );
    assert_eq!(revision(&app, &id), before);
    let created = apply(DatabaseMutation::CreateColumns {
        columns: vec![("x".into(), "Int8".into()), ("y".into(), "Utf8".into())],
    })
    .unwrap();
    assert_eq!(created.data.edit_state.undo_count, 1);
    assert_eq!(rows(&app, &id).rows.columns().len(), 4);
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(snapshot().metadata().schema, initial.metadata().schema);
    edit(&app, &id, DatabaseMutation::Redo);
    let renamed = apply(DatabaseMutation::RenameColumns {
        columns: vec![("a".into(), "b".into()), ("b".into(), "a".into())],
    })
    .unwrap();
    assert_eq!(renamed.data.edit_state.undo_count, 2);
    let values = || serde_json::to_value(rows(&app, &id).rows).unwrap();
    assert_eq!(values()["columns"]["b"], json!([10, 20]));
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(values()["columns"]["a"], json!([10, 20]));
    edit(&app, &id, DatabaseMutation::Redo);
    let before = revision(&app, &id);
    assert!(
        apply(DatabaseMutation::CastColumns {
            columns: vec![
                ("b".into(), "Int8".into(), false),
                ("a".into(), "Int8".into(), false)
            ]
        })
        .is_err()
    );
    assert_eq!(revision(&app, &id), before);
    let casted = apply(DatabaseMutation::CastColumns {
        columns: vec![
            ("b".into(), "Int8".into(), false),
            ("a".into(), "Float32".into(), false),
        ],
    })
    .unwrap();
    assert_eq!(casted.data.edit_state.undo_count, 3);
    assert_eq!(
        snapshot()
            .metadata()
            .schema
            .field_with_name("a")
            .unwrap()
            .data_type(),
        &arrow::datatypes::DataType::Float32
    );
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(
        snapshot()
            .metadata()
            .schema
            .field_with_name("a")
            .unwrap()
            .data_type(),
        &arrow::datatypes::DataType::Float64
    );
    edit(&app, &id, DatabaseMutation::Redo);
    let identifier = yss_data_contract::ColumnSemantic {
        kind: yss_data_contract::SemanticType::Identifier,
        values: vec![],
        positive_value: None,
        numeric: None,
    };
    let semantic = apply(DatabaseMutation::SetColumnSemantics {
        columns: vec![("a".into(), identifier.clone()), ("b".into(), identifier)],
    })
    .unwrap();
    assert_eq!(semantic.data.edit_state.undo_count, 4);
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(
        yss_database_arrow::column_semantic(
            snapshot().metadata().schema.field_with_name("a").unwrap()
        )
        .unwrap()
        .kind,
        yss_data_contract::SemanticType::Numeric
    );
    edit(&app, &id, DatabaseMutation::Redo);
    let before = revision(&app, &id);
    assert!(
        apply(DatabaseMutation::DeleteColumns {
            columns: vec!["x".into(), "missing".into()]
        })
        .is_err()
    );
    assert_eq!(revision(&app, &id), before);
    let deleted = apply(DatabaseMutation::DeleteColumns {
        columns: vec!["x".into(), "y".into()],
    })
    .unwrap();
    assert_eq!(deleted.data.edit_state.undo_count, 5);
    assert_eq!(rows(&app, &id).rows.columns().len(), 2);
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(rows(&app, &id).rows.columns().len(), 4);
    edit(&app, &id, DatabaseMutation::Redo);
    app.save_database_for_application(
        instance,
        id.clone(),
        revision(&app, &id),
        OperationId::new(),
    )
    .unwrap();
    assert!(
        !app.query_database_edit_state_for_application(
            app.capture_session().unwrap().project_instance_id().clone(),
            id.clone(),
            revision(&app, &id)
        )
        .unwrap()
        .can_undo
    );
}
