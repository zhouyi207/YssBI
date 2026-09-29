use super::*;
use yss_data_contract::{
    ColumnSemantic, SemanticType as S, SemanticValue, TabularScalar as V,
    aggregation::{AggregateOperation as A, ColumnAggregate},
};

fn meaning(name: &str, kind: S, codes: &[&str]) -> Field {
    let mut semantic = ColumnSemantic::new(kind);
    semantic.values = codes
        .iter()
        .map(|code| SemanticValue {
            value: (*code).into(),
            label: (*code).into(),
        })
        .collect();
    yss_database_arrow::with_column_semantic(Field::new(name, DataType::Int64, true), &semantic)
        .unwrap()
}
fn number(value: &V) -> f64 {
    match value {
        V::Float64(v) => v.as_f64(),
        V::Integer(v) => *v as f64,
        V::Unsigned(v) => *v as f64,
        _ => panic!("expected number: {value:?}"),
    }
}
fn column_values<'a>(page: &'a yss_relational_contract::RelationPage, name: &str) -> &'a [V] {
    page.data
        .columns()
        .iter()
        .find(|c| c.name().as_str() == name)
        .unwrap()
        .values()
}

#[test]
fn descriptive_tables_dispatch_by_semantics_and_keep_ordinal_frequency_order() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![
                    Field::new("amount", DataType::Int64, true),
                    meaning("category", S::Categorical, &["1", "2"]),
                    meaning("level", S::Ordinal, &["20", "10"]),
                    meaning("flag", S::Binary, &["0", "1"]),
                    Field::new("text", DataType::Utf8, false),
                ])),
                vec![
                    Arc::new(Int64Array::from(vec![Some(1), Some(2), None, Some(4)])),
                    Arc::new(Int64Array::from(vec![Some(2), Some(1), Some(2), None])),
                    Arc::new(Int64Array::from(vec![20, 10, 10, 20])),
                    Arc::new(Int64Array::from(vec![Some(0), Some(1), None, Some(1)])),
                    Arc::new(StringArray::from(vec!["a", "b", "a", "b"])),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let description = source.describe(&[]).unwrap();
    let page = description.page(0, 10, &control()).unwrap();
    assert_eq!(page.row_count, 4);
    assert_eq!(
        column_values(&page, "semantic"),
        ["Numeric", "Categorical", "Ordinal", "Binary"].map(|s| V::String(s.into()))
    );
    assert!((number(&column_values(&page, "mean")[0]) - 7.0 / 3.0).abs() < 1e-12);
    assert!((number(&column_values(&page, "std")[0]) - (7.0_f64 / 3.0).sqrt()).abs() < 1e-12);
    for (key, expected) in [("q25", 1.5), ("median", 2.0), ("q75", 3.0)] {
        assert_eq!(number(&column_values(&page, key)[0]), expected);
    }
    assert_eq!(
        column_values(&page, "mean")[1..],
        [V::Null, V::Null, V::Null]
    );
    assert_eq!(
        column_values(&page, "mode")[1..],
        [
            V::String("2".into()),
            V::String("20".into()),
            V::String("1".into())
        ]
    );
    assert_eq!(
        number(&column_values(&page, "mode_proportion")[1]),
        2.0 / 3.0
    );
    assert!(source.describe(&["text".into()]).is_err());
    let frequency = source.frequency("level", true).unwrap();
    let page = frequency.page(0, 10, &control()).unwrap();
    assert_eq!(
        column_values(&page, "value")
            .iter()
            .map(number)
            .collect::<Vec<_>>(),
        [20.0, 10.0]
    );
    assert_eq!(
        yss_database_arrow::column_semantic(frequency.schema().field(0))
            .unwrap()
            .kind,
        S::Ordinal
    );
    assert_eq!(number(&column_values(&page, "proportion")[0]), 0.5);
    let page = source
        .frequency("category", true)
        .unwrap()
        .page(0, 10, &control())
        .unwrap();
    assert_eq!(column_values(&page, "value").last(), Some(&V::Null));
    assert_eq!(number(&column_values(&page, "proportion")[1]), 0.5);
    let page = source
        .frequency("category", false)
        .unwrap()
        .page(0, 10, &control())
        .unwrap();
    assert_eq!(page.row_count, 2);
    assert_eq!(number(&column_values(&page, "proportion")[1]), 2.0 / 3.0);
}

