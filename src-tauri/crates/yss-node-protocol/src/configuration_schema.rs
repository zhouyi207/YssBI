//! JSON Schema projections of protocol-owned configuration, independent of a graph instance.

use schemars::{Schema, SchemaGenerator, generate::SchemaSettings, json_schema};
use serde_json::{Map, Value, json};

use crate::{
    NodeProtocol, Parameter, ParameterConstraint, ParameterEditorSpec, Parameters, PortCardinality,
    TypeExpr, parameter_value_to_json,
};

impl NodeProtocol {
    /// Creation and partial editing permit omitted values. Completeness, nominal codecs,
    /// coupled ports and connection-dependent constraints remain checked by their owners.
    pub fn configuration_schema(&self) -> Schema {
        let mut generator = SchemaSettings::draft2020_12()
            .with(|settings| settings.inline_subschemas = true)
            .into_generator();
        let parameters: Map<String, Value> = self
            .parameters
            .iter()
            .filter(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden))
            .map(|parameter| {
                (
                    parameter.key.to_string(),
                    parameter_schema(parameter, &self.parameters, &mut generator).into(),
                )
            })
            .collect();
        let counts: Map<String, Value> = self
            .interface
            .ports
            .iter()
            .filter(|port| matches!(port.cardinality, PortCardinality::UserCreated { .. }))
            .map(|port| {
                let (min, max) = self.interface.port_instance_bounds(port);
                let mut schema = generator.subschema_for::<u16>();
                schema.insert("minimum".into(), json!(min));
                schema.insert("maximum".into(), json!(max.unwrap_or(u16::MAX)));
                schema.insert("default".into(), json!(min));
                schema.insert("title".into(), json!(port.title));
                if let Some(group) = self.interface.member_group_for_template(&port.key) {
                    schema.insert("x-yss-linkedPorts".into(), json!(group.templates));
                    schema.insert("description".into(), json!(
                        "Set one linked template's total to configure the whole group. Explicit totals in a group must agree."
                    ));
                }
                (port.key.to_string(), schema.into())
            })
            .collect();
        json_schema!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "parameters": {
                    "type": "object",
                    "properties": parameters,
                    "additionalProperties": false,
                    "description": "Omit values to use protocol defaults or leave configuration incomplete. Null resets a value. Inactive conditional fields are removed by the host."
                },
                "portCounts": {
                    "type": "object",
                    "properties": counts,
                    "additionalProperties": false,
                    "description": "Initial total per configurable pin template. Omitted totals use protocol minimums. Fixed and derived pins cannot be overridden."
                }
            },
            "required": ["parameters", "portCounts"],
            "additionalProperties": false
        })
    }
}

fn parameter_schema(
    parameter: &Parameter,
    parameters: &Parameters,
    generator: &mut SchemaGenerator,
) -> Schema {
    let mut rules = vec![type_schema(&parameter.value_type, generator)];
    rules.extend(
        parameter.constraints.iter().filter_map(|constraint| {
            constraint_schema(constraint, &parameter.value_type, generator)
        }),
    );
    if matches!(parameter.editor, ParameterEditorSpec::SemanticDomain) {
        rules.push(generator.subschema_for::<yss_data_contract::ConversionDomain>());
    }
    if matches!(parameter.editor, ParameterEditorSpec::Resource { .. }) {
        rules.push(json_schema!({"type": "string", "minLength": 1}));
    }
    let mut schema = json_schema!({
        "anyOf": [{"allOf": rules}, {"type": "null"}]
    });
    if let Some(default) = parameter.default_json() {
        schema.insert("default".into(), default);
    }
    if parameter
        .constraints
        .contains(&ParameterConstraint::Required)
    {
        schema.insert("x-yss-requiredForExecution".into(), json!(true));
    }
    if let Some(condition) = &parameter.visible_when {
        let selector = parameters
            .get(&condition.key)
            .expect("registered visibility selector");
        schema.insert(
            "x-yss-activeWhen".into(),
            json!({
                "parameter": condition.key,
                "values": condition.values.iter().map(|value|
                    parameter_value_to_json(value, &selector.value_type)
                ).collect::<Vec<_>>()
            }),
        );
    }
    if matches!(parameter.editor, ParameterEditorSpec::Resource { .. }) {
        schema.insert("x-yss-resourceBound".into(), json!(true));
    }
    schema
}

fn type_schema(value_type: &TypeExpr, generator: &mut SchemaGenerator) -> Schema {
    match value_type {
        TypeExpr::Concrete(id) => match id.as_str() {
            "core.binary" => generator.subschema_for::<bool>(),
            "core.numeric" => json_schema!({"type": "number"}),
            "core.text" => generator.subschema_for::<String>(),
            "core.object" => json_schema!({"type": "object"}),
            crate::dataframe::PROJECT_COLUMNS_TYPE_ID => {
                generator.subschema_for::<crate::dataframe::ProjectColumns>()
            }
            crate::dataframe::FILTER_PREDICATE_TYPE_ID => {
                generator.subschema_for::<crate::dataframe::FilterPredicate>()
            }
            _ => json_schema!({"x-yss-type": value_type}),
        },
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == crate::DATA_SERIES_CONSTRUCTOR_ID => {
            match arguments.as_slice() {
                [element] => json_schema!({
                    "type": "array",
                    "items": type_schema(element, generator)
                }),
                _ => Schema::from(false),
            }
        }
        TypeExpr::Union(members) => json_schema!({
            "anyOf": members.iter().map(|member| type_schema(member, generator)).collect::<Vec<_>>()
        }),
        _ => json_schema!({"x-yss-type": value_type}),
    }
}

fn constraint_schema(
    constraint: &ParameterConstraint,
    value_type: &TypeExpr,
    generator: &mut SchemaGenerator,
) -> Option<Schema> {
    Some(match constraint {
        ParameterConstraint::Required => return None,
        ParameterConstraint::Positive => json_schema!({"type": "number", "exclusiveMinimum": 0}),
        ParameterConstraint::ColumnName => {
            generator.subschema_for::<yss_data_contract::TabularColumnName>()
        }
        ParameterConstraint::ColumnNames => json_schema!({
            "type": "array", "uniqueItems": true,
            "items": generator.subschema_for::<yss_data_contract::TabularColumnName>()
        }),
        ParameterConstraint::OneOf(values) => json_schema!({
            "enum": values.iter().map(|value| parameter_value_to_json(value, value_type)).collect::<Vec<_>>()
        }),
        ParameterConstraint::IntegerRange { min, max } => {
            let mut schema = generator.subschema_for::<i64>();
            schema.insert("minimum".into(), json!(min.unwrap_or(i64::MIN)));
            schema.insert("maximum".into(), json!(max.unwrap_or(i64::MAX)));
            schema
        }
        ParameterConstraint::Length { min, max } => {
            let branches: Vec<_> = [
                ("string", "minLength", "maxLength"),
                ("array", "minItems", "maxItems"),
                ("object", "minProperties", "maxProperties"),
            ]
            .into_iter()
            .map(|(kind, minimum, maximum)| {
                let mut schema = json_schema!({"type": kind});
                if let Some(min) = min {
                    schema.insert(minimum.into(), json!(min));
                }
                if let Some(max) = max {
                    schema.insert(maximum.into(), json!(max));
                }
                schema
            })
            .collect();
            json_schema!({"anyOf": branches})
        }
    })
}
