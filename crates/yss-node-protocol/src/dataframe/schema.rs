use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use yss_data_contract::{FilterLiteral, TabularColumnName};

use super::{FilterOperator, FilterPredicate};

impl JsonSchema for FilterPredicate {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FilterPredicate".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "properties": {
                "column": generator.subschema_for::<TabularColumnName>(),
                "operator": generator.subschema_for::<FilterOperator>(),
                "value": generator.subschema_for::<FilterLiteral>()
            },
            "required": ["column", "operator"],
            "additionalProperties": false,
            "if": {
                "properties": {"operator": {"enum": [FilterOperator::IsNull, FilterOperator::IsNotNull]}}
            },
            "then": {"not": {"required": ["value"]}},
            "else": {"required": ["value"]}
        })
    }
}