#[test]
fn aggregate_groups_keep_null_keys_and_frequency_pages_cover_every_value() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![
                    Field::new("key", DataType::Utf8, true),
                    Field::new("amount", DataType::Int64, true),
                ])),
                vec![
                    Arc::new(StringArray::from(vec![Some("a"), Some("a"), None, None])),
                    Arc::new(Int64Array::from(vec![Some(1), None, Some(3), Some(5)])),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let aggregates = A::ALL.map(|operation| ColumnAggregate {
        column: "amount".into(),
        operation,
    });
    let grouped = source.aggregate(&["key".into()], &aggregates).unwrap();
    let page = grouped.page(0, 10, &control()).unwrap();
    assert_eq!(
        column_values(&page, "key"),
        &[V::String("a".into()), V::Null]
    );
    assert_eq!(
        column_values(&page, "row_count")
            .iter()
            .map(number)
            .collect::<Vec<_>>(),
        [2.0, 2.0]
    );
    for (key, expected) in [
        ("amount_count", 2.0),
        ("amount_sum", 8.0),
        ("amount_mean", 4.0),
        ("amount_min", 3.0),
        ("amount_max", 5.0),
        ("amount_median", 4.0),
    ] {
        assert_eq!(number(&column_values(&page, key)[1]), expected);
    }
    assert_eq!(column_values(&page, "amount_std")[0], V::Null);
    assert!((number(&column_values(&page, "amount_std")[1]) - 2.0_f64.sqrt()).abs() < 1e-12);
    assert!(
        source
            .aggregate(
                &["key".into()],
                &[ColumnAggregate {
                    column: "key".into(),
                    operation: A::Mean
                }]
            )
            .is_err()
    );
    assert!(
        source
            .aggregate(
                &["key".into()],
                &[aggregates[0].clone(), aggregates[0].clone()]
            )
            .is_err()
    );
    let many = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![Field::new("x", DataType::Int64, false)])),
                vec![Arc::new(Int64Array::from_iter_values(0..25))],
            )
            .unwrap(),
        )
        .unwrap()
        .frequency("x", true)
        .unwrap();
    let mut values = Vec::new();
    for offset in [0, 10, 20] {
        let page = many.page(offset, 10, &control()).unwrap();
        values.extend(column_values(&page, "value").iter().map(number));
    }
    assert_eq!(values, (0..25).map(f64::from).collect::<Vec<_>>());
}

#[test]
fn descriptive_numeric_errors_fail_instead_of_becoming_null_and_empty_is_defined() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let relation = |values: Vec<Option<f64>>| {
        runtime
            .batch_relation(
                binding(),
                RecordBatch::try_new(
                    Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, true)])),
                    vec![Arc::new(Float64Array::from(values))],
                )
                .unwrap(),
            )
            .unwrap()
    };
    for values in [
        vec![Some(f64::NAN)],
        vec![Some(f64::INFINITY)],
        vec![Some(1e308), Some(-1e308), Some(1e308), Some(-1e308)],
    ] {
        let result = relation(values)
            .describe(&[])
            .unwrap()
            .page(0, 10, &control());
        assert!(
            result.is_err(),
            "invalid arithmetic must not become a null report"
        );
    }
    for values in [vec![], vec![None, None]] {
        let source = relation(values);
        let page = source
            .describe(&[])
            .unwrap()
            .page(0, 10, &control())
            .unwrap();
        assert_eq!(number(&column_values(&page, "count")[0]), 0.0);
        assert_eq!(column_values(&page, "mean"), &[V::Null]);
        assert_eq!(column_values(&page, "std"), &[V::Null]);
        assert_eq!(
            source
                .frequency("x", false)
                .unwrap()
                .page(0, 10, &control())
                .unwrap()
                .row_count,
            0
        );
    }
    for kind in [S::Categorical, S::Ordinal, S::Binary] {
        for values in [vec![], vec![None, None]] {
            let source = runtime
                .batch_relation(
                    binding(),
                    RecordBatch::try_new(
                        Arc::new(Schema::new(vec![meaning("x", kind, &["0", "1"])])),
                        vec![Arc::new(Int64Array::from(values))],
                    )
                    .unwrap(),
                )
                .unwrap();
            let page = source
                .describe(&[])
                .unwrap()
                .page(0, 10, &control())
                .unwrap();
            assert_eq!(number(&column_values(&page, "count")[0]), 0.0);
            assert_eq!(number(&column_values(&page, "unique")[0]), 0.0);
            assert_eq!(column_values(&page, "mode"), &[V::Null]);
        }
    }
}
