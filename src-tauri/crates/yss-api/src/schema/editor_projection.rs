use yss_application::editor_projection::{
    EditorCompilationOutcome, EditorCompilationStage, EditorDiagnosticModel,
    EditorDiagnosticSeverity, EditorEffectiveInputBinding, EditorFilterLiteralType,
    EditorParameterConfiguration, EditorParameterModel, EditorParameterValueSource,
    EditorPortModel, EditorPortStatus, EditorPortTypeState, EditorProjectionModel,
    EditorSchemaSummary, EditorSchemaSummaryKind, ParameterEditorKind,
};
use yss_graph_registry::RegistryFingerprint;

pub use super::editor_projection_types::*;

impl From<&EditorProjectionModel> for EditorGraphProjectionDto {
    fn from(model: &EditorProjectionModel) -> Self {
        Self {
            basis: ProjectionBasis {
                graph_path: model.basis.graph_path.as_str().into(),
                registry_fingerprint: RegistryFingerprint::from_bytes(
                    model.basis.registry_fingerprint,
                ),
                resource_versions: model.basis.resource_versions.clone(),
            },
            graph_path: model.graph_path.as_str().into(),
            source_revision: model.source_revision.get(),
            nodes: model
                .nodes
                .iter()
                .map(|node| map_node(model, node))
                .collect(),
            connections: model
                .connections
                .iter()
                .map(|connection| EditorConnectionProjectionDto {
                    connection_id: connection.connection_id.to_string().into(),
                    output: (&connection.output).into(),
                    input: (&connection.input).into(),
                    order: connection.order.clone(),
                })
                .collect(),
            diagnostics: model.diagnostics.iter().map(map_diagnostic).collect(),
            outcome: map_outcome(&model.outcome),
            has_blocking_diagnostics: model
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == EditorDiagnosticSeverity::Error)
                || !matches!(model.outcome, EditorCompilationOutcome::Complete),
        }
    }
}

/// Explicit command-facing entry point for the sole editor wire mapper.
pub fn map_editor_projection(model: &EditorProjectionModel) -> EditorGraphProjectionDto {
    model.into()
}

fn map_node(
    model: &EditorProjectionModel,
    node: &yss_application::editor_projection::EditorNodeModel,
) -> EditorNodeProjectionDto {
    EditorNodeProjectionDto {
        graph_path: model.graph_path.as_str().into(),
        node_id: node.node_id.to_string().into(),
        node_type_id: node.node_type.as_str().into(),
        position: NodePositionDto {
            x: node.position.x,
            y: node.position.y,
        },
        display: NodeDisplayDto {
            title: node.display.title.clone(),
            user_label: node.display.user_label.clone(),
            icon_id: node.display.icon_id.clone(),
            style_id: node.display.style_id.clone(),
        },
        ports: node.ports.iter().map(map_port).collect(),
        port_instance_additions: node
            .port_instance_additions
            .iter()
            .map(|addition| PortInstanceAdditionDto {
                template_key: addition.template_key.as_str().into(),
                label: addition.label.clone(),
                direction: map_port_direction(addition.direction),
                can_add: addition.can_add,
            })
            .collect(),
        parameter_editors: node.parameters.iter().map(map_parameter).collect(),
        capabilities: NodeCapabilitiesDto {
            managed: node.capabilities.managed,
            can_copy: node.capabilities.can_copy,
            can_delete: node.capabilities.can_delete,
            can_edit_label: node.capabilities.can_edit_label,
            can_edit_parameters: node.capabilities.can_edit_parameters,
            supports_inline_literals: node.capabilities.supports_inline_literals,
        },
        diagnostics: node.diagnostics.iter().map(map_diagnostic).collect(),
    }
}

