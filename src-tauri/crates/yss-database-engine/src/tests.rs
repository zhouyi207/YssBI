use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use yss_data_contract::FilterLiteral;

use arrow::array::{Float64Array, StringArray};
use arrow::datatypes::{Field, Schema};
use arrow::record_batch::RecordBatch;
use yss_database_contract::DatabaseId;
use yss_relational_contract::{RelationComparison, RelationPredicate};

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
        value: Some(FilterLiteral::Integer(value)),
    }
}

#[test]
fn drop_rows_and_columns_preserve_nulls_order_and_source() {
    use yss_data_contract::TabularScalar as V;
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("x.value", DataType::Int64, true),
        Field::new("unused", DataType::Utf8, false),
        Field::new("id", DataType::Int64, false),
    ]));
    let source = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(Int64Array::from(vec![Some(-1), None, Some(0), Some(2)])),
                    Arc::new(StringArray::from(vec!["a", "b", "c", "d"])),
                    Arc::new(Int64Array::from(vec![1, 2, 3, 4])),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let projected = source.drop_columns(&["unused".into()]).unwrap();
    assert_eq!(projected.schema().fields().len(), 2);
    assert_eq!(projected.schema().field(0), schema.field(0));
    assert_eq!(projected.schema().field(1), schema.field(2));
    let renamed = projected.rename("x.value", "amount.value").unwrap();
    let mut condition = predicate(RelationComparison::Less, 0);
    condition.column = "amount.value".into();
    let kept = renamed.drop_rows(&condition).unwrap();
    let page = kept.page(0, 10, &control()).unwrap();
    assert_eq!(
        page.data.columns()[0].values(),
        &[V::Null, V::Unsigned(0), V::Unsigned(2)]
    );
    assert_eq!(
        page.data.columns()[1].values(),
        &[V::Unsigned(2), V::Unsigned(3), V::Unsigned(4)]
    );
    condition.comparison = RelationComparison::IsNull;
    condition.value = None;
    assert_eq!(
        kept.drop_rows(&condition)
            .unwrap()
            .page(0, 10, &control())
            .unwrap()
            .row_count,
        2
    );
    condition.comparison = RelationComparison::IsNotNull;
    assert_eq!(
        kept.drop_rows(&condition)
            .unwrap()
            .page(0, 10, &control())
            .unwrap()
            .data
            .columns()[0]
            .values(),
        &[V::Null]
    );
    assert_eq!(source.page(0, 10, &control()).unwrap().row_count, 4);
    assert_eq!(source.schema(), schema);
    for names in [
        vec![],
        vec!["missing"],
        vec!["unused", "unused"],
        vec!["x.value", "unused", "id"],
    ] {
        assert_eq!(
            source.drop_columns(&names.into_iter().map(Into::into).collect::<Vec<_>>()),
            Err(RelationError::InvalidInput)
        );
    }
}

#[test]
fn decimal_filters_preserve_precision_beyond_float64() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![Field::new(
                    "amount",
                    DataType::Decimal128(20, 2),
                    false,
                )])),
                vec![Arc::new(
                    arrow::array::Decimal128Array::from(vec![
                        900_719_925_474_099_301_i128,
                        900_719_925_474_099_302_i128,
                    ])
                    .with_precision_and_scale(20, 2)
                    .unwrap(),
                )],
            )
            .unwrap(),
        )
        .unwrap();
    let page = source
        .drop_rows(&RelationPredicate {
            column: "amount".into(),
            comparison: RelationComparison::Equal,
            value: Some(FilterLiteral::Decimal(
                yss_data_contract::DecimalLiteral::new("9007199254740993.01").unwrap(),
            )),
        })
        .unwrap()
        .page(0, 10, &control())
        .unwrap();
    assert_eq!(
        page.data.columns()[0].values(),
        &[yss_data_contract::TabularScalar::String(
            "9007199254740993.02".into()
        )]
    );
}

