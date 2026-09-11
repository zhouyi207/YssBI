use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use super::*;
use arrow::array::{Array, Decimal128Array, TimestampNanosecondArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::{RecordBatch, RecordBatchReader};
use uuid::Uuid;
use yss_relational_contract::{RelationBinding, RelationControl};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("yss-dataset [catalog]-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn control() -> RelationControl {
    RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(20),
        max_input_bytes: 16 * 1024 * 1024,
    }
}

#[test]
fn datetime_column_cast_retains_clock_values_and_forced_nulls_in_persisted_data() {
    use arrow::array::StringArray;
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new("at", DataType::Utf8, true)])),
        vec![Arc::new(StringArray::from(vec![
            "2026-09-11T10:00:00+08:00",
            "2026-09-11T10:00:00-05:00",
            "bad date",
        ]))],
    )
    .unwrap();
    let original = store
        .commit(
            store
                .prepare_import(identity(), "Dates", "import", batch.schema(), [Ok(batch)])
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let engine = yss_datafusion::DataFusionRuntime::new(64 * 1024 * 1024, 8192).unwrap();
    let target = DataType::Timestamp(TimeUnit::Nanosecond, None);
    assert!(
        store
            .prepare_cast_column(
                &original,
                &engine,
                "strict",
                DatasetColumnCast {
                    column: "at",
                    data_type: target.clone(),
                    force: false
                },
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
    let cast = store
        .commit(
            store
                .prepare_cast_column(
                    &original,
                    &engine,
                    "forced",
                    DatasetColumnCast {
                        column: "at",
                        data_type: target.clone(),
                        force: true,
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let reopened = DatasetStore::open(directory.path())
        .unwrap()
        .snapshot(&cast.metadata().id)
        .unwrap();
    assert_eq!(
        reopened
            .metadata()
            .schema
            .field_with_name("at")
            .unwrap()
            .data_type(),
        &target
    );
    let data = yss_tabular_io::read_parquet_batches(&reopened.file_paths()[0], 10, None)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(
        yss_tabular_arrow::array_to_json(data.column_by_name("at").unwrap().as_ref()).unwrap(),
        vec![
            serde_json::json!("2026-09-11T10:00:00"),
            serde_json::json!("2026-09-11T10:00:00"),
            serde_json::json!(null)
        ]
    );
}

#[cfg(any(unix, windows))]
fn directory_link(link: &Path, target: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
}

#[cfg(any(unix, windows))]
fn remove_directory_link(link: &Path) {
    #[cfg(unix)]
    std::fs::remove_file(link).unwrap();
    #[cfg(windows)]
    std::fs::remove_dir(link).unwrap();
}

#[cfg(any(unix, windows))]
#[test]
fn catalog_and_import_reject_redirected_directories() {
    let project = Directory::new();
    let outside = Directory::new();
    let database = project.path().join(yss_project_layout::DATABASE_DIR);
    directory_link(&database, outside.path());
    assert!(matches!(
        DatasetStore::create(project.path()),
        Err(DatasetStoreError::InvalidIdentity)
    ));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(matches!(
        DatasetStore::open(project.path()),
        Err(DatasetStoreError::InvalidIdentity)
    ));
    remove_directory_link(&database);

    let store = DatasetStore::create(project.path()).unwrap();
    let datasets = database.join("datasets");
    directory_link(&datasets, outside.path());
    let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Int64, true)]));
    assert!(matches!(
        store.prepare_import(identity(), "Redirect", "import", schema, []),
        Err(DatasetStoreError::InvalidIdentity)
    ));
    assert!(matches!(
        store.collect_garbage(),
        Err(DatasetStoreError::InvalidIdentity)
    ));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    remove_directory_link(&datasets);
}

#[cfg(any(unix, windows))]
#[test]
fn snapshot_and_cleanup_reject_redirected_generations() {
    let (_directory, store, original, _engine) = editable_fixture();
    let original_files = original.file_paths();
    let original_directory = original_files[0].parent().unwrap();
    let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Int64, true)]));
    let prepared = store
        .prepare_import(identity(), "Uncommitted", "other", schema, [])
        .unwrap();
    let prepared_directory = prepared.directory.clone().unwrap();
    for file in &prepared.files {
        std::fs::remove_file(store.root.join(file.relative_path())).unwrap();
    }
    std::fs::remove_dir(&prepared_directory).unwrap();
    directory_link(&prepared_directory, original_directory);
    drop(prepared);
    assert!(original_files[0].is_file());
    store.collect_garbage().unwrap();
    assert!(original_files[0].is_file());
    remove_directory_link(&prepared_directory);

    let moved = original_directory.with_file_name("retained-test-generation");
    std::fs::rename(original_directory, &moved).unwrap();
    directory_link(original_directory, &moved);
    assert!(matches!(
        store.snapshot(&original.metadata().id),
        Err(DatasetStoreError::InvalidIdentity)
    ));
    assert!(moved.join(original_files[0].file_name().unwrap()).is_file());
    remove_directory_link(original_directory);
    std::fs::rename(moved, original_directory).unwrap();
}

