//! Schema Variables 测试

use yssbi_lib::schema::{VariableDefinition, VariableDataType, DataSourceConfig};

#[test]
fn test_create_primitive_variable() {
    let var = VariableDefinition::new_primitive(
        "var_001".into(),
        "threshold".into(),
        VariableDataType::Float,
        serde_json::json!(0.8),
    );

    assert_eq!(var.name, "threshold");
    assert!(var.is_primitive());
    assert!(!var.is_complex());
}

#[test]
fn test_create_complex_variable() {
    let var = VariableDefinition::new_complex(
        "var_002".into(),
        "sales_data".into(),
        VariableDataType::Dataframe,
        DataSourceConfig::Csv {
            path: "./data/sales.csv".into(),
            delimiter: ",".into(),
            encoding: "utf-8".into(),
            has_header: true,
        },
    );

    assert_eq!(var.name, "sales_data");
    assert!(var.is_complex());
}

#[test]
fn test_serialize_variable() {
    let var = VariableDefinition::new_primitive(
        "var_001".into(),
        "count".into(),
        VariableDataType::Int,
        serde_json::json!(42),
    );

    let json = serde_json::to_string_pretty(&var).unwrap();
    assert!(json.contains("\"name\": \"count\""));
}