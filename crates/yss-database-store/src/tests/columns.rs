use super::*;
use serde_json::json;

fn values(
    snapshot: &Arc<DatasetSnapshot>,
    engine: &Arc<yss_database_engine::DataFusionRuntime>,
) -> serde_json::Value {
    serde_json::to_value(
        snapshot
            .query(engine, "columns")
            .unwrap()
            .relation()
            .unwrap()
            .page(0, 20, &control())
            .unwrap()
            .data,
    )
    .unwrap()
}

#[test]
fn column_membership_batches_validate_the_final_namespace_and_preserve_patch_identity() {
    let (_directory, store, original, engine) = editable_fixture();
    let original = store
        .commit(
            store
                .prepare_cell_edit(
                    &original,
                    &engine,
                    "patch",
                    DatasetCellEdit {
                        row_id: 0,
                        column: "income",
                        value: json!(42),
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let original_id = original.metadata().snapshot_id.clone();
    assert!(
        store
            .prepare_add_columns(
                &original,
                "duplicate",
                &[("new", DataType::Int8), ("new", DataType::Utf8)]
            )
            .is_err()
    );
    assert!(
        store
            .prepare_add_columns(
                &original,
                "invalid-last",
                &[("new", DataType::Int8), ("income", DataType::Utf8)]
            )
            .is_err()
    );
    assert!(
        store
            .prepare_rename_columns(&original, "collision", &[("income", "marker")])
            .is_err()
    );
    assert!(
        store
            .prepare_rename_columns(
                &original,
                "missing",
                &[("income", "value"), ("absent", "x")]
            )
            .is_err()
    );
    let internal = yss_database_arrow::dataset_row_columns(&original.metadata().schema)
        .unwrap()
        .unwrap();
    assert!(
        store
            .prepare_rename_columns(&original, "internal", &[("income", &internal.row_id)])
            .is_err()
    );
    assert!(
        store
            .prepare_delete_columns(&original, "missing-delete", &["income", "absent"])
            .is_err()
    );
    assert!(
        store
            .prepare_delete_columns(&original, "all", &["income", "marker"])
            .is_err()
    );
    assert_eq!(
        store
            .snapshot(&original.metadata().id)
            .unwrap()
            .metadata()
            .snapshot_id,
        original_id
    );
    let swapped = store
        .commit(
            store
                .prepare_rename_columns(
                    &original,
                    "swap",
                    &[("income", "marker"), ("marker", "income")],
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(
        yss_database_arrow::column_identity(
            swapped.metadata().schema.field_with_name("marker").unwrap()
        )
        .unwrap(),
        yss_database_arrow::column_identity(
            original
                .metadata()
                .schema
                .field_with_name("income")
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        values(&swapped, &engine)["columns"]["marker"],
        json!([42.0, 9000.0, 7000.0])
    );
    assert_eq!(
        swapped.metadata().data_revision,
        original.metadata().data_revision
    );
    assert_eq!(
        swapped.metadata().schema_revision,
        original.metadata().schema_revision + 1
    );
    let added = store
        .commit(
            store
                .prepare_add_columns(
                    &swapped,
                    "add",
                    &[("code", DataType::Int8), ("label", DataType::Utf8)],
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(
        values(&added, &engine)["columns"]["code"],
        json!([null, null, null])
    );
    assert_eq!(
        added.metadata().generation_id,
        swapped.metadata().generation_id
    );
    assert_eq!(
        added.metadata().data_revision,
        swapped.metadata().data_revision + 1
    );
    let deleted = store
        .commit(
            store
                .prepare_delete_columns(&added, "delete", &["marker", "label"])
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert!(deleted.metadata().schema.field_with_name("marker").is_err());
    assert!(deleted.metadata().schema.field_with_name("label").is_err());
    let restored = store
        .commit(store.prepare_restore(&deleted, &added, "restore").unwrap())
        .unwrap()
        .snapshot;
    assert_eq!(
        values(&restored, &engine)["columns"]["marker"],
        json!([42.0, 9000.0, 7000.0])
    );
}

#[test]
fn column_value_batches_reject_the_last_failure_and_materialize_all_casts_once() {
    use yss_data_contract::{ColumnSemantic, NumericConstraints, SemanticType};
    let (_directory, store, original, engine) = editable_fixture();
    let cast = |column, data_type, force| DatasetColumnCast {
        column,
        data_type,
        force,
    };
    // The first conversion is valid; non-finite values in the last column are not integers.
    assert!(
        store
            .prepare_cast_columns(
                &original,
                &engine,
                "bad-last-cast",
                vec![
                    cast("income", DataType::Int64, false),
                    cast("marker", DataType::Int64, false)
                ],
                &control()
            )
            .is_err()
    );
    assert_eq!(
        store
            .snapshot(&original.metadata().id)
            .unwrap()
            .metadata()
            .snapshot_id,
        original.metadata().snapshot_id
    );
    let cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(
        store
            .prepare_cast_columns(
                &original,
                &engine,
                "cancelled",
                vec![cast("income", DataType::Int64, false)],
                &cancelled
            )
            .is_err()
    );
    let casted = store
        .commit(
            store
                .prepare_cast_columns(
                    &original,
                    &engine,
                    "cast",
                    vec![
                        cast("income", DataType::Int64, false),
                        cast("marker", DataType::Int64, true),
                    ],
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_ne!(
        casted.metadata().generation_id,
        original.metadata().generation_id
    );
    assert_eq!(
        casted.metadata().data_revision,
        original.metadata().data_revision + 1
    );
    assert_eq!(
        casted.metadata().schema_revision,
        original.metadata().schema_revision + 1
    );
    assert_eq!(
        values(&casted, &engine)["columns"]["income"],
        json!([8000, 9000, 7000])
    );
    assert_eq!(
        values(&casted, &engine)["columns"]["marker"],
        json!([null, null, 1])
    );
    let identifier = ColumnSemantic {
        kind: SemanticType::Identifier,
        values: vec![],
        positive_value: None,
        numeric: None,
    };
    let invalid = ColumnSemantic {
        kind: SemanticType::Numeric,
        values: vec![],
        positive_value: None,
        numeric: Some(NumericConstraints {
            integer: false,
            minimum: Some("2".into()),
            maximum: None,
        }),
    };
    assert!(
        store
            .prepare_column_semantics(
                &casted,
                &engine,
                "bad-last-semantic",
                &[("income", &identifier), ("marker", &invalid)],
                &control()
            )
            .is_err()
    );
    assert_eq!(
        store
            .snapshot(&casted.metadata().id)
            .unwrap()
            .metadata()
            .snapshot_id,
        casted.metadata().snapshot_id
    );
    let semantic = store
        .commit(
            store
                .prepare_column_semantics(
                    &casted,
                    &engine,
                    "semantics",
                    &[("income", &identifier), ("marker", &identifier)],
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(
        semantic.metadata().generation_id,
        casted.metadata().generation_id
    );
    assert_eq!(
        semantic.metadata().data_revision,
        casted.metadata().data_revision
    );
    assert_eq!(
        semantic.metadata().schema_revision,
        casted.metadata().schema_revision + 1
    );
    for name in ["income", "marker"] {
        assert_eq!(
            yss_database_arrow::column_semantic(
                semantic.metadata().schema.field_with_name(name).unwrap()
            )
            .unwrap()
            .kind,
            SemanticType::Identifier
        );
    }
    let restored = store
        .commit(
            store
                .prepare_restore(&semantic, &original, "restore")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(restored.metadata().schema, original.metadata().schema);
}
