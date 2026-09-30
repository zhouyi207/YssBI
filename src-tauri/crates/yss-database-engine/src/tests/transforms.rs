use super::*;
use arrow::array::{ArrayRef, Date32Array};
use yss_data_contract::{SemanticType, TabularScalar as V};
use yss_relational_contract::*;

fn table(
    runtime: &Arc<DataFusionRuntime>,
    fields: Vec<Field>,
    arrays: Vec<ArrayRef>,
) -> RelationHandle {
    runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays).unwrap(),
        )
        .unwrap()
}
#[track_caller]
fn numbers(
    relation: &RelationHandle,
    column: usize,
    offset: usize,
    size: usize,
) -> Vec<Option<f64>> {
    relation
        .page(offset, size, &control())
        .unwrap()
        .data
        .columns()[column]
        .values()
        .iter()
        .map(|v| match v {
            V::Null => None,
            V::Integer(v) => Some(*v as f64),
            V::Unsigned(v) => Some(*v as f64),
            V::Float64(v) => Some(v.as_f64()),
            _ => panic!("numeric result expected"),
        })
        .collect()
}
#[track_caller]
fn series_numbers(series: &SeriesHandle) -> Vec<Option<f64>> {
    numbers(&series.as_relation().unwrap(), 0, 0, 100)
}
fn text_values(relation: &RelationHandle, column: usize) -> Vec<Option<String>> {
    relation.page(0, 100, &control()).unwrap().data.columns()[column]
        .values()
        .iter()
        .map(|v| match v {
            V::Null => None,
            V::String(v) => Some(v.to_string()),
            _ => panic!("text result expected"),
        })
        .collect()
}

