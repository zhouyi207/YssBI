use std::sync::Arc;

use arrow::array::{Array, DictionaryArray, Int8Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Int8Type, Schema, TimeUnit};
use serde_json::json;
use yss_database_contract::DatabaseId;
use yss_tabular_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

use super::*;

#[test]
fn column_semantics_enforce_explicit_domains_and_numeric_constraints() {
    let field = Field::new("code", DataType::Int64, true);
    let values = vec![
        SemanticValue {
            value: "2".into(),
            label: "Low".into(),
        },
        SemanticValue {
            value: "1".into(),
            label: "High".into(),
        },
    ];
    let mut semantic = ColumnSemantic::new(SemanticType::Ordinal);
    semantic.values = values.clone();
    let ordered = with_column_semantic(field.clone(), &semantic).unwrap();
    assert_eq!(ordered.data_type(), &DataType::Int64);
    assert_eq!(column_semantic(&ordered).unwrap().values, values);
    assert!(json_to_array(&ordered, &[json!(1), json!(2), json!(null)]).is_ok());
    assert!(json_to_array(&ordered, &[json!(3)]).is_err());
    semantic.kind = SemanticType::Binary;
    semantic.positive_value = Some("1".into());
    let binary = with_column_semantic(field.clone(), &semantic).unwrap();
    assert!(json_to_array(&binary, &[json!(null), json!(2), json!(1)]).is_ok());
    semantic.values[1].value = "2".into();
    assert!(with_column_semantic(field, &semantic).is_err());

    let mut numeric = ColumnSemantic::new(SemanticType::Numeric);
    numeric.numeric = Some(NumericConstraints {
        integer: true,
        minimum: Some("1".into()),
        maximum: Some("9007199254740993".into()),
    });
    let integer =
        with_column_semantic(Field::new("wide", DataType::Int64, true), &numeric).unwrap();
    assert!(json_to_array(&integer, &[json!("9007199254740993"), json!(null)]).is_ok());
    assert!(json_to_array(&integer, &[json!("9007199254740994")]).is_err());
    assert!(json_to_array(&integer, &[json!(0)]).is_err());
    numeric.numeric = Some(NumericConstraints {
        integer: true,
        ..Default::default()
    });
    let float =
        with_column_semantic(Field::new("number", DataType::Float64, true), &numeric).unwrap();
    assert!(json_to_array(&float, &[json!(1.5)]).is_err());
    assert!(json_to_array(&float, &[json!(1e20), json!(null)]).is_ok());
    let text = Field::new("text", DataType::Utf8, true);
    assert_eq!(column_semantic(&text).unwrap().kind, SemanticType::Text);
    assert_eq!(
        array_to_json(
            json_to_array(&text, &[json!(""), json!("001"), json!(null)])
                .unwrap()
                .as_ref()
        )
        .unwrap(),
        vec![json!(""), json!("001"), json!(null)]
    );
}

#[test]
fn physical_casts_reject_precision_loss_and_preserve_semantics() {
    use arrow::array::{Float64Array, Int64Array, TimestampNanosecondArray};
    assert!(
        lossless_cast(
            &StringArray::from(vec!["2026-09-16T12:00:00.000000001+08:00"]),
            &DataType::Timestamp(TimeUnit::Microsecond, None),
            false
        )
        .is_err()
    );
    assert!(
        lossless_cast(
            &StringArray::from(vec!["2026-09-16T12:00:00+08:00"]),
            &DataType::Date32,
            false
        )
        .is_err()
    );
    assert!(
        lossless_cast(
            &StringArray::from(vec!["2026-09-16T12:00:00+08:00"]),
            &DataType::Timestamp(TimeUnit::Second, None),
            false
        )
        .is_ok()
    );
    assert!(lossless_cast(&Float64Array::from(vec![9.25]), &DataType::Int64, false).is_err());
    assert!(
        lossless_cast(
            &Int64Array::from(vec![9_007_199_254_740_993]),
            &DataType::Float64,
            false
        )
        .is_err()
    );
    assert!(lossless_cast(&StringArray::from(vec!["001"]), &DataType::Int64, false).is_err());
    assert!(
        lossless_cast(
            &TimestampNanosecondArray::from(vec![1]),
            &DataType::Timestamp(TimeUnit::Microsecond, None),
            false
        )
        .is_err()
    );
    assert_eq!(
        array_to_json(
            lossless_cast(&Int64Array::from(vec![1, 2]), &DataType::Float64, false)
                .unwrap()
                .as_ref()
        )
        .unwrap(),
        vec![json!(1.0), json!(2.0)]
    );
    let source = with_column_semantic(
        Field::new("id", DataType::Int64, true),
        &ColumnSemantic::new(SemanticType::Identifier),
    )
    .unwrap();
    let target = source.clone().with_data_type(DataType::Utf8);
    assert_eq!(
        cast_column_semantic(&source, &target).unwrap().kind,
        SemanticType::Identifier
    );
    assert!(!is_numeric_field(&source));
    assert_eq!(
        physical_type_name(&DataType::Decimal128(30, 4)),
        "Decimal128(30, 4)"
    );
}

