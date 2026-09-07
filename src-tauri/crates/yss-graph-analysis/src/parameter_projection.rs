use super::*;

pub(super) fn project_schema_parameter_editors(node: &mut GraphNodeSemanticFact) {
    use yss_graph_protocol::dataframe::{
        FILTER_PREDICATE_TYPE_ID, FilterLiteral, FilterOperator, PROJECT_COLUMNS_TYPE_ID,
        filter_comparison_is_compatible, prepare_filter_predicate_json,
        prepare_project_columns_json,
    };
    let schema = node
        .ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input)
        .find_map(|port| port.schema_state.exact());
    let fields = schema.map_or(&[][..], |schema| schema.fields.as_slice());
    let unavailable_reason = schema
        .is_none()
        .then(|| "editors.dataframe.connect_source".into());
    for parameter in &mut node.parameters {
        let TypeExpr::Concrete(type_id) = &parameter.value_type else {
            continue;
        };
        let value = match &parameter.effective_value {
            Some(GraphResolvedParameterValue::Literal(value)) => Some(value.clone()),
            Some(GraphResolvedParameterValue::DefaultLiteral(value)) => {
                Some(yss_graph_protocol::protocol_value_to_json(value))
            }
            _ => None,
        };
        match type_id.as_str() {
            PROJECT_COLUMNS_TYPE_ID => {
                parameter.configuration = Some(GraphParameterConfigurationFact::ProjectColumns {
                    available: schema.is_some(),
                    unavailable_reason: unavailable_reason.clone(),
                    options: fields
                        .iter()
                        .map(|field| GraphColumnFact {
                            name: field.name.0.clone(),
                            data_type: field.scalar_type,
                        })
                        .collect(),
                    value: value
                        .as_ref()
                        .and_then(|value| prepare_project_columns_json(value).ok())
                        .map(|columns| columns.as_slice().into())
                        .unwrap_or_default(),
                });
            }
            FILTER_PREDICATE_TYPE_ID => {
                let literals = [
                    (
                        GraphFilterLiteralType::Boolean,
                        FilterLiteral::Boolean(false),
                    ),
                    (GraphFilterLiteralType::Integer, FilterLiteral::Integer(0)),
                    (
                        GraphFilterLiteralType::Decimal,
                        FilterLiteral::Decimal(
                            yss_graph_protocol::CanonicalDecimal::new("0")
                                .expect("zero is a decimal"),
                        ),
                    ),
                    (
                        GraphFilterLiteralType::String,
                        FilterLiteral::String("".into()),
                    ),
                ];
                let operators = [
                    FilterOperator::Equal,
                    FilterOperator::NotEqual,
                    FilterOperator::LessThan,
                    FilterOperator::LessThanOrEqual,
                    FilterOperator::GreaterThan,
                    FilterOperator::GreaterThanOrEqual,
                    FilterOperator::IsNull,
                    FilterOperator::IsNotNull,
                ];
                parameter.configuration = Some(GraphParameterConfigurationFact::FilterPredicate {
                    available: schema.is_some(),
                    unavailable_reason: unavailable_reason.clone(),
                    columns: fields
                        .iter()
                        .map(|field| GraphFilterColumnFact {
                            name: field.name.0.clone(),
                            data_type: field.scalar_type,
                            operators: operators
                                .iter()
                                .copied()
                                .filter(|operator| {
                                    filter_comparison_is_compatible(
                                        field.scalar_type,
                                        *operator,
                                        None,
                                    ) || literals.iter().any(|(_, literal)| {
                                        filter_comparison_is_compatible(
                                            field.scalar_type,
                                            *operator,
                                            Some(literal),
                                        )
                                    })
                                })
                                .collect(),
                            literal_types: literals
                                .iter()
                                .filter_map(|(kind, literal)| {
                                    filter_comparison_is_compatible(
                                        field.scalar_type,
                                        FilterOperator::Equal,
                                        Some(literal),
                                    )
                                    .then_some(*kind)
                                })
                                .collect(),
                        })
                        .collect(),
                    value: value.filter(|value| prepare_filter_predicate_json(value).is_ok()),
                });
            }
            "core.string"
                if matches!(parameter.editor, ParameterEditorSpec::Select)
                    && parameter.key.as_str() == "column"
                    && schema.is_some() =>
            {
                parameter.configuration = Some(GraphParameterConfigurationFact::SelectOptions {
                    options: fields.iter().map(|field| field.name.0.clone()).collect(),
                });
            }
            _ => {}
        }
    }
}

pub(super) fn parameter_fact(
    parameter: &yss_graph_protocol::ParameterSpec,
    effective_value: Option<GraphResolvedParameterValue>,
) -> GraphParameterFact {
    let configuration = match &parameter.editor {
        ParameterEditorSpec::Configuration(schema) => {
            let raw = match &effective_value {
                Some(GraphResolvedParameterValue::Literal(value)) => value.clone(),
                Some(GraphResolvedParameterValue::DefaultLiteral(value)) => {
                    yss_graph_protocol::protocol_value_to_json(value)
                }
                _ => serde_json::Value::Null,
            };
            let values = schema.effective_values(&raw);
            Some(GraphParameterConfigurationFact::Configuration {
                fields: schema
                    .fields
                    .iter()
                    .filter(|field| field.is_visible(&values))
                    .map(|field| {
                        parameter_fact(
                            &field.parameter,
                            values
                                .get(field.parameter.key.as_str())
                                .cloned()
                                .map(GraphResolvedParameterValue::Literal),
                        )
                    })
                    .collect(),
            })
        }
        _ => parameter
            .constraints
            .iter()
            .find_map(|constraint| match constraint {
                yss_graph_protocol::ParameterConstraint::OneOf(options)
                    if matches!(parameter.editor, ParameterEditorSpec::Select) =>
                {
                    Some(GraphParameterConfigurationFact::SelectOptions {
                        options: options
                            .iter()
                            .filter_map(|value| match value {
                                yss_graph_protocol::Value::String(value) => Some(value.clone()),
                                _ => None,
                            })
                            .collect(),
                    })
                }
                _ => None,
            }),
    };
    GraphParameterFact {
        key: parameter.key.clone(),
        title: parameter.title_key.as_str().into(),
        description: parameter
            .description_key
            .as_ref()
            .map(|key| key.as_str().into()),
        editor: parameter.editor.clone(),
        presentation: parameter.presentation,
        value_type: parameter.value_type.clone(),
        effective_value,
        configuration,
    }
}

pub(crate) fn effective_parameter_value(
    node: &yss_graph_document::DocumentNode,
    parameter: &yss_graph_protocol::ParameterSpec,
) -> Option<serde_json::Value> {
    node.parameters.get(&parameter.key).cloned().or_else(|| {
        parameter
            .default_value
            .as_ref()
            .map(|value| yss_graph_protocol::protocol_value_to_json(&value.value))
    })
}
