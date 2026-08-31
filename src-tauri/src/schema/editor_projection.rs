use yss_application::editor_projection::{
    EditorCompilationOutcome, EditorCompilationStage, EditorDiagnosticModel,
    EditorDiagnosticSeverity, EditorEffectiveInputBinding, EditorParameterModel,
    EditorPortInstanceKind, EditorPortModel, EditorPortStatus, EditorProjectionModel,
    ParameterEditorKind,
};
use yss_graph_registry::RegistryFingerprint;

pub use super::editor_projection_types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransportMappingError {
    #[error("editor projection parameter configuration has no accessible typed wire contract")]
    ParameterConfigurationWireContractUnavailable,
    #[error("editor projection parameter value source has no accessible typed wire contract")]
    ParameterValueSourceWireContractUnavailable,
    #[error("editor projection schema summary has no accessible typed wire contract")]
    ResolvedSchemaWireContractUnavailable,
}

impl TryFrom<&EditorProjectionModel> for EditorGraphProjectionDto {
    type Error = TransportMappingError;

    fn try_from(model: &EditorProjectionModel) -> Result<Self, Self::Error> {
        Ok(Self {
            basis: ProjectionBasis {
                graph_path: model.basis.graph_path.as_str().into(),
                graph_revision: model.basis.graph_revision.get(),
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
                .collect::<Result<Vec<_>, _>>()?,
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
        })
    }
}

/// Explicit command-facing entry point for the sole editor wire mapper.
pub fn map_editor_projection(
    model: &EditorProjectionModel,
) -> Result<EditorGraphProjectionDto, TransportMappingError> {
    model.try_into()
}

fn map_node(
    model: &EditorProjectionModel,
    node: &yss_application::editor_projection::EditorNodeModel,
) -> Result<EditorNodeProjectionDto, TransportMappingError> {
    Ok(EditorNodeProjectionDto {
        graph_path: model.graph_path.as_str().into(),
        source_revision: model.source_revision.get(),
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
        ports: node
            .ports
            .iter()
            .map(map_port)
            .collect::<Result<Vec<_>, _>>()?,
        parameter_editors: node
            .parameters
            .iter()
            .map(map_parameter)
            .collect::<Result<Vec<_>, _>>()?,
        capabilities: NodeCapabilitiesDto {
            managed: node.capabilities.managed,
            can_copy: node.capabilities.can_copy,
            can_delete: node.capabilities.can_delete,
            can_edit_label: node.capabilities.can_edit_label,
            can_edit_parameters: node.capabilities.can_edit_parameters,
            has_dynamic_ports: node.capabilities.has_dynamic_ports,
            supports_inline_literals: node.capabilities.supports_inline_literals,
        },
        diagnostics: node.diagnostics.iter().map(map_diagnostic).collect(),
    })
}

fn map_port(port: &EditorPortModel) -> Result<ResolvedPortDto, TransportMappingError> {
    if port.resolved_schema.is_some() {
        return Err(TransportMappingError::ResolvedSchemaWireContractUnavailable);
    }
    Ok(ResolvedPortDto {
        address: (&port.address).into(),
        template_key: port.template_key.as_str().into(),
        display: PortDisplayDto {
            label: port.display.label.clone(),
            instance_label: port.display.instance_label.clone(),
        },
        direction: match port.direction {
            yss_graph_protocol::PortDirection::Input => PortDirectionDto::Input,
            yss_graph_protocol::PortDirection::Output => PortDirectionDto::Output,
        },
        kind: match port.kind {
            yss_graph_protocol::PortKind::Data => PortKindDto::Data,
            yss_graph_protocol::PortKind::Control => PortKindDto::Control,
            yss_graph_protocol::PortKind::Effect => PortKindDto::Effect,
        },
        instance_kind: match port.instance_kind {
            EditorPortInstanceKind::Declared => PortInstanceKindDto::Declared,
            EditorPortInstanceKind::UserCreated => PortInstanceKindDto::UserCreated,
            EditorPortInstanceKind::Derived => PortInstanceKindDto::Derived,
        },
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
        resolved_type: port.resolved_type.as_ref().map(|value| TypeSummaryDto {
            display: value.display.clone(),
            resolved: value.resolved,
            data_type: value.data_type.clone(),
            internal_type_expr: Some(value.internal_type_expr.clone()),
        }),
        resolved_schema: None,
        status: match port.status {
            EditorPortStatus::Resolved => ResolvedPortStatusDto::Resolved,
            EditorPortStatus::Orphan => ResolvedPortStatusDto::Orphan,
        },
    })
}

fn map_parameter(
    parameter: &EditorParameterModel,
) -> Result<ParameterEditorDto, TransportMappingError> {
    if parameter.configuration.is_some() {
        return Err(TransportMappingError::ParameterConfigurationWireContractUnavailable);
    }
    if parameter.value_source.is_some() {
        return Err(TransportMappingError::ParameterValueSourceWireContractUnavailable);
    }
    Ok(ParameterEditorDto {
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
        configuration: None,
        inherited_value: parameter.inherited_value.clone(),
        value_source: None,
        options: parameter.options.as_ref().map(|options| options.to_vec()),
    })
}

fn map_diagnostic(diagnostic: &EditorDiagnosticModel) -> DiagnosticDto {
    DiagnosticDto {
        code: diagnostic.code.clone(),
        // The legacy wire has no structured argument field. Keep this field a
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
        EditorConnectionModel, EditorDiagnosticModel, EditorNodeCapabilities, EditorNodeDisplay,
        EditorNodeModel, EditorPortConnectionCapabilities, EditorPortDisplay,
        EditorPortInstanceKind, EditorPortModel, EditorPortStatus, EditorProjectionBasis,
        EditorTypeSummary,
    };
    use yss_graph_analysis_contract::{
        DiagnosticArguments, DiagnosticLocation, ResourceKey, ResourceVersion,
    };
    use yss_graph_document::{
        ConnectionId, GraphResourcePath, GraphRevision, NodeId, NodePosition, PortAddress,
    };
    use yss_graph_protocol::{NodeTypeId, PortDirection, PortKey, PortKind, TypeExpr, TypeId};

    #[test]
    fn application_model_preserves_existing_camel_case_wire_and_safe_diagnostics() {
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
                graph_revision: GraphRevision::new(7),
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
                    template_key: PortKey::new("value").expect("test port key is valid"),
                    display: EditorPortDisplay {
                        label: "Value".into(),
                        instance_label: None,
                    },
                    direction: PortDirection::Output,
                    kind: PortKind::Data,
                    instance_kind: EditorPortInstanceKind::Declared,
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
                    resolved_type: Some(EditorTypeSummary {
                        display: "core.bool".into(),
                        resolved: true,
                        data_type: Some(yss_data_contract::DataType::Boolean),
                        internal_type_expr: TypeExpr::Concrete(
                            TypeId::new("core.bool").expect("test type id is valid"),
                        ),
                    }),
                    resolved_schema: None,
                    status: EditorPortStatus::Resolved,
                }]),
                parameters: Box::new([]),
                capabilities: EditorNodeCapabilities {
                    managed: false,
                    can_copy: true,
                    can_delete: true,
                    can_edit_label: true,
                    can_edit_parameters: true,
                    has_dynamic_ports: false,
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

        let wire = serde_json::to_value(
            EditorGraphProjectionDto::try_from(&model).expect("typed model is mappable"),
        )
        .expect("editor wire should serialize");
        assert_eq!(wire["basis"]["graphPath"], "events/contract.yssbi-event");
        assert_eq!(wire["basis"]["graphRevision"], 7);
        assert_eq!(
            wire["basis"]["resourceVersions"],
            json!({"database/source": "12"})
        );
        assert_eq!(wire["nodes"][0]["nodeId"], node_id.to_string());
        assert_eq!(wire["nodes"][0]["display"]["iconId"], "builtin.constants");
        assert_eq!(wire["nodes"][0]["ports"][0]["templateKey"], "value");
        assert_eq!(wire["connections"][0]["output"]["portKey"], "value");
        assert_eq!(wire["diagnostics"][0]["code"], "graph.invalid");
        assert_eq!(wire["diagnostics"][0]["message"], "graph.invalid");
        assert_ne!(wire["diagnostics"][0]["message"], "raw backend failure");
        assert!(wire["nodes"][0].get("node_id").is_none());
    }
}
