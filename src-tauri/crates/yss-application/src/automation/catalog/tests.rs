use serde_json::{Value, json};
use yss_node_catalog::build_builtin_node_system;
use yss_node_protocol::{ParameterEditorSpec, ParameterValues, TypeExpr};

#[test]
fn catalog_configuration_schemas_accept_protocol_defaults_and_incomplete_creation() {
    let system = build_builtin_node_system().unwrap();
    let empty = ParameterValues::new();
    for (id, node) in system.registry.iter() {
        let protocol = node.protocol();
        let schema = Value::from(protocol.configuration_schema());
        let validator =
            jsonschema::validator_for(&schema).unwrap_or_else(|error| panic!("{id}: {error}"));
        let defaults: serde_json::Map<_, _> = protocol
            .parameters
            .iter()
            .filter(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden))
            .filter(|parameter| protocol.parameters.is_visible(parameter, &empty))
            .filter_map(|parameter| {
                parameter
                    .default_json()
                    .map(|value| (parameter.key.to_string(), value))
            })
            .collect();
        for configuration in [
            json!({"parameters": {}, "portCounts": {}}),
            json!({"parameters": defaults, "portCounts": {}}),
        ] {
            validator
                .validate(&configuration)
                .unwrap_or_else(|error| panic!("{id}: {error}"));
        }
        assert!(
            !validator.is_valid(&json!({
                "parameters": {"undeclared_parameter": true}, "portCounts": {}
            })),
            "{id}"
        );
        for parameter in protocol
            .parameters
            .iter()
            .filter(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden))
        {
            assert!(
                validator.is_valid(&json!({
                    "parameters": {parameter.key.as_str(): null}, "portCounts": {}
                })),
                "{id}: clearing {}",
                parameter.key
            );
        }
    }
}

#[test]
fn structured_parameter_schemas_preserve_column_names_and_lossless_filter_literals() {
    use yss_node_protocol::dataframe::{
        FILTER_PREDICATE_TYPE_ID, FilterPredicate, PROJECT_COLUMNS_TYPE_ID, ProjectColumns,
    };
    let system = build_builtin_node_system().unwrap();
    let examples = [
        (
            PROJECT_COLUMNS_TYPE_ID,
            vec![
                json!(["分组", " income "]),
                json!([]),
                json!(["x", "x"]),
                json!([" "]),
                json!([1]),
            ],
        ),
        (
            FILTER_PREDICATE_TYPE_ID,
            vec![
                json!({"column": "收入", "operator": "greaterThan", "value": {"type": "integer", "value": "9007199254740993"}}),
                json!({"column": "收入", "operator": "equal", "value": {"type": "decimal", "value": "-0.125"}}),
                json!({"column": "收入", "operator": "equal", "value": {"type": "integer", "value": 9007199254740993_u64}}),
                json!({"column": "收入", "operator": "equal", "value": {"type": "decimal", "value": "1.0"}}),
                json!({"column": "收入", "operator": "isNull"}),
                json!({"column": "收入", "operator": "isNull", "value": {"type": "string", "value": ""}}),
                json!({"column": "收入", "operator": "equal"}),
                json!({"column": " ", "operator": "isNull"}),
            ],
        ),
    ];
    for (type_id, examples) in examples {
        let (protocol, parameter) = system
            .registry
            .iter()
            .find_map(|(_, node)| {
                node.protocol().parameters.iter().find(|parameter|
                matches!(&parameter.value_type, TypeExpr::Concrete(id) if id.as_str() == type_id)
            ).map(|parameter| (node.protocol(), parameter))
            })
            .expect("built-in structured parameter");
        let schema = Value::from(protocol.configuration_schema());
        let validator = jsonschema::validator_for(&schema).unwrap();
        for value in examples {
            let accepted_by_owner = if type_id == PROJECT_COLUMNS_TYPE_ID {
                serde_json::from_value::<ProjectColumns>(value.clone()).is_ok()
            } else {
                serde_json::from_value::<FilterPredicate>(value.clone()).is_ok()
            };
            let configuration =
                json!({"parameters": {parameter.key.as_str(): value}, "portCounts": {}});
            assert_eq!(
                validator.is_valid(&configuration),
                accepted_by_owner,
                "{type_id}: {configuration}"
            );
        }
    }
}
