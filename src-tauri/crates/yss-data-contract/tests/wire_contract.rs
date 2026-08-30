use yss_data_contract::{
    CategoricalRole, DataSeriesValue, DataType, DataTypeParseError, DataValue, DummyInfo,
    TimeSeriesState,
};

#[test]
fn persisted_data_contract_preserves_wire_and_uses_typed_parse_errors() {
    let id_only = DataValue::DataSeries(DataSeriesValue::new("series-id"));
    assert_eq!(
        serde_json::to_value(&id_only).expect("id-only data series must serialize"),
        serde_json::json!({"DataSeries": "series-id"})
    );

    let full = DataValue::DataSeries(DataSeriesValue {
        id: "series-id".to_owned(),
        element_type: Some(DataType::String),
        dummy_info: Some(DummyInfo {
            drop_category: Some("baseline".to_owned()),
            role: CategoricalRole::Individual,
        }),
        time_series_state: Some(TimeSeriesState::Aligned),
    });
    let expected = serde_json::json!({
        "DataSeries": {
            "id": "series-id",
            "elementType": {"kind": "String"},
            "dummyInfo": {
                "dropCategory": "baseline",
                "role": "individual"
            },
            "timeSeriesState": "aligned"
        }
    });
    assert_eq!(
        serde_json::to_value(&full).expect("full data series must serialize"),
        expected
    );
    assert_eq!(
        serde_json::from_value::<DataValue>(expected)
            .expect("persisted full data series must deserialize"),
        full
    );

    assert_eq!("".parse::<DataType>(), Err(DataTypeParseError::Empty));
    assert_eq!(
        "Array<Int64".parse::<DataType>(),
        Err(DataTypeParseError::MalformedComposite)
    );
    assert_eq!(
        "Unknown".parse::<DataType>(),
        Err(DataTypeParseError::UnknownKind)
    );
}