#[test]
fn native_series_windows_remain_lazy_aligned_and_page_consistently() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = table(
        &runtime,
        vec![Field::new("x", DataType::Int64, true)],
        vec![Arc::new(Int64Array::from(vec![
            Some(10),
            Some(16),
            Some(7),
            None,
            Some(13),
        ]))],
    );
    let x = source.select_series("x").unwrap();
    let lag = source
        .transform_series(
            &x,
            &SeriesTransform::Shift {
                periods: 1,
                lead: false,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    let diff = source
        .transform_series(
            &x,
            &SeriesTransform::Difference {
                order: 2,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    let rolled = source
        .transform_series(
            &x,
            &SeriesTransform::Rolling {
                operation: WindowOperation::Mean,
                size: 2,
                min_periods: 2,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    assert_eq!(
        series_numbers(&lag),
        vec![None, Some(10.0), Some(16.0), Some(7.0), None]
    );
    assert_eq!(
        series_numbers(&diff),
        vec![None, None, Some(-15.0), None, None]
    );
    let high_order = source
        .transform_series(
            &x,
            &SeriesTransform::Difference {
                order: 129,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    assert_eq!(series_numbers(&high_order), vec![None; 5]);
    let large = table(
        &runtime,
        vec![Field::new("x", DataType::Int64, false)],
        vec![Arc::new(Int64Array::from(
            (0_i64..10_000).map(|x| x * x).collect::<Vec<_>>(),
        ))],
    );
    let large_diff = large
        .transform_series(
            &large.select_series("x").unwrap(),
            &SeriesTransform::Difference {
                order: 2,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    assert_eq!(
        numbers(&large_diff.as_relation().unwrap(), 0, 8190, 5),
        vec![Some(2.0); 5]
    );
    assert_eq!(
        series_numbers(&rolled),
        vec![None, Some(13.0), Some(11.5), None, None]
    );
    let projected = source
        .project_series(&[x.clone(), lag.clone(), diff, rolled])
        .unwrap();
    assert_eq!(numbers(&projected, 1, 2, 2), vec![Some(16.0), Some(7.0)]);
    let limited = lag.as_relation().unwrap().limit(2, 2).unwrap();
    assert_eq!(numbers(&limited, 0, 0, 10), vec![Some(16.0), Some(7.0)]);
    let replaced = source.set_column("x", &lag).unwrap();
    let lag_twice = replaced
        .transform_series(
            &replaced.select_series("x").unwrap(),
            &SeriesTransform::Shift {
                periods: 1,
                lead: false,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    assert_eq!(
        series_numbers(&lag_twice),
        vec![None, None, Some(10.0), Some(16.0), Some(7.0)]
    );
    let standardized = source
        .transform_series(&x, &SeriesTransform::Standardize)
        .unwrap();
    let restored = source
        .transform_series(
            &standardized,
            &SeriesTransform::InverseStandardize {
                mean: 11.5,
                standard_deviation: 15.0_f64.sqrt(),
            },
        )
        .unwrap();
    for (actual, expected) in
        series_numbers(&restored)
            .iter()
            .zip([Some(10.0), Some(16.0), Some(7.0), None, Some(13.0)])
    {
        match (actual, expected) {
            (Some(a), Some(e)) => assert!((a - e).abs() < 1e-10),
            (None, None) => {}
            _ => panic!("null position changed"),
        }
    }
    let plan = projected
        .plan()
        .as_any()
        .downcast_ref::<crate::relation::DataFusionRelation>()
        .unwrap()
        .frame
        .logical_plan()
        .display_indent()
        .to_string();
    assert!(plan.contains("Window"));
    let bad = table(
        &runtime,
        vec![Field::new("x", DataType::Int64, false)],
        vec![Arc::new(Int64Array::from(vec![
            9_007_199_254_740_993_i64,
            1,
        ]))],
    );
    let deferred = bad
        .transform_series(
            &bad.select_series("x").unwrap(),
            &SeriesTransform::Difference {
                order: 1,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    assert_eq!(
        deferred.as_relation().unwrap().page(0, 10, &control()),
        Err(RelationError::InvalidInput)
    );
    let clean = table(
        &runtime,
        vec![Field::new("x", DataType::Int64, false)],
        vec![Arc::new(Int64Array::from(vec![1, 2]))],
    );
    let alias = clean
        .rename("x", "renamed")
        .unwrap()
        .select_series("renamed")
        .unwrap();
    let sum = clean
        .numeric_series(
            NumericOperation::Add,
            &[
                SeriesOperand::Series(clean.select_series("x").unwrap()),
                SeriesOperand::Series(alias),
            ],
            NumericType::Int64,
        )
        .unwrap();
    assert_eq!(series_numbers(&sum), vec![Some(2.0), Some(4.0)]);
    let increment = clean
        .numeric_series(
            NumericOperation::Add,
            &[
                SeriesOperand::Series(clean.select_series("x").unwrap()),
                SeriesOperand::Scalar(V::Integer(1)),
            ],
            NumericType::Int64,
        )
        .unwrap();
    let changed = clean.set_column("x", &increment).unwrap();
    let increment_again = changed
        .numeric_series(
            NumericOperation::Add,
            &[
                SeriesOperand::Series(changed.select_series("x").unwrap()),
                SeriesOperand::Scalar(V::Integer(1)),
            ],
            NumericType::Int64,
        )
        .unwrap();
    assert_eq!(series_numbers(&increment_again), vec![Some(3.0), Some(4.0)]);
}

#[test]
fn native_table_transformations_have_fixed_schemas_and_stable_rows() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = table(
        &runtime,
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("a", DataType::Int64, true),
            Field::new("b", DataType::Int64, true),
        ],
        vec![
            Arc::new(Int64Array::from(vec![1, 2])),
            Arc::new(Int64Array::from(vec![Some(2), None])),
            Arc::new(Int64Array::from(vec![Some(5), Some(7)])),
        ],
    );
    let long = source
        .unpivot(&UnpivotSpec {
            keys: vec!["id".into()],
            columns: vec!["a".into(), "b".into()],
            variable_name: "variable".into(),
            value_name: "value".into(),
            include_null: true,
        })
        .unwrap();
    assert_eq!(
        numbers(&long, 0, 0, 10),
        vec![Some(1.0), Some(1.0), Some(2.0), Some(2.0)]
    );
    assert_eq!(
        text_values(&long, 1),
        ["a", "b", "a", "b"]
            .into_iter()
            .map(|v| Some(v.to_owned()))
            .collect::<Vec<_>>()
    );
    let wide = long
        .pivot(&PivotSpec {
            keys: vec!["id".into()],
            category: "variable".into(),
            value: "value".into(),
            levels: vec![
                PivotLevel {
                    value: V::String("a".into()),
                    name: "a".into(),
                },
                PivotLevel {
                    value: V::String("b".into()),
                    name: "b".into(),
                },
            ],
            aggregate: PivotAggregate::Sum,
        })
        .unwrap();
    assert_eq!(
        wide.schema()
            .fields()
            .iter()
            .map(|f| f.name().as_str())
            .collect::<Vec<_>>(),
        vec!["id", "a", "b"]
    );
    assert_eq!(numbers(&wide, 1, 0, 10), vec![Some(2.0), None]);
    assert_eq!(numbers(&wide, 2, 0, 10), vec![Some(5.0), Some(7.0)]);
    let sorted = source
        .sort_rows(&[SortColumn {
            column: "b".into(),
            ascending: false,
            nulls_first: false,
        }])
        .unwrap();
    assert_eq!(numbers(&sorted, 0, 0, 10), vec![Some(2.0), Some(1.0)]);
    let mask = source
        .compare_series(
            ComparisonOperation::Greater,
            &[
                ComparisonOperand::Series(source.select_series("b").unwrap()),
                ComparisonOperand::Scalar(V::Integer(5)),
            ],
        )
        .unwrap();
    assert_eq!(
        numbers(&source.filter_mask(&mask, false).unwrap(), 0, 0, 10),
        vec![Some(2.0)]
    );
    assert_eq!(
        numbers(&source.filter_mask(&mask, true).unwrap(), 0, 0, 10),
        vec![Some(1.0)]
    );
    let duplicates = table(
        &runtime,
        vec![
            Field::new("key", DataType::Int64, true),
            Field::new("text", DataType::Utf8, false),
        ],
        vec![
            Arc::new(Int64Array::from(vec![
                Some(1),
                Some(1),
                Some(2),
                None,
                None,
            ])),
            Arc::new(StringArray::from(vec!["a", "b", "c", "d", "e"])),
        ],
    );
    let right = duplicates.limit(0, 2).unwrap();
    for (kind, expected) in [
        (
            yss_data_contract::table::TableJoinKind::Semi,
            vec!["a", "b"],
        ),
        (
            yss_data_contract::table::TableJoinKind::Anti,
            vec!["c", "d", "e"],
        ),
    ] {
        let joined = duplicates
            .join(
                &right,
                &yss_data_contract::table::TableJoin {
                    kind,
                    left_keys: vec!["key".into()],
                    right_keys: vec!["key".into()],
                    right_suffix: "_right".into(),
                },
            )
            .unwrap();
        assert_eq!(joined.schema().fields().len(), 2);
        assert_eq!(
            text_values(&joined, 1),
            expected
                .into_iter()
                .map(|value| Some(value.to_owned()))
                .collect::<Vec<_>>()
        );
    }
    for (keep, expected) in [
        (DuplicateKeep::First, vec!["a", "c", "d"]),
        (DuplicateKeep::Last, vec!["b", "c", "e"]),
        (DuplicateKeep::None, vec!["c"]),
    ] {
        assert_eq!(
            text_values(&duplicates.deduplicate(&["key".into()], keep).unwrap(), 1),
            expected
                .into_iter()
                .map(|v| Some(v.to_owned()))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn native_grids_align_without_collecting_and_validate_keys_on_consumption() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = table(
        &runtime,
        vec![
            Field::new("time", DataType::Int64, false),
            Field::new("x", DataType::Int64, true),
        ],
        vec![
            Arc::new(Int64Array::from(vec![1, 3])),
            Arc::new(Int64Array::from(vec![10, 30])),
        ],
    );
    let aligned = source.align_grid("time", None, 1, 1024 * 1024).unwrap();
    assert_eq!(
        numbers(&aligned, 0, 0, 10),
        vec![Some(1.0), Some(2.0), Some(3.0)]
    );
    assert_eq!(
        numbers(&aligned, 1, 0, 10),
        vec![Some(10.0), None, Some(30.0)]
    );
    assert!(
        aligned
            .plan()
            .as_any()
            .downcast_ref::<crate::relation::DataFusionRelation>()
            .unwrap()
            .frame
            .logical_plan()
            .display_indent()
            .to_string()
            .contains("Unnest")
    );
    let panel = table(
        &runtime,
        vec![
            Field::new("entity", DataType::Utf8, false),
            Field::new("time", DataType::Int64, false),
            Field::new("x", DataType::Int64, true),
        ],
        vec![
            Arc::new(StringArray::from(vec!["b", "a", "b", "a"])),
            Arc::new(Int64Array::from(vec![3, 1, 1, 2])),
            Arc::new(Int64Array::from(vec![30, 11, 10, 12])),
        ],
    );
    let aligned = panel
        .align_grid("time", Some("entity"), 1, 1024 * 1024)
        .unwrap();
    assert_eq!(
        text_values(&aligned, 0),
        ["b", "b", "b", "a", "a"]
            .into_iter()
            .map(|v| Some(v.to_owned()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        numbers(&aligned, 1, 0, 10),
        vec![Some(1.0), Some(2.0), Some(3.0), Some(1.0), Some(2.0)]
    );
    assert_eq!(
        numbers(&aligned, 2, 0, 10),
        vec![Some(10.0), None, Some(30.0), Some(11.0), Some(12.0)]
    );
    let duplicate = table(
        &runtime,
        vec![Field::new("time", DataType::Int64, false)],
        vec![Arc::new(Int64Array::from(vec![1, 1]))],
    );
    assert_eq!(
        duplicate
            .align_grid("time", None, 1, 1024)
            .unwrap()
            .page(0, 10, &control()),
        Err(RelationError::InvalidInput)
    );
    let range = Arc::clone(&runtime)
        .integer_range(5, -2, -2, &control())
        .unwrap();
    assert_eq!(
        numbers(&range, 0, 0, 10),
        vec![Some(5.0), Some(3.0), Some(1.0), Some(-1.0)]
    );
    let literal = Arc::clone(&runtime)
        .literal_series(
            Field::new("value", DataType::Int64, false),
            Arc::new(Int64Array::from(vec![1, 2, 3, 4])),
            &control(),
        )
        .unwrap();
    let sum = range
        .numeric_series(
            NumericOperation::Add,
            &[
                SeriesOperand::Series(range.select_series("value").unwrap()),
                SeriesOperand::Series(literal),
            ],
            NumericType::Int64,
        )
        .unwrap();
    assert_eq!(
        series_numbers(&sum),
        vec![Some(6.0), Some(5.0), Some(4.0), Some(3.0)]
    );
}

#[test]
fn native_text_dates_and_grouped_windows_preserve_metadata_nulls_and_order() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = table(
        &runtime,
        vec![
            Field::new("group", DataType::Utf8, false),
            Field::new("time", DataType::Int64, false),
            Field::new("x", DataType::Int64, true),
            Field::new("text", DataType::Utf8, true),
            Field::new("date", DataType::Date32, false),
        ],
        vec![
            Arc::new(StringArray::from(vec!["a", "b", "a", "b"])),
            Arc::new(Int64Array::from(vec![2, 1, 1, 2])),
            Arc::new(Int64Array::from(vec![None, Some(10), Some(5), Some(20)])),
            Arc::new(StringArray::from(vec![
                Some("  Ä One "),
                Some("B TWO "),
                None,
                Some(""),
            ])),
            Arc::new(Date32Array::from(vec![19358, 19359, 19360, 19361])),
        ],
    );
    let x = source.select_series("x").unwrap();
    let spec = SeriesWindow {
        context: Some(source.clone()),
        partition_by: vec!["group".into()],
        order_by: vec![SortColumn {
            column: "time".into(),
            ascending: true,
            nulls_first: false,
        }],
        require_unique_keys: false,
    };
    assert_eq!(
        series_numbers(
            &source
                .transform_series(
                    &x,
                    &SeriesTransform::Cumulative {
                        operation: WindowOperation::Sum,
                        window: spec.clone()
                    }
                )
                .unwrap()
        ),
        vec![Some(5.0), Some(10.0), Some(5.0), Some(30.0)]
    );
    assert_eq!(
        series_numbers(
            &source
                .transform_series(
                    &x,
                    &SeriesTransform::FillDirection {
                        forward: true,
                        window: spec.clone()
                    }
                )
                .unwrap()
        ),
        vec![Some(5.0), Some(10.0), Some(5.0), Some(20.0)]
    );
    assert_eq!(
        series_numbers(
            &source
                .transform_series(
                    &x,
                    &SeriesTransform::FillDirection {
                        forward: false,
                        window: spec
                    }
                )
                .unwrap()
        ),
        vec![None, Some(10.0), Some(5.0), Some(20.0)]
    );
    let bin = source
        .transform_series(
            &x,
            &SeriesTransform::Bin {
                edges: vec![10.0, 20.0],
                labels: vec!["low".into(), "middle".into(), "high".into()],
            },
        )
        .unwrap();
    assert_eq!(
        yss_database_arrow::column_semantic(bin.plan().field())
            .unwrap()
            .kind,
        SemanticType::Ordinal
    );
    assert_eq!(
        text_values(&bin.as_relation().unwrap(), 0),
        vec![
            None,
            Some("middle".into()),
            Some("low".into()),
            Some("high".into())
        ]
    );
    let ranked = source
        .transform_series(
            &bin,
            &SeriesTransform::Rank {
                dense: false,
                descending: true,
                nulls_first: false,
                window: SeriesWindow::default(),
            },
        )
        .unwrap();
    assert_eq!(
        series_numbers(&ranked),
        vec![Some(4.0), Some(2.0), Some(3.0), Some(1.0)]
    );
    let mapped = source
        .transform_series(
            &bin,
            &SeriesTransform::Map {
                from: vec![V::String("middle".into())],
                to: vec![V::String("medium".into())],
                keep_unmatched: true,
            },
        )
        .unwrap();
    assert_eq!(
        yss_database_arrow::column_semantic(mapped.plan().field())
            .unwrap()
            .values
            .iter()
            .map(|v| v.value.as_str())
            .collect::<Vec<_>>(),
        vec!["low", "medium", "high"]
    );
    let encoded = source
        .encode_series(
            &bin,
            &[
                PivotLevel {
                    value: V::String("low".into()),
                    name: "low".into(),
                },
                PivotLevel {
                    value: V::String("middle".into()),
                    name: "middle".into(),
                },
                PivotLevel {
                    value: V::String("high".into()),
                    name: "high".into(),
                },
            ],
            Some("low"),
        )
        .unwrap();
    assert_eq!(encoded.schema().fields().len(), 2);
    assert_eq!(
        encoded.page(0, 10, &control()).unwrap().data.columns()[0].values(),
        &[V::Null, V::Bool(true), V::Bool(false), V::Bool(false)]
    );
    let trimmed = source
        .transform_series(
            &source.select_series("text").unwrap(),
            &SeriesTransform::Trim,
        )
        .unwrap();
    let lowered = source
        .transform_series(&trimmed, &SeriesTransform::Lower)
        .unwrap();
    assert_eq!(
        text_values(&lowered.as_relation().unwrap(), 0),
        vec![
            Some("ä one".into()),
            Some("b two".into()),
            None,
            Some("".into())
        ]
    );
    let date = source.select_series("date").unwrap();
    let resampled = source
        .resample(&ResampleSpec {
            time: "date".into(),
            unit: "month".into(),
            keys: vec!["group".into()],
            columns: vec!["x".into()],
            aggregate: PivotAggregate::Sum,
        })
        .unwrap();
    assert_eq!(
        text_values(&resampled, 0),
        vec![Some("2023-01-01".into()); 2]
    );
    assert_eq!(numbers(&resampled, 2, 0, 10), vec![Some(5.0), Some(30.0)]);
    assert_eq!(
        series_numbers(
            &source
                .transform_series(
                    &date,
                    &SeriesTransform::DatePart {
                        part: "year".into()
                    }
                )
                .unwrap()
        ),
        vec![Some(2023.0); 4]
    );
    assert_eq!(
        series_numbers(
            &source
                .transform_series(
                    &date,
                    &SeriesTransform::DateDifference {
                        unit: "day".into(),
                        other: SeriesOperand::Scalar(V::String("2023-01-01".into()))
                    }
                )
                .unwrap()
        ),
        vec![Some(0.0), Some(1.0), Some(2.0), Some(3.0)]
    );
    let added = source
        .transform_series(
            &date,
            &SeriesTransform::DateAdd {
                unit: "day".into(),
                amount: 1,
            },
        )
        .unwrap();
    assert_eq!(
        text_values(&added.as_relation().unwrap(), 0)[0],
        Some("2023-01-02".into())
    );
    assert_eq!(
        source
            .transform_series(
                &date,
                &SeriesTransform::DateAdd {
                    unit: "hour".into(),
                    amount: 1
                }
            )
            .err(),
        Some(RelationError::InvalidInput)
    );
}
