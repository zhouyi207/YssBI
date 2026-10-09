use super::*;
use serde_json::json;

fn fixture() -> serde_json::Value {
    json!({"columns": {
        " z/~ ": {"position":1,"semantic":"Numeric","count":u64::MAX,"missing":0,
            "mean":1.5,"std":null,"min":-2,"q25":0,"median":1,"q75":2,"max":u64::MAX},
        "a": {"position":2,"semantic":"Ordinal","count":3,"missing":1,"unique":3,
            "categories": {
                "1":{"value":u64::MAX,"label":"wide","frequency":1,"proportion":0.25},
                "2":{"value":"","label":"","frequency":1,"proportion":0.25},
                "3":{"value":false,"frequency":1,"proportion":0.25}
            }}
    }})
}

#[test]
fn description_projection_preserves_order_nulls_and_exact_scalar_carriers() {
    let columns = columns(&RuntimeValue::try_from(fixture()).unwrap()).unwrap();
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        [" z/~ ", "a"]
    );
    assert_eq!(columns[0].count, u64::MAX);
    let DescriptionStatistics::Numeric { metrics } = &columns[0].statistics else {
        panic!("numeric")
    };
    assert_eq!(metrics[1], TabularScalar::Null);
    assert_eq!(metrics[6], TabularScalar::Unsigned(u64::MAX));
    let DescriptionStatistics::Categorical {
        semantic,
        categories,
        ..
    } = &columns[1].statistics
    else {
        panic!("ordinal")
    };
    assert_eq!(*semantic, "Ordinal");
    assert_eq!(categories[0].value, TabularScalar::Unsigned(u64::MAX));
    assert_eq!(categories[0].label.as_deref(), Some("wide"));
    assert_eq!(categories[1].value, TabularScalar::String("".into()));
    assert_eq!(categories[1].label.as_deref(), Some(""));
    assert_eq!(categories[2].value, TabularScalar::Bool(false));
    assert_eq!(categories[2].label, None);
    assert_eq!(categories[2].frequency, 1);
    assert_eq!(categories[2].proportion, 0.25);
}

#[test]
fn description_projection_rejects_incomplete_or_mixed_column_contracts() {
    let valid = fixture();
    let mut invalid = Vec::new();
    let mut value = valid.clone();
    value["columns"][" z/~ "]
        .as_object_mut()
        .unwrap()
        .remove("std");
    invalid.push(value);
    let mut value = valid.clone();
    value["columns"][" z/~ "]["unique"] = json!(1);
    invalid.push(value);
    let mut value = valid.clone();
    value["columns"]["a"]["position"] = json!(1);
    invalid.push(value);
    let mut value = valid.clone();
    value["columns"]["a"]["categories"]
        .as_object_mut()
        .unwrap()
        .remove("2");
    invalid.push(value);
    let mut value = valid.clone();
    value["columns"]["a"]["categories"]["1"]["value"] = json!(null);
    invalid.push(value);
    let mut value = valid.clone();
    value["columns"]["a"]["categories"]["1"]["proportion"] = json!(1.5);
    invalid.push(value);
    let mut value = valid;
    value["columns"]["a"]["missing"] = json!(-1);
    invalid.push(value);
    for value in invalid {
        assert!(matches!(
            columns(&RuntimeValue::try_from(value).unwrap()),
            Err(Invalid::UnrepresentableValue)
        ));
    }
}
