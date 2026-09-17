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
fn comparison_series_are_lazy_exact_nullable_and_aligned() {
    use yss_relational_contract::{ComparisonOperand as O, ComparisonOperation as C};
    use yss_tabular_contract::TabularScalar as V;
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let path = std::env::temp_dir().join(format!(
        "yss-comparison-{}-{}.parquet",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let schema = Arc::new(
        yss_tabular_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("text", DataType::Utf8, true),
                Field::new("integer", DataType::Int64, true),
                Field::new("real", DataType::Float64, true),
                Field::new("fixed", DataType::Decimal128(20, 2), true),
                Field::new("row_id", DataType::Int64, false),
                Field::new("display_order", DataType::Utf8, false),
            ]),
            "row_id",
            "display_order",
        )
        .unwrap(),
    );
    let source = runtime
        .parquet_relation(
            binding(),
            schema.clone(),
            std::slice::from_ref(&path),
            Arc::new(RemoveFile(path.clone())),
        )
        .unwrap();
    let integer = source.select_series("integer").unwrap();
    let real = source.select_series("real").unwrap();
    let text = source.select_series("text").unwrap();
    let operations = [
        C::Equal,
        C::NotEqual,
        C::Less,
        C::LessEqual,
        C::Greater,
        C::GreaterEqual,
    ];
    let mut columns = operations
        .map(|op| {
            source
                .compare_series(op, &[O::Series(integer.clone()), O::Series(real.clone())])
                .unwrap()
        })
        .to_vec();
    for operands in [
        [O::Series(text.clone()), O::Scalar(V::String("1".into()))],
        [O::Scalar(V::String("1".into())), O::Series(text.clone())],
    ] {
        columns.push(source.compare_series(C::Equal, &operands).unwrap());
    }
    columns.push(
        source
            .compare_series(
                C::Less,
                &[O::Series(integer.clone()), O::Scalar(V::Unsigned(u64::MAX))],
            )
            .unwrap(),
    );
    columns.push(
        source
            .compare_series(
                C::Equal,
                &[
                    O::Series(source.select_series("fixed").unwrap()),
                    O::Scalar(V::Integer(1)),
                ],
            )
            .unwrap(),
    );
    let combined = source.project_series(&columns).unwrap();
    assert!(
        !path.exists(),
        "constructing comparisons must not read rows"
    );
    let other = source
        .limit(0, 2)
        .unwrap()
        .select_series("integer")
        .unwrap();
    assert_eq!(
        source.compare_series(C::Equal, &[O::Series(integer), O::Series(other)]),
        Err(RelationError::UnalignedSeries)
    );
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec![Some("001"), None, Some("1")])),
            Arc::new(Int64Array::from(vec![
                Some(9_007_199_254_740_993),
                None,
                Some(-1),
            ])),
            Arc::new(Float64Array::from(vec![
                Some(9_007_199_254_740_992.0),
                Some(1.0),
                Some(0.0),
            ])),
            Arc::new(
                arrow::array::Decimal128Array::from(vec![Some(100), None, Some(101)])
                    .with_precision_and_scale(20, 2)
                    .unwrap(),
            ),
            Arc::new(Int64Array::from(vec![0, 1, 2])),
            Arc::new(StringArray::from(vec!["000", "001", "002"])),
        ],
    )
    .unwrap();
    yss_tabular_io::write_parquet_batches(&path, schema, [Ok(batch)]).unwrap();
    let page = combined.page(0, 3, &control()).unwrap();
    for (index, (first, last)) in [
        (false, false),
        (true, true),
        (false, true),
        (false, true),
        (true, false),
        (true, false),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            page.data.columns()[index].values(),
            &[V::Bool(first), V::Null, V::Bool(last)]
        );
    }
    for index in [6, 7] {
        assert_eq!(
            page.data.columns()[index].values(),
            &[V::Bool(false), V::Null, V::Bool(true)]
        );
    }
    assert_eq!(
        page.data.columns()[8].values(),
        &[V::Bool(true), V::Null, V::Bool(true)]
    );
    assert!(combined.schema().field(0).is_nullable());
    assert_eq!(
        page.data.columns()[9].values(),
        &[V::Bool(true), V::Null, V::Bool(false)]
    );
    assert_eq!(
        yss_tabular_arrow::column_semantic(combined.schema().field(0))
            .unwrap()
            .kind,
        yss_data_contract::SemanticType::Binary
    );
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
fn semantic_conversion_is_lazy_aligned_and_preserves_configuration_identity() {
    use yss_data_contract::{NumericRepresentation as N, SemanticConversion, SemanticType as S};
    use yss_tabular_contract::TabularScalar as V;
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let path = std::env::temp_dir().join(format!(
        "yss-conversion-{}-{}.parquet",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let schema = Arc::new(
        yss_tabular_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("text", DataType::Utf8, true),
                Field::new("row_id", DataType::Int64, false),
                Field::new("display_order", DataType::Utf8, false),
            ]),
            "row_id",
            "display_order",
        )
        .unwrap(),
    );
    let relation = runtime
        .parquet_relation(
            binding(),
            schema.clone(),
            std::slice::from_ref(&path),
            Arc::new(RemoveFile(path.clone())),
        )
        .unwrap();
    let input = relation.select_series("text").unwrap();
    let convert = |target, numeric| {
        relation
            .convert_series(&input, SemanticConversion::new(target, numeric))
            .unwrap()
    };
    let integer = convert(S::Numeric, N::Integer);
    let real = convert(S::Numeric, N::Real);
    let binary = convert(S::Binary, N::Auto);
    assert_ne!(integer, real);
    assert_eq!(integer.relation(), &relation);
    let combined = relation.project_series(&[integer.clone(), real]).unwrap();
    assert!(
        !path.exists(),
        "conversion and projection must not read rows"
    );
    let batches = [vec![Some("001"), None], vec![Some("2"), Some("3")]]
        .into_iter()
        .enumerate()
        .map(|(index, values)| {
            RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(StringArray::from(values)),
                    Arc::new(Int64Array::from(vec![
                        (index * 2) as i64,
                        (index * 2 + 1) as i64,
                    ])),
                    Arc::new(StringArray::from(vec![
                        format!("{:04}", index * 2),
                        format!("{:04}", index * 2 + 1),
                    ])),
                ],
            )
            .unwrap()
        });
    yss_tabular_io::write_parquet_batches(&path, schema.clone(), batches.map(Ok)).unwrap();
    let page = combined.page(0, 4, &control()).unwrap();
    assert_eq!(combined.schema().field(0).data_type(), &DataType::Int64);
    assert_eq!(combined.schema().field(1).data_type(), &DataType::Float64);
    assert_eq!(
        page.data.columns()[0].values(),
        &[V::Unsigned(1), V::Null, V::Unsigned(2), V::Unsigned(3)]
    );
    assert_eq!(
        page.data.columns()[1].values(),
        &[
            V::Decimal(1.0.try_into().unwrap()),
            V::Null,
            V::Decimal(2.0.try_into().unwrap()),
            V::Decimal(3.0.try_into().unwrap())
        ]
    );
    assert_eq!(
        yss_tabular_arrow::column_semantic(combined.schema().field(0))
            .unwrap()
            .kind,
        S::Numeric
    );
    assert_eq!(
        binary.as_relation().unwrap().page(0, 4, &control()),
        Err(RelationError::InvalidConversion)
    );
    let other = relation.limit(0, 2).unwrap();
    assert_eq!(
        other.convert_series(&input, SemanticConversion::new(S::Text, N::Auto)),
        Err(RelationError::UnalignedSeries)
    );
    let mut cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        combined.page(0, 1, &cancelled),
        Err(RelationError::Cancelled)
    );
    cancelled
        .cancellation
        .store(false, std::sync::atomic::Ordering::Release);
    cancelled.deadline = Instant::now();
    assert_eq!(
        combined.page(0, 1, &cancelled),
        Err(RelationError::DeadlineExceeded)
    );
}