fn editable_fixture() -> (
    Directory,
    Arc<DatasetStore>,
    Arc<DatasetSnapshot>,
    Arc<yss_datafusion::DataFusionRuntime>,
) {
    use arrow::array::Float64Array;
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("income", DataType::Float64, false),
        Field::new("marker", DataType::Float64, true),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Float64Array::from(vec![8000., 9000., 7000.])),
            Arc::new(Float64Array::from(vec![f64::NAN, f64::INFINITY, 1.])),
        ],
    )
    .unwrap();
    let prepared = store
        .prepare_import(identity(), "Edits", "import", schema, [Ok(batch)])
        .unwrap();
    let snapshot = store.commit(prepared).unwrap().snapshot;
    let engine = yss_datafusion::DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    (directory, store, snapshot, engine)
}

#[test]
fn garbage_collection_respects_queries_preparations_and_pending_publications_after_reopen() {
    let (directory, store, original, engine) = editable_fixture();
    let old_files = original.file_paths();
    let relation = original
        .query(&engine, "old-result")
        .unwrap()
        .relation()
        .unwrap();
    let compact = store
        .prepare_compaction(&original, &engine, "compact", &control())
        .unwrap();
    let committed = store.commit(compact).unwrap();
    let current_files = committed.snapshot.file_paths();
    let restore = store
        .prepare_restore(&committed.snapshot, &original, "restore")
        .unwrap();
    drop(original);
    let reopened = DatasetStore::open(directory.path()).unwrap();

    // A durable handoff keeps both sides until Project publishes the committed revision.
    drop(relation);
    assert_eq!(reopened.collect_garbage().unwrap(), 0);
    for publication in reopened.pending_publications().unwrap() {
        reopened.acknowledge_publication(&publication).unwrap();
    }
    // An uncommitted undo still needs the old generation, even through another store handle.
    assert_eq!(reopened.collect_garbage().unwrap(), 0);
    let relation = restore.snapshot_leases[1]
        .query(&engine, "old-result")
        .unwrap()
        .relation()
        .unwrap();
    drop(restore);
    assert_eq!(reopened.collect_garbage().unwrap(), 0);
    assert_eq!(relation.page(0, 3, &control()).unwrap().row_count, 3);
    assert!(old_files.iter().all(|file| file.is_file()));
    drop(relation);

    assert_eq!(reopened.collect_garbage().unwrap(), old_files.len());
    assert!(old_files.iter().all(|file| !file.exists()));
    assert!(current_files.iter().all(|file| file.is_file()));
    assert_eq!(
        committed
            .snapshot
            .query(&engine, "current")
            .unwrap()
            .page(0, 3, &control())
            .unwrap()
            .row_count,
        3
    );
    let source = batch();
    let live = store
        .prepare_import(
            identity(),
            "Live preparation",
            "live",
            source.schema(),
            [Ok(source)],
        )
        .unwrap();
    let live_file = store.root.join(&live.files[0].relative_path);
    let orphan = store
        .root
        .join("datasets")
        .join(Uuid::new_v4().to_string())
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&orphan).unwrap();
    std::fs::write(orphan.join("part-000000.parquet"), b"interrupted writer").unwrap();
    assert_eq!(reopened.collect_garbage().unwrap(), 1);
    assert!(!orphan.exists());
    assert!(live_file.is_file());
    drop(live);

    let old_query = committed.snapshot.query(&engine, "deleted-result").unwrap();
    let deleted = store
        .commit(store.prepare_delete(&committed.snapshot, "delete").unwrap())
        .unwrap();
    assert!(reopened.catalog_metadata().unwrap().is_empty());
    reopened
        .acknowledge_publication(&deleted.publication)
        .unwrap();
    drop(committed);
    drop(deleted);
    assert_eq!(reopened.collect_garbage().unwrap(), 0);
    assert_eq!(old_query.page(0, 3, &control()).unwrap().row_count, 3);
    drop(old_query);
    assert_eq!(reopened.collect_garbage().unwrap(), current_files.len());
    assert_eq!(reopened.collect_garbage().unwrap(), 0);
}

