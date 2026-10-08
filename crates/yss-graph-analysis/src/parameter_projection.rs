use crate::{
    GraphColumnFact, GraphDiagnosticFact, GraphDiagnosticLocation, GraphFilterColumnFact,
    GraphFilterLiteralType, GraphNodeSemanticFact, GraphParameterConfigurationFact,
    GraphParameterFact, GraphParameterGroupFact, GraphPortSemanticFact,
    GraphResolvedParameterValue, GraphSchemaState, graph_problem,
};
use std::borrow::Cow;
use yss_data_contract::FilterLiteral;
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{DocumentNode, GraphDocument, GraphResourcePath, PortRef};
use yss_graph_resource_contract::{GraphResourceId, ResourceCatalogSnapshot};
use yss_node_protocol::{
    NodeProtocol, ParameterEditorSpec, ParameterIssueKind, PortDirection, RelationalScalarType,
    ResolvedSchemaFact, ResourceDisplayKind, TypeExpr, validate_parameter_values,
};
use yss_node_registry::NodeRegistry;

pub(crate) fn effective_conversion_target(
    node: &DocumentNode,
    target: &yss_node_protocol::ConversionTarget,
    registry: &NodeRegistry,
) -> Option<yss_node_protocol::SemanticType> {
    match target {
        yss_node_protocol::ConversionTarget::Fixed(target) => Some(*target),
        yss_node_protocol::ConversionTarget::Parameter(parameter) => {
            let value = effective_text_parameter(node, parameter, registry)?;
            yss_node_protocol::SemanticType::ALL
                .into_iter()
                .find(|target| target.type_id() == value)
        }
    }
}

pub(super) fn aggregate_parameter_accepts(
    node: &str,
    key: &str,
    kind: RelationalScalarType,
) -> Option<bool> {
    use yss_data_contract::aggregation::AggregateOperation;
    match (node, key) {
        ("yssbi.dataframe.sort", "descending_columns")
        | ("yssbi.dataframe.deduplicate", "keys")
        | ("yssbi.dataframe.unpivot", "keys")
        | ("yssbi.dataframe.pivot", "keys")
        | ("yssbi.dataframe.resample", "keys") => Some(true),
        (node, "partition_by" | "order_by") if node.starts_with("yssbi.dataframe.") => Some(true),
        ("yssbi.dataframe.groupby", key) => AggregateOperation::ALL
            .into_iter()
            .find(|op| op.key() == key)
            .map(|op| matches!(kind, RelationalScalarType::Known(s) if op.accepts(s))),
        _ => None,
    }
}

pub(super) fn parameter_schema<'a>(
    ports: &'a [GraphPortSemanticFact],
    key: &str,
) -> Option<&'a ResolvedSchemaFact> {
    parameter_schema_state(ports, key).and_then(GraphSchemaState::exact)
}

fn parameter_schema_state<'a>(
    ports: &'a [GraphPortSemanticFact],
    key: &str,
) -> Option<&'a GraphSchemaState> {
    let source = match key {
        "left_keys" => Some("left"),
        "right_keys" => Some("right"),
        "partition_by" | "order_by" => Some("context"),
        _ => None,
    };
    ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input)
        .filter(|port| {
            source.is_none_or(|source| match &port.address.port {
                PortRef::Declared { key } => key.as_str() == source,
                PortRef::Instance { template, .. } => template.as_str() == source,
            })
        })
        .map(|port| &port.schema_state)
        .find(|state| !matches!(state, GraphSchemaState::NotApplicable))
}

pub(super) fn project_schema_parameter_editors(node: &mut GraphNodeSemanticFact) {
    project_parameter_context(node.node_type.as_str(), &node.ports, &mut node.parameters);
}

