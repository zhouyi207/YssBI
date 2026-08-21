use super::super::{AnalysisSnapshot, DiagnosticArguments, ResolvedPortStatus};
use super::ProjectionError;
use super::types::{
    CompilationOutcomeDto, EditorConnectionProjectionDto, EditorGraphProjectionDto,
    EditorInputBindingDto, EditorNodeProjectionDto, LocalizationLookup, NodeDisplayDto,
    NodePositionDto, ParameterDisplayDto, ParameterEditorDto, ParameterValueSourceDto,
    PortDisplayDto, ProjectionBasis, ResolvedPortDto,
};
use super::{
    can_remove_port, diagnostic_belongs_to_node, inherited_statistics_parameter_value,
    project_address, project_connection_capability, project_data_type, project_diagnostic,
    project_effective_input_binding, project_instance_kind, project_node_capabilities,
    project_parameter_editor, project_schema_aware_editor, project_schema_summary,
    project_type_summary, statistics_parameter_options,
};
use crate::node_system::document::{
    ConnectionId, GraphDocument, GraphRevision, NodeId, PortAddress, port_member_group_state,
};
use crate::node_system::protocol::{
    I18nKey, PortDirection, PortInstances, PortKey, SchemaExpr, TypeExpr,
};
use crate::node_system::registry::NodeRegistry;
use std::collections::BTreeMap;

type EditorAnalysis = AnalysisSnapshot<
    GraphRevision,
    NodeId,
    PortAddress,
    ConnectionId,
    Box<str>,
    serde_json::Value,
    TypeExpr,
    SchemaExpr,
>;

pub fn build_editor_graph_projection(
    graph_path: &str,
    document: &GraphDocument,
    analysis: &EditorAnalysis,
    outcome: &crate::node_system::compiler::CompilationOutcome,
    registry: &NodeRegistry,
    localization: &impl LocalizationLookup,
) -> Result<EditorGraphProjectionDto, ProjectionError> {
    EditorGraphProjectionDto::from_compilation_sources(
        graph_path,
        analysis,
        outcome,
        document,
        registry,
        localization,
        &crate::project::ProjectComputationSettings::default(),
    )
}

impl EditorGraphProjectionDto {
    #[cfg(test)]
    pub fn from_sources(
        graph_path: impl Into<Box<str>>,
        analysis: &EditorAnalysis,
        document: &GraphDocument,
        registry: &NodeRegistry,
        localization: &impl LocalizationLookup,
    ) -> Result<Self, ProjectionError> {
        let outcome = if analysis.has_blocking_errors() {
            crate::node_system::compiler::CompilationOutcome::AnalysisBlocked
        } else {
            crate::node_system::compiler::CompilationOutcome::Succeeded
        };
        Self::from_compilation_sources(
            graph_path,
            analysis,
            &outcome,
            document,
            registry,
            localization,
            &crate::project::ProjectComputationSettings::default(),
        )
    }

