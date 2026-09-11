use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use arrow::array::{Float64Array, StringArray};
use arrow::datatypes::{Field, Schema};
use arrow::record_batch::RecordBatch;
use yss_database_contract::DatabaseId;
use yss_relational_contract::{RelationComparison, RelationLiteral, RelationPredicate};

use super::*;

struct RemoveFile(PathBuf);
impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

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
fn computed_series_remain_lazy_and_aligned_through_broadcasts_and_chained_arithmetic() {
    use yss_relational_contract::{
        NumericOperation as Op, NumericType as Type, SeriesOperand as Operand,
    };
    use yss_tabular_contract::TabularScalar;
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(
        yss_tabular_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("x.value", DataType::Float64, false),
                Field::new("count", DataType::Int64, false),
                Field::new("row_id", DataType::Int64, false),
                Field::new("display_order", DataType::Utf8, false),
            ]),
            "row_id",
            "display_order",
        )
        .unwrap(),
    );
    let path = std::env::temp_dir().join(format!(
        "yss-arithmetic-{}-{}.parquet",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = runtime
        .parquet_relation(
            binding(),
            schema.clone(),
            std::slice::from_ref(&path),
            Arc::new(RemoveFile(path.clone())),
        )
        .unwrap();
    let x = source.select_series("x.value").unwrap();
    let count = source.select_series("count").unwrap();
    let scalar = |value| Operand::Scalar(RelationLiteral::Integer(value));
    let scaled = source
        .numeric_series(
            Op::Multiply,
            &[Operand::Series(x.clone()), scalar(123)],
            Type::Float64,
        )
        .unwrap();
    let subtracted = source
        .numeric_series(
            Op::Subtract,
            &[scalar(10), Operand::Series(x.clone())],
            Type::Float64,
        )
        .unwrap();
    let divided = source
        .numeric_series(
            Op::Divide,
            &[scalar(10), Operand::Series(x.clone())],
            Type::Float64,
        )
        .unwrap();
    let added = source
        .numeric_series(
            Op::Add,
            &[
                Operand::Series(scaled.clone()),
                Operand::Series(x.clone()),
                scalar(1),
            ],
            Type::Float64,
        )
        .unwrap();
    let integral = source
        .numeric_series(
            Op::Multiply,
            &[Operand::Series(count), scalar(1)],
            Type::Int64,
        )
        .unwrap();
    assert_eq!(scaled.relation(), &source);
    let projected = integral.as_relation().unwrap();
    assert!(
        !path.exists(),
        "constructing arithmetic and projections must not read rows"
    );
    const BASE: i64 = 9_007_199_254_740_992;
    let batches = [[4., 5., 6.], [1., 2., 3.]].map(|values| {
        RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Float64Array::from(values.to_vec())),
                Arc::new(Int64Array::from_iter_values(
                    values.map(|value| BASE + value as i64),
                )),
                Arc::new(Int64Array::from_iter_values(
                    values.map(|value| 100 - value as i64),
                )),
                Arc::new(StringArray::from_iter_values(
                    values.map(|value| format!("{:020}", value as i64)),
                )),
            ],
        )
        .unwrap()
    });
    yss_tabular_io::write_parquet_batches(&path, schema, batches.into_iter().map(Ok)).unwrap();
    let columns = source
        .numeric_columns(&[scaled, x, subtracted, divided, added], &control())
        .unwrap();
    let expected = [
        vec![123., 246., 369., 492., 615., 738.],
        vec![1., 2., 3., 4., 5., 6.],
        vec![9., 8., 7., 6., 5., 4.],
        vec![10., 5., 10. / 3., 2.5, 2., 10. / 6.],
        vec![125., 249., 373., 497., 621., 745.],
    ];
    for (actual, expected) in columns.iter().zip(expected) {
        assert_eq!(actual, &expected);
    }
    let page = projected.page(0, 2, &control()).unwrap();
    assert_eq!(page.columns[0].data_type.as_ref(), "Int64");
    assert_eq!(
        page.data.columns()[0].values()[0],
        TabularScalar::String((BASE + 1).to_string().into())
    );
    assert!(page.has_more);
}