fn project_parameter_context(
    node_type: &str,
    ports: &[GraphPortSemanticFact],
    parameters: &mut [GraphParameterFact],
) {
    use yss_node_protocol::dataframe::{
        FILTER_PREDICATE_TYPE_ID, FilterOperator, PROJECT_COLUMNS_TYPE_ID,
        filter_comparison_is_compatible, prepare_filter_predicate_json,
        prepare_project_columns_json,
    };
    for parameter in parameters {
        let schema_state = parameter_schema_state(ports, parameter.key.as_str());
        let schema = schema_state.and_then(GraphSchemaState::exact);
        let fields = schema.map_or(&[][..], |schema| schema.fields.as_slice());
        let context_hint: Option<Box<str>> = match schema_state {
            Some(GraphSchemaState::Exact(_) | GraphSchemaState::Observed { .. }) => None,
            Some(GraphSchemaState::Deferred) => Some("editors.dataframe.deferred_columns".into()),
            Some(
                GraphSchemaState::Unavailable(_)
                | GraphSchemaState::Conflict(_)
                | GraphSchemaState::InternalFailure(_),
            ) => Some("editors.dataframe.schema_error".into()),
            Some(GraphSchemaState::Pending(issue))
                if *issue != crate::GraphSchemaIssue::UnconnectedInput =>
            {
                Some("editors.dataframe.pending_columns".into())
            }
            _ => Some("editors.dataframe.connect_source".into()),
        };
        if (parameter.key.as_str() == "subset"
            && matches!(
                node_type,
                "yssbi.dataframe.dropna.rows" | "yssbi.dataframe.dropna.columns"
            ))
            || aggregate_parameter_accepts(
                node_type,
                parameter.key.as_str(),
                RelationalScalarType::Unknown,
            )
            .is_some()
        {
            let value = parameter_literal_value(parameter);
            parameter.configuration = Some(GraphParameterConfigurationFact::ProjectColumns {
                allow_empty: true,
                schema_known: schema.is_some(),
                context_hint,
                options: fields
                    .iter()
                    .filter(|field| {
                        aggregate_parameter_accepts(
                            node_type,
                            parameter.key.as_str(),
                            field.scalar_type,
                        )
                        .unwrap_or(true)
                    })
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
                let value = parameter_literal_value(parameter);
                parameter.configuration = Some(GraphParameterConfigurationFact::ProjectColumns {
                    allow_empty: false,
                    schema_known: schema.is_some(),
                    context_hint,
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
                let value = parameter_literal_value(parameter);
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
                    schema_known: schema.is_some(),
                    context_hint,
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
                    value: value
                        .filter(|value| prepare_filter_predicate_json(value).is_ok())
                        .map(Cow::into_owned),
                });
            }
            "core.text"
                if matches!(parameter.editor, ParameterEditorSpec::Select)
                    && matches!(
                        parameter.key.as_str(),
                        "column"
                            | "from"
                            | "entity_column"
                            | "time_column"
                            | "category_column"
                            | "value_column"
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

pub(super) fn parameter_literal_value(
    parameter: &GraphParameterFact,
) -> Option<Cow<'_, serde_json::Value>> {
    match &parameter.effective_value {
        Some(GraphResolvedParameterValue::Literal(value)) => Some(Cow::Borrowed(value)),
        Some(GraphResolvedParameterValue::DefaultLiteral(value)) => Some(Cow::Owned(
            yss_node_protocol::parameter_value_to_json(value, &parameter.value_type),
        )),
        _ => None,
    }
}

fn parameter_fact(
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

pub(crate) fn effective_parameter_value<'a>(
    node: &'a DocumentNode,
    parameter: &yss_node_protocol::Parameter,
) -> Option<Cow<'a, serde_json::Value>> {
    node.parameters
        .get(&parameter.key)
        .map(Cow::Borrowed)
        .or_else(|| parameter.default_json().map(Cow::Owned))
}

pub(crate) fn effective_json_parameter<'a>(
    node: &'a DocumentNode,
    key: &yss_node_protocol::ParameterKey,
    registry: &NodeRegistry,
) -> Option<Cow<'a, serde_json::Value>> {
    effective_parameter_value(node, applicable_parameter(node, key, registry)?)
}

pub(crate) fn effective_text_parameter<'a>(
    node: &'a DocumentNode,
    key: &yss_node_protocol::ParameterKey,
    registry: &'a NodeRegistry,
) -> Option<&'a str> {
    registry
        .protocol(&node.node_type)?
        .parameters
        .effective_text(key, &node.parameters)
}

fn applicable_parameter<'a>(
    node: &DocumentNode,
    key: &yss_node_protocol::ParameterKey,
    registry: &'a NodeRegistry,
) -> Option<&'a yss_node_protocol::Parameter> {
    let parameters = &registry.protocol(&node.node_type)?.parameters;
    parameters
        .get(key)
        .filter(|parameter| parameters.is_visible(parameter, &node.parameters))
}

/// Borrow the constant selected by the node's applicable ConstantOutput parameter.
/// Uses protocol defaults without writing them into the document.
pub fn referenced_constant<'a>(
    document: &'a GraphDocument,
    node: &DocumentNode,
    protocol: &NodeProtocol,
) -> Option<&'a yss_graph_document::GraphConstant> {
    let yss_node_protocol::NodeTypingSpec::ConstantOutput { parameter, .. } = &protocol.typing
    else {
        return None;
    };
    let id = protocol
        .parameters
        .effective_text(parameter, &node.parameters)?
        .parse()
        .ok()?;
    document.constants.get(&id)
}