#[test]
fn deleted_dataset_reopens_absent_and_restore_preserves_content_and_names() {
    let (directory, store, original, engine) = editable_fixture();
    let renamed = store
        .commit(
            store
                .prepare_rename(&original, "rename-dataset", "Renamed")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let deleted = store
        .commit(store.prepare_delete(&renamed, "delete-dataset").unwrap())
        .unwrap()
        .snapshot;
    let reopened = DatasetStore::open(directory.path()).unwrap();
    assert!(reopened.catalog_metadata().unwrap().is_empty());
    assert!(matches!(
        reopened.snapshot(&deleted.metadata().id),
        Err(DatasetStoreError::NotFound)
    ));
    let restored = store
        .commit(
            store
                .prepare_restore(&deleted, &original, "undo-delete-rename")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let metadata = reopened.catalog_metadata().unwrap();
    assert_eq!(metadata[0].name.as_ref(), "Edits");
    assert_eq!(metadata[0].id, original.metadata().id);
    assert!(!restored.metadata().deleted);
    assert_eq!(
        restored
            .query(&engine, "restored")
            .unwrap()
            .page(0, 3, &control())
            .unwrap()
            .row_count,
        3
    );
}

#[test]
fn delta_overflow_compacts_the_requested_edit_in_one_atomic_commit() {
    use arrow::array::StringArray;
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let engine = yss_datafusion::DataFusionRuntime::new(192 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new(vec![Field::new("text", DataType::Utf8, true)]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(StringArray::from(vec!["first", "second"]))],
    )
    .unwrap();
    let original = store
        .commit(
            store
                .prepare_import(identity(), "Large edits", "import", schema, [Ok(batch)])
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let large = "x".repeat(9 * 1024 * 1024);
    let first = store
        .commit(
            store
                .prepare_cell_edit(
                    &original,
                    &engine,
                    "edit-first",
                    DatasetCellEdit {
                        row_id: 0,
                        column: "text",
                        value: serde_json::Value::String(large.clone()),
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(
        first.metadata().generation_id,
        original.metadata().generation_id
    );
    let prepared = store
        .prepare_cell_edit(
            &first,
            &engine,
            "edit-second",
            DatasetCellEdit {
                row_id: 1,
                column: "text",
                value: serde_json::Value::String(large.clone()),
            },
            &control(),
        )
        .unwrap();
    assert_ne!(
        prepared.metadata().generation_id,
        first.metadata().generation_id
    );
    assert!(prepared.overlay.columns.is_empty());
    assert_eq!(
        store
            .snapshot(&first.metadata().id)
            .unwrap()
            .metadata()
            .snapshot_id,
        first.metadata().snapshot_id
    );
    let committed = store.commit(prepared).unwrap();
    assert_eq!(
        committed.snapshot.metadata().data_revision,
        first.metadata().data_revision + 1
    );
    assert_eq!(store.pending_publications().unwrap().len(), 3);
    let reopened = DatasetStore::open(directory.path())
        .unwrap()
        .snapshot(&first.metadata().id)
        .unwrap();
    let query = reopened.query(&engine, "large").unwrap();
    for id in [0, 1] {
        let row = query.row(id, &control()).unwrap().unwrap();
        assert_eq!(
            row.column_by_name("text")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            large
        );
    }
    let row = first
        .query(&engine, "undo-value")
        .unwrap()
        .row(1, &control())
        .unwrap()
        .unwrap();
    assert_eq!(
        row.column_by_name("text")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .value(0),
        "second"
    );
}

#[test]
fn native_profiles_follow_the_snapshot_and_preserve_finite_mode_and_empty_table_semantics() {
    use arrow::array::{BooleanArray, Float64Array, StringArray};
    use yss_dataset_profile::{ColumnDistribution, ColumnStats};
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let engine = yss_datafusion::DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("value", DataType::Float64, true),
        Field::new("labels", DataType::Utf8, true),
        Field::new("flags", DataType::Boolean, false),
        Field::new("decimal", DataType::Decimal128(12, 2), false),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Float64Array::from(vec![
                Some(1.),
                Some(2.),
                Some(f64::NAN),
                Some(f64::INFINITY),
                None,
                None,
            ])),
            Arc::new(StringArray::from(vec![
                Some("beta"),
                Some("alpha"),
                Some("beta"),
                Some("alpha"),
                Some(""),
                None,
            ])),
            Arc::new(BooleanArray::from(vec![
                true, false, true, false, true, true,
            ])),
            Arc::new(
                Decimal128Array::from(vec![100_i128, 250, 100, 250, 100, 100])
                    .with_precision_and_scale(12, 2)
                    .unwrap(),
            ),
        ],
    )
    .unwrap();
    let original = store
        .commit(
            store
                .prepare_import(
                    identity(),
                    "Profiles",
                    "import-profile",
                    schema.clone(),
                    [Ok(batch)],
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let original_query = original.query(&engine, "old-profile").unwrap();
    let tiny_engine = yss_datafusion::DataFusionRuntime::new(1, 2).unwrap();
    assert!(matches!(
        original
            .query(&tiny_engine, "bounded-profile")
            .unwrap()
            .column_stats(&control()),
        Err(yss_relational_contract::RelationError::MemoryLimitExceeded)
    ));
    let edited = store
        .commit(
            store
                .prepare_cell_edit(
                    &original,
                    &engine,
                    "profile-edit",
                    DatasetCellEdit {
                        row_id: 0,
                        column: "value",
                        value: serde_json::json!(10),
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let query = edited.query(&engine, "profile").unwrap();
    let stats = query.column_stats(&control()).unwrap();
    let ColumnStats::Numeric(numeric) = &stats[0] else {
        panic!("numeric column");
    };
    assert_eq!((numeric.count, numeric.null_count), (6, 2));
    assert_eq!(
        (numeric.min, numeric.max, numeric.mean, numeric.median),
        (Some(2.), Some(10.), Some(6.), Some(6.))
    );
    assert!((numeric.variance.unwrap() - 32.).abs() < 1e-10);
    let ColumnStats::String(labels) = &stats[1] else {
        panic!("string column");
    };
    assert_eq!(
        (labels.empty_count, labels.unique, labels.mode_count),
        (1, 3, 2)
    );
    assert_eq!(labels.mode.as_deref(), Some("alpha"));
    let ColumnStats::String(flags) = &stats[2] else {
        panic!("boolean projection");
    };
    assert_eq!(flags.mode.as_deref(), Some("true"));
    let ColumnStats::Numeric(decimal) = &stats[3] else {
        panic!("decimal numeric column");
    };
    assert_eq!((decimal.min, decimal.max), (Some(1.), Some(2.5)));
    let distributions = query.column_distributions(&control()).unwrap();
    let ColumnDistribution::Numeric(histogram) = &distributions[0] else {
        panic!("numeric distribution");
    };
    assert_eq!(histogram.bins.iter().map(|bin| bin.count).sum::<usize>(), 2);
    assert!(histogram.bins.last().unwrap().label.ends_with(']'));
    let ColumnDistribution::String(categories) = &distributions[1] else {
        panic!("string distribution");
    };
    assert_eq!(
        categories
            .categories
            .iter()
            .map(|category| (category.label.as_str(), category.value))
            .collect::<Vec<_>>(),
        vec![("alpha", 2), ("beta", 2)]
    );
    let overview = query.dataset_overview(&control()).unwrap();
    assert_eq!(
        (overview.size_shape.n_rows, overview.size_shape.n_columns),
        (6, 4)
    );
    assert_eq!(
        (
            overview.schema_overview.numeric_cols,
            overview.schema_overview.string_cols,
            overview.schema_overview.bool_cols
        ),
        (2, 1, 1)
    );
    assert_eq!(
        (
            overview.data_completeness.total_nulls,
            overview.data_completeness.rows_with_nulls,
            overview.data_completeness.cols_with_nulls
        ),
        (3, 2, 2)
    );
    assert_eq!(overview.size_shape.duplicated_rows, None);
    let old = original_query.column_stats(&control()).unwrap();
    let ColumnStats::Numeric(old) = &old[0] else {
        panic!("original numeric column");
    };
    assert_eq!(old.mean, Some(1.5));

    let empty = store
        .commit(
            store
                .prepare_import(
                    identity(),
                    "Empty profile",
                    "import-empty",
                    schema.clone(),
                    [Ok(RecordBatch::new_empty(schema))],
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let query = empty.query(&engine, "empty").unwrap();
    for stats in query.column_stats(&control()).unwrap() {
        match stats {
            ColumnStats::Numeric(stats) => {
                assert_eq!(stats.count, 0);
                assert_eq!(stats.mean, None);
            }
            ColumnStats::String(stats) => {
                assert_eq!(stats.valid_ratio, 0.);
                assert_eq!(stats.mode, None);
            }
        }
    }
    for distribution in query.column_distributions(&control()).unwrap() {
        match distribution {
            ColumnDistribution::Numeric(distribution) => assert!(distribution.bins.is_empty()),
            ColumnDistribution::String(distribution) => assert!(distribution.categories.is_empty()),
        }
    }
    assert_eq!(
        query
            .dataset_overview(&control())
            .unwrap()
            .data_completeness
            .null_ratio,
        0.
    );
}

#[test]
fn sparse_edits_merge_before_filters_and_nulls_survive_column_rename_and_reopen() {
    use arrow::array::Float64Array;
    use yss_relational_contract::{RelationComparison, RelationLiteral, RelationPredicate};
    let (directory, store, original, engine) = editable_fixture();
    let column_id = yss_tabular_arrow::column_identity(original.metadata().schema.field(0))
        .unwrap()
        .to_owned();
    let edit = store
        .prepare_cell_edit(
            &original,
            &engine,
            "edit-enter",
            DatasetCellEdit {
                row_id: 0,
                column: "income",
                value: serde_json::json!(9000),
            },
            &control(),
        )
        .unwrap();
    let edited = store.commit(edit).unwrap().snapshot;
    let filtered = |snapshot: &Arc<DatasetSnapshot>, name: &str, limit| {
        snapshot
            .query(&engine, "test-session")
            .unwrap()
            .relation()
            .unwrap()
            .filter(&RelationPredicate {
                column: name.into(),
                comparison: RelationComparison::Greater,
                value: Some(RelationLiteral::Integer(8500)),
            })
            .unwrap()
            .page(0, limit, &control())
            .unwrap()
    };
    let page = filtered(&edited, "income", 1);
    assert_eq!(page.row_count, 1);
    assert!(page.has_more);
    let cleared = store
        .commit(
            store
                .prepare_cell_edit(
                    &edited,
                    &engine,
                    "edit-null",
                    DatasetCellEdit {
                        row_id: 1,
                        column: "income",
                        value: serde_json::Value::Null,
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert!(!filtered(&cleared, "income", 1).has_more);
    let renamed = store
        .commit(
            store
                .prepare_rename_column(&cleared, "rename", "income", "salary")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(
        yss_tabular_arrow::column_identity(renamed.metadata().schema.field(0)).unwrap(),
        column_id
    );
    assert_eq!(filtered(&renamed, "salary", 10).row_count, 1);
    let projected = renamed
        .query(&engine, "schema")
        .unwrap()
        .relation()
        .unwrap()
        .project(&["salary".into()])
        .unwrap();
    assert_eq!(
        yss_tabular_arrow::column_identity(projected.schema().field(0)).unwrap(),
        column_id
    );
    let batch = store.runtime.as_ref().unwrap().block_on(async {
        let mut stream = projected.stream(control()).await.unwrap();
        std::future::poll_fn(|context| stream.as_mut().poll_next(context))
            .await
            .unwrap()
            .unwrap()
    });
    assert_eq!(batch.schema(), projected.schema());
    let row = renamed
        .query(&engine, "test-session")
        .unwrap()
        .row(1, &control())
        .unwrap()
        .unwrap();
    assert!(row.column_by_name("salary").unwrap().is_null(0));
    let row = renamed
        .query(&engine, "test-session")
        .unwrap()
        .row(0, &control())
        .unwrap()
        .unwrap();
    assert!(
        row.column_by_name("marker")
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .value(0)
            .is_nan()
    );
    let old = original
        .query(&engine, "test-session")
        .unwrap()
        .row(0, &control())
        .unwrap()
        .unwrap();
    assert_eq!(
        old.column_by_name("income")
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .value(0),
        8000.
    );
    let stale = store
        .prepare_cell_edit(
            &original,
            &engine,
            "stale",
            DatasetCellEdit {
                row_id: 0,
                column: "income",
                value: serde_json::json!(1),
            },
            &control(),
        )
        .unwrap();
    assert!(matches!(
        store.commit(stale),
        Err(DatasetStoreError::Conflict)
    ));
    let reopened = DatasetStore::open(directory.path()).unwrap();
    let restored = reopened.snapshot(&renamed.metadata().id).unwrap();
    assert_eq!(filtered(&restored, "salary", 10).row_count, 1);
}

#[test]
fn filtered_edit_branches_preserve_multicolumn_null_insert_delete_and_snapshot_semantics() {
    use arrow::array::{Float64Array, Int64Array};
    use yss_relational_contract::{RelationComparison, RelationLiteral, RelationPredicate};

    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let engine = yss_datafusion::DataFusionRuntime::new(128 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("x", DataType::Float64, true),
        Field::new("y", DataType::Float64, true),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Float64Array::from_iter_values((0..6).map(f64::from))),
            Arc::new(Float64Array::from_iter_values((10..16).map(f64::from))),
        ],
    )
    .unwrap();
    let original = store
        .commit(
            store
                .prepare_import(identity(), "Branches", "import", schema, [Ok(batch)])
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let mut current = store
        .commit(
            store
                .prepare_add_row(&original, &engine, "insert", 2, &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    for (index, (row_id, column, value)) in [
        (0, "x", serde_json::json!(10)),
        (4, "x", serde_json::Value::Null),
        (2, "y", serde_json::json!(99)),
        (6, "x", serde_json::json!(20)),
        (6, "y", serde_json::json!(60)),
        (0, "y", serde_json::json!(100)),
        (1, "y", serde_json::json!(21)),
    ]
    .into_iter()
    .enumerate()
    {
        current = store
            .commit(
                store
                    .prepare_cell_edit(
                        &current,
                        &engine,
                        &format!("edit-{index}"),
                        DatasetCellEdit {
                            row_id,
                            column,
                            value,
                        },
                        &control(),
                    )
                    .unwrap(),
            )
            .unwrap()
            .snapshot;
    }
    current = store
        .commit(
            store
                .prepare_delete_rows(&current, &engine, "delete", &[5], &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let filtered = |snapshot: &Arc<DatasetSnapshot>| {
        let relation = snapshot
            .query(&engine, "branches")
            .unwrap()
            .relation()
            .unwrap()
            .project(&["x".into(), "y".into()])
            .unwrap()
            .filter(&RelationPredicate {
                column: "x".into(),
                comparison: RelationComparison::Greater,
                value: Some(RelationLiteral::Integer(2)),
            })
            .unwrap();
        relation
            .numeric_columns(
                &[
                    relation.select_series("x").unwrap(),
                    relation.select_series("y").unwrap(),
                ],
                &control(),
            )
            .unwrap()
    };
    assert_eq!(
        filtered(&current),
        vec![vec![10., 20., 3.], vec![100., 60., 13.]]
    );
    assert_eq!(
        filtered(&original),
        vec![vec![3., 4., 5.], vec![13., 14., 15.]]
    );
    let nulls = current
        .query(&engine, "nulls")
        .unwrap()
        .relation()
        .unwrap()
        .filter(&RelationPredicate {
            column: "x".into(),
            comparison: RelationComparison::IsNull,
            value: None,
        })
        .unwrap();
    assert_eq!(
        nulls
            .numeric_columns(&[nulls.select_series("y").unwrap()], &control())
            .unwrap(),
        vec![vec![14.]]
    );
    let compacted = store
        .commit(
            store
                .prepare_compaction(&current, &engine, "compact", &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(filtered(&compacted), filtered(&current));
    let row_name = yss_tabular_arrow::dataset_row_columns(&compacted.metadata().schema)
        .unwrap()
        .unwrap()
        .row_id;
    let page = compacted
        .query(&engine, "ordered")
        .unwrap()
        .page(0, 10, &control())
        .unwrap();
    let ids = page
        .batches
        .iter()
        .flat_map(|batch| {
            batch
                .column_by_name(&row_name)
                .unwrap()
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap()
                .values()
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![0, 1, 6, 2, 3, 4]);
    let reopened = DatasetStore::open(directory.path())
        .unwrap()
        .snapshot(&current.metadata().id)
        .unwrap();
    assert_eq!(filtered(&reopened), filtered(&current));
}

#[test]
fn inserted_rows_keep_display_position_and_restore_does_not_reuse_row_ids() {
    use arrow::array::Int64Array;
    let (_directory, store, original, engine) = editable_fixture();
    let ids = |snapshot: &Arc<DatasetSnapshot>| {
        let name = yss_tabular_arrow::dataset_row_columns(&snapshot.metadata().schema)
            .unwrap()
            .unwrap()
            .row_id;
        snapshot
            .query(&engine, "test-session")
            .unwrap()
            .page(0, 20, &control())
            .unwrap()
            .batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column_by_name(&name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect::<Vec<_>>()
    };
    let inserted = store
        .commit(
            store
                .prepare_add_row(&original, &engine, "insert", 1, &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(ids(&inserted), vec![0, 3, 1, 2]);
    let edited = store
        .commit(
            store
                .prepare_cell_edit(
                    &inserted,
                    &engine,
                    "insert-value",
                    DatasetCellEdit {
                        row_id: 3,
                        column: "income",
                        value: serde_json::json!(42),
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let deleted = store
        .commit(
            store
                .prepare_delete_rows(&edited, &engine, "delete", &[0, 2], &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(ids(&deleted), vec![3, 1]);
    let undo = store
        .commit(store.prepare_restore(&deleted, &edited, "undo").unwrap())
        .unwrap()
        .snapshot;
    assert_eq!(ids(&undo), vec![0, 3, 1, 2]);
    let redo = store
        .commit(store.prepare_restore(&undo, &deleted, "redo").unwrap())
        .unwrap()
        .snapshot;
    assert_eq!(ids(&redo), vec![3, 1]);
    let restored = store
        .commit(
            store
                .prepare_restore(&redo, &original, "restore-original")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let inserted = store
        .commit(
            store
                .prepare_add_row(&restored, &engine, "insert-after-undo", 0, &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(ids(&inserted), vec![4, 0, 1, 2]);
    let added = store
        .commit(
            store
                .prepare_add_column(&inserted, "add-column", "new", DataType::Int64)
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert!(
        added
            .query(&engine, "test-session")
            .unwrap()
            .row(4, &control())
            .unwrap()
            .unwrap()
            .column_by_name("new")
            .unwrap()
            .is_null(0)
    );
    let dropped = store
        .commit(
            store
                .prepare_delete_column(&added, "drop-column", "marker")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_eq!(ids(&dropped), vec![4, 0, 1, 2]);
    assert!(dropped.metadata().schema.index_of("marker").is_err());
}

#[test]
fn compaction_and_cast_publish_new_files_without_losing_undo_values_or_categories() {
    use arrow::array::{Float64Array, Int64Array};
    let (_directory, store, original, engine) = editable_fixture();
    let before = store
        .commit(
            store
                .prepare_cell_edit(
                    &original,
                    &engine,
                    "fraction",
                    DatasetCellEdit {
                        row_id: 0,
                        column: "income",
                        value: serde_json::json!(8000.5),
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let compacted = store
        .commit(
            store
                .prepare_compaction(&before, &engine, "compact", &control())
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    assert_ne!(
        compacted.metadata().generation_id,
        before.metadata().generation_id
    );
    assert_eq!(
        compacted.metadata().data_revision,
        before.metadata().data_revision
    );
    assert!(compacted.overlay.columns.is_empty());
    assert!(before.file_paths().iter().all(|path| path.exists()));
    let cast = store
        .commit(
            store
                .prepare_cast_column(
                    &compacted,
                    &engine,
                    "cast",
                    DatasetColumnCast {
                        column: "income",
                        data_type: DataType::Int64,
                        force: false,
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let row = cast
        .query(&engine, "test")
        .unwrap()
        .row(0, &control())
        .unwrap()
        .unwrap();
    assert_eq!(
        row.column_by_name("income")
            .unwrap()
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .value(0),
        8000
    );
    let restored = store
        .commit(
            store
                .prepare_restore(&cast, &compacted, "undo-cast")
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let row = restored
        .query(&engine, "test")
        .unwrap()
        .row(0, &control())
        .unwrap()
        .unwrap();
    assert_eq!(
        row.column_by_name("income")
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .value(0),
        8000.5
    );
    assert!(
        store
            .prepare_cast_column(
                &restored,
                &engine,
                "failed-cast",
                DatasetColumnCast {
                    column: "marker",
                    data_type: DataType::Int64,
                    force: false
                },
                &control()
            )
            .is_err()
    );
    assert_eq!(
        store
            .snapshot(&restored.metadata().id)
            .unwrap()
            .metadata()
            .snapshot_id,
        restored.metadata().snapshot_id
    );
    let category = DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8));
    let categorical = store
        .commit(
            store
                .prepare_cast_column(
                    &restored,
                    &engine,
                    "category",
                    DatasetColumnCast {
                        column: "income",
                        data_type: category,
                        force: false,
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let labels =
        yss_tabular_arrow::CategoryDomain::from_field(categorical.metadata().schema.field(0))
            .unwrap()
            .unwrap();
    assert_eq!(labels.labels.len(), 3);
    let label = labels.labels[0].clone();
    let changed = store
        .commit(
            store
                .prepare_cell_edit(
                    &categorical,
                    &engine,
                    "category-edit",
                    DatasetCellEdit {
                        row_id: 0,
                        column: "income",
                        value: serde_json::json!(label),
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let page = changed
        .query(&engine, "test")
        .unwrap()
        .relation()
        .unwrap()
        .page(0, 1, &control())
        .unwrap();
    assert_eq!(
        page.data.columns()[0].values()[0],
        yss_tabular_contract::TabularScalar::String(label.into())
    );
}

fn identity() -> DatabaseId {
    DatabaseId::from_existing(Uuid::new_v4().to_string().into())
}

fn batch() -> RecordBatch {
    RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::UInt64, false),
            Field::new("amount", DataType::Decimal128(38, 12), true),
            Field::new(
                "at",
                DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                true,
            ),
        ])),
        vec![
            Arc::new(UInt64Array::from(vec![u64::MAX, 2])),
            Arc::new(
                Decimal128Array::from(vec![Some(12345678901234567890123456789), None])
                    .with_precision_and_scale(38, 12)
                    .unwrap(),
            ),
            Arc::new(TimestampNanosecondArray::from(vec![Some(-1), None]).with_timezone("UTC")),
        ],
    )
    .unwrap()
}

#[test]
fn committed_catalog_reopens_exact_data_and_recovers_pending_publication() {
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let id = identity();
    let batch = batch();
    let prepared = store
        .prepare_import(
            id.clone(),
            "Exact data",
            "import-1",
            batch.schema(),
            [Ok(batch.clone())],
        )
        .unwrap();
    let expected_schema = prepared.metadata().schema.clone();
    assert_eq!(
        expected_schema.field(2).data_type(),
        &DataType::Timestamp(TimeUnit::Nanosecond, None)
    );
    assert!(store.catalog_metadata().unwrap().is_empty());
    assert!(
        DatasetStore::open(directory.path())
            .unwrap()
            .catalog_metadata()
            .unwrap()
            .is_empty()
    );
    let committed = store.commit(prepared).unwrap();
    let publication = committed.publication.clone();
    let old_files = committed.snapshot.file_paths();
    assert_eq!(committed.snapshot.metadata().row_count, 2);
    drop(committed);
    drop(store);
    let reopened = DatasetStore::open(directory.path()).unwrap();
    assert_eq!(
        reopened.pending_publications().unwrap(),
        vec![publication.clone()]
    );
    let snapshot = reopened.snapshot(&id).unwrap();
    assert_eq!(snapshot.metadata().schema, expected_schema);
    assert_eq!(snapshot.file_paths(), old_files);
    let reader = yss_tabular_io::read_parquet_batches(&snapshot.file_paths()[0], 1, None).unwrap();
    assert_eq!(reader.schema(), expected_schema);
    let rows = reader.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        rows[0]
            .column(0)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap()
            .value(0),
        u64::MAX
    );
    let engine = yss_datafusion::DataFusionRuntime::new(32 * 1024 * 1024, 2).unwrap();
    let relation = engine
        .parquet_relation(
            RelationBinding {
                project_session: "catalog-session".into(),
                dataset: id,
                snapshot: snapshot.metadata().snapshot_id.clone(),
                revision: 1,
            },
            expected_schema,
            &snapshot.file_paths(),
            snapshot.clone(),
        )
        .unwrap();
    let page = relation
        .page(
            0,
            2,
            &RelationControl {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: Instant::now() + Duration::from_secs(10),
                max_input_bytes: 1024 * 1024,
            },
        )
        .unwrap();
    assert_eq!(page.row_count, 2);
    assert_eq!(page.columns.len(), 3);
    reopened.acknowledge_publication(&publication).unwrap();
    assert!(reopened.pending_publications().unwrap().is_empty());
    // A completed or failed external writer cannot enroll unlisted files in the catalog.
    std::fs::write(reopened.root.join("unlisted.parquet"), b"uncommitted").unwrap();
    assert_eq!(reopened.catalog_metadata().unwrap().len(), 1);
    assert_eq!(
        reopened
            .snapshot(&publication.dataset)
            .unwrap()
            .files()
            .len(),
        1
    );
}

#[test]
fn imported_dictionary_domains_survive_reordered_parquet_batches() {
    use arrow::array::{DictionaryArray, Int8Array};
    use arrow::datatypes::Int8Type;
    let directory = Directory::new();
    let schema = Arc::new(Schema::new(vec![Field::new(
        "grade",
        DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
        true,
    )]));
    let batches = [["small", "large", "unused"], ["large", "small", "unused"]].map(|labels| {
        RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(
                DictionaryArray::<Int8Type>::try_new(
                    Int8Array::from(vec![0, 1]),
                    Arc::new(arrow::array::StringArray::from(labels.to_vec())),
                )
                .unwrap(),
            )],
        )
        .unwrap()
    });
    let file = directory.path().join("external.parquet");
    yss_tabular_io::write_parquet_batches(&file, schema, batches.into_iter().map(Ok)).unwrap();
    let reader = yss_tabular_io::read_parquet_batches(&file, 2, None).unwrap();
    let store = DatasetStore::create(directory.path()).unwrap();
    let snapshot = store
        .commit(
            store
                .prepare_import(
                    identity(),
                    "Grades",
                    "import-grades",
                    reader.schema(),
                    reader,
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let domain = yss_tabular_arrow::CategoryDomain::from_field(snapshot.metadata().schema.field(0))
        .unwrap()
        .unwrap();
    assert_eq!(
        domain
            .labels
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        ["small".to_owned(), "large".to_owned()].into()
    );
    let engine = yss_datafusion::DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let before = snapshot
        .query(&engine, "categories")
        .unwrap()
        .relation()
        .unwrap()
        .page(0, 4, &control())
        .unwrap();
    assert_eq!(
        serde_json::to_value(before.data.columns()[0].values()).unwrap(),
        serde_json::json!(["small", "large", "large", "small"])
    );
    let edited = store
        .commit(
            store
                .prepare_cell_edit(
                    &snapshot,
                    &engine,
                    "category-edit",
                    DatasetCellEdit {
                        row_id: 0,
                        column: "grade",
                        value: serde_json::json!("large"),
                    },
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let reopened = DatasetStore::open(directory.path())
        .unwrap()
        .snapshot(&edited.metadata().id)
        .unwrap();
    assert_eq!(reopened.metadata().schema, edited.metadata().schema);
    assert_eq!(
        serde_json::to_value(
            reopened
                .query(&engine, "reopened")
                .unwrap()
                .relation()
                .unwrap()
                .page(0, 1, &control())
                .unwrap()
                .data
                .columns()[0]
                .values()
        )
        .unwrap(),
        serde_json::json!(["large"])
    );
}

#[test]
fn failed_preparation_and_conflicting_commit_leave_the_committed_generation_intact() {
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let id = identity();
    let batch = batch();
    let first = store
        .prepare_import(
            id.clone(),
            "First",
            "first",
            batch.schema(),
            [Ok(batch.clone())],
        )
        .unwrap();
    let second = store
        .prepare_import(
            id.clone(),
            "Second",
            "second",
            batch.schema(),
            [Ok(batch.clone())],
        )
        .unwrap();
    let second_directory = store
        .root
        .join("datasets")
        .join(id.as_str())
        .join(second.metadata().generation_id.as_ref());
    let committed = store.commit(first).unwrap();
    assert!(matches!(
        store.commit(second),
        Err(DatasetStoreError::Conflict)
    ));
    assert!(!second_directory.exists());
    assert_eq!(
        store.snapshot(&id).unwrap().metadata().snapshot_id,
        committed.snapshot.metadata().snapshot_id
    );
    assert!(
        committed
            .snapshot
            .file_paths()
            .iter()
            .all(|path| path.is_file())
    );
    let failed_id = identity();
    let failed = store.prepare_import(
        failed_id.clone(),
        "Failed",
        "failed",
        batch.schema(),
        [
            Ok(batch),
            Err(arrow::error::ArrowError::ComputeError(
                "fixture source failure".into(),
            )),
        ],
    );
    assert!(failed.is_err());
    assert!(matches!(
        store.snapshot(&failed_id),
        Err(DatasetStoreError::NotFound)
    ));
    let failed_directory = store.root.join("datasets").join(failed_id.as_str());
    assert_eq!(std::fs::read_dir(failed_directory).unwrap().count(), 0);
}

#[test]
fn multipart_import_keeps_row_identity_continuous_across_batch_and_file_boundaries() {
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let schema = Arc::new(Schema::new(vec![Field::new(
        "value",
        DataType::UInt64,
        false,
    )]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(UInt64Array::from_iter_values(0..1_000_001))],
    )
    .unwrap();
    let prepared = store
        .prepare_import(identity(), "Parts", "parts", schema, [Ok(batch)])
        .unwrap();
    let committed = store.commit(prepared).unwrap();
    let snapshot = committed.snapshot;
    assert_eq!(
        snapshot
            .files()
            .iter()
            .map(|file| file.row_count)
            .collect::<Vec<_>>(),
        vec![1_000_000, 1]
    );
    let batch = yss_tabular_io::read_parquet_batches(&snapshot.file_paths()[1], 1, None)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let rows = yss_tabular_arrow::dataset_row_columns(&snapshot.metadata().schema)
        .unwrap()
        .unwrap();
    let row_id = batch
        .column_by_name(&rows.row_id)
        .unwrap()
        .as_any()
        .downcast_ref::<arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(row_id, 1_000_000);
    let engine = yss_datafusion::DataFusionRuntime::new(128 * 1024 * 1024, 8192).unwrap();
    let mut input = snapshot.relation_input();
    input.files.reverse();
    let query = engine
        .dataset_query(
            RelationBinding {
                project_session: "reversed-parts".into(),
                dataset: snapshot.metadata().id.clone(),
                snapshot: snapshot.metadata().snapshot_id.clone(),
                revision: snapshot.metadata().data_revision,
            },
            input,
            snapshot.clone(),
        )
        .unwrap();
    let relation = query
        .relation()
        .unwrap()
        .project(&["value".into()])
        .unwrap();
    for (offset, expected) in [
        (0, vec![0., 1., 2.]),
        (999_998, vec![999_998., 999_999., 1_000_000.]),
    ] {
        let page = relation.limit(offset, 3).unwrap();
        assert_eq!(
            page.numeric_columns(&[page.select_series("value").unwrap()], &control())
                .unwrap(),
            vec![expected]
        );
    }
}