#[test]
fn timezone_removal_retains_clock_precision_nulls_nested_fields_and_dst() {
    use arrow::array::{StructArray, TimestampNanosecondArray, TimestampSecondArray};
    let timestamps =
        Arc::new(TimestampNanosecondArray::from(vec![Some(-1), None]).with_timezone("+08:00"));
    let field = with_column_metadata(
        Field::new("at", timestamps.data_type().clone(), true),
        "clock-column",
        None,
    )
    .unwrap();
    let nested = StructArray::from(vec![(
        Arc::new(field),
        timestamps as arrow::array::ArrayRef,
    )]);
    let normalized = timezone_free_array(&nested).unwrap();
    let normalized = normalized.as_any().downcast_ref::<StructArray>().unwrap();
    assert_eq!(
        column_identity(normalized.fields()[0].as_ref()).unwrap(),
        "clock-column"
    );
    assert_eq!(
        normalized.column(0).data_type(),
        &DataType::Timestamp(TimeUnit::Nanosecond, None)
    );
    assert_eq!(
        array_to_json(normalized.column(0).as_ref()).unwrap(),
        vec![json!("1970-01-01T07:59:59.999999999"), json!(null)]
    );
    let ticks = ["2024-03-10T06:30:00Z", "2024-03-10T07:30:00Z"].map(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .timestamp()
    });
    let dst = TimestampSecondArray::from(ticks.to_vec()).with_timezone("America/New_York");
    assert_eq!(
        array_to_json(&dst).unwrap(),
        vec![json!("2024-03-10T01:30:00"), json!("2024-03-10T03:30:00")]
    );
    assert_eq!(data_type_name(dst.data_type()), "Datetime(s)");
}

#[test]
fn storage_schema_preserves_identity_exact_types_and_category_domain() {
    let domain = CategoryDomain {
        labels: vec!["high".into(), "low".into()],
        ordered: true,
    };
    let field = with_column_metadata(
        Field::new(
            "grade",
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        "column-3",
        Some(&domain),
    )
    .unwrap();
    let schema = Schema::new(vec![
        with_column_metadata(Field::new("id", DataType::UInt64, false), "column-0", None).unwrap(),
        with_column_metadata(
            Field::new("amount", DataType::Decimal128(38, 12), true),
            "column-1",
            None,
        )
        .unwrap(),
        with_column_metadata(
            Field::new(
                "at",
                DataType::Timestamp(TimeUnit::Nanosecond, Some("Asia/Shanghai".into())),
                true,
            ),
            "column-2",
            None,
        )
        .unwrap(),
        field.clone().with_name("renamed"),
    ]);
    validate_storage_schema(&schema).unwrap();
    let restored: Schema = serde_json::from_str(&serde_json::to_string(&schema).unwrap()).unwrap();
    assert_eq!(restored, schema);
    assert_eq!(column_identity(restored.field(3)).unwrap(), "column-3");
    assert_eq!(
        CategoryDomain::from_field(restored.field(3)).unwrap(),
        Some(domain)
    );
    let facts = database_schema_fact(&DatabaseId::from_existing("data".into()), &restored)
        .unwrap()
        .with_revisions(7, 3);
    assert_eq!(facts.runtime_revision().get(), 7);
    assert_eq!(facts.schema_revision().get(), 3);
    assert!(!facts.columns()[0].nullable());
    assert!(facts.columns()[2].nullable());
    assert_eq!(
        facts.columns()[2].data_type(),
        &yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Datetime)time
    );
    assert!(
        validate_storage_schema(&Schema::new(vec![field.clone(), field.with_name("other")]))
            .is_err()
    );

    let rows = with_row_columns(
        Schema::new(vec![
            Field::new("stable_id", DataType::Int64, false),
            Field::new("order_key", DataType::Utf8, false),
            Field::new("user_value", DataType::Float64, true),
        ]),
        "stable_id",
        "order_key",
    )
    .unwrap();
    let columns = database_schema_fact(&DatabaseId::from_existing("data".into()), &rows).unwrap();
    assert_eq!(columns.columns().len(), 1);
    assert_eq!(columns.columns()[0].name().as_str(), "user_value");
    let restored: Schema = serde_json::from_str(&serde_json::to_string(&rows).unwrap()).unwrap();
    assert_eq!(
        dataset_row_columns(&restored).unwrap(),
        Some(DatasetRowColumns {
            row_id: "stable_id".into(),
            display_order: "order_key".into()
        })
    );
    assert!(with_row_columns(rows.clone(), "user_value", "order_key").is_err());

    // The same key has different meanings in independent Arrow dictionaries.
    for (labels, expected) in [(["high", "low"], "high"), (["low", "high"], "low")] {
        let array = DictionaryArray::<Int8Type>::try_new(
            Int8Array::from(vec![Some(0), None]),
            Arc::new(StringArray::from(labels.to_vec())),
        )
        .unwrap();
        assert_eq!(
            array_to_json(&array).unwrap(),
            vec![json!(expected), json!(null)]
        );
    }
}

