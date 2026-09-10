use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use arrow::array::{Float64Array, StringArray};
use arrow::datatypes::{Field, Schema};
use arrow::record_batch::RecordBatch;
use yss_database_contract::DatabaseId;
use yss_relational_contract::{RelationComparison, RelationLiteral, RelationPredicate};

use super::*;

fn binding() -> RelationBinding {
    RelationBinding {
        project_session: "session-1".into(),
        dataset: DatabaseId::from_existing("data".into()),
        snapshot: "snapshot-1".into(),
        revision: 7,
    }
}

fn control() -> RelationControl {
    RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(30),
        max_input_bytes: 1024 * 1024,
    }
}

fn predicate(comparison: RelationComparison, value: i64) -> RelationPredicate {
    RelationPredicate {
        column: "x.value".into(),
        comparison,
        value: Some(RelationLiteral::Integer(value)),
    }
}

#[test]
fn parquet_plans_are_lazy_and_joint_statistics_projection_preserves_alignment() {
    let runtime = DataFusionRuntime::new(32 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(
        yss_tabular_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("x.value", DataType::Float64, true),
                Field::new("y", DataType::Float64, true),
                Field::new("unused", DataType::Utf8, true),
                Field::new("row_id", DataType::Int64, false),
                Field::new("display_order", DataType::Utf8, false),
            ]),
            "row_id",
            "display_order",
        )
        .unwrap(),
    );
    let path = std::env::temp_dir().join(format!(
        "yss-datafusion-{}-{}.parquet",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    struct RemoveFile(PathBuf);
    impl Drop for RemoveFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let lease = Arc::new(RemoveFile(path.clone()));
    let source = runtime
        .parquet_relation(
            binding(),
            schema.clone(),
            std::slice::from_ref(&path),
            lease.clone(),
        )
        .unwrap();
    let projected = source.project(&["x.value".into(), "y".into()]).unwrap();
    let filtered = projected
        .filter(&predicate(RelationComparison::Greater, 2))
        .unwrap();
    assert!(
        !path.exists(),
        "source/project/filter must not scan data while constructing plans"
    );
    let batches = [[4., 5., 6.], [1., 2., 3.]].map(|x| {
        RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Float64Array::from(x.to_vec())),
                Arc::new(Float64Array::from(x.map(|x| 1. + 2. * x).to_vec())),
                Arc::new(StringArray::from(vec!["a", "b", "c"])),
                Arc::new(Int64Array::from(x.map(|value| 100 - value as i64).to_vec())),
                Arc::new(StringArray::from_iter_values(
                    x.map(|value| format!("{:020}", value as i64)),
                )),
            ],
        )
        .unwrap()
    });
    yss_tabular_io::write_parquet_batches(&path, schema, batches.into_iter().map(Ok)).unwrap();
    let series = [
        filtered.select_series("y").unwrap(),
        filtered.select_series("x.value").unwrap(),
    ];
    drop(source);
    drop(projected);
    drop(lease);
    assert!(
        path.exists(),
        "relation and series retain the immutable snapshot lease"
    );
    assert_eq!(
        runtime.numeric_columns(&series, &control()).unwrap(),
        vec![vec![7., 9., 11., 13.], vec![3., 4., 5., 6.]]
    );
    assert_eq!(series[0].relation().binding(), &binding());
    let other = filtered
        .filter(&predicate(RelationComparison::Greater, 4))
        .unwrap();
    assert_eq!(
        runtime.numeric_columns(
            &[series[0].clone(), other.select_series("x.value").unwrap()],
            &control()
        ),
        Err(RelationError::UnalignedSeries)
    );
    drop(series);
    drop(other);
    drop(filtered);
    assert!(!path.exists(), "the last relation releases the file lease");
}

#[test]
fn numeric_materialization_enforces_missing_values_cancellation_and_memory_budget() {
    let runtime = DataFusionRuntime::new(32 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, true)]));
    let batch = RecordBatch::try_new(
        schema,
        vec![Arc::new(Float64Array::from(vec![Some(1.), None, Some(3.)]))],
    )
    .unwrap();
    let source = runtime.batch_relation(binding(), batch).unwrap();
    let series = [source.select_series("x").unwrap()];
    assert_eq!(
        runtime.numeric_columns(&series, &control()),
        Err(RelationError::InvalidInput)
    );
    let mut cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        runtime.numeric_columns(&series, &cancelled),
        Err(RelationError::Cancelled)
    );
    cancelled
        .cancellation
        .store(false, std::sync::atomic::Ordering::Release);
    cancelled.deadline = Instant::now();
    assert_eq!(
        runtime.numeric_columns(&series, &cancelled),
        Err(RelationError::DeadlineExceeded)
    );
    let valid = source
        .filter(&RelationPredicate {
            column: "x".into(),
            comparison: RelationComparison::IsNotNull,
            value: None,
        })
        .unwrap();
    let series = [valid.select_series("x").unwrap()];
    let mut bounded = control();
    bounded.max_input_bytes = 1;
    assert_eq!(
        runtime.numeric_columns(&series, &bounded),
        Err(RelationError::MemoryLimitExceeded)
    );
    assert_eq!(
        runtime.numeric_columns(&series, &control()).unwrap(),
        vec![vec![1., 3.]]
    );
}

#[test]
fn relation_pages_probe_one_extra_row_and_preserve_wide_integer_display() {
    use arrow::array::UInt64Array;
    use yss_tabular_contract::TabularScalar;
    let runtime = DataFusionRuntime::new(32 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::UInt64, true)]));
    let source = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                schema.clone(),
                vec![Arc::new(UInt64Array::from(vec![
                    Some(u64::MAX),
                    Some(1),
                    None,
                ]))],
            )
            .unwrap(),
        )
        .unwrap();
    let first = source.page(0, 2, &control()).unwrap();
    assert_eq!(first.row_count, 2);
    assert!(first.has_more);
    assert_eq!(first.columns[0].data_type.as_ref(), "UInt64");
    assert_eq!(
        first.data.columns()[0].values(),
        &[
            TabularScalar::String(u64::MAX.to_string().into()),
            TabularScalar::Unsigned(1)
        ]
    );
    let last = source.page(2, 2, &control()).unwrap();
    assert_eq!(last.row_count, 1);
    assert!(!last.has_more);
    assert_eq!(last.data.columns()[0].values(), &[TabularScalar::Null]);
    assert!(source.page(0, 0, &control()).is_err());
    let mut budget = control();
    budget.max_input_bytes = 1;
    assert_eq!(
        source.page(0, 2, &budget),
        Err(RelationError::MemoryLimitExceeded)
    );
    let mut next = binding();
    next.snapshot = "snapshot-2".into();
    let _next = runtime
        .batch_relation(
            next,
            RecordBatch::try_new(schema, vec![Arc::new(UInt64Array::from(vec![9]))]).unwrap(),
        )
        .unwrap();
    assert_eq!(source.page(0, 2, &control()).unwrap(), first);
}
