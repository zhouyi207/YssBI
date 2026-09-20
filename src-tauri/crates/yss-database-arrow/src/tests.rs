use std::sync::Arc;

use arrow::array::{Array, DictionaryArray, Int8Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Int8Type, Schema, TimeUnit};
use serde_json::json;
use yss_database_contract::DatabaseId;
use yss_tabular_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

use super::*;

#[test]
#[ignore = "manual conversion timing probe"]
fn conversion_hot_path_timing() {
    use std::{hint::black_box, time::Instant};
    use yss_data_contract::{
        NumericRepresentation, SemanticConversion, SemanticType, SemanticValue,
    };
    use yss_tabular_contract::TabularScalar;
    let field = Field::new("code", DataType::Utf8, true);
    let mut spec = SemanticConversion::new(SemanticType::Ordinal, NumericRepresentation::Auto);
    spec.domain.values = (0..1024)
        .map(|i| SemanticValue {
            value: i.to_string(),
            label: i.to_string(),
        })
        .collect();
    let array = StringArray::from_iter_values((0..512).map(|i| i.to_string()));
    let start = Instant::now();
    let conversion = PreparedConversion::new(&field, &spec).unwrap();
    for _ in 0..100 {
        black_box(conversion.convert(&array).unwrap());
    }
    println!(
        "domain_batches_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let values = (0..10_000)
        .map(|_| TabularScalar::String("12345.67".into()))
        .collect::<Vec<_>>();
    let spec = SemanticConversion::new(SemanticType::Numeric, NumericRepresentation::Auto);
    let start = Instant::now();
    for _ in 0..20 {
        black_box(convert_semantic_values(&values, None, &spec).unwrap());
    }
    println!(
        "materialized_values_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.
    );
}

#[test]
fn prepared_conversion_validates_every_batch_without_mutating_declared_domains() {
    use arrow::array::Int64Array;
    use yss_data_contract::{NumericRepresentation, SemanticConversion};
    let mut spec = SemanticConversion::new(SemanticType::Ordinal, NumericRepresentation::Auto);
    spec.domain.values = ["high", "low"]
        .map(|code| SemanticValue {
            value: code.into(),
            label: code.into(),
        })
        .into();
    let conversion =
        PreparedConversion::new(&Field::new("value", DataType::Utf8, true), &spec).unwrap();
    for values in [vec![Some("low"), None], vec![Some("high")]] {
        assert!(conversion.convert(&StringArray::from(values)).is_ok());
        assert!(
            conversion
                .convert(&StringArray::from(vec!["unknown"]))
                .is_err()
        );
        assert_eq!(
            column_semantic(conversion.field()).unwrap().values,
            spec.domain.values
        );
    }
    let mut semantic = ColumnSemantic::new(SemanticType::Numeric);
    semantic.numeric = Some(NumericConstraints {
        integer: true,
        minimum: Some("1".into()),
        maximum: Some("9007199254740993".into()),
    });
    let source =
        with_column_semantic(Field::new("value", DataType::Int64, true), &semantic).unwrap();
    let conversion = PreparedConversion::new(
        &source,
        &SemanticConversion::new(SemanticType::Text, NumericRepresentation::Auto),
    )
    .unwrap();
    assert!(
        conversion
            .convert(&Int64Array::from(vec![Some(9_007_199_254_740_993), None]))
            .is_ok()
    );
    for invalid in [0, 9_007_199_254_740_994] {
        assert!(
            conversion
                .convert(&Int64Array::from(vec![invalid]))
                .is_err()
        );
    }
    assert!(conversion.convert(&Int64Array::from(vec![1])).is_ok());
    assert!(conversion.convert(&StringArray::from(vec!["1"])).is_err());
}

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
fn semantic_conversion_preserves_nulls_and_rejects_loss_without_requiring_text_identity() {
    use yss_data_contract::{NumericRepresentation as N, SemanticConversion, SemanticType as S};
    use yss_tabular_contract::TabularScalar as V;
    let spec = |target, numeric| SemanticConversion::new(target, numeric);
    let values = |values: Vec<V>, target, numeric| {
        convert_semantic_values(&values, None, &spec(target, numeric)).map(|result| result.values)
    };
    let real = |value: f64| V::Decimal(value.try_into().unwrap());
    assert_eq!(
        values(
            vec![
                V::String(" 001.0 ".into()),
                V::Null,
                V::String("1e3".into()),
                V::String("0.1".into())
            ],
            S::Numeric,
            N::Auto
        )
        .unwrap(),
        vec![real(1.), V::Null, real(1000.), real(0.1)]
    );
    assert_eq!(
        values(
            vec![V::Integer(9_007_199_254_740_993), V::Null],
            S::Numeric,
            N::Auto
        )
        .unwrap(),
        vec![V::Integer(9_007_199_254_740_993), V::Null]
    );
    assert_eq!(
        values(vec![V::String("001".into())], S::Numeric, N::Integer).unwrap(),
        vec![V::Integer(1)]
    );
    for text in [
        "9007199254740993",
        "9007199254740993.0",
        "9007199254740993e0",
        "18446744073709551617",
        "1.234567890123456789",
        "NaN",
        "inf",
        "",
        "oops",
    ] {
        assert!(
            values(vec![V::String(text.into())], S::Numeric, N::Real).is_err(),
            "{text}"
        );
    }
    assert!(values(vec![real(1.5)], S::Numeric, N::Integer).is_err());
    assert!(values(vec![V::Integer(9_007_199_254_740_993)], S::Numeric, N::Real).is_err());
    assert_eq!(
        values(
            vec![V::String("true".into()), V::Null, V::String("0".into())],
            S::Binary,
            N::Auto
        )
        .unwrap(),
        vec![V::Bool(true), V::Null, V::Bool(false)]
    );
    assert!(values(vec![V::Integer(2)], S::Binary, N::Auto).is_err());
    assert_eq!(
        values(vec![V::Bool(true), V::Null], S::Numeric, N::Auto).unwrap(),
        vec![V::Integer(1), V::Null]
    );
    for target in [S::Numeric, S::Text, S::Binary] {
        assert_eq!(
            values(vec![V::Null], target, N::Auto).unwrap(),
            vec![V::Null]
        );
    }
    let field = Field::new("source", DataType::Utf8, true);
    let conversion = PreparedConversion::new(&field, &spec(S::Numeric, N::Integer)).unwrap();
    let output = conversion.field();
    assert_eq!(output.data_type(), &DataType::Int64);
    assert!(output.is_nullable());
    assert!(column_semantic(&output).unwrap().numeric.unwrap().integer);
    let invalid = with_column_semantic(
        Field::new("id", DataType::Int64, true),
        &ColumnSemantic::new(S::Identifier),
    )
    .unwrap();
    assert_eq!(
        column_semantic(
            PreparedConversion::new(&invalid, &spec(S::Numeric, N::Auto))
                .unwrap()
                .field()
        )
        .unwrap()
        .kind,
        S::Numeric
    );
    let mut binary = column_semantic(&Field::new("flag", DataType::Boolean, true)).unwrap();
    binary.positive_value = Some("false".into());
    let field = with_column_semantic(Field::new("flag", DataType::Boolean, true), &binary).unwrap();
    let array = arrow::array::BooleanArray::from(vec![Some(false), Some(true), None]);
    let result = PreparedConversion::new(&field, &spec(S::Numeric, N::Auto))
        .unwrap()
        .convert(&array)
        .unwrap();
    assert_eq!(
        array_to_json(result.as_ref()).unwrap(),
        vec![json!(1), json!(0), json!(null)]
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
fn semantic_domains_and_identifiers_keep_codes_and_reject_undeclared_levels() {
    use yss_data_contract::{
        ConversionDomain, NumericRepresentation as N, SemanticConversion, SemanticType as S,
        SemanticValue,
    };
    use yss_tabular_contract::TabularScalar as V;
    let input = vec![V::String("001".into()), V::Null, V::String("002".into())];
    let id = convert_semantic_values(
        &input,
        None,
        &SemanticConversion::new(S::Identifier, N::Auto),
    )
    .unwrap();
    assert_eq!(id.values, input);
    assert_eq!(id.metadata.semantic.kind, S::Identifier);
    let mut ordinal = SemanticConversion::new(S::Ordinal, N::Auto);
    assert!(convert_semantic_values(&input, None, &ordinal).is_err());
    ordinal.domain = ConversionDomain {
        values: vec![
            SemanticValue {
                value: "002".into(),
                label: "low".into(),
            },
            SemanticValue {
                value: "001".into(),
                label: "high".into(),
            },
        ],
        positive_value: None,
    };
    let ranked = convert_semantic_values(&id.values, Some(&id.metadata), &ordinal).unwrap();
    assert_eq!(ranked.values, input);
    assert_eq!(ranked.metadata.semantic.values, ordinal.domain.values);
    let categorical = convert_semantic_values(
        &ranked.values,
        Some(&ranked.metadata),
        &SemanticConversion::new(S::Categorical, N::Auto),
    )
    .unwrap();
    assert_eq!(categorical.metadata.semantic.kind, S::Categorical);
    assert_eq!(categorical.metadata.semantic.values, ordinal.domain.values);
    assert!(
        convert_semantic_values(
            &categorical.values,
            Some(&categorical.metadata),
            &SemanticConversion::new(S::Ordinal, N::Auto)
        )
        .is_err(),
        "an unordered category domain must not silently become an ordinal order"
    );
    let text = convert_semantic_values(
        &categorical.values,
        Some(&categorical.metadata),
        &SemanticConversion::new(S::Text, N::Auto),
    )
    .unwrap();
    assert_eq!(text.values, input);
    assert!(convert_semantic_values(&[V::String("003".into())], None, &ordinal).is_err());
    ordinal.domain.values.push(ordinal.domain.values[0].clone());
    assert!(convert_semantic_values(&input, None, &ordinal).is_err());
}

#[test]
fn semantic_calendar_conversion_parses_formats_preserves_clock_and_rejects_precision_loss() {
    use yss_data_contract::{
        DatetimeRepresentation as D, NumericRepresentation as N, SemanticConversion,
        SemanticType as S, TemporalPrecision as P,
    };
    use yss_tabular_contract::TabularScalar as V;
    let mut spec = SemanticConversion::new(S::Datetime, N::Auto);
    spec.format = "%d/%m/%Y %H:%M:%S %z".into();
    let result = convert_semantic_values(
        &[V::String("17/09/2026 08:30:00 +0800".into()), V::Null],
        None,
        &spec,
    )
    .unwrap();
    assert_eq!(
        result.values,
        vec![V::String("2026-09-17T08:30:00".into()), V::Null]
    );
    let text = convert_semantic_values(
        &result.values,
        Some(&result.metadata),
        &SemanticConversion::new(S::Text, N::Auto),
    )
    .unwrap();
    assert_eq!(text.values, result.values);
    spec.format.clear();
    let value = vec![V::String("2026-09-17T08:30:00.000000001+08:00".into())];
    assert!(convert_semantic_values(&value, None, &spec).is_err());
    spec.precision = P::Nanoseconds;
    let nanos = convert_semantic_values(&value, None, &spec).unwrap();
    assert_eq!(
        nanos.values,
        vec![V::String("2026-09-17T08:30:00.000000001".into())]
    );
    spec.datetime = D::Date;
    assert!(convert_semantic_values(&nanos.values, Some(&nanos.metadata), &spec).is_err());
    let date = convert_semantic_values(&[V::String("2026-09-17".into())], None, &spec).unwrap();
    assert_eq!(date.values, vec![V::String("2026-09-17".into())]);
    spec.datetime = D::Time;
    let time =
        convert_semantic_values(&[V::String("08:30:00.000000001".into())], None, &spec).unwrap();
    assert_eq!(time.values, vec![V::String("08:30:00.000000001".into())]);
    assert!(convert_semantic_values(&[V::String("bad".into())], None, &spec).is_err());
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
        &yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Datetime)
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
