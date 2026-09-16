use super::model::*;
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::ValueType;
use yss_graph_analysis::{
    GraphDiagnosticFact, GraphNodeSemanticFact, GraphParameterConfigurationFact,
    GraphParameterFact, GraphPortBacking, GraphPortInstanceAdditionFact, GraphPortSemanticFact,
    GraphResolutionOutcome, GraphSemanticSnapshot,
};
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_graph_document::{GraphDocument, NodeId, PortAddress, PortRef};
use yss_node_protocol::{
    ParameterEditorSpec, PortDirection, ResolvedType, TypeDomain, TypeExpr, TypeState,
};

pub fn build_editor_projection(
    input: EditorProjectionInput<'_>,
) -> Result<EditorProjectionModel, EditorProjectionError> {
    if input.analysis.registry_fingerprint() != &input.registry_fingerprint {
        return Err(EditorProjectionError::RegistryMismatch);
    }

    let snapshot = input.analysis.semantic_snapshot();
    if matches!(
        snapshot.outcome(),
        GraphResolutionOutcome::InternalFailure { .. }
    ) {
        return Err(EditorProjectionError::ResolutionFailed);
    }
    let node_semantics = validate_semantic_snapshot(input.document, snapshot)?;
    let diagnostics = diagnostics_by_node(snapshot.diagnostics(), input.document);

    let nodes = input
        .document
        .nodes
        .values()
        .map(|node| {
            let node_semantics = node_semantics
                .get(&node.id)
                .ok_or(EditorProjectionError::SemanticSnapshotMismatch)?;
            project_node(
                node,
                node_semantics,
                diagnostics
                    .get(&node.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                input.document,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let connections = input
        .document
        .connections
        .values()
        .map(|connection| EditorConnectionModel {
            connection_id: connection.id,
            output: connection.output.clone(),
            input: connection.input.clone(),
            order: connection.order.as_ref().map(|order| order.as_str().into()),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let diagnostics = snapshot
        .diagnostics()
        .iter()
        .map(project_diagnostic)
        .collect::<Vec<_>>()
        .into_boxed_slice();

    Ok(EditorProjectionModel {
        basis: EditorProjectionBasis {
            graph_path: input.graph_path.clone(),
            registry_fingerprint: input.registry_fingerprint,
            semantic_input_hash: *input.analysis.semantic_input_hash(),
            resource_versions: input.analysis.resource_versions().clone(),
            resource_observations: input.analysis.resource_observations().clone(),
        },
        graph_path: input.graph_path.clone(),
        nodes,
        connections,
        diagnostics,
        outcome: project_outcome(snapshot.outcome()),
    })
}

fn validate_semantic_snapshot<'a>(
    document: &GraphDocument,
    snapshot: &'a GraphSemanticSnapshot,
) -> Result<BTreeMap<NodeId, &'a GraphNodeSemanticFact>, EditorProjectionError> {
    let mut connection_counts = BTreeMap::<&PortAddress, u32>::new();
    for connection in document.connections.values() {
        *connection_counts.entry(&connection.input).or_default() += 1;
        if connection.output != connection.input {
            *connection_counts.entry(&connection.output).or_default() += 1;
        }
    }
    let mut node_ids = BTreeMap::new();
    for node in snapshot.nodes() {
        let Some(document_node) = document.nodes.get(&node.node_id) else {
            return Err(EditorProjectionError::SemanticSnapshotMismatch);
        };
        if document_node.node_type != node.node_type
            || node_ids.insert(node.node_id, node).is_some()
        {
            return Err(EditorProjectionError::SemanticSnapshotMismatch);
        }
        for port in &node.ports {
            if port.address.node_id != node.node_id
                || !port_fact_has_concrete_address(port, document)
                || connection_counts
                    .get(&port.address)
                    .copied()
                    .unwrap_or_default()
                    != port.connections.current
            {
                return Err(EditorProjectionError::SemanticSnapshotMismatch);
            }
        }
        let mut addition_keys = BTreeSet::new();
        if node
            .port_instance_additions
            .iter()
            .any(|addition| !addition_keys.insert(&addition.template_key))
        {
            return Err(EditorProjectionError::SemanticSnapshotMismatch);
        }
    }
    if node_ids.len() != document.nodes.len() {
        return Err(EditorProjectionError::SemanticSnapshotMismatch);
    }
    Ok(node_ids)
}

fn project_node(
    node: &yss_graph_document::DocumentNode,
    facts: &GraphNodeSemanticFact,
    diagnostics: &[&GraphDiagnosticFact],
    document: &GraphDocument,
) -> Result<EditorNodeModel, EditorProjectionError> {
    let parameters = facts
        .parameters
        .iter()
        .filter_map(project_parameter)
        .map(|mut parameter| {
            parameter.value = node
                .parameters
                .get(&parameter.key)
                .cloned()
                .or(parameter.value);
            parameter
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let ports = facts
        .ports
        .iter()
        .map(|port| project_port(port, document))
        .collect::<Result<Vec<_>, _>>()?;
    let port_instance_additions = facts
        .port_instance_additions
        .iter()
        .map(project_port_instance_addition)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let capabilities = EditorNodeCapabilities {
        managed: facts.managed,
        can_copy: !facts.managed,
        can_delete: !facts.managed,
        can_edit_label: true,
        can_edit_parameters: facts
            .parameters
            .iter()
            .any(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden)),
        supports_inline_literals: facts.ports.iter().any(|port| {
            matches!(
                port.editor,
                yss_graph_analysis::GraphPortEditorFact::InlineLiteral
            )
        }),
    };
    Ok(EditorNodeModel {
        node_id: node.id,
        node_type: node.node_type.clone(),
        position: node.position,
        display: EditorNodeDisplay {
            title: facts
                .instance_title
                .clone()
                .unwrap_or_else(|| facts.title.clone()),
            user_label: node.user_label.as_deref().map(Into::into),
            icon_id: facts.icon_id.clone(),
            style_id: facts.style_id.clone(),
        },
        ports: ports.into_boxed_slice(),
        port_instance_additions,
        parameters,
        capabilities,
        diagnostics: diagnostics
            .iter()
            .map(|diagnostic| project_diagnostic(diagnostic))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    })
}

fn project_port(
    port: &GraphPortSemanticFact,
    document: &GraphDocument,
) -> Result<EditorPortModel, EditorProjectionError> {
    let input = (port.direction == PortDirection::Input).then(|| {
        let literal_override = document
            .input_states
            .get(&port.address)
            .and_then(|state| state.literal_override.as_ref())
            .map(|value| yss_node_protocol::protocol_value_to_json(&value.value));
        let effective = if port.connections.current > 0 {
            EditorEffectiveInputBinding::Connections
        } else if literal_override.is_some() {
            EditorEffectiveInputBinding::Literal
        } else if port.protocol_default.is_some() {
            EditorEffectiveInputBinding::ProtocolDefault
        } else {
            EditorEffectiveInputBinding::Unbound
        };
        EditorInputBinding {
            literal_override,
            protocol_default: port
                .protocol_default
                .as_ref()
                .map(|value| yss_node_protocol::protocol_value_to_json(&value.value)),
            effective,
        }
    });
    let current = port.connections.current;
    let connections = EditorPortConnectionCapabilities {
        current,
        maximum: port.connections.maximum,
        ordered: port.connections.ordered,
        can_append: !port.orphan
            && port
                .connections
                .maximum
                .is_none_or(|maximum| current < maximum),
        can_replace: !port.orphan && port.connections.maximum == Some(1) && current == 1,
        can_move: !port.orphan && current > 0,
    };
    Ok(EditorPortModel {
        address: port.address.clone(),
        display: EditorPortDisplay {
            label: port
                .instance_label
                .clone()
                .unwrap_or_else(|| port.label.clone()),
            instance_label: port.instance_label.clone(),
        },
        direction: port.direction,
        orphan: port.orphan,
        can_remove: port.can_remove,
        connections,
        input,
        accepted_type: project_accepted_type(&port.accepted_type, port.accepted_domain.as_ref()),
        type_state: project_type_state(&port.type_state),
        resolved_schema: port
            .schema_state
            .exact()
            .map(|fact| project_schema_summary(&fact.expression, Some(fact))),
        status: if port.orphan {
            EditorPortStatus::Orphan
        } else {
            EditorPortStatus::Resolved
        },
    })
}

fn port_fact_has_concrete_address(port: &GraphPortSemanticFact, document: &GraphDocument) -> bool {
    match (&port.backing, &port.address.port) {
        (GraphPortBacking::Declared, PortRef::Declared { .. }) => !port.orphan && !port.can_remove,
        (GraphPortBacking::DocumentInstance, PortRef::Instance { .. }) => {
            match document.port_bindings.get(&port.address) {
                Some(yss_graph_document::DynamicPortBinding::UserCreated { .. }) => !port.orphan,
                Some(yss_graph_document::DynamicPortBinding::Resolved { .. }) => {
                    !port.orphan && !port.can_remove
                }
                Some(yss_graph_document::DynamicPortBinding::Orphan { .. }) => {
                    port.orphan && port.can_remove
                }
                None => false,
            }
        }
        (GraphPortBacking::ProjectedDerived { .. }, PortRef::Instance { .. }) => {
            !document.port_bindings.contains_key(&port.address) && !port.orphan && !port.can_remove
        }
        _ => false,
    }
}

fn project_port_instance_addition(
    addition: &GraphPortInstanceAdditionFact,
) -> EditorPortInstanceAdditionModel {
    EditorPortInstanceAdditionModel {
        template_key: addition.template_key.clone(),
        label: addition.label.clone(),
        direction: addition.direction,
        can_add: addition.can_add,
    }
}

fn project_parameter(fact: &GraphParameterFact) -> Option<EditorParameterModel> {
    let (editor, multiline) = match fact.editor {
        ParameterEditorSpec::Auto => (ParameterEditorKind::Auto, false),
        ParameterEditorSpec::Hidden => return None,
        ParameterEditorSpec::Text { multiline } => (ParameterEditorKind::Text, multiline),
        ParameterEditorSpec::Number => (ParameterEditorKind::Number, false),
        ParameterEditorSpec::Toggle => (ParameterEditorKind::Toggle, false),
        ParameterEditorSpec::Configuration(_) => (ParameterEditorKind::Configuration, false),
        ParameterEditorSpec::Select => (ParameterEditorKind::Select, false),
        ParameterEditorSpec::GraphConstant => (ParameterEditorKind::GraphConstant, false),
        ParameterEditorSpec::Resource { .. } => (ParameterEditorKind::Resource, false),
    };
    Some(EditorParameterModel {
        key: fact.key.clone(),
        display: EditorParameterDisplay {
            title: fact.title.clone(),
            description: fact.description.clone(),
        },
        editor,
        presentation: fact.presentation,
        value_type: data_type_for(&fact.value_type),
        multiline,
        value: fact.effective_value.as_ref().map(|value| match value {
            yss_graph_analysis::GraphResolvedParameterValue::Literal(value) => value.clone(),
            yss_graph_analysis::GraphResolvedParameterValue::DefaultLiteral(value) => {
                yss_node_protocol::protocol_value_to_json(value)
            }
            yss_graph_analysis::GraphResolvedParameterValue::Resource(identity) => {
                serde_json::Value::String(identity.as_str().to_owned())
            }
        }),
        configuration: fact.configuration.as_ref().map(project_configuration),
    })
}

fn project_configuration(fact: &GraphParameterConfigurationFact) -> EditorParameterConfiguration {
    match fact {
        GraphParameterConfigurationFact::Configuration { fields } => {
            EditorParameterConfiguration::Configuration {
                fields: fields.iter().filter_map(project_parameter).collect(),
            }
        }
        GraphParameterConfigurationFact::SelectOptions { options } => {
            EditorParameterConfiguration::SelectOptions {
                options: options.clone(),
            }
        }
        GraphParameterConfigurationFact::ProjectColumns {
            available,
            unavailable_reason,
            options,
            value,
        } => EditorParameterConfiguration::ProjectColumns {
            available: *available,
            unavailable_reason: unavailable_reason.clone(),
            options: options
                .iter()
                .map(|option| EditorColumnOption {
                    name: option.name.clone(),
                    data_type: option.data_type,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            value: value.clone(),
        },
        GraphParameterConfigurationFact::FilterPredicate {
            available,
            unavailable_reason,
            columns,
            value,
        } => EditorParameterConfiguration::FilterPredicate {
            available: *available,
            unavailable_reason: unavailable_reason.clone(),
            columns: columns
                .iter()
                .map(|column| EditorFilterColumnOption {
                    name: column.name.clone(),
                    data_type: column.data_type,
                    operators: column.operators.clone(),
                    literal_types: column
                        .literal_types
                        .iter()
                        .copied()
                        .map(project_filter_literal_type)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            value: value.clone(),
        },
    }
}

fn project_filter_literal_type(
    value: yss_graph_analysis::GraphFilterLiteralType,
) -> EditorFilterLiteralType {
    match value {
        yss_graph_analysis::GraphFilterLiteralType::Boolean => EditorFilterLiteralType::Boolean,
        yss_graph_analysis::GraphFilterLiteralType::Integer => EditorFilterLiteralType::Integer,
        yss_graph_analysis::GraphFilterLiteralType::Decimal => EditorFilterLiteralType::Decimal,
        yss_graph_analysis::GraphFilterLiteralType::String => EditorFilterLiteralType::String,
    }
}

fn project_accepted_type(value: &TypeExpr, domain: Option<&TypeDomain>) -> EditorAcceptedType {
    EditorAcceptedType {
        display: type_display(value).into(),
        domain: domain.map(|domain| {
            domain
                .types()
                .iter()
                .filter_map(yss_graph_type_mapping::data_type_from_resolved_type)
                .collect::<Vec<_>>()
                .into_boxed_slice()
        }),
    }
}

fn project_type_state(value: &TypeState) -> EditorPortTypeState {
    match value {
        TypeState::Exact(value) => EditorPortTypeState::Exact {
            display: resolved_type_display(value).into(),
            data_type: yss_graph_type_mapping::data_type_from_resolved_type(value),
        },
        TypeState::Constrained(domain) => EditorPortTypeState::Constrained {
            display: resolved_domain_display(domain).into(),
            domain: domain
                .types()
                .iter()
                .filter_map(yss_graph_type_mapping::data_type_from_resolved_type)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
        TypeState::Unknown(reason) => EditorPortTypeState::Unknown { reason: *reason },
        TypeState::Conflict(conflict) => EditorPortTypeState::Conflict {
            conflict: *conflict,
        },
    }
}

fn resolved_domain_display(value: &TypeDomain) -> String {
    value
        .types()
        .iter()
        .map(resolved_type_display)
        .collect::<Vec<_>>()
        .join(" | ")
}

fn resolved_type_display(value: &ResolvedType) -> String {
    if let Some(value_type) = yss_graph_type_mapping::data_type_from_resolved_type(value) {
        return value_type.to_string();
    }
    match value {
        ResolvedType::Nominal(id) => id.as_str().to_owned(),
        ResolvedType::Applied {
            constructor,
            arguments,
        } => format!(
            "{}<{}>",
            constructor.as_str(),
            arguments
                .iter()
                .map(resolved_type_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn project_schema_summary(
    expression: &yss_node_protocol::SchemaExpr,
    resolved: Option<&yss_node_protocol::ResolvedSchemaFact>,
) -> EditorSchemaSummary {
    let kind = match expression {
        yss_node_protocol::SchemaExpr::Input(_) => EditorSchemaSummaryKind::Input,
        yss_node_protocol::SchemaExpr::Project { .. } => EditorSchemaSummaryKind::Project,
        yss_node_protocol::SchemaExpr::Append { .. } => EditorSchemaSummaryKind::Append,
        yss_node_protocol::SchemaExpr::Rename { .. } => EditorSchemaSummaryKind::Rename,
        yss_node_protocol::SchemaExpr::Filter { .. } => EditorSchemaSummaryKind::Filter,
        yss_node_protocol::SchemaExpr::Derived { .. } => EditorSchemaSummaryKind::Derived,
    };
    let fields = resolved
        .into_iter()
        .flat_map(|fact| fact.fields.iter())
        .map(|field| EditorSchemaField {
            name: field.name.0.clone(),
            scalar_type: field.scalar_type,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    EditorSchemaSummary { kind, fields }
}

fn project_diagnostic(fact: &GraphDiagnosticFact) -> EditorDiagnosticModel {
    EditorDiagnosticModel {
        code: fact.code.as_str().into(),
        message_key: fact.message_key.clone(),
        severity: fact.severity.into(),
        blocking: fact.blocking,
        arguments: fact.arguments.clone(),
        location: fact.primary.clone(),
        related: fact.related.clone(),
    }
}

fn project_outcome(value: &GraphResolutionOutcome) -> EditorResolutionOutcome {
    match value {
        GraphResolutionOutcome::Complete => EditorResolutionOutcome::Complete,
        GraphResolutionOutcome::Incomplete => EditorResolutionOutcome::Incomplete,
        GraphResolutionOutcome::InternalFailure {
            stage,
            code,
            node_id,
        } => EditorResolutionOutcome::InternalFailure {
            stage: match stage {
                yss_graph_analysis::GraphResolutionStage::Analysis => {
                    EditorResolutionStage::Analysis
                }
            },
            code: code.clone(),
            node_id: *node_id,
        },
    }
}

fn diagnostics_by_node<'a>(
    diagnostics: &'a [GraphDiagnosticFact],
    document: &GraphDocument,
) -> BTreeMap<NodeId, Vec<&'a GraphDiagnosticFact>> {
    let mut by_node = BTreeMap::<_, Vec<_>>::new();
    for diagnostic in diagnostics {
        let nodes = match &diagnostic.primary {
            DiagnosticLocation::Node(id) | DiagnosticLocation::Parameter { node_id: id, .. } => {
                [Some(*id), None]
            }
            DiagnosticLocation::Port(address) => [Some(address.node_id), None],
            DiagnosticLocation::Connection(connection_id) => document
                .connections
                .get(connection_id)
                .map(|connection| {
                    [
                        Some(connection.input.node_id),
                        (connection.input.node_id != connection.output.node_id)
                            .then_some(connection.output.node_id),
                    ]
                })
                .unwrap_or_default(),
            DiagnosticLocation::Graph | DiagnosticLocation::Resource(_) => [None, None],
        };
        for node in nodes.into_iter().flatten() {
            by_node.entry(node).or_default().push(diagnostic);
        }
    }
    by_node
}

fn data_type_for(value: &TypeExpr) -> Option<ValueType> {
    match value {
        TypeExpr::Concrete(id) => yss_graph_type_mapping::data_type_from_resolved_type(
            &yss_node_protocol::ResolvedType::Nominal(id.clone()),
        ),
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" && arguments.len() == 1 => {
            data_type_for(&arguments[0]).map(|element| ValueType::DataSeries(Box::new(element)))
        }
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.array" && arguments.len() == 1 => {
            data_type_for(&arguments[0]).map(|element| ValueType::Array(Box::new(element)))
        }
        TypeExpr::Applied { .. } => None,
        TypeExpr::Union(values) if !values.is_empty() => values
            .iter()
            .map(data_type_for)
            .collect::<Option<Vec<_>>>()
            .map(ValueType::one_of),
        TypeExpr::Class(_) | TypeExpr::Generic(_) | TypeExpr::Unknown | TypeExpr::Union(_) => None,
    }
}

fn type_display(value: &TypeExpr) -> String {
    if let Some(value_type) = data_type_for(value) {
        return value_type.to_string();
    }
    match value {
        TypeExpr::Concrete(id) => id.as_str().to_owned(),
        TypeExpr::Class(id) => id.as_str().to_owned(),
        TypeExpr::Generic(id) => id.as_str().to_owned(),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => format!(
            "{}<{}>",
            constructor.as_str(),
            arguments
                .iter()
                .map(type_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Union(values) => values
            .iter()
            .map(type_display)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeExpr::Unknown => "unknown".to_owned(),
    }
}