fn map_port(port: &EditorPortModel) -> EditorPortDto {
    EditorPortDto {
        address: (&port.address).into(),
        display: PortDisplayDto {
            label: port.display.label.clone(),
            instance_label: port.display.instance_label.clone(),
        },
        direction: map_port_direction(port.direction),
        orphan: port.orphan,
        can_remove: port.can_remove,
        connections: PortConnectionCapabilityDto {
            current: port.connections.current,
            maximum: port.connections.maximum,
            ordered: port.connections.ordered,
            can_append: port.connections.can_append,
            can_replace: port.connections.can_replace,
            can_move: port.connections.can_move,
        },
        input: port.input.as_ref().map(|input| EditorInputBindingDto {
            literal_override: input.literal_override.clone(),
            protocol_default: input.protocol_default.clone(),
            effective: match input.effective {
                EditorEffectiveInputBinding::Connections => {
                    EffectiveInputBindingKindDto::Connections
                }
                EditorEffectiveInputBinding::Literal => EffectiveInputBindingKindDto::Literal,
                EditorEffectiveInputBinding::ProtocolDefault => {
                    EffectiveInputBindingKindDto::ProtocolDefault
                }
                EditorEffectiveInputBinding::Unbound => EffectiveInputBindingKindDto::Unbound,
            },
        }),
        accepted_type: AcceptedTypeDto {
            display: port.accepted_type.display.clone(),
            domain: port
                .accepted_type
                .domain
                .as_ref()
                .map(|domain| domain.to_vec()),
        },
        type_state: map_type_state(&port.type_state),
        resolved_schema: port.resolved_schema.as_ref().map(map_schema_summary),
        status: match port.status {
            EditorPortStatus::Resolved => ResolvedPortStatusDto::Resolved,
            EditorPortStatus::Orphan => ResolvedPortStatusDto::Orphan,
        },
    }
}

fn map_type_state(value: &EditorPortTypeState) -> PortTypeStateDto {
    match value {
        EditorPortTypeState::Exact {
            display, data_type, ..
        } => PortTypeStateDto::Exact {
            display: display.clone(),
            data_type: data_type.clone(),
        },
        EditorPortTypeState::Constrained {
            display, domain, ..
        } => PortTypeStateDto::Constrained {
            display: display.clone(),
            domain: domain.to_vec(),
        },
        EditorPortTypeState::Unknown { reason } => PortTypeStateDto::Unknown {
            reason_code: match reason {
                yss_graph_protocol::TypeUnknownReason::UnconnectedInput => "unconnected_input",
                yss_graph_protocol::TypeUnknownReason::UnresolvedUpstream => "unresolved_upstream",
                yss_graph_protocol::TypeUnknownReason::MissingResource => "missing_resource",
                yss_graph_protocol::TypeUnknownReason::UnsupportedDeclaration => {
                    "unsupported_declaration"
                }
                yss_graph_protocol::TypeUnknownReason::OrphanedPort => "orphaned_port",
            }
            .into(),
        },
        EditorPortTypeState::Conflict { conflict } => PortTypeStateDto::Conflict {
            diagnostic_code: match conflict {
                yss_graph_protocol::TypeConflict::InputNotAccepted => "input_not_accepted",
                yss_graph_protocol::TypeConflict::IncompatibleInputs => "incompatible_inputs",
                yss_graph_protocol::TypeConflict::MissingParameter => "missing_parameter",
                yss_graph_protocol::TypeConflict::UnsupportedParameter => "unsupported_parameter",
            }
            .into(),
        },
    }
}

fn map_port_direction(direction: yss_graph_protocol::PortDirection) -> PortDirectionDto {
    match direction {
        yss_graph_protocol::PortDirection::Input => PortDirectionDto::Input,
        yss_graph_protocol::PortDirection::Output => PortDirectionDto::Output,
    }
}

fn map_parameter(parameter: &EditorParameterModel) -> ParameterEditorDto {
    ParameterEditorDto {
        key: parameter.key.as_str().into(),
        display: ParameterDisplayDto {
            title: parameter.display.title.clone(),
            description: parameter.display.description.clone(),
        },
        editor: match parameter.editor {
            ParameterEditorKind::Auto => ParameterEditorKindDto::Auto,
            ParameterEditorKind::Text => ParameterEditorKindDto::Text,
            ParameterEditorKind::Number => ParameterEditorKindDto::Number,
            ParameterEditorKind::Toggle => ParameterEditorKindDto::Toggle,
            ParameterEditorKind::Select => ParameterEditorKindDto::Select,
            ParameterEditorKind::Resource => ParameterEditorKindDto::Resource,
        },
        presentation: parameter.presentation.into(),
        value_type: parameter.value_type.clone(),
        multiline: parameter.multiline,
        value: parameter.value.clone(),
        configuration: parameter
            .configuration
            .as_ref()
            .map(map_parameter_configuration),
        inherited_value: parameter.inherited_value.clone(),
        value_source: parameter.value_source.map(|source| match source {
            EditorParameterValueSource::Project => ParameterValueSourceDto::Project,
            EditorParameterValueSource::Node => ParameterValueSourceDto::Node,
        }),
        options: parameter.options.as_ref().map(|options| options.to_vec()),
    }
}

