use super::*;
use yss_data_contract::FilterLiteral;

pub(super) fn parameter_schema<'a>(
    ports: &'a [GraphPortSemanticFact],
    key: &str,
) -> Option<&'a ResolvedSchemaFact> {
    let source = match key {
        "left_keys" => Some("left"),
        "right_keys" => Some("right"),
        _ => None,
    };
    ports.iter().filter(|port| port.direction == PortDirection::Input)
        .filter(|port| source.is_none_or(|source| matches!(&port.address.port, PortRef::Declared { key } if key.as_str() == source)))
        .find_map(|port| port.schema_state.exact())
}

pub(super) fn project_schema_parameter_editors(node: &mut GraphNodeSemanticFact) {
    use yss_node_protocol::dataframe::{
        FILTER_PREDICATE_TYPE_ID, FilterOperator, PROJECT_COLUMNS_TYPE_ID,
        filter_comparison_is_compatible, prepare_filter_predicate_json,
        prepare_project_columns_json,
    };
    for parameter in &mut node.parameters {
        let schema = parameter_schema(&node.ports, parameter.key.as_str());
        let fields = schema.map_or(&[][..], |schema| schema.fields.as_slice());
        let unavailable_reason = schema
            .is_none()
            .then(|| "editors.dataframe.connect_source".into());
        let value = match &parameter.effective_value {
            Some(GraphResolvedParameterValue::Literal(value)) => Some(value.clone()),
            Some(GraphResolvedParameterValue::DefaultLiteral(value)) => {
                Some(yss_node_protocol::protocol_value_to_json(value))
            }
            _ => None,
        };
        if parameter.key.as_str() == "subset"
            && matches!(
                node.node_type.as_str(),
                "yssbi.dataframe.dropna.rows" | "yssbi.dataframe.dropna.columns"
            )
        {
            parameter.configuration = Some(GraphParameterConfigurationFact::ProjectColumns {
                allow_empty: true,
                available: schema.is_some(),
                unavailable_reason: if node
                    .ports
                    .iter()
                    .filter(|port| port.direction == PortDirection::Input)
                    .any(|port| matches!(port.schema_state, GraphSchemaState::Deferred))
                {
                    Some("editors.dataframe.deferred_columns".into())
                } else {
                    unavailable_reason.clone()
                },
                options: fields
                    .iter()
                    .map(|field| GraphColumnFact {
                        name: field.name.0.clone(),
                        data_type: field.scalar_type,
                    })
                    .collect(),
                value: value
                    .as_ref()
                    .and_then(|value| value.as_array())
                    .map(|columns| {
                        columns
                            .iter()
                            .filter_map(|column| column.as_str().map(Into::into))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
            continue;
        }
        let TypeExpr::Concrete(type_id) = &parameter.value_type else {
            continue;
        };
        match type_id.as_str() {
            PROJECT_COLUMNS_TYPE_ID => {
                parameter.configuration = Some(GraphParameterConfigurationFact::ProjectColumns {
                    allow_empty: false,
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
                            yss_data_contract::DecimalLiteral::new("0").expect("zero is a decimal"),
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
            "core.text"
                if matches!(parameter.editor, ParameterEditorSpec::Select)
                    && matches!(
                        parameter.key.as_str(),
                        "column" | "entity_column" | "time_column"
                    )
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
    group_key: &yss_node_protocol::ParameterGroupKey,
    parameter: &yss_node_protocol::Parameter,
    effective_value: Option<GraphResolvedParameterValue>,
) -> GraphParameterFact {
    let configuration = parameter
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            yss_node_protocol::ParameterConstraint::OneOf(options)
                if matches!(parameter.editor, ParameterEditorSpec::Select) =>
            {
                Some(GraphParameterConfigurationFact::SelectOptions {
                    options: options
                        .iter()
                        .filter_map(|value| match value {
                            yss_data_contract::DataValue::String(value) => Some(value.clone()),
                            _ => None,
                        })
                        .collect(),
                })
            }
            _ => None,
        });
    GraphParameterFact {
        group_key: group_key.clone(),
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
    parameter: &yss_node_protocol::Parameter,
) -> Option<serde_json::Value> {
    node.parameters
        .get(&parameter.key)
        .cloned()
        .or_else(|| parameter.default_json())
}