    pub fn from_compilation_sources(
        graph_path: impl Into<Box<str>>,
        analysis: &EditorAnalysis,
        outcome: &crate::node_system::compiler::CompilationOutcome,
        document: &GraphDocument,
        registry: &NodeRegistry,
        localization: &impl LocalizationLookup,
        computation_settings: &crate::project::ProjectComputationSettings,
    ) -> Result<Self, ProjectionError> {
        validate_sources(analysis, document, registry)?;

        let graph_path = graph_path.into();
        let source_revision = document.revision.get();
        let basis = ProjectionBasis {
            graph_path: graph_path.clone(),
            graph_revision: analysis.basis.graph_revision.get(),
            registry_fingerprint: analysis.basis.registry_fingerprint.clone(),
            resource_versions: analysis.basis.resource_versions.clone(),
        };
        let analyzed_nodes = analysis
            .nodes
            .iter()
            .map(|node| (node.node_id, node))
            .collect::<BTreeMap<_, _>>();
        let interfaces = analysis
            .resolved_interfaces
            .iter()
            .map(|interface| (interface.node_id, interface))
            .collect::<BTreeMap<_, _>>();
        let diagnostics = analysis
            .diagnostics
            .iter()
            .map(|diagnostic| project_diagnostic(diagnostic, localization))
            .collect::<Vec<_>>();
        let connections = document
            .connections
            .values()
            .map(|connection| EditorConnectionProjectionDto {
                connection_id: connection.id.to_string().into(),
                output: project_address(&connection.output),
                input: project_address(&connection.input),
                order: connection.order.as_ref().map(|order| order.0.clone()),
            })
            .collect();

        let nodes = document
            .nodes
            .values()
            .map(|node| {
                let protocol = registry.get(&node.node_type).map(|entry| entry.protocol());
                let normalized_node = analyzed_nodes.get(&node.id).copied();
                let normalized = normalized_node.map(|node| &node.normalized_parameters);
                let ports = interfaces
                    .get(&node.id)
                    .map(|interface| {
                        interface
                            .ports
                            .iter()
                            .filter_map(|port| {
                                let protocol = protocol?;
                                let spec = protocol
                                    .interface
                                    .ports
                                    .iter()
                                    .find(|spec| spec.key == port.template)?;
                                let orphan = port.status == ResolvedPortStatus::Orphan;
                                let instance_kind = project_instance_kind(&spec.instances);
                                let group =
                                    protocol.interface.member_group_for_template(&port.template);
                                let (minimum, instance_count, member_complete) =
                                    if let Some(group) = group {
                                        let state = port_member_group_state(
                                            node.id,
                                            group,
                                            document.port_bindings.iter(),
                                        );
                                        (
                                            group.min,
                                            state.complete_count(),
                                            state.address_is_complete(&port.address),
                                        )
                                    } else {
                                        (
                                            match &spec.instances {
                                                PortInstances::UserCreated { min, .. } => *min,
                                                _ => 0,
                                            },
                                            interface
                                                .ports
                                                .iter()
                                                .filter(|candidate| {
                                                    candidate.template == port.template
                                                })
                                                .filter(|candidate| candidate.address.is_instance())
                                                .count(),
                                            true,
                                        )
                                    };
                                let can_remove = can_remove_port(
                                    &port.address,
                                    orphan,
                                    &spec.instances,
                                    minimum,
                                    instance_count,
                                    member_complete,
                                );
                                let connections = project_connection_capability(
                                    document,
                                    &port.address,
                                    spec.connections,
                                    orphan,
                                );
                                let input = (port.direction == PortDirection::Input).then(|| {
                                    let literal_override = document
                                        .input_states
                                        .get(&port.address)
                                        .and_then(|state| state.literal_override.clone());
                                    let protocol_default = spec
                                        .input_binding
                                        .as_ref()
                                        .and_then(|binding| binding.default_value.as_ref())
                                        .map(|default| {
                                            serde_json::to_value(&default.value)
                                                .expect("protocol values must serialize")
                                        });
                                    let effective = project_effective_input_binding(
                                        document.effective_input_binding(
                                            &port.address,
                                            protocol_default.clone(),
                                        ),
                                    );
                                    EditorInputBindingDto {
                                        literal_override,
                                        protocol_default,
                                        effective,
                                    }
                                });
                                let instance_label = port.instance_label.clone();
                                let label = instance_label.clone().unwrap_or_else(|| {
                                    localization.text(&spec.label_key, &DiagnosticArguments::new())
                                });
                                Some(ResolvedPortDto {
                                    address: project_address(&port.address),
                                    template_key: port.template.as_str().into(),
                                    display: PortDisplayDto {
                                        label,
                                        instance_label,
                                    },
                                    direction: port.direction.into(),
                                    kind: port.kind.into(),
                                    instance_kind,
                                    orphan,
                                    can_remove,
                                    connections,
                                    input,
                                    resolved_type: Some(project_type_summary(
                                        analysis
                                            .partial_types
                                            .get(&port.address)
                                            .unwrap_or(&port.value_type),
                                    )),
                                    resolved_schema: analysis
                                        .partial_schemas
                                        .get(&port.address)
                                        .map(|expression| {
                                            project_schema_summary(
                                                expression,
                                                analysis.resolved_schemas.get(&port.address),
                                            )
                                        }),
                                    status: port.status.into(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let parameter_editors = protocol
                    .map(|protocol| {
                        protocol
                            .parameters
                            .parameters
                            .iter()
                            .filter_map(|parameter| {
                                let (editor, multiline) =
                                    project_parameter_editor(&parameter.editor)?;
                                let value = normalized
                                    .and_then(|values| values.get(&parameter.key))
                                    .cloned()
                                    .or_else(|| node.parameters.get(&parameter.key).cloned());
                                let source_schema =
                                    analysis.resolved_schemas.get(&PortAddress::declared(
                                        node.id,
                                        PortKey::new("source").expect("static source port key"),
                                    ));
                                let unavailable_reason = localization.text(
                                    &I18nKey::new("editors.dataframe.connect_source")
                                        .expect("static editor localization key"),
                                    &DiagnosticArguments::new(),
                                );
                                Some(ParameterEditorDto {
                                    key: parameter.key.as_str().into(),
                                    display: ParameterDisplayDto {
                                        title: localization.text(
                                            &parameter.title_key,
                                            &DiagnosticArguments::new(),
                                        ),
                                        description: parameter.description_key.as_ref().map(
                                            |key| {
                                                localization.text(key, &DiagnosticArguments::new())
                                            },
                                        ),
                                    },
                                    editor,
                                    presentation: parameter.presentation.into(),
                                    value_type: project_data_type(&parameter.value_type),
                                    multiline,
                                    value: value.clone(),
                                    configuration: project_schema_aware_editor(
                                        node.node_type.as_str(),
                                        value.as_ref(),
                                        source_schema,
                                        unavailable_reason,
                                    ),
                                    inherited_value: inherited_statistics_parameter_value(
                                        node.node_type.as_str(),
                                        parameter.key.as_str(),
                                        computation_settings,
                                    ),
                                    value_source: inherited_statistics_parameter_value(
                                        node.node_type.as_str(),
                                        parameter.key.as_str(),
                                        computation_settings,
                                    )
                                    .map(|_| {
                                        if value.is_some() {
                                            ParameterValueSourceDto::Node
                                        } else {
                                            ParameterValueSourceDto::Project
                                        }
                                    }),
                                    options: statistics_parameter_options(
                                        node.node_type.as_str(),
                                        parameter.key.as_str(),
                                    ),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let node_diagnostics = analysis
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic_belongs_to_node(diagnostic, node.id, document))
                    .map(|diagnostic| project_diagnostic(diagnostic, localization))
                    .collect();
                let display = protocol.map_or_else(
                    || NodeDisplayDto {
                        title: node.node_type.as_str().into(),
                        description: None,
                        user_label: node.user_label.as_deref().map(Into::into),
                        icon_id: None,
                        style_id: None,
                    },
                    |protocol| NodeDisplayDto {
                        title: normalized_node
                            .and_then(|node| node.instance_title.clone())
                            .unwrap_or_else(|| {
                                localization
                                    .text(&protocol.catalog.title_key, &DiagnosticArguments::new())
                            }),
                        description: protocol
                            .catalog
                            .description_key
                            .as_ref()
                            .map(|key| localization.text(key, &DiagnosticArguments::new())),
                        user_label: node.user_label.as_deref().map(Into::into),
                        icon_id: Some(protocol.catalog.icon_id.as_str().into()),
                        style_id: Some(protocol.catalog.style_id.as_str().into()),
                    },
                );
                let capabilities = project_node_capabilities(protocol);
                EditorNodeProjectionDto {
                    graph_path: graph_path.clone(),
                    source_revision,
                    node_id: node.id.to_string().into(),
                    node_type_id: node.node_type.as_str().into(),
                    position: NodePositionDto {
                        x: node.position.x,
                        y: node.position.y,
                    },
                    display,
                    ports,
                    parameter_editors,
                    capabilities,
                    diagnostics: node_diagnostics,
                }
            })
            .collect();

        let outcome = CompilationOutcomeDto::from(outcome);
        let has_blocking_diagnostics = !matches!(outcome, CompilationOutcomeDto::Success)
            || diagnostics.iter().any(|diagnostic| diagnostic.blocking);
        Ok(Self {
            basis,
            graph_path,
            source_revision,
            nodes,
            connections,
            diagnostics,
            outcome,
            has_blocking_diagnostics,
        })
    }
}

fn validate_sources(
    analysis: &EditorAnalysis,
    document: &GraphDocument,
    registry: &NodeRegistry,
) -> Result<(), ProjectionError> {
    if analysis.basis.graph_revision != document.revision {
        return Err(ProjectionError::RevisionMismatch {
            analysis: analysis.basis.graph_revision,
            document: document.revision,
        });
    }
    if &analysis.basis.registry_fingerprint != registry.fingerprint() {
        return Err(ProjectionError::RegistryMismatch);
    }
    Ok(())
}