fn map_schema_summary(summary: &EditorSchemaSummary) -> SchemaSummaryDto {
    SchemaSummaryDto {
        kind: match summary.kind {
            EditorSchemaSummaryKind::Input => SchemaSummaryKindDto::Input,
            EditorSchemaSummaryKind::Project => SchemaSummaryKindDto::Project,
            EditorSchemaSummaryKind::Append => SchemaSummaryKindDto::Append,
            EditorSchemaSummaryKind::Rename => SchemaSummaryKindDto::Rename,
            EditorSchemaSummaryKind::Filter => SchemaSummaryKindDto::Filter,
            EditorSchemaSummaryKind::Derived => SchemaSummaryKindDto::Derived,
        },
        fields: summary
            .fields
            .iter()
            .map(|field| SchemaFieldDto {
                name: field.name.clone(),
                scalar_type: map_relational_scalar_type(field.scalar_type),
            })
            .collect(),
    }
}

fn map_parameter_configuration(
    configuration: &EditorParameterConfiguration,
) -> SchemaAwareParameterEditorDto {
    match configuration {
        EditorParameterConfiguration::ProjectColumns {
            available,
            unavailable_reason,
            options,
            value,
        } => SchemaAwareParameterEditorDto::ProjectColumns {
            available: *available,
            unavailable_reason: unavailable_reason.clone(),
            options: options
                .iter()
                .map(|option| DataframeColumnOptionDto {
                    name: option.name.clone(),
                    data_type: map_relational_scalar_type(option.data_type),
                })
                .collect(),
            value: value.to_vec(),
        },
        EditorParameterConfiguration::FilterPredicate {
            available,
            unavailable_reason,
            columns,
            value,
        } => SchemaAwareParameterEditorDto::FilterPredicate {
            available: *available,
            unavailable_reason: unavailable_reason.clone(),
            columns: columns
                .iter()
                .map(|column| FilterColumnOptionDto {
                    name: column.name.clone(),
                    data_type: map_relational_scalar_type(column.data_type),
                    operators: column.operators.to_vec(),
                    literal_types: column
                        .literal_types
                        .iter()
                        .copied()
                        .map(map_filter_literal_type)
                        .collect(),
                })
                .collect(),
            value: value.clone(),
        },
    }
}

fn map_relational_scalar_type(
    scalar_type: yss_graph_protocol::RelationalScalarType,
) -> RelationalScalarTypeDto {
    match scalar_type {
        yss_graph_protocol::RelationalScalarType::Boolean => RelationalScalarTypeDto::Boolean,
        yss_graph_protocol::RelationalScalarType::Int64 => RelationalScalarTypeDto::Int64,
        yss_graph_protocol::RelationalScalarType::Float64 => RelationalScalarTypeDto::Float64,
        yss_graph_protocol::RelationalScalarType::String => RelationalScalarTypeDto::String,
        yss_graph_protocol::RelationalScalarType::Date => RelationalScalarTypeDto::Date,
        yss_graph_protocol::RelationalScalarType::DateTime => RelationalScalarTypeDto::DateTime,
        yss_graph_protocol::RelationalScalarType::Unknown => RelationalScalarTypeDto::Unknown,
    }
}

fn map_filter_literal_type(literal_type: EditorFilterLiteralType) -> FilterLiteralTypeDto {
    match literal_type {
        EditorFilterLiteralType::Boolean => FilterLiteralTypeDto::Boolean,
        EditorFilterLiteralType::Integer => FilterLiteralTypeDto::Integer,
        EditorFilterLiteralType::Decimal => FilterLiteralTypeDto::Decimal,
        EditorFilterLiteralType::String => FilterLiteralTypeDto::String,
    }
}

fn map_diagnostic(diagnostic: &EditorDiagnosticModel) -> DiagnosticDto {
    DiagnosticDto {
        code: diagnostic.code.clone(),
        // The current wire has no structured argument field. Keep this field a
        // stable localization token rather than copying internal backend text.
        message: diagnostic.code.clone(),
        severity: match diagnostic.severity {
            EditorDiagnosticSeverity::Error => DiagnosticSeverityDto::Error,
            EditorDiagnosticSeverity::Warning => DiagnosticSeverityDto::Warning,
            EditorDiagnosticSeverity::Information => DiagnosticSeverityDto::Information,
        },
        blocking: diagnostic.severity == EditorDiagnosticSeverity::Error,
        location: map_location(&diagnostic.location),
        related: diagnostic.related.iter().map(map_location).collect(),
    }
}