#[test]
fn table_composition_preserves_order_alignment_and_combined_sources() {
    use yss_data_contract::TabularScalar as V;
    use yss_data_contract::table::{RowConcatMode as Mode, TableJoin, TableJoinKind as Join};
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let make = |id: &str, keys: Vec<Option<i64>>, texts: Vec<&str>| {
        let mut source = binding();
        source.dataset = DatabaseId::from_existing(id.into());
        source.snapshot = format!("snapshot-{id}").into();
        runtime
            .batch_relation(
                source,
                RecordBatch::try_new(
                    Arc::new(Schema::new(vec![
                        Field::new("id.key", DataType::Int64, true),
                        Field::new("label", DataType::Utf8, false),
                    ])),
                    vec![
                        Arc::new(Int64Array::from(keys)),
                        Arc::new(StringArray::from(texts)),
                    ],
                )
                .unwrap(),
            )
            .unwrap()
    };
    let left = make(
        "left",
        vec![Some(2), Some(1), None],
        vec!["b", "a", "missing-left"],
    );
    let right = make(
        "right",
        vec![Some(1), Some(1), Some(3), None],
        vec!["r1", "r2", "r3", "missing-right"],
    );
    let mask = left
        .compare_series(
            yss_relational_contract::ComparisonOperation::Equal,
            &[
                yss_relational_contract::ComparisonOperand::Series(
                    left.select_series("id.key").unwrap(),
                ),
                yss_relational_contract::ComparisonOperand::Scalar(V::Integer(1)),
            ],
        )
        .unwrap();
    let normalized = left
        .convert_series(
            &mask,
            yss_data_contract::SemanticConversion::new(
                yss_data_contract::SemanticType::Binary,
                yss_data_contract::NumericRepresentation::Auto,
            ),
        )
        .unwrap();
    let plain = mask.as_relation().unwrap();
    let normalized = normalized.as_relation().unwrap();
    assert_eq!(
        plain
            .concat_rows(std::slice::from_ref(&normalized), Mode::ByName)
            .unwrap()
            .page(0, 10, &control())
            .unwrap()
            .row_count,
        6
    );
    assert_eq!(
        plain
            .join(
                &normalized,
                &TableJoin {
                    kind: Join::Inner,
                    left_keys: vec!["result".into()],
                    right_keys: vec!["result".into()],
                    right_suffix: "_right".into()
                }
            )
            .unwrap()
            .page(0, 10, &control())
            .unwrap()
            .row_count,
        2
    );
    let concatenated = left
        .concat_rows(std::slice::from_ref(&right), Mode::ByName)
        .unwrap();
    assert_eq!(concatenated.bindings().len(), 2);
    let page = concatenated.page(0, 10, &control()).unwrap();
    assert_eq!(
        page.data.columns()[1].values(),
        ["b", "a", "missing-left", "r1", "r2", "r3", "missing-right"].map(|v| V::String(v.into()))
    );
    assert_eq!(
        concatenated.page(3, 2, &control()).unwrap().data.columns()[1].values(),
        &[V::String("r1".into()), V::String("r2".into())]
    );
    let a = left.project(&["label".into()]).unwrap();
    let b = left
        .project(&["id.key".into()])
        .unwrap()
        .rename("id.key", "key")
        .unwrap();
    let columns = a.concat_columns(std::slice::from_ref(&b)).unwrap();
    assert_eq!(
        columns.page(0, 10, &control()).unwrap().data.columns()[0].values(),
        &page.data.columns()[1].values()[..3]
    );
    assert_eq!(columns.schema().field(1).name(), "key");
    assert_eq!(
        a.concat_columns(&[left.limit(0, 3).unwrap()]),
        Err(RelationError::UnalignedSeries)
    );
    assert_eq!(
        a.concat_columns(std::slice::from_ref(&right)),
        Err(RelationError::UnalignedSeries)
    );
    let assembled = a
        .assemble_series(
            &[
                a.select_series("label").unwrap(),
                b.select_series("key").unwrap(),
            ],
            &["name".into(), "id".into()],
        )
        .unwrap();
    assert_eq!(assembled.schema().field(0).name(), "name");
    assert_eq!(assembled.page(0, 10, &control()).unwrap().row_count, 3);
    for (kind, count) in [
        (Join::Inner, 2),
        (Join::Left, 4),
        (Join::Right, 4),
        (Join::Full, 6),
    ] {
        let spec = TableJoin {
            kind,
            left_keys: vec!["id.key".into()],
            right_keys: vec!["id.key".into()],
            right_suffix: "_right".into(),
        };
        let joined = left.join(&right, &spec).unwrap();
        assert_eq!(joined.bindings().len(), 2);
        assert_eq!(joined.schema().field(2).name(), "id.key_right");
        let page = joined.page(0, 20, &control()).unwrap();
        assert_eq!(page.row_count, count, "{kind:?}");
        for offset in 0..count {
            let one = joined.page(offset, 1, &control()).unwrap();
            for (column, entire) in one.data.columns().iter().zip(page.data.columns()) {
                assert_eq!(column.values()[0], entire.values()[offset]);
            }
        }
        if kind == Join::Full {
            assert!(joined.schema().fields().iter().all(|f| f.is_nullable()));
        }
    }
    let missing = right
        .project(&["label".into()])
        .unwrap()
        .rename("label", "other")
        .unwrap();
    let padded = left
        .concat_rows(std::slice::from_ref(&missing), Mode::ByName)
        .unwrap();
    let page = padded.page(0, 10, &control()).unwrap();
    assert_eq!(page.data.columns()[2].values()[0], V::Null);
    assert_eq!(page.data.columns()[0].values()[3], V::Null);
    assert!(left.concat_rows(&[missing], Mode::ByPosition).is_err());
    let positioned = right.rename("id.key", "other-key").unwrap();
    assert_eq!(
        left.concat_rows(&[positioned], Mode::ByPosition)
            .unwrap()
            .schema()
            .field(0)
            .name(),
        "id.key"
    );
}

