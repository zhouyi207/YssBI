use serde_json::json;
use yss_data_contract::{DataType, DataValue};
use yss_variable_contract::{VariableId, VariableIdParseError, VariableInstance, VariableScope};

#[test]
fn variable_id_preserves_uuid_wire_and_uses_a_typed_parse_error() {
    let id = VariableId::nil();
    assert_eq!(
        serde_json::to_value(id).unwrap(),
        json!("00000000-0000-0000-0000-000000000000")
    );
    assert_eq!(
        VariableId::try_from("00000000-0000-0000-0000-000000000000"),
        Ok(id)
    );
    assert_eq!(
        VariableId::try_from("not-a-uuid"),
        Err(VariableIdParseError::Invalid)
    );
}

#[test]
fn variable_scope_preserves_tagged_camel_case_wire_shapes() {
    assert_eq!(
        serde_json::to_value(VariableScope::Global).unwrap(),
        json!({"type": "global"})
    );
    assert_eq!(
        serde_json::to_value(VariableScope::Event {
            event_path: "events/on-open".into(),
        })
        .unwrap(),
        json!({"type": "event", "eventPath": "events/on-open"})
    );
    assert_eq!(
        serde_json::to_value(VariableScope::Function {
            function_path: "functions/normalize".into(),
        })
        .unwrap(),
        json!({"type": "function", "functionPath": "functions/normalize"})
    );
}

#[test]
fn variable_instance_preserves_persisted_field_names_and_omits_absent_tabular_data() {
    let variable = VariableInstance {
        id: VariableId::nil(),
        name: "label".into(),
        data_type: DataType::String,
        data_value: DataValue::String("sample".into()),
        tabular: None,
        description: "Example variable".into(),
        scope: VariableScope::Global,
        tags: vec!["demo".into()],
    };
    let expected = json!({
        "id": "00000000-0000-0000-0000-000000000000",
        "name": "label",
        "dataType": {"kind": "String"},
        "dataValue": {"String": "sample"},
        "description": "Example variable",
        "scope": {"type": "global"},
        "tags": ["demo"]
    });

    assert_eq!(serde_json::to_value(&variable).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<VariableInstance>(expected).unwrap(),
        variable
    );
}
