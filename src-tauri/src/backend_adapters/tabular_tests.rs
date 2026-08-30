use crate::backend_adapters::tabular::polars::{
    TabularMaterializationError, json_to_anyvalue, to_dataframe,
};
use polars::prelude::{AnyValue, DataType};
use serde_json::json;
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
fn review_fix_polars_adapter_rejects_out_of_range_strict_conversion() {
    let oversized_unsigned = TabularSnapshot::try_from_columns(
        [TabularColumn::new(
            TabularColumnName::try_from("ids").expect("valid test name"),
            vec![TabularScalar::Unsigned(i64::MAX as u64 + 1)].into_boxed_slice(),
        )]
        .into(),
    )
    .expect("valid snapshot");

    assert_eq!(
        to_dataframe(&oversized_unsigned),
        Err(TabularMaterializationError::BuildFailed)
    );
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