#[test]
fn computed_series_reject_unaligned_inputs_and_propagate_numeric_errors_and_budgets() {
    use yss_relational_contract::{
        NumericOperation as Op, NumericType as Type, SeriesOperand as Operand,
    };
    let runtime = DataFusionRuntime::new(32 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("x.value", DataType::Float64, false),
        Field::new("zero", DataType::Float64, false),
        Field::new("huge", DataType::Float64, false),
        Field::new("missing", DataType::Float64, true),
    ]));
    let source = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                schema,
                vec![
                    Arc::new(Float64Array::from(vec![1., 2.])),
                    Arc::new(Float64Array::from(vec![1., 0.])),
                    Arc::new(Float64Array::from(vec![f64::MAX, 1.])),
                    Arc::new(Float64Array::from(vec![None, Some(2.)])),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let scalar = |value| Operand::Scalar(RelationLiteral::Integer(value));
    let series = |name| Operand::Series(source.select_series(name).unwrap());
    let filtered = source
        .filter(&predicate(RelationComparison::Greater, 1))
        .unwrap();
    assert_eq!(
        source.numeric_series(
            Op::Multiply,
            &[
                series("x.value"),
                Operand::Series(filtered.select_series("x.value").unwrap())
            ],
            Type::Float64
        ),
        Err(RelationError::UnalignedSeries)
    );
    for (operation, operands, expected) in [
        (
            Op::Divide,
            [scalar(1), series("zero")],
            RelationError::DivisionByZero,
        ),
        (
            Op::Multiply,
            [series("huge"), scalar(2)],
            RelationError::NonFiniteResult,
        ),
        (
            Op::Add,
            [series("missing"), scalar(1)],
            RelationError::InvalidInput,
        ),
    ] {
        let computed = source
            .numeric_series(operation, &operands, Type::Float64)
            .unwrap();
        assert_eq!(
            source.numeric_columns(&[computed], &control()),
            Err(expected)
        );
    }
    let valid = source
        .numeric_series(Op::Multiply, &[series("x.value"), scalar(2)], Type::Float64)
        .unwrap();
    let mut budget = control();
    budget.max_input_bytes = 1;
    assert_eq!(
        source.numeric_columns(std::slice::from_ref(&valid), &budget),
        Err(RelationError::MemoryLimitExceeded)
    );
    let mut cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        valid.as_relation().unwrap().page(0, 1, &cancelled),
        Err(RelationError::Cancelled)
    );
    cancelled
        .cancellation
        .store(false, std::sync::atomic::Ordering::Release);
    cancelled.deadline = Instant::now();
    assert_eq!(
        source.numeric_columns(&[valid], &cancelled),
        Err(RelationError::DeadlineExceeded)
    );
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
fn managed_file_prefixes_preserve_projection_offset_and_filter_order() {
    use yss_relational_contract::{DatasetOverlay, DatasetRelationInput};

    let runtime = DataFusionRuntime::new(512 * 1024 * 1024, 8).unwrap();
    let schema = Arc::new(
        yss_tabular_arrow::with_row_columns(
            Schema::new(
                [
                    Field::new("x.value", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                    Field::new("unused", DataType::Utf8, false),
                    Field::new("row_id", DataType::Int64, false),
                    Field::new("display_order", DataType::Utf8, false),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, field)| {
                    yss_tabular_arrow::with_column_metadata(field, &format!("column-{index}"), None)
                        .unwrap()
                })
                .collect::<Vec<_>>(),
            ),
            "row_id",
            "display_order",
        )
        .unwrap(),
    );
    let path = std::env::temp_dir().join(format!(
        "yss-prefix-{}-{}.parquet",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let lease = Arc::new(RemoveFile(path.clone()));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Float64Array::from_iter_values((0..32).map(f64::from))),
            Arc::new(Float64Array::from_iter_values(
                (0..32).map(|row| 3. + 2. * f64::from(row)),
            )),
            Arc::new(StringArray::from(vec!["unused"; 32])),
            Arc::new(Int64Array::from_iter_values((0..32).map(|row| 100 - row))),
            Arc::new(StringArray::from_iter_values(
                (0..32).map(|row| format!("{row:020}")),
            )),
        ],
    )
    .unwrap();
    yss_tabular_io::write_parquet_batches(&path, schema.clone(), [Ok(batch)]).unwrap();
    let query = runtime
        .dataset_query(
            binding(),
            DatasetRelationInput {
                base_schema: schema.clone(),
                schema: schema.clone(),
                files: vec![path].into_boxed_slice(),
                overlay: DatasetOverlay::default(),
            },
            lease,
        )
        .unwrap();
    let relation = query
        .relation()
        .unwrap()
        .project(&["x.value".into(), "y".into()])
        .unwrap();
    let sample = relation
        .limit(3, 5)
        .unwrap()
        .rename("y", "response")
        .unwrap();
    assert_eq!(
        sample
            .numeric_columns(
                &[
                    sample.select_series("response").unwrap(),
                    sample.select_series("x.value").unwrap()
                ],
                &control()
            )
            .unwrap(),
        vec![vec![9., 11., 13., 15., 17.], vec![3., 4., 5., 6., 7.]]
    );
    let before_limit = relation
        .filter(&predicate(RelationComparison::Greater, 10))
        .unwrap()
        .limit(1, 3)
        .unwrap();
    assert_eq!(
        before_limit
            .numeric_columns(
                &[before_limit.select_series("x.value").unwrap()],
                &control()
            )
            .unwrap(),
        vec![vec![12., 13., 14.]]
    );
    let after_limit = relation
        .limit(3, 5)
        .unwrap()
        .filter(&predicate(RelationComparison::Greater, 5))
        .unwrap();
    assert_eq!(
        after_limit
            .numeric_columns(&[after_limit.select_series("x.value").unwrap()], &control())
            .unwrap(),
        vec![vec![6., 7.]]
    );
    let page = query.page(3, 5, &control()).unwrap();
    assert_eq!(page.row_count, 5);
    assert!(page.has_more);

    // Guard the managed source's optimization contract without wall-clock thresholds.
    let frame = ordered_user_frame(query.frame.clone(), &schema)
        .unwrap()
        .0
        .select([
            Expr::Column(Column::from_name("x.value")),
            Expr::Column(Column::from_name("y")),
        ])
        .unwrap();
    let frame = limit_frame(frame, 0, 5, true).unwrap();
    let plan = runtime
        .runtime
        .as_ref()
        .unwrap()
        .block_on(frame.create_physical_plan())
        .unwrap();
    let plan = format!(
        "{}",
        datafusion::physical_plan::displayable(plan.as_ref()).indent(true)
    );
    assert!(!plan.contains("SortExec"), "{plan}");
    assert!(!plan.contains("SortPreservingMergeExec"), "{plan}");
    assert!(plan.contains("projection=[x.value, y]"), "{plan}");
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
