use yss_data_contract::{DataValue, ValueType, ValueTypeParseError};

#[test]
fn literal_wire_preserves_wide_integers_and_rejects_noncanonical_spellings() {
    for value in [
        DataValue::Integer(i64::MIN),
        DataValue::Integer(i64::MAX),
        DataValue::Unsigned(u64::MAX),
    ] {
        let wire = serde_json::to_value(&value).unwrap();
        assert!(
            wire.as_object()
                .unwrap()
                .values()
                .next()
                .unwrap()
                .is_string()
        );
        assert_eq!(serde_json::from_value::<DataValue>(wire).unwrap(), value);
    }
    for wire in [
        r#"{"Integer":1}"#,
        r#"{"Integer":"01"}"#,
        r#"{"Integer":"-0"}"#,
        r#"{"Unsigned":"18446744073709551616"}"#,
    ] {
        assert!(serde_json::from_str::<DataValue>(wire).is_err());
    }
    assert_eq!("".parse::<ValueType>(), Err(ValueTypeParseError::Empty));
    assert_eq!(
        "Array<Numeric".parse::<ValueType>(),
        Err(ValueTypeParseError::MalformedComposite)
    );
    assert_eq!(
        "Unknown".parse::<ValueType>(),
        Err(ValueTypeParseError::UnknownKind)
    );
}

#[test]
fn named_struct_types_require_a_key_before_union_normalization() {
    for source in [
        "Any | Struct<>",
        "Struct<>",
        "Struct<   >",
        "Array<Struct<>>",
    ] {
        assert_eq!(
            source.parse::<ValueType>(),
            Err(ValueTypeParseError::MalformedComposite),
            "{source}"
        );
    }
    assert_eq!(
        "Any | Struct<tabular.grouped_dataframe>".parse::<ValueType>(),
        Ok(ValueType::Any)
    );
}