#[test]
fn composed_plans_are_lazy_and_retain_every_source_lease() {
    use yss_data_contract::table::{RowConcatMode, TableJoin, TableJoinKind};
    struct Lease(Arc<std::sync::atomic::AtomicUsize>, PathBuf);
    impl Drop for Lease {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = std::fs::remove_file(&self.1);
        }
    }
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(
        yss_database_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("id", DataType::Int64, false),
                Field::new("row_id", DataType::Int64, false),
                Field::new("order", DataType::Utf8, false),
            ]),
            "row_id",
            "order",
        )
        .unwrap(),
    );
    let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let make = |id: &str| {
        let mut source = binding();
        source.dataset = DatabaseId::from_existing(id.into());
        source.snapshot = id.into();
        let path = std::env::temp_dir().join(format!(
            "yss-compose-{}-{id}-{}.parquet",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let relation = runtime
            .parquet_relation(
                source,
                schema.clone(),
                std::slice::from_ref(&path),
                Arc::new(Lease(dropped.clone(), path.clone())),
            )
            .unwrap();
        (relation, path)
    };
    let (a, a_path) = make("a");
    let (b, b_path) = make("b");
    let result = a
        .concat_rows(std::slice::from_ref(&b), RowConcatMode::ByName)
        .unwrap();
    let joined = result
        .join(
            &a,
            &TableJoin {
                kind: TableJoinKind::Left,
                left_keys: vec!["id".into()],
                right_keys: vec!["id".into()],
                right_suffix: "_right".into(),
            },
        )
        .unwrap();
    assert_eq!(joined.bindings().len(), 2);
    assert!(!a_path.exists() && !b_path.exists());
    for path in [&a_path, &b_path] {
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int64Array::from(vec![1])),
                Arc::new(Int64Array::from(vec![0])),
                Arc::new(StringArray::from(vec!["0"])),
            ],
        )
        .unwrap();
        yss_database_io::write_parquet_batches(path, schema.clone(), [Ok(batch)]).unwrap();
    }
    drop(a);
    drop(b);
    drop(result);
    assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(joined.page(0, 3, &control()).unwrap().row_count, 2);
    let stream = runtime
        .runtime
        .as_ref()
        .unwrap()
        .block_on(joined.stream(control()))
        .unwrap();
    drop(joined);
    assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(stream);
    assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn boolean_series_use_native_lazy_expressions_with_nulls_and_alignment() {
    use arrow::array::BooleanArray;
    use datafusion::logical_expr::{Expr, Operator};
    use yss_data_contract::TabularScalar as V;
    use yss_relational_contract::{BooleanOperand as O, BooleanOperation as Op};
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let path = std::env::temp_dir().join(format!(
        "yss-boolean-{}-{}.parquet",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut semantic =
        yss_data_contract::ColumnSemantic::new(yss_data_contract::SemanticType::Binary);
    semantic.values = ["0", "1"]
        .map(|v| yss_data_contract::SemanticValue {
            value: v.into(),
            label: v.into(),
        })
        .into();
    semantic.positive_value = Some("0".into());
    let coded = yss_database_arrow::with_column_semantic(
        Field::new("coded", DataType::Int64, true),
        &semantic,
    )
    .unwrap();
    let schema = Arc::new(
        yss_database_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("left", DataType::Boolean, true),
                Field::new("right", DataType::Boolean, true),
                coded,
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
    let left = source.select_series("left").unwrap();
    let right = source.select_series("right").unwrap();
    let operands = [O::Series(left.clone()), O::Series(right.clone())];
    let and = source.boolean_series(Op::And, &operands).unwrap();
    let or = source.boolean_series(Op::Or, &operands).unwrap();
    let not = source
        .boolean_series(Op::Not, &[O::Series(and.clone())])
        .unwrap();
    let expression = |value: &SeriesHandle| {
        value
            .plan()
            .as_any()
            .downcast_ref::<crate::series::DataFusionSeries>()
            .unwrap()
            .expression
            .clone()
    };
    assert!(matches!(expression(&and), Expr::BinaryExpr(v) if v.op == Operator::And));
    assert!(matches!(expression(&or), Expr::BinaryExpr(v) if v.op == Operator::Or));
    assert!(matches!(expression(&not), Expr::Not(_)));
    let broadcast = source
        .boolean_series(Op::And, &[O::Scalar(Some(false)), O::Series(left.clone())])
        .unwrap();
    let normalized = source
        .boolean_series(
            Op::Or,
            &[
                O::Series(source.select_series("coded").unwrap()),
                O::Scalar(Some(false)),
            ],
        )
        .unwrap();
    let projected = source
        .project_series(&[and, or, not, broadcast, normalized])
        .unwrap();
    assert!(!path.exists(), "building boolean plans must not read rows");
    let other = source.limit(0, 1).unwrap().select_series("left").unwrap();
    assert_eq!(
        source.boolean_series(Op::And, &[O::Series(left), O::Series(other)]),
        Err(RelationError::UnalignedSeries)
    );
    let f = Some(false);
    let t = Some(true);
    let n = None;
    let left = [f, f, f, t, t, t, n, n, n];
    let right = [f, t, n, f, t, n, f, t, n];
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(BooleanArray::from(left.to_vec())),
            Arc::new(BooleanArray::from(right.to_vec())),
            Arc::new(Int64Array::from(
                left.map(|v| v.map(|v| if v { 0 } else { 1 })).to_vec(),
            )),
            Arc::new(Int64Array::from_iter_values(0..9)),
            Arc::new(StringArray::from_iter_values(
                (0..9).map(|v| format!("{v:02}")),
            )),
        ],
    )
    .unwrap();
    yss_database_io::write_parquet_batches(&path, schema, [Ok(batch)]).unwrap();
    let page = projected.page(0, 9, &control()).unwrap();
    for (column, expected) in page.data.columns().iter().zip([
        [f, f, f, f, t, n, f, n, n],
        [f, t, n, t, t, t, n, t, n],
        [t, t, t, t, f, n, t, n, n],
        [f; 9],
        left,
    ]) {
        assert_eq!(
            column.values(),
            expected.map(|v| v.map_or(V::Null, V::Bool))
        );
    }
}

#[test]
fn comparison_series_are_lazy_exact_nullable_and_aligned() {
    use yss_data_contract::TabularScalar as V;
    use yss_relational_contract::{ComparisonOperand as O, ComparisonOperation as C};
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
        yss_database_arrow::with_row_columns(
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
    yss_database_io::write_parquet_batches(&path, schema, [Ok(batch)]).unwrap();
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
        yss_database_arrow::column_semantic(combined.schema().field(0))
            .unwrap()
            .kind,
        yss_data_contract::SemanticType::Binary
    );
}

#[test]
fn computed_series_remain_lazy_and_aligned_through_broadcasts_and_chained_arithmetic() {
    use yss_data_contract::TabularScalar;
    use yss_relational_contract::{
        NumericOperation as Op, NumericType as Type, SeriesOperand as Operand,
    };
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(
        yss_database_arrow::with_row_columns(
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
    let scalar = |value| Operand::Scalar(yss_data_contract::TabularScalar::Integer(value));
    let unary = [Op::Ln, Op::Log2, Op::Log10, Op::Square, Op::Sqrt].map(|operation| {
        source
            .numeric_series(operation, &[Operand::Series(x.clone())], Type::Float64)
            .unwrap()
    });
    let powered = source
        .numeric_series(
            Op::Power,
            &[scalar(2), Operand::Series(x.clone())],
            Type::Float64,
        )
        .unwrap();
    let logarithm = source
        .numeric_series(
            Op::Logarithm,
            &[Operand::Series(powered.clone()), scalar(2)],
            Type::Float64,
        )
        .unwrap();
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
    yss_database_io::write_parquet_batches(&path, schema, batches.into_iter().map(Ok)).unwrap();
    let unary_values = source.numeric_columns(&unary, &control()).unwrap();
    for (actual, (first, second)) in unary_values.iter().zip([
        (0., std::f64::consts::LN_2),
        (0., 1.),
        (0., std::f64::consts::LOG10_2),
        (1., 4.),
        (1., std::f64::consts::SQRT_2),
    ]) {
        assert_eq!(actual.len(), 6);
        assert_eq!(actual[0], first);
        assert!((actual[1] - second).abs() < 1e-14);
    }
    let columns = source
        .numeric_columns(
            &[scaled, x, subtracted, divided, added, powered, logarithm],
            &control(),
        )
        .unwrap();
    let expected = [
        vec![123., 246., 369., 492., 615., 738.],
        vec![1., 2., 3., 4., 5., 6.],
        vec![9., 8., 7., 6., 5., 4.],
        vec![10., 5., 10. / 3., 2.5, 2., 10. / 6.],
        vec![125., 249., 373., 497., 621., 745.],
        vec![2., 4., 8., 16., 32., 64.],
        vec![1., 2., 3., 4., 5., 6.],
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
    let scalar = |value| Operand::Scalar(yss_data_contract::TabularScalar::Integer(value));
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
            Op::Power,
            [series("huge"), scalar(2)],
            RelationError::NonFiniteResult,
        ),
        (
            Op::Power,
            [
                scalar(-2),
                Operand::Series(
                    source
                        .numeric_series(Op::Divide, &[series("x.value"), scalar(2)], Type::Float64)
                        .unwrap(),
                ),
            ],
            RelationError::InvalidInput,
        ),
        (
            Op::Logarithm,
            [series("x.value"), scalar(1)],
            RelationError::InvalidInput,
        ),
        (
            Op::Logarithm,
            [series("zero"), scalar(2)],
            RelationError::InvalidInput,
        ),
        (
            Op::Power,
            [series("missing"), scalar(2)],
            RelationError::InvalidInput,
        ),
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
    let negative = source
        .numeric_series(Op::Subtract, &[scalar(0), series("x.value")], Type::Float64)
        .unwrap();
    for (operation, operand, expected) in [
        (Op::Ln, series("zero"), RelationError::InvalidInput),
        (Op::Log2, series("missing"), RelationError::InvalidInput),
        (Op::Log10, series("zero"), RelationError::InvalidInput),
        (Op::Square, series("huge"), RelationError::NonFiniteResult),
        (
            Op::Sqrt,
            Operand::Series(negative),
            RelationError::InvalidInput,
        ),
    ] {
        let result = source
            .numeric_series(operation, &[operand], Type::Float64)
            .unwrap();
        assert_eq!(source.numeric_columns(&[result], &control()), Err(expected));
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
    use yss_data_contract::TabularScalar as V;
    use yss_data_contract::{NumericRepresentation as N, SemanticConversion, SemanticType as S};
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
        yss_database_arrow::with_row_columns(
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
    yss_database_io::write_parquet_batches(&path, schema.clone(), batches.map(Ok)).unwrap();
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
            V::Float64(1.0.try_into().unwrap()),
            V::Null,
            V::Float64(2.0.try_into().unwrap()),
            V::Float64(3.0.try_into().unwrap())
        ]
    );
    assert_eq!(
        yss_database_arrow::column_semantic(combined.schema().field(0))
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
        yss_database_arrow::column_semantic(schema.field(0))
            .unwrap()
            .values[0]
            .value,
        "002"
    );
    assert_eq!(
        yss_database_arrow::column_semantic(schema.field(1))
            .unwrap()
            .values[0]
            .value,
        "001"
    );
    assert_eq!(
        yss_database_arrow::column_semantic(schema.field(2))
            .unwrap()
            .kind,
        S::Categorical
    );
    assert_eq!(
        yss_database_arrow::column_semantic(schema.field(3))
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
        yss_database_arrow::with_row_columns(
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
        "yss-database-engine-{}-{}.parquet",
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
    yss_database_io::write_parquet_batches(&path, schema, batches.into_iter().map(Ok)).unwrap();
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
    assert_eq!(series[0].relation().bindings(), &[binding()]);
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
        yss_database_arrow::with_row_columns(
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
                    yss_database_arrow::with_column_metadata(
                        field,
                        &format!("column-{index}"),
                        None,
                    )
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
    yss_database_io::write_parquet_batches(&path, schema.clone(), [Ok(batch)]).unwrap();
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
    for (integer, exact) in [
        (9_007_199_254_740_992i64, true),
        (9_007_199_254_740_993, false),
    ] {
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new("x", DataType::Int64, false)])),
            vec![Arc::new(Int64Array::from(vec![integer]))],
        )
        .unwrap();
        let source = runtime.batch_relation(binding(), batch).unwrap();
        let converted = runtime.numeric_columns(&[source.select_series("x").unwrap()], &control());
        if exact {
            assert_eq!(converted.unwrap(), vec![vec![integer as f64]]);
        } else {
            assert_eq!(converted, Err(RelationError::InvalidInput));
        }
    }
}

#[test]
fn relation_pages_probe_one_extra_row_and_preserve_wide_integer_display() {
    use arrow::array::UInt64Array;
    use yss_data_contract::TabularScalar;
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

#[test]
fn drop_na_rows_and_columns_preserve_values_and_resolve_before_paging() {
    use yss_relational_contract::DropNaMode::{All, Any};
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 1).unwrap();
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("a.b", DataType::Int64, true),
            Field::new("empty", DataType::Int64, true),
            Field::new("text", DataType::Utf8, true),
            Field::new("float", DataType::Float64, true),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![Some(1), None, Some(3)])),
            Arc::new(Int64Array::from(vec![None, None, None])),
            Arc::new(StringArray::from(vec!["", "ok", "last"])),
            Arc::new(Float64Array::from(vec![f64::NAN, 0.0, 1.0])),
        ],
    )
    .unwrap();
    let source = runtime.batch_relation(binding(), batch.clone()).unwrap();
    let subset = vec!["a.b".into(), "empty".into()];
    assert_eq!(
        source
            .drop_na_rows(&subset, Any)
            .unwrap()
            .page(0, 5, &control())
            .unwrap()
            .row_count,
        0
    );
    assert_eq!(
        source
            .drop_na_rows(&subset, All)
            .unwrap()
            .page(0, 5, &control())
            .unwrap()
            .row_count,
        2
    );
    assert_eq!(
        source
            .drop_na_rows(&["text".into(), "float".into()], Any)
            .unwrap()
            .page(0, 5, &control())
            .unwrap()
            .row_count,
        3
    );
    let any = source.drop_na_columns(&[], Any).unwrap();
    assert!(any.schema_is_deferred());
    // a.b is non-null in the first page but must still be removed for its later Null.
    let page = any.page(0, 1, &control()).unwrap();
    assert_eq!(
        page.columns
            .iter()
            .map(|c| c.name.as_ref())
            .collect::<Vec<_>>(),
        ["text", "float"]
    );
    assert_eq!(page.row_count, 1);
    assert!(page.has_more);
    let all = source.drop_na_columns(&[], All).unwrap();
    assert_eq!(all.page(0, 5, &control()).unwrap().columns.len(), 3);
    assert_eq!(
        all.drop_na_rows(&["a.b".into()], Any)
            .unwrap()
            .page(0, 5, &control())
            .unwrap()
            .row_count,
        2
    );
    let one = source
        .project(&["empty".into()])
        .unwrap()
        .drop_na_columns(&[], All)
        .unwrap()
        .page(0, 5, &control())
        .unwrap();
    assert!(one.columns.is_empty());
    assert_eq!(one.row_count, 3);
    let empty = source
        .limit(0, 0)
        .unwrap()
        .drop_na_columns(&[], All)
        .unwrap()
        .page(0, 5, &control())
        .unwrap();
    assert_eq!(empty.columns.len(), 4);
    assert_eq!(empty.row_count, 0);
    assert!(source.drop_na_rows(&["missing".into()], Any).is_err());
    assert!(
        source
            .drop_na_columns(&["a.b".into(), "a.b".into()], All)
            .is_err()
    );
    assert_eq!(source.schema(), batch.schema());
    let cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(any.page(0, 1, &cancelled), Err(RelationError::Cancelled));
}

