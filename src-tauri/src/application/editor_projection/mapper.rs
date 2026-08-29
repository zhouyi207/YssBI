use super::model::*;
use crate::data_contract::DataType;
use crate::graph::analysis::contracts::DiagnosticLocation;
use crate::graph::analysis::{
    GraphCompilationOutcome, GraphDiagnosticFact, GraphNodeProjectionFacts,
    GraphParameterConfigurationFact, GraphParameterFact, GraphPortFact, GraphPortInstanceKind,
    GraphProjectionFacts,
};
use crate::graph::protocol::{ParameterEditorSpec, PortDirection, TypeExpr};
use crate::graph_document::{GraphDocument, GraphRevision, NodeId, PortAddress};
use std::collections::BTreeMap;

pub fn build_editor_projection(
    input: EditorProjectionInput<'_>,
) -> Result<EditorProjectionModel, EditorProjectionError> {
    let analysis_revision = GraphRevision::new(input.analysis.graph_revision());
    if analysis_revision != input.document.revision {
        return Err(EditorProjectionError::RevisionMismatch {
            analysis: analysis_revision,
            document: input.document.revision,
        });
    }
    if input.analysis.registry_fingerprint() != &input.registry_fingerprint {
        return Err(EditorProjectionError::RegistryMismatch);
    }

    let empty_facts = GraphProjectionFacts::new([], [], GraphCompilationOutcome::Complete);
    let facts = input.analysis.projection_facts().unwrap_or(&empty_facts);
    validate_facts(input.document, input.analysis, facts)?;

    let nodes = input
        .document
        .nodes
        .values()
        .map(|node| {
            let node_facts = facts
                .nodes()
                .iter()
                .find(|facts| facts.node_id == node.id)
                .ok_or(EditorProjectionError::ProjectionFactsMismatch)?;
            project_node(node, node_facts, facts.diagnostics(), input.document)
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
    let diagnostics = facts
        .diagnostics()
        .iter()
        .map(project_diagnostic)
        .collect::<Vec<_>>()
        .into_boxed_slice();

    Ok(EditorProjectionModel {
        basis: EditorProjectionBasis {
            graph_path: input.graph_path.clone(),
            graph_revision: input.document.revision,
            registry_fingerprint: input.registry_fingerprint,
            resource_versions: input.analysis.resource_versions().clone(),
        },
        graph_path: input.graph_path.clone(),
        source_revision: input.document.revision,
        nodes,
        connections,
        diagnostics,
        outcome: project_outcome(facts.outcome()),
    })
}

fn validate_facts(
    document: &GraphDocument,
    analysis: &crate::graph::analysis::GraphAnalysis,
    facts: &GraphProjectionFacts,
) -> Result<(), EditorProjectionError> {
    if analysis.nodes().len() != document.nodes.len()
        || analysis.nodes().iter().any(|analysis_node| {
            document
                .nodes
                .get(&analysis_node.node_id)
                .is_none_or(|node| node.node_type != analysis_node.node_type)
        })
    {
        return Err(EditorProjectionError::ProjectionFactsMismatch);
    }

    let mut node_ids = BTreeMap::new();
    for node in facts.nodes() {
        let Some(document_node) = document.nodes.get(&node.node_id) else {
            return Err(EditorProjectionError::ProjectionFactsMismatch);
        };
        if document_node.node_type != node.node_type || node_ids.insert(node.node_id, ()).is_some()
        {
            return Err(EditorProjectionError::ProjectionFactsMismatch);
        }
        for port in &node.ports {
            if port.address.node_id != node.node_id
                || count_connections(document, &port.address) != port.connections.current
            {
                return Err(EditorProjectionError::ProjectionFactsMismatch);
            }
        }
    }
    if node_ids.len() != document.nodes.len() {
        return Err(EditorProjectionError::ProjectionFactsMismatch);
    }
    Ok(())
}

fn project_node(
    node: &crate::graph_document::DocumentNode,
    facts: &GraphNodeProjectionFacts,
    diagnostics: &[GraphDiagnosticFact],
    document: &GraphDocument,
) -> Result<EditorNodeModel, EditorProjectionError> {
    let parameters = facts
        .parameters
        .iter()
        .filter_map(project_parameter)
        .map(|mut parameter| {
            parameter.value = node.parameters.get(&parameter.key).cloned();
            parameter
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let ports = facts
        .ports
        .iter()
        .map(|port| project_port(port, document))
        .collect::<Result<Vec<_>, _>>()?;
    let capabilities = EditorNodeCapabilities {
        managed: facts.managed,
        can_copy: !facts.managed,
        can_delete: !facts.managed,
        can_edit_label: true,
        can_edit_parameters: facts
            .parameters
            .iter()
            .any(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden)),
        has_dynamic_ports: facts.ports.iter().any(|port| {
            matches!(
                port.instance_kind,
                GraphPortInstanceKind::UserCreated | GraphPortInstanceKind::Derived
            )
        }),
        supports_inline_literals: facts.ports.iter().any(|port| {
            matches!(
                port.editor,
                crate::graph::analysis::GraphPortEditorFact::InlineLiteral
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
        parameters,
        capabilities,
        diagnostics: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic_belongs_to_node(diagnostic, node.id, document))
            .map(project_diagnostic)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    })
}

fn project_port(
    port: &GraphPortFact,
    document: &GraphDocument,
) -> Result<EditorPortModel, EditorProjectionError> {
    let input = (port.direction == PortDirection::Input).then(|| {
        let literal_override = document
            .input_states
            .get(&port.address)
            .and_then(|state| state.literal_override.clone());
        let effective = if has_connection(document, &port.address) {
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
            protocol_default: port.protocol_default.clone(),
            effective,
        }
    });
    let can_remove = port.orphan && port.address.is_instance()
        || matches!(port.instance_kind, GraphPortInstanceKind::UserCreated)
            && (!port.member_complete
                || port.member_instance_count > usize::from(port.member_minimum));
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
        template_key: port.template_key.clone(),
        display: EditorPortDisplay {
            label: port
                .instance_label
                .clone()
                .unwrap_or_else(|| port.label.clone()),
            instance_label: port.instance_label.clone(),
        },
        direction: port.direction,
        kind: port.kind,
        instance_kind: project_instance_kind(port.instance_kind),
        orphan: port.orphan,
        can_remove,
        connections,
        input,
        resolved_type: Some(project_type_summary(&port.value_type)),
        resolved_schema: port
            .schema
            .as_ref()
            .map(|expression| project_schema_summary(expression, port.resolved_schema.as_ref())),
        status: if port.orphan {
            EditorPortStatus::Orphan
        } else {
            EditorPortStatus::Resolved
        },
    })
}

fn project_parameter(fact: &GraphParameterFact) -> Option<EditorParameterModel> {
    let (editor, multiline) = match fact.editor {
        ParameterEditorSpec::Auto => (ParameterEditorKind::Auto, false),
        ParameterEditorSpec::Hidden => return None,
        ParameterEditorSpec::Text { multiline } => (ParameterEditorKind::Text, multiline),
        ParameterEditorSpec::Number => (ParameterEditorKind::Number, false),
        ParameterEditorSpec::Toggle => (ParameterEditorKind::Toggle, false),
        ParameterEditorSpec::Select => (ParameterEditorKind::Select, false),
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
        value: None,
        configuration: fact.configuration.as_ref().map(project_configuration),
        inherited_value: fact.inherited_value.clone(),
        value_source: fact.value_source.map(|source| match source {
            crate::graph::analysis::GraphParameterValueSource::Project => {
                EditorParameterValueSource::Project
            }
            crate::graph::analysis::GraphParameterValueSource::Node => {
                EditorParameterValueSource::Node
            }
        }),
        options: (!fact.options.is_empty()).then(|| fact.options.clone()),
    })
}

fn project_configuration(fact: &GraphParameterConfigurationFact) -> EditorParameterConfiguration {
    match fact {
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
    value: crate::graph::analysis::GraphFilterLiteralType,
) -> EditorFilterLiteralType {
    match value {
        crate::graph::analysis::GraphFilterLiteralType::Boolean => EditorFilterLiteralType::Boolean,
        crate::graph::analysis::GraphFilterLiteralType::Integer => EditorFilterLiteralType::Integer,
        crate::graph::analysis::GraphFilterLiteralType::Decimal => EditorFilterLiteralType::Decimal,
        crate::graph::analysis::GraphFilterLiteralType::String => EditorFilterLiteralType::String,
    }
}

fn project_instance_kind(value: GraphPortInstanceKind) -> EditorPortInstanceKind {
    match value {
        GraphPortInstanceKind::Declared => EditorPortInstanceKind::Declared,
        GraphPortInstanceKind::UserCreated => EditorPortInstanceKind::UserCreated,
        GraphPortInstanceKind::Derived => EditorPortInstanceKind::Derived,
    }
}

fn project_type_summary(value: &TypeExpr) -> EditorTypeSummary {
    EditorTypeSummary {
        display: type_display(value).into(),
        resolved: type_is_resolved(value),
        data_type: data_type_for(value),
        internal_type_expr: value.clone(),
    }
}

fn project_schema_summary(
    expression: &crate::graph::protocol::SchemaExpr,
    resolved: Option<&crate::graph::protocol::ResolvedSchemaFact>,
) -> EditorSchemaSummary {
    let kind = match expression {
        crate::graph::protocol::SchemaExpr::Input(_) => EditorSchemaSummaryKind::Input,
        crate::graph::protocol::SchemaExpr::Project { .. } => EditorSchemaSummaryKind::Project,
        crate::graph::protocol::SchemaExpr::Append { .. } => EditorSchemaSummaryKind::Append,
        crate::graph::protocol::SchemaExpr::Rename { .. } => EditorSchemaSummaryKind::Rename,
        crate::graph::protocol::SchemaExpr::Filter { .. } => EditorSchemaSummaryKind::Filter,
        crate::graph::protocol::SchemaExpr::Derived { .. } => EditorSchemaSummaryKind::Derived,
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
        severity: fact.severity.into(),
        arguments: fact.arguments.clone(),
        location: fact.primary.clone(),
        related: fact.related.clone(),
    }
}

fn project_outcome(value: &GraphCompilationOutcome) -> EditorCompilationOutcome {
    match value {
        GraphCompilationOutcome::Complete => EditorCompilationOutcome::Complete,
        GraphCompilationOutcome::Incomplete => EditorCompilationOutcome::Incomplete,
        GraphCompilationOutcome::InternalFailure {
            stage,
            code,
            node_id,
        } => EditorCompilationOutcome::InternalFailure {
            stage: match stage {
                crate::graph::analysis::GraphCompilationStage::Analysis => {
                    EditorCompilationStage::Analysis
                }
                crate::graph::analysis::GraphCompilationStage::Lowering => {
                    EditorCompilationStage::Lowering
                }
            },
            code: code.clone(),
            node_id: *node_id,
        },
    }
}

fn diagnostic_belongs_to_node(
    diagnostic: &GraphDiagnosticFact,
    node_id: NodeId,
    document: &GraphDocument,
) -> bool {
    match &diagnostic.primary {
        DiagnosticLocation::Node(id) | DiagnosticLocation::Parameter { node_id: id, .. } => {
            *id == node_id
        }
        DiagnosticLocation::Port(address) => address.node_id == node_id,
        DiagnosticLocation::Connection(connection_id) => document
            .connections
            .get(connection_id)
            .is_some_and(|connection| {
                connection.input.node_id == node_id || connection.output.node_id == node_id
            }),
        DiagnosticLocation::Graph | DiagnosticLocation::Resource(_) => false,
    }
}

fn has_connection(document: &GraphDocument, address: &PortAddress) -> bool {
    document
        .connections
        .values()
        .any(|connection| connection.input == *address || connection.output == *address)
}

fn count_connections(document: &GraphDocument, address: &PortAddress) -> u32 {
    document
        .connections
        .values()
        .filter(|connection| connection.input == *address || connection.output == *address)
        .count() as u32
}

fn data_type_for(value: &TypeExpr) -> Option<DataType> {
    match value {
        TypeExpr::Concrete(id) => Some(match id.as_str() {
            "core.bool" => DataType::Boolean,
            "core.int64" => DataType::Int64,
            "core.float64" => DataType::Float64,
            "core.string" => DataType::String,
            "core.date" => DataType::Date,
            "core.datetime" => DataType::Datetime,
            "core.time" => DataType::Time,
            "core.categorical" => DataType::Categorical,
            "core.object" => DataType::Object,
            "tabular.dataframe" => DataType::DataFrame,
            semantic_id => DataType::Struct(semantic_id.to_owned()),
        }),
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" && arguments.len() == 1 => {
            data_type_for(&arguments[0]).map(|element| DataType::DataSeries(Box::new(element)))
        }
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.array" && arguments.len() == 1 => {
            data_type_for(&arguments[0]).map(|element| DataType::Array(Box::new(element)))
        }
        TypeExpr::Applied { .. } => None,
        TypeExpr::Union(values) if !values.is_empty() => values
            .iter()
            .map(data_type_for)
            .collect::<Option<Vec<_>>>()
            .map(DataType::one_of),
        TypeExpr::Generic(_) | TypeExpr::Unknown | TypeExpr::Union(_) => None,
    }
}

fn type_display(value: &TypeExpr) -> String {
    match value {
        TypeExpr::Concrete(id) => id.as_str().to_owned(),
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

fn type_is_resolved(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::Concrete(_) => true,
        TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => {
            arguments.iter().all(type_is_resolved)
        }
        TypeExpr::Generic(_) | TypeExpr::Unknown => false,
    }
}
