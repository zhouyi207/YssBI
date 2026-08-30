use super::{
    TabularMaterializationError, anyvalue_to_json, apply_operation, cast_column, json_to_anyvalue,
    reverse_operation, to_dataframe,
};
use polars::prelude::{AnyValue, DataType, TimeUnit, df};
use serde_json::json;
use yss_database_edit::EditOperation;
use yss_tabular_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

fn snapshot() -> TabularSnapshot {
    TabularSnapshot::try_from_columns(
        [
            TabularColumn::new(
                TabularColumnName::try_from("ids").expect("valid test name"),
                vec![TabularScalar::Integer(1), TabularScalar::Integer(2)].into_boxed_slice(),
            ),
            TabularColumn::new(
                TabularColumnName::try_from("labels").expect("valid test name"),
                vec![
                    TabularScalar::String("a".into()),
                    TabularScalar::String("b".into()),
                ]
                .into_boxed_slice(),
            ),
        ]
        .into(),
    )
    .expect("valid snapshot")
}

#[test]
fn polars_adapter_materializes_inferred_signed_int_and_shape() {
    let dataframe = to_dataframe(&snapshot()).expect("snapshot should materialize");

    assert_eq!(dataframe.height(), 2);
    assert_eq!(dataframe.width(), 2);
    assert_eq!(
        dataframe.column("ids").expect("ids column").dtype(),
        &DataType::Int64
    );
    assert_eq!(
        dataframe.column("labels").expect("labels column").dtype(),
        &DataType::String
    );
}

#[test]
fn polars_adapter_preserves_unsigned_values_without_narrowing() {
    let oversized_unsigned = TabularSnapshot::try_from_columns(
        [TabularColumn::new(
            TabularColumnName::try_from("ids").expect("valid test name"),
            vec![TabularScalar::Unsigned(u64::MAX)].into_boxed_slice(),
        )]
        .into(),
    )
    .expect("valid snapshot");

    let dataframe = to_dataframe(&oversized_unsigned).expect("unsigned snapshot materializes");
    assert_eq!(
        dataframe.column("ids").expect("ids column").dtype(),
        &DataType::UInt64
    );
    assert_eq!(
        dataframe
            .column("ids")
            .expect("ids column")
            .get(0)
            .expect("ids value"),
        AnyValue::UInt64(u64::MAX)
    );
}

#[test]
fn strict_json_conversion_rejects_incompatible_values() {
    assert_eq!(
        json_to_anyvalue(&json!(300), &DataType::Int8),
        Err(TabularMaterializationError::BuildFailed)
    );
    assert_eq!(
        json_to_anyvalue(&json!("abc"), &DataType::Float64),
        Err(TabularMaterializationError::BuildFailed)
    );
    assert_eq!(
        json_to_anyvalue(&json!(true), &DataType::Int64),
        Err(TabularMaterializationError::BuildFailed)
    );
    assert_eq!(
        json_to_anyvalue(&json!(7), &DataType::Int8),
        Ok(AnyValue::Int8(7))
    );
}

#[test]
fn json_projection_preserves_pre_epoch_datetimes() {
    assert_eq!(
        anyvalue_to_json(AnyValue::Datetime(-1, TimeUnit::Milliseconds, None)),
        json!("1969-12-31 23:59:59.999")
    );
    assert_eq!(anyvalue_to_json(AnyValue::Date(1)), json!("1970-01-02"));
}

#[test]
fn database_edit_operation_applies_and_reverses_without_dtype_drift() {
    let mut dataframe = df!("value" => &[1_i64, 2]).expect("dataframe");
    let operation = EditOperation::EditCell {
        row: 0,
        row_id: None,
        col: "value".to_owned(),
        old_value: json!(1),
        new_value: json!(3),
    };

    apply_operation(&mut dataframe, &operation).expect("apply edit");
    assert_eq!(
        dataframe.column("value").expect("value").dtype(),
        &DataType::Int64
    );
    assert_eq!(
        dataframe
            .column("value")
            .expect("value")
            .get(0)
            .expect("cell"),
        AnyValue::Int64(3)
    );

    reverse_operation(&mut dataframe, &operation).expect("reverse edit");
    assert_eq!(
        dataframe
            .column("value")
            .expect("value")
            .get(0)
            .expect("cell"),
        AnyValue::Int64(1)
    );
}

#[test]
fn strict_cast_failure_leaves_the_original_column_unchanged() {
    let mut dataframe = df!("value" => &["1", "not-an-integer"]).expect("dataframe");

    assert!(cast_column(&mut dataframe, "value", "Int64", false).is_err());
    assert_eq!(
        dataframe.column("value").expect("value").dtype(),
        &DataType::String
    );
    assert_eq!(
        dataframe
            .column("value")
            .expect("value")
            .get(1)
            .expect("cell"),
        AnyValue::String("not-an-integer")
    );
}