fn map_location(location: &yss_graph_analysis::GraphDiagnosticLocation) -> DiagnosticLocationDto {
    match location {
        yss_graph_analysis_contract::DiagnosticLocation::Graph => DiagnosticLocationDto::Graph,
        yss_graph_analysis_contract::DiagnosticLocation::Node(node_id) => {
            DiagnosticLocationDto::Node {
                node_id: node_id.to_string().into(),
            }
        }
        yss_graph_analysis_contract::DiagnosticLocation::Port(address) => {
            DiagnosticLocationDto::Port {
                address: address.into(),
            }
        }
        yss_graph_analysis_contract::DiagnosticLocation::Connection(connection_id) => {
            DiagnosticLocationDto::Connection {
                connection_id: connection_id.to_string().into(),
            }
        }
        yss_graph_analysis_contract::DiagnosticLocation::Parameter { node_id, key } => {
            DiagnosticLocationDto::Parameter {
                node_id: node_id.to_string().into(),
                key: key.as_str().into(),
            }
        }
        yss_graph_analysis_contract::DiagnosticLocation::Resource(identity) => {
            DiagnosticLocationDto::Resource {
                identity: identity.clone(),
            }
        }
    }
}

fn map_outcome(outcome: &EditorCompilationOutcome) -> CompilationOutcomeDto {
    match outcome {
        EditorCompilationOutcome::Complete => CompilationOutcomeDto::Success,
        EditorCompilationOutcome::Incomplete => CompilationOutcomeDto::AnalysisBlocked,
        EditorCompilationOutcome::InternalFailure {
            stage,
            code,
            node_id,
        } => CompilationOutcomeDto::InternalFailure {
            stage: match stage {
                EditorCompilationStage::Analysis => CompilationStageDto::Analysis,
                EditorCompilationStage::Lowering => CompilationStageDto::Lowering,
            },
            code: code.clone(),
            node_id: node_id.map(|node_id| node_id.to_string().into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;
    use yss_application::editor_projection::{
        EditorAcceptedType, EditorColumnOption, EditorConnectionModel, EditorDiagnosticModel,
        EditorNodeCapabilities, EditorNodeDisplay, EditorNodeModel, EditorParameterConfiguration,
        EditorParameterDisplay, EditorParameterModel, EditorParameterValueSource,
        EditorPortConnectionCapabilities, EditorPortDisplay, EditorPortModel, EditorPortStatus,
        EditorPortTypeState, EditorProjectionBasis, EditorSchemaField, EditorSchemaSummary,
        EditorSchemaSummaryKind,
    };
    use yss_graph_analysis_contract::{
        DiagnosticArguments, DiagnosticLocation, ResourceKey, ResourceVersion,
    };
    use yss_graph_document::{
        ConnectionId, GraphResourcePath, GraphRevision, NodeId, NodePosition, PortAddress,
    };
    use yss_graph_protocol::{
        NodeTypeId, ParameterKey, ParameterPresentation, PortDirection, PortKey,
        RelationalScalarType,
    };

    #[test]
    fn editor_projection_serializes_canonical_camel_case_wire_and_safe_diagnostics() {
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(2));
        let graph_path = GraphResourcePath::new("events/contract.yssbi-event")
            .expect("test graph path is valid");
        let port_address = PortAddress::declared(
            node_id,
            PortKey::new("value").expect("test port key is valid"),
        );
        let node_type = NodeTypeId::new("yssbi.constant.bool").expect("test node type is valid");
        let diagnostic = EditorDiagnosticModel {
            code: "graph.invalid".into(),
            severity: EditorDiagnosticSeverity::Warning,
            arguments: DiagnosticArguments::from([("field".into(), "value".into())]),
            location: DiagnosticLocation::Node(node_id),
            related: Box::new([]),
        };
        let model = EditorProjectionModel {
            basis: EditorProjectionBasis {
                graph_path: graph_path.clone(),
                registry_fingerprint: [0x6f; 32],
                resource_versions: BTreeMap::from([(
                    ResourceKey::new("database/source"),
                    ResourceVersion::new("12"),
                )]),
            },
            graph_path: graph_path.clone(),
            source_revision: GraphRevision::new(7),
            nodes: Box::new([EditorNodeModel {
                node_id,
                node_type,
                position: NodePosition { x: 120.5, y: -32.0 },
                display: EditorNodeDisplay {
                    title: "Boolean Constant".into(),
                    user_label: Some("Contract Boolean".into()),
                    icon_id: Some("builtin.constants".into()),
                    style_id: Some("builtin.default".into()),
                },
                ports: Box::new([EditorPortModel {
                    address: port_address.clone(),
                    display: EditorPortDisplay {
                        label: "Value".into(),
                        instance_label: None,
                    },
                    direction: PortDirection::Output,
                    orphan: false,
                    can_remove: false,
                    connections: EditorPortConnectionCapabilities {
                        current: 0,
                        maximum: Some(1),
                        ordered: false,
                        can_append: true,
                        can_replace: false,
                        can_move: false,
                    },
                    input: None,
                    accepted_type: EditorAcceptedType {
                        display: "core.bool".into(),
                        domain: Some(Box::new([yss_data_contract::DataType::Boolean])),
                    },
                    type_state: EditorPortTypeState::Exact {
                        display: "core.bool".into(),
                        data_type: Some(yss_data_contract::DataType::Boolean),
                    },
                    resolved_schema: Some(EditorSchemaSummary {
                        kind: EditorSchemaSummaryKind::Derived,
                        fields: Box::new([EditorSchemaField {
                            name: "sales".into(),
                            scalar_type: RelationalScalarType::Float64,
                        }]),
                    }),
                    status: EditorPortStatus::Resolved,
                }]),
                port_instance_additions: Box::new([]),
                parameters: Box::new([EditorParameterModel {
                    key: ParameterKey::new("columns").expect("test parameter key is valid"),
                    display: EditorParameterDisplay {
                        title: "Columns".into(),
                        description: None,
                    },
                    editor: ParameterEditorKind::Select,
                    presentation: ParameterPresentation::DetailPanel,
                    value_type: Some(yss_data_contract::DataType::String),
                    multiline: false,
                    value: Some(json!(["sales"])),
                    configuration: Some(EditorParameterConfiguration::ProjectColumns {
                        available: true,
                        unavailable_reason: None,
                        options: Box::new([EditorColumnOption {
                            name: "sales".into(),
                            data_type: RelationalScalarType::Float64,
                        }]),
                        value: Box::new(["sales".into()]),
                    }),
                    inherited_value: None,
                    value_source: Some(EditorParameterValueSource::Node),
                    options: None,
                }]),
                capabilities: EditorNodeCapabilities {
                    managed: false,
                    can_copy: true,
                    can_delete: true,
                    can_edit_label: true,
                    can_edit_parameters: true,
                    supports_inline_literals: false,
                },
                diagnostics: Box::new([diagnostic.clone()]),
            }]),
            connections: Box::new([EditorConnectionModel {
                connection_id: ConnectionId::from_uuid(uuid::Uuid::from_u128(4)),
                output: port_address.clone(),
                input: port_address.clone(),
                order: Some("0".into()),
            }]),
            diagnostics: Box::new([diagnostic]),
            outcome: EditorCompilationOutcome::Complete,
        };

        let wire = serde_json::to_value(EditorGraphProjectionDto::from(&model))
            .expect("editor wire should serialize");
        assert_eq!(wire["basis"]["graphPath"], "events/contract.yssbi-event");
        assert!(wire["basis"].get("graphRevision").is_none());
        assert_eq!(
            wire["basis"]["resourceVersions"],
            json!({"database/source": "12"})
        );
        assert_eq!(wire["nodes"][0]["nodeId"], node_id.to_string());
        assert!(wire["nodes"][0].get("sourceRevision").is_none());
        assert_eq!(wire["nodes"][0]["display"]["iconId"], "builtin.constants");
        assert!(wire["nodes"][0]["ports"][0].get("templateKey").is_none());
        assert!(wire["nodes"][0]["ports"][0].get("origin").is_none());
        assert!(wire["nodes"][0]["ports"][0].get("instanceKind").is_none());
        assert!(wire["nodes"][0]["ports"][0].get("kind").is_none());
        assert_eq!(
            wire["nodes"][0]["ports"][0]["resolvedSchema"],
            json!({
                "kind": "derived",
                "fields": [{"name": "sales", "scalarType": "float64"}],
            })
        );
        assert_eq!(wire["nodes"][0]["portInstanceAdditions"], json!([]));
        assert_eq!(
            wire["nodes"][0]["parameterEditors"][0]["valueSource"],
            "node"
        );
        assert_eq!(
            wire["nodes"][0]["parameterEditors"][0]["configuration"],
            json!({
                "kind": "projectColumns",
                "available": true,
                "unavailableReason": null,
                "options": [{"name": "sales", "dataType": "float64"}],
                "value": ["sales"],
            })
        );
        assert_eq!(wire["connections"][0]["output"]["portKey"], "value");
        assert_eq!(wire["diagnostics"][0]["code"], "graph.invalid");
        assert_eq!(wire["diagnostics"][0]["message"], "graph.invalid");
        assert_ne!(wire["diagnostics"][0]["message"], "raw backend failure");
        assert!(wire["nodes"][0].get("node_id").is_none());
    }
}