pub(super) fn validate_node_parameters(
    node: &DocumentNode,
    protocol: &NodeProtocol,
    parameters: &[GraphParameterFact],
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    diagnostics: &mut Vec<GraphDiagnosticFact>,
) {
    for issue in validate_parameter_values(protocol, &node.parameters, registry) {
        let kind = match issue.kind {
            ParameterIssueKind::Unknown => GraphDiagnosticKind::ParameterUnknown,
            ParameterIssueKind::Required => GraphDiagnosticKind::ParameterRequired,
            ParameterIssueKind::InvalidType
            | ParameterIssueKind::Constraint
            | ParameterIssueKind::InvalidNominal(_)
            | ParameterIssueKind::InvalidResourceId => GraphDiagnosticKind::ParameterInvalid,
        };
        diagnostics.push(graph_problem(
            kind,
            GraphDiagnosticLocation::Parameter {
                node_id: node.id,
                key: issue.key.clone(),
            },
            [("parameter_key", issue.key.as_str().into())],
        ));
    }
    for parameter in parameters {
        let (
            ParameterEditorSpec::Resource { kind },
            Some(GraphResolvedParameterValue::Resource(identity)),
        ) = (&parameter.editor, &parameter.effective_value)
        else {
            continue;
        };
        let identity = identity.as_str();
        if resource_exists(resources, *kind, identity) {
            continue;
        }
        diagnostics.push(graph_problem(
            GraphDiagnosticKind::ResourceResolutionFailed,
            GraphDiagnosticLocation::Resource(identity.into()),
            [("resource_key", identity.into())],
        ));
    }
}

fn resource_exists(
    resources: &ResourceCatalogSnapshot,
    kind: ResourceDisplayKind,
    identity: &str,
) -> bool {
    match kind {
        ResourceDisplayKind::Function => GraphResourcePath::new(identity)
            .ok()
            .is_some_and(|path| resources.function_signature(&path).is_some()),
        ResourceDisplayKind::Database => resources
            .database_schema(&GraphResourceId::new(identity))
            .is_some(),
    }
}

pub(super) fn project_parameter_groups(protocol: &NodeProtocol) -> Box<[GraphParameterGroupFact]> {
    protocol
        .parameters
        .groups
        .iter()
        .map(|group| GraphParameterGroupFact {
            key: group.key.clone(),
            title: group.title_key.as_str().into(),
            description: group
                .description_key
                .as_ref()
                .map(|key| key.as_str().into()),
        })
        .collect()
}

pub(super) fn project_node_parameters(
    node: &DocumentNode,
    protocol: &NodeProtocol,
) -> Box<[GraphParameterFact]> {
    project_parameter_values(protocol, &node.parameters)
}

/// Project a definition and explicit form values without a document or invented input schema.
pub fn project_parameter_form(
    protocol: &NodeProtocol,
    values: &yss_node_protocol::ParameterValues,
) -> (Box<[GraphParameterGroupFact]>, Box<[GraphParameterFact]>) {
    let mut parameters = project_parameter_values(protocol, values);
    project_parameter_context(protocol.type_id.as_str(), &[], &mut parameters);
    (project_parameter_groups(protocol), parameters)
}

fn project_parameter_values(
    protocol: &NodeProtocol,
    values: &yss_node_protocol::ParameterValues,
) -> Box<[GraphParameterFact]> {
    protocol
        .parameters
        .groups
        .iter()
        .flat_map(|group| {
            group
                .parameters
                .iter()
                .map(move |parameter| (group, parameter))
        })
        .filter(|(_, parameter)| protocol.parameters.is_visible(parameter, values))
        .map(|(group, parameter)| {
            parameter_fact(
                &group.key,
                parameter,
                project_parameter_value(parameter, values),
            )
        })
        .collect()
}

fn project_parameter_value(
    parameter: &yss_node_protocol::Parameter,
    values: &yss_node_protocol::ParameterValues,
) -> Option<GraphResolvedParameterValue> {
    if let Some(value) = values.get(&parameter.key) {
        return Some(match (&parameter.editor, value.as_str()) {
            (ParameterEditorSpec::Resource { .. }, Some(identity)) => {
                GraphResolvedParameterValue::Resource(GraphResourceId::new(identity))
            }
            _ => GraphResolvedParameterValue::Literal(value.clone()),
        });
    }

    let default = parameter.default_value.as_ref()?;
    if matches!(parameter.editor, ParameterEditorSpec::Resource { .. })
        && let Some(value) = parameter.default_json()
        && let Some(identity) = value.as_str()
    {
        return Some(GraphResolvedParameterValue::Resource(GraphResourceId::new(
            identity,
        )));
    }
    Some(GraphResolvedParameterValue::DefaultLiteral(
        default.value.clone(),
    ))
}
