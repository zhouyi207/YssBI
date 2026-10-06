use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn row_batches_validate_every_value_and_publish_one_snapshot_with_stable_ids() {
    let (_directory, store, original, engine) = editable_fixture();
    let cell = |row_id, column, value| DatasetCellEdit {
        row_id,
        column,
        value,
    };
    let current_id = || {
        store
            .snapshot(&original.metadata().id)
            .unwrap()
            .metadata()
            .snapshot_id
            .clone()
    };
    let original_id = current_id();
    assert!(
        store
            .prepare_cell_edits(
                &original,
                &engine,
                "invalid-last",
                vec![
                    cell(0, "income", json!(12)),
                    cell(1, "marker", json!("invalid-number")),
                ],
                &control()
            )
            .is_err()
    );
    assert_eq!(current_id(), original_id);
    assert!(
        store
            .prepare_cell_edits(
                &original,
                &engine,
                "duplicate-cell",
                vec![cell(0, "income", json!(12)), cell(0, "income", json!(15)),],
                &control()
            )
            .is_err()
    );
    assert!(matches!(
        store.prepare_cell_edits(
            &original,
            &engine,
            "missing-last",
            vec![cell(0, "income", json!(12)), cell(99, "income", json!(15)),],
            &control()
        ),
        Err(DatasetStoreError::RowNotFound)
    ));
    let edited = store
        .commit(
            store
                .prepare_cell_edits(
                    &original,
                    &engine,
                    "cells",
                    vec![
                        cell(0, "income", json!(12)),
                        cell(2, "income", json!(null)),
                        cell(1, "marker", json!(25)),
                    ],
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(
        edited.metadata().data_revision,
        original.metadata().data_revision + 1
    );
    let edited = store
        .commit(
            store
                .prepare_cell_edits(
                    &edited,
                    &engine,
                    "merge-patches",
                    vec![cell(0, "income", json!(13)), cell(1, "income", json!(14))],
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let values = |snapshot: &Arc<DatasetSnapshot>| {
        serde_json::to_value(
            snapshot
                .query(&engine, "rows")
                .unwrap()
                .relation()
                .unwrap()
                .page(0, 20, &control())
                .unwrap()
                .data,
        )
        .unwrap()
    };
    assert_eq!(
        values(&edited)["columns"]["income"],
        json!([13.0, 14.0, null])
    );
    let row =
        |income, marker| BTreeMap::from([("income".into(), income), ("marker".into(), marker)]);
    let invalid_rows = [
        row(json!(15), json!(16)),
        row(json!("invalid-number"), json!(18)),
    ];
    assert!(
        store
            .prepare_insert_rows(
                &edited,
                &engine,
                "invalid-insert",
                DatasetRowInsertion {
                    position: DatasetInsertPosition::BeforeRow(1),
                    rows: &invalid_rows,
                },
                &control()
            )
            .is_err()
    );
    assert_eq!(current_id(), edited.metadata().snapshot_id);
    let rows = [
        row(json!(15), json!(16)),
        BTreeMap::from([("income".into(), json!(17))]),
    ];
    let (prepared, ids) = store
        .prepare_insert_rows(
            &edited,
            &engine,
            "insert",
            DatasetRowInsertion {
                position: DatasetInsertPosition::BeforeRow(1),
                rows: &rows,
            },
            &control(),
        )
        .unwrap();
    assert_eq!(
        ids,
        [3, 4],
        "failed preparation cannot consume committed row allocation"
    );
    assert_eq!(current_id(), edited.metadata().snapshot_id);
    let inserted = store.commit(prepared).unwrap().snapshot;
    assert_eq!(
        values(&inserted)["columns"]["income"],
        json!([13.0, 15.0, 17.0, 14.0, null])
    );
    assert_eq!(values(&inserted)["columns"]["marker"][2], json!(null));
    assert_eq!(
        inserted.metadata().data_revision,
        edited.metadata().data_revision + 1
    );
    let page = inserted
        .query(&engine, "row-ids")
        .unwrap()
        .page(0, 20, &control())
        .unwrap();
    let roles = yss_database_arrow::dataset_row_columns(&inserted.metadata().schema)
        .unwrap()
        .unwrap();
    let actual_ids = page
        .batches
        .iter()
        .flat_map(|batch| {
            batch
                .column_by_name(&roles.row_id)
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::Int64Array>()
                .unwrap()
                .values()
                .to_vec()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_ids, [0, 3, 4, 1, 2]);
    assert!(matches!(
        store.prepare_delete_rows(&inserted, &engine, "invalid-delete", &[3, 99], &control()),
        Err(DatasetStoreError::RowNotFound)
    ));
    assert_eq!(current_id(), inserted.metadata().snapshot_id);
    let deleted = store
        .commit(
            store
                .prepare_delete_rows(&inserted, &engine, "delete", &[3, 1], &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(
        values(&deleted)["columns"]["income"],
        json!([13.0, 17.0, null])
    );
    assert!(matches!(
        store.prepare_insert_rows(
            &deleted,
            &engine,
            "deleted-anchor",
            DatasetRowInsertion {
                position: DatasetInsertPosition::BeforeRow(1),
                rows: &rows,
            },
            &control()
        ),
        Err(DatasetStoreError::RowNotFound)
    ));
    let restored = store
        .commit(
            store
                .prepare_restore(&deleted, &original, "restore")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let (prepared, new_ids) = store
        .prepare_insert_rows(
            &restored,
            &engine,
            "append-after-undo",
            DatasetRowInsertion {
                position: DatasetInsertPosition::End,
                rows: &rows,
            },
            &control(),
        )
        .unwrap();
    assert_eq!(new_ids, [5, 6], "undo must not reuse committed identities");
    let appended = store.commit(prepared).unwrap().snapshot;
    assert_eq!(
        values(&appended)["columns"]["income"],
        json!([8000.0, 9000.0, 7000.0, 15.0, 17.0])
    );
    let cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        store.prepare_cell_edits(
            &appended,
            &engine,
            "cancelled",
            vec![cell(0, "income", json!(1))],
            &cancelled
        ),
        Err(DatasetStoreError::Query(
            yss_relational_contract::RelationError::Cancelled
        ))
    ));
    assert_eq!(current_id(), appended.metadata().snapshot_id);
}
