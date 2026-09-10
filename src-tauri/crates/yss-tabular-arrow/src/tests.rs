use std::sync::Arc;

use arrow::array::{Array, DictionaryArray, Int8Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Int8Type, Schema, TimeUnit};
use serde_json::json;
use yss_database_contract::DatabaseId;
use yss_tabular_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

use super::*;

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
        &yss_data_contract::DataType::Datetime
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