#[test]
fn semantic_domains_and_calendar_fields_survive_lazy_projection_and_chained_conversion() {
    use yss_data_contract::{
        NumericRepresentation as N, SemanticConversion, SemanticType as S, SemanticValue,
    };
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 1).unwrap();
    let source = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![
                    Field::new("code", DataType::Utf8, true),
                    Field::new("date", DataType::Utf8, true),
                ])),
                vec![
                    Arc::new(StringArray::from(vec![Some("001"), Some("002"), None])),
                    Arc::new(StringArray::from(vec![
                        Some("2026-09-17T08:30:00+08:00"),
                        Some("2026-09-18T09:30:00+08:00"),
                        None,
                    ])),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let code = source.select_series("code").unwrap();
    let mut spec = SemanticConversion::new(S::Ordinal, N::Auto);
    spec.domain.values = vec![
        SemanticValue {
            value: "002".into(),
            label: "low".into(),
        },
        SemanticValue {
            value: "001".into(),
            label: "high".into(),
        },
    ];
    let ordinal = source.convert_series(&code, spec.clone()).unwrap();
    spec.domain.values.reverse();
    let reversed = source.convert_series(&code, spec).unwrap();
    assert_ne!(ordinal, reversed);
    let categorical = source
        .convert_series(&ordinal, SemanticConversion::new(S::Categorical, N::Auto))
        .unwrap();
    let identifier = source
        .convert_series(
            &categorical,
            SemanticConversion::new(S::Identifier, N::Auto),
        )
        .unwrap();
    let datetime = source
        .convert_series(
            &source.select_series("date").unwrap(),
            SemanticConversion::new(S::Datetime, N::Auto),
        )
        .unwrap();
    let projected = source
        .project_series(&[ordinal, reversed, categorical, identifier, datetime])
        .unwrap();
    let schema = projected.schema();
    assert_eq!(
        yss_tabular_arrow::column_semantic(schema.field(0))
            .unwrap()
            .values[0]
            .value,
        "002"
    );
    assert_eq!(
        yss_tabular_arrow::column_semantic(schema.field(1))
            .unwrap()
            .values[0]
            .value,
        "001"
    );
    assert_eq!(
        yss_tabular_arrow::column_semantic(schema.field(2))
            .unwrap()
            .kind,
        S::Categorical
    );
    assert_eq!(
        yss_tabular_arrow::column_semantic(schema.field(3))
            .unwrap()
            .kind,
        S::Identifier
    );
    assert_eq!(
        schema.field(4).data_type(),
        &DataType::Timestamp(arrow::datatypes::TimeUnit::Microsecond, None)
    );
    let page = projected.page(0, 3, &control()).unwrap();
    for index in 0..4 {
        assert_eq!(
            serde_json::to_value(page.data.columns()[index].values()).unwrap(),
            serde_json::json!(["001", "002", null])
        );
    }
    assert_eq!(
        serde_json::to_value(page.data.columns()[4].values()).unwrap(),
        serde_json::json!(["2026-09-17T08:30:00", "2026-09-18T09:30:00", null])
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