#[test]
fn edited_values_reject_overflow_and_decimal_truncation() {
    for (dtype, value) in [
        (DataType::Int8, json!(300)),
        (DataType::Int8, json!(1.5)),
        (DataType::UInt8, json!(-1)),
        (DataType::Float32, json!(f64::MAX)),
        (DataType::Int64, json!(true)),
        (DataType::Decimal128(5, 2), json!("1.234")),
        (DataType::Decimal128(5, -2), json!("123")),
        (DataType::Decimal128(5, 2), json!("1000.00")),
    ] {
        assert!(json_to_array(&Field::new("value", dtype, true), &[value]).is_err());
    }
    assert!(
        json_to_array(
            &Field::new("value", DataType::UInt64, false),
            &[json!(null)]
        )
        .is_err()
    );
    for (dtype, text, expected) in [
        (
            DataType::UInt64,
            u64::MAX.to_string(),
            json!(u64::MAX.to_string()),
        ),
        (
            DataType::Decimal128(38, 12),
            "12345678901234567890123456.123456789012".into(),
            json!("12345678901234567890123456.123456789012"),
        ),
        (
            DataType::Decimal256(76, 4),
            "123456789012345678901234567890123456789012345678901234567890123456789012.3456".into(),
            json!("123456789012345678901234567890123456789012345678901234567890123456789012.3456"),
        ),
        (
            DataType::Decimal128(5, 2),
            "1.2300e1".into(),
            json!("12.30"),
        ),
        (DataType::Decimal128(5, -2), "12300".into(), json!("12300")),
        (
            DataType::Timestamp(TimeUnit::Nanosecond, None),
            "1969-12-31T23:59:59.999999999".into(),
            json!("1969-12-31T23:59:59.999999999"),
        ),
    ] {
        let array = json_to_array(
            &Field::new("value", dtype, true),
            &[json!(text), json!(null)],
        )
        .unwrap();
        assert_eq!(
            array_to_json(array.as_ref()).unwrap(),
            vec![expected, json!(null)]
        );
    }
}

#[test]
fn document_literals_keep_unsigned_values_and_reject_lossy_numeric_mixing() {
    let snapshot = |values: Vec<TabularScalar>| {
        TabularSnapshot::try_from_columns(
            vec![TabularColumn::new(
                TabularColumnName::try_from("value").unwrap(),
                values.into_boxed_slice(),
            )]
            .into_boxed_slice(),
        )
        .unwrap()
    };
    let batch = to_record_batch(&snapshot(vec![
        TabularScalar::Unsigned(u64::MAX),
        TabularScalar::Null,
    ]))
    .unwrap();
    assert_eq!(batch.column(0).data_type(), &DataType::UInt64);
    assert_eq!(
        batch
            .column(0)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap()
            .value(0),
        u64::MAX
    );
    assert!(batch.column(0).is_null(1));
    assert!(
        to_record_batch(&snapshot(vec![
            TabularScalar::Unsigned(u64::MAX),
            TabularScalar::Integer(-1)
        ]))
        .is_err()
    );
    assert!(
        to_record_batch(&snapshot(vec![
            TabularScalar::Unsigned(u64::MAX),
            TabularScalar::Decimal(1.0.try_into().unwrap())
        ]))
        .is_err()
    );
    let batch = to_record_batch(&snapshot(vec![
        TabularScalar::Integer(-1),
        TabularScalar::Unsigned(10),
    ]))
    .unwrap();
    assert_eq!(
        array_to_json(batch.column(0).as_ref()).unwrap(),
        vec![json!(-1), json!(10)]
    );
}