#[test]
fn drop_na_planning_never_scans_and_streaming_retains_the_source_lease() {
    use yss_relational_contract::DropNaMode::{All, Any};
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 1).unwrap();
    let schema = Arc::new(
        yss_database_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("x", DataType::Int64, true),
                Field::new("row_id", DataType::Int64, false),
                Field::new("order", DataType::Utf8, false),
            ]),
            "row_id",
            "order",
        )
        .unwrap(),
    );
    let path = std::env::temp_dir().join(format!(
        "yss-dropna-{}-{}.parquet",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let lease = Arc::new(RemoveFile(path.clone()));
    let weak = Arc::downgrade(&lease);
    let source = runtime
        .parquet_relation(
            binding(),
            schema.clone(),
            std::slice::from_ref(&path),
            lease.clone(),
        )
        .unwrap();
    let rows = source.drop_na_rows(&[], Any).unwrap();
    let columns = rows.drop_na_columns(&[], All).unwrap();
    assert!(!path.exists());
    assert_eq!(columns.bindings(), source.bindings());
    let cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        columns.page(0, 1, &cancelled),
        Err(RelationError::Cancelled)
    );
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![None, Some(2)])),
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(StringArray::from(vec!["0", "1"])),
        ],
    )
    .unwrap();
    yss_database_io::write_parquet_batches(&path, schema, [Ok(batch)]).unwrap();
    drop(source);
    drop(rows);
    drop(lease);
    let mut stream = runtime
        .runtime
        .as_ref()
        .unwrap()
        .block_on(columns.stream(control()))
        .unwrap();
    drop(columns);
    assert!(weak.upgrade().is_some());
    let first = runtime
        .runtime
        .as_ref()
        .unwrap()
        .block_on(stream.next())
        .unwrap()
        .unwrap();
    assert_eq!(first.num_rows(), 1);
    drop(stream);
    assert!(weak.upgrade().is_none());
    assert!(!path.exists());
}
