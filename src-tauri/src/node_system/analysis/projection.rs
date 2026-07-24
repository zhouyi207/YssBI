use super::{
    AnalysisSnapshot, DiagnosticArguments, DiagnosticLocation, DiagnosticSeverity, NodeDiagnostic,
    ResolvedPortStatus, ResourceVersionSet,
};
use crate::node_system::document::{
    ConnectionId, GraphDocument, GraphRevision, NodeId, PortAddress, PortRef,
};
use crate::node_system::protocol::{
    ConnectionsPerPort, I18nKey, ParameterEditorSpec, PortDirection, PortEditorSpec, PortInstances,
    PortKind, SchemaExpr, TypeExpr,
};
use crate::node_system::registry::{NodeRegistry, RegistryFingerprint};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Localization boundary owned by analysis. Catalogs and callers can implement it
/// without making analysis depend on a catalog implementation.
pub trait LocalizationLookup {
    fn text(&self, key: &I18nKey, arguments: &DiagnosticArguments) -> Box<str>;
}

/// Compatibility boundary for existing catalog implementations.
pub trait LocalizationBundle {
    fn text(&self, key: &I18nKey, arguments: &DiagnosticArguments) -> Box<str>;
}

impl<T: LocalizationBundle + ?Sized> LocalizationLookup for T {
    fn text(&self, key: &I18nKey, arguments: &DiagnosticArguments) -> Box<str> {
        LocalizationBundle::text(self, key, arguments)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionBasis {
    pub graph_path: Box<str>,
    pub graph_revision: u64,
    pub registry_fingerprint: RegistryFingerprint,
    pub resource_versions: ResourceVersionSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorGraphProjectionDto {
    pub basis: ProjectionBasis,
    pub graph_path: Box<str>,
    pub source_revision: u64,
    pub nodes: Vec<EditorNodeProjectionDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub has_blocking_diagnostics: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNodeProjectionDto {
    pub graph_path: Box<str>,
    pub source_revision: u64,
    pub node_id: Box<str>,
    pub node_type_id: Box<str>,
    pub display: NodeDisplayDto,
    pub ports: Vec<ResolvedPortDto>,
    pub parameter_editors: Vec<ParameterEditorDto>,
    pub capabilities: NodeCapabilitiesDto,
    pub diagnostics: Vec<DiagnosticDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDisplayDto {
    pub title: Box<str>,
    pub description: Option<Box<str>>,
    pub user_label: Option<Box<str>>,
    pub icon_id: Option<Box<str>>,
    pub style_id: Option<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCapabilitiesDto {
    pub managed: bool,
    pub can_copy: bool,
    pub can_delete: bool,
    pub can_edit_label: bool,
    pub can_edit_parameters: bool,
    pub has_dynamic_ports: bool,
    pub supports_inline_literals: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPortDto {
    pub address: PortAddressDto,
    pub template_key: Box<str>,
    pub display: PortDisplayDto,
    pub direction: PortDirectionDto,
    pub kind: PortKindDto,
    pub instance_kind: PortInstanceKindDto,
    pub orphan: bool,
    pub can_remove: bool,
    pub connections: PortConnectionCapabilityDto,
    pub resolved_type: Option<TypeSummaryDto>,
    pub resolved_schema: Option<SchemaSummaryDto>,
    pub status: ResolvedPortStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PortAddressDto {
    Declared {
        node_id: Box<str>,
        port_key: Box<str>,
    },
    Instance {
        node_id: Box<str>,
        template_key: Box<str>,
        instance_id: Box<str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortDisplayDto {
    pub label: Box<str>,
    pub instance_label: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortDirectionDto {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortKindDto {
    Data,
    Control,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortInstanceKindDto {
    Declared,
    UserCreated,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortConnectionCapabilityDto {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
    pub can_connect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeSummaryDto {
    pub display: Box<str>,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSummaryDto {
    pub kind: SchemaSummaryKindDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaSummaryKindDto {
    Input,
    Project,
    Append,
    Rename,
    Filter,
    Derived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolvedPortStatusDto {
    Resolved,
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterEditorDto {
    pub key: Box<str>,
    pub display: ParameterDisplayDto,
    pub editor: ParameterEditorKindDto,
    pub multiline: bool,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterDisplayDto {
    pub title: Box<str>,
    pub description: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterEditorKindDto {
    Auto,
    Text,
    Number,
    Toggle,
    Select,
    Resource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDto {
    pub code: Box<str>,
    pub message: Box<str>,
    pub severity: DiagnosticSeverityDto,
    pub blocking: bool,
    pub location: DiagnosticLocationDto,
    pub related: Vec<DiagnosticLocationDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverityDto {
    Error,
    Warning,
    Information,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiagnosticLocationDto {
    Graph,
    Node { node_id: Box<str> },
    Port { address: PortAddressDto },
    Connection { connection_id: Box<str> },
    Parameter { node_id: Box<str>, key: Box<str> },
    Resource { identity: Box<str> },
}

/// A revision transition containing complete node replacements. Port additions and
/// removals are intentionally not represented as independently applicable fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphProjectionDelta {
    pub from_basis: ProjectionBasis,
    pub to_basis: ProjectionBasis,
    pub removed_node_ids: Vec<Box<str>>,
    pub node_replacements: Vec<EditorNodeProjectionDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub has_blocking_diagnostics: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    RevisionMismatch {
        analysis: GraphRevision,
        document: GraphRevision,
    },
    RegistryMismatch,
    StaleProjectionBasis,
    IncompatibleProjectionGraphs,
    InvalidDelta,
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RevisionMismatch { analysis, document } => write!(
                formatter,
                "analysis revision {} does not match document revision {}",
                analysis.get(),
                document.get()
            ),
            Self::RegistryMismatch => {
                formatter.write_str("analysis registry fingerprint does not match registry")
            }
            Self::StaleProjectionBasis => {
                formatter.write_str("projection delta does not start at the current basis")
            }
            Self::IncompatibleProjectionGraphs => {
                formatter.write_str("projection snapshots belong to different graphs")
            }
            Self::InvalidDelta => {
                formatter.write_str("projection delta is internally inconsistent")
            }
        }
    }
}

impl std::error::Error for ProjectionError {}

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

type EditorDiagnostic = NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>;

pub fn build_editor_graph_projection(
    graph_path: impl Into<Box<str>>,
    document: &GraphDocument,
    analysis: &EditorAnalysis,
    registry: &NodeRegistry,
    localization: &impl LocalizationLookup,
) -> Result<EditorGraphProjectionDto, ProjectionError> {
    EditorGraphProjectionDto::from_sources(graph_path, analysis, document, registry, localization)
}

impl EditorGraphProjectionDto {
    pub fn from_sources(
        graph_path: impl Into<Box<str>>,
        analysis: &EditorAnalysis,
        document: &GraphDocument,
        registry: &NodeRegistry,
        localization: &impl LocalizationLookup,
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

        let nodes = document
            .nodes
            .values()
            .map(|node| {
                let protocol = registry.get(&node.node_type).map(|entry| &entry.protocol);
                let normalized = analyzed_nodes
                    .get(&node.id)
                    .map(|node| &node.normalized_parameters);
                let ports = interfaces
                    .get(&node.id)
                    .map(|interface| {
                        interface
                            .ports
                            .iter()
                            .filter_map(|port| {
                                let spec = protocol?
                                    .interface
                                    .ports
                                    .iter()
                                    .find(|spec| spec.key == port.template)?;
                                let orphan = port.status == ResolvedPortStatus::Orphan;
                                let instance_kind = project_instance_kind(&spec.instances);
                                let instance_count = interface
                                    .ports
                                    .iter()
                                    .filter(|candidate| candidate.template == port.template)
                                    .filter(|candidate| candidate.address.is_instance())
                                    .count();
                                let can_remove = can_remove_port(
                                    &port.address,
                                    orphan,
                                    &spec.instances,
                                    instance_count,
                                );
                                let connections = project_connection_capability(
                                    document,
                                    &port.address,
                                    spec.connections,
                                    orphan,
                                );
                                let instance_label = orphan_label(document, &port.address);
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
                                    resolved_type: analysis
                                        .partial_types
                                        .get(&port.address)
                                        .map(project_type_summary),
                                    resolved_schema: analysis
                                        .partial_schemas
                                        .get(&port.address)
                                        .map(project_schema_summary),
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
                                    multiline,
                                    value: normalized
                                        .and_then(|values| values.get(&parameter.key))
                                        .cloned()
                                        .or_else(|| node.parameters.get(&parameter.key).cloned()),
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
                        title: localization
                            .text(&protocol.catalog.title_key, &DiagnosticArguments::new()),
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
                let capabilities = project_node_capabilities(protocol.map(|value| value.as_ref()));
                EditorNodeProjectionDto {
                    graph_path: graph_path.clone(),
                    source_revision,
                    node_id: node.id.to_string().into(),
                    node_type_id: node.node_type.as_str().into(),
                    display,
                    ports,
                    parameter_editors,
                    capabilities,
                    diagnostics: node_diagnostics,
                }
            })
            .collect();

        Ok(Self {
            basis,
            graph_path,
            source_revision,
            nodes,
            has_blocking_diagnostics: diagnostics.iter().any(|diagnostic| diagnostic.blocking),
            diagnostics,
        })
    }

    /// Applies a complete revision transition only when its old basis exactly
    /// matches this projection. Validation and replacement happen before commit.
    pub fn apply_delta(&mut self, delta: GraphProjectionDelta) -> Result<(), ProjectionError> {
        if self.basis != delta.from_basis {
            return Err(ProjectionError::StaleProjectionBasis);
        }
        validate_delta(&delta)?;

        let removed = delta.removed_node_ids.iter().collect::<BTreeSet<_>>();
        let replacements = delta
            .node_replacements
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let mut next_nodes = self
            .nodes
            .iter()
            .filter(|node| !removed.contains(&node.node_id))
            .filter(|node| !replacements.contains_key(node.node_id.as_ref()))
            .cloned()
            .collect::<Vec<_>>();
        next_nodes.extend(delta.node_replacements.iter().cloned());
        next_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));

        self.basis = delta.to_basis;
        self.graph_path = self.basis.graph_path.clone();
        self.source_revision = self.basis.graph_revision;
        self.nodes = next_nodes;
        self.diagnostics = delta.diagnostics;
        self.has_blocking_diagnostics = delta.has_blocking_diagnostics;
        Ok(())
    }
}

impl GraphProjectionDelta {
    pub fn between(
        previous: &EditorGraphProjectionDto,
        next: &EditorGraphProjectionDto,
    ) -> Result<Self, ProjectionError> {
        if previous.basis.graph_path != next.basis.graph_path {
            return Err(ProjectionError::IncompatibleProjectionGraphs);
        }
        let previous_nodes = previous
            .nodes
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let next_nodes = next
            .nodes
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let removed_node_ids = previous_nodes
            .keys()
            .filter(|node_id| !next_nodes.contains_key(**node_id))
            .map(|node_id| Box::<str>::from(*node_id))
            .collect();
        let node_replacements = next
            .nodes
            .iter()
            .filter(|node| previous_nodes.get(node.node_id.as_ref()).copied() != Some(*node))
            .cloned()
            .collect();

        Ok(Self {
            from_basis: previous.basis.clone(),
            to_basis: next.basis.clone(),
            removed_node_ids,
            node_replacements,
            diagnostics: next.diagnostics.clone(),
            has_blocking_diagnostics: next.has_blocking_diagnostics,
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

fn validate_delta(delta: &GraphProjectionDelta) -> Result<(), ProjectionError> {
    if delta.from_basis.graph_path != delta.to_basis.graph_path
        || delta.to_basis.graph_revision < delta.from_basis.graph_revision
        || delta.node_replacements.iter().any(|node| {
            node.graph_path != delta.to_basis.graph_path
                || node.source_revision != delta.to_basis.graph_revision
        })
    {
        return Err(ProjectionError::InvalidDelta);
    }
    let mut identities = BTreeSet::new();
    if delta
        .node_replacements
        .iter()
        .any(|node| !identities.insert(node.node_id.as_ref()))
        || delta
            .removed_node_ids
            .iter()
            .any(|node_id| identities.contains(node_id.as_ref()))
    {
        return Err(ProjectionError::InvalidDelta);
    }
    Ok(())
}

fn project_node_capabilities(
    protocol: Option<&crate::node_system::protocol::NodeProtocol>,
) -> NodeCapabilitiesDto {
    let managed = protocol.is_some_and(|protocol| protocol.managed_role.is_some());
    NodeCapabilitiesDto {
        managed,
        can_copy: !managed,
        can_delete: !managed,
        can_edit_label: true,
        can_edit_parameters: protocol.is_some_and(|protocol| {
            protocol
                .parameters
                .parameters
                .iter()
                .any(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden))
        }),
        has_dynamic_ports: protocol.is_some_and(|protocol| {
            protocol
                .interface
                .ports
                .iter()
                .any(|port| !matches!(port.instances, PortInstances::Declared))
        }),
        supports_inline_literals: protocol.is_some_and(|protocol| {
            protocol
                .interface
                .ports
                .iter()
                .any(|port| matches!(port.editor, PortEditorSpec::InlineLiteral))
        }),
    }
}

fn project_instance_kind(instances: &PortInstances) -> PortInstanceKindDto {
    match instances {
        PortInstances::Declared => PortInstanceKindDto::Declared,
        PortInstances::UserCreated { .. } => PortInstanceKindDto::UserCreated,
        PortInstances::Derived { .. } => PortInstanceKindDto::Derived,
    }
}

fn can_remove_port(
    address: &PortAddress,
    orphan: bool,
    instances: &PortInstances,
    instance_count: usize,
) -> bool {
    if !address.is_instance() {
        return false;
    }
    if orphan {
        return true;
    }
    matches!(instances, PortInstances::UserCreated { min, .. } if instance_count > *min as usize)
}

fn project_connection_capability(
    document: &GraphDocument,
    address: &PortAddress,
    capability: ConnectionsPerPort,
    orphan: bool,
) -> PortConnectionCapabilityDto {
    let current = document
        .connections
        .values()
        .filter(|connection| connection.input == *address || connection.output == *address)
        .count() as u32;
    let (maximum, ordered) = match capability {
        ConnectionsPerPort::Single => (Some(1), false),
        ConnectionsPerPort::Multiple { max, ordered } => (max.map(u32::from), ordered),
    };
    PortConnectionCapabilityDto {
        current,
        maximum,
        ordered,
        can_connect: !orphan && maximum.is_none_or(|maximum| current < maximum),
    }
}

fn project_type_summary(value: &TypeExpr) -> TypeSummaryDto {
    TypeSummaryDto {
        display: type_display(value).into(),
        resolved: type_is_resolved(value),
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

fn project_schema_summary(value: &SchemaExpr) -> SchemaSummaryDto {
    let kind = match value {
        SchemaExpr::Input(_) => SchemaSummaryKindDto::Input,
        SchemaExpr::Project { .. } => SchemaSummaryKindDto::Project,
        SchemaExpr::Append { .. } => SchemaSummaryKindDto::Append,
        SchemaExpr::Rename { .. } => SchemaSummaryKindDto::Rename,
        SchemaExpr::Filter { .. } => SchemaSummaryKindDto::Filter,
        SchemaExpr::Derived { .. } => SchemaSummaryKindDto::Derived,
    };
    SchemaSummaryDto { kind }
}

fn orphan_label(document: &GraphDocument, address: &PortAddress) -> Option<Box<str>> {
    match document.port_bindings.get(address) {
        Some(crate::node_system::document::DynamicPortBinding::Orphan { last_known, .. }) => {
            Some(last_known.label.as_str().into())
        }
        _ => None,
    }
}

fn project_parameter_editor(
    editor: &ParameterEditorSpec,
) -> Option<(ParameterEditorKindDto, bool)> {
    Some(match editor {
        ParameterEditorSpec::Auto => (ParameterEditorKindDto::Auto, false),
        ParameterEditorSpec::Hidden => return None,
        ParameterEditorSpec::Text { multiline } => (ParameterEditorKindDto::Text, *multiline),
        ParameterEditorSpec::Number => (ParameterEditorKindDto::Number, false),
        ParameterEditorSpec::Toggle => (ParameterEditorKindDto::Toggle, false),
        ParameterEditorSpec::Select => (ParameterEditorKindDto::Select, false),
        ParameterEditorSpec::Resource => (ParameterEditorKindDto::Resource, false),
    })
}

fn project_address(address: &PortAddress) -> PortAddressDto {
    match &address.port {
        PortRef::Declared { key } => PortAddressDto::Declared {
            node_id: address.node_id.to_string().into(),
            port_key: key.as_str().into(),
        },
        PortRef::Instance {
            template,
            instance_id,
        } => PortAddressDto::Instance {
            node_id: address.node_id.to_string().into(),
            template_key: template.as_str().into(),
            instance_id: instance_id.to_string().into(),
        },
    }
}

fn project_diagnostic(
    diagnostic: &EditorDiagnostic,
    localization: &impl LocalizationLookup,
) -> DiagnosticDto {
    DiagnosticDto {
        code: diagnostic.code.as_str().into(),
        message: localization.text(&diagnostic.message_key, &diagnostic.arguments),
        severity: diagnostic.severity.into(),
        blocking: diagnostic.severity.is_blocking(),
        location: project_location(&diagnostic.primary),
        related: diagnostic.related.iter().map(project_location).collect(),
    }
}

fn project_location(
    location: &DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
) -> DiagnosticLocationDto {
    match location {
        DiagnosticLocation::Graph => DiagnosticLocationDto::Graph,
        DiagnosticLocation::Node(node_id) => DiagnosticLocationDto::Node {
            node_id: node_id.to_string().into(),
        },
        DiagnosticLocation::Port(address) => DiagnosticLocationDto::Port {
            address: project_address(address),
        },
        DiagnosticLocation::Connection(connection_id) => DiagnosticLocationDto::Connection {
            connection_id: connection_id.to_string().into(),
        },
        DiagnosticLocation::Parameter { node_id, key } => DiagnosticLocationDto::Parameter {
            node_id: node_id.to_string().into(),
            key: key.as_str().into(),
        },
        DiagnosticLocation::Resource(identity) => DiagnosticLocationDto::Resource {
            identity: identity.clone(),
        },
    }
}

fn diagnostic_belongs_to_node(
    diagnostic: &EditorDiagnostic,
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

impl From<PortDirection> for PortDirectionDto {
    fn from(value: PortDirection) -> Self {
        match value {
            PortDirection::Input => Self::Input,
            PortDirection::Output => Self::Output,
        }
    }
}

impl From<PortKind> for PortKindDto {
    fn from(value: PortKind) -> Self {
        match value {
            PortKind::Data => Self::Data,
            PortKind::Control => Self::Control,
            PortKind::Effect => Self::Effect,
        }
    }
}

impl From<ResolvedPortStatus> for ResolvedPortStatusDto {
    fn from(value: ResolvedPortStatus) -> Self {
        match value {
            ResolvedPortStatus::Resolved => Self::Resolved,
            ResolvedPortStatus::Orphan => Self::Orphan,
        }
    }
}

impl From<DiagnosticSeverity> for DiagnosticSeverityDto {
    fn from(value: DiagnosticSeverity) -> Self {
        match value {
            DiagnosticSeverity::Error => Self::Error,
            DiagnosticSeverity::Warning => Self::Warning,
            DiagnosticSeverity::Information => Self::Information,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::registry::RegistryFingerprint;

    fn basis(revision: u64) -> ProjectionBasis {
        ProjectionBasis {
            graph_path: "functions/main".into(),
            graph_revision: revision,
            registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
            resource_versions: BTreeMap::new(),
        }
    }

    fn capabilities() -> NodeCapabilitiesDto {
        NodeCapabilitiesDto {
            managed: false,
            can_copy: true,
            can_delete: true,
            can_edit_label: true,
            can_edit_parameters: false,
            has_dynamic_ports: true,
            supports_inline_literals: false,
        }
    }

    fn port(key: &str) -> ResolvedPortDto {
        ResolvedPortDto {
            address: PortAddressDto::Declared {
                node_id: "node-1".into(),
                port_key: key.into(),
            },
            template_key: key.into(),
            display: PortDisplayDto {
                label: key.into(),
                instance_label: None,
            },
            direction: PortDirectionDto::Input,
            kind: PortKindDto::Data,
            instance_kind: PortInstanceKindDto::Declared,
            orphan: false,
            can_remove: false,
            connections: PortConnectionCapabilityDto {
                current: 0,
                maximum: Some(1),
                ordered: false,
                can_connect: true,
            },
            resolved_type: Some(TypeSummaryDto {
                display: "core.string".into(),
                resolved: true,
            }),
            resolved_schema: None,
            status: ResolvedPortStatusDto::Resolved,
        }
    }

    fn node(revision: u64, ports: Vec<ResolvedPortDto>) -> EditorNodeProjectionDto {
        EditorNodeProjectionDto {
            graph_path: "functions/main".into(),
            source_revision: revision,
            node_id: "node-1".into(),
            node_type_id: "test.node".into(),
            display: NodeDisplayDto {
                title: "Test".into(),
                description: None,
                user_label: None,
                icon_id: None,
                style_id: None,
            },
            ports,
            parameter_editors: Vec::new(),
            capabilities: capabilities(),
            diagnostics: Vec::new(),
        }
    }

    fn projection(revision: u64, ports: Vec<ResolvedPortDto>) -> EditorGraphProjectionDto {
        EditorGraphProjectionDto {
            basis: basis(revision),
            graph_path: "functions/main".into(),
            source_revision: revision,
            nodes: vec![node(revision, ports)],
            diagnostics: Vec::new(),
            has_blocking_diagnostics: false,
        }
    }

    #[test]
    fn projection_basis_is_consistent_with_envelope() {
        let projection = projection(4, vec![port("value")]);

        assert_eq!(projection.basis.graph_path, projection.graph_path);
        assert_eq!(projection.basis.graph_revision, projection.source_revision);
    }

    #[test]
    fn stale_basis_is_rejected_without_mutation() {
        let mut current = projection(2, vec![port("old")]);
        let original = current.clone();
        let stale = projection(1, vec![port("stale")]);
        let next = projection(3, vec![port("new")]);
        let delta = GraphProjectionDelta::between(&stale, &next).unwrap();

        assert_eq!(
            current.apply_delta(delta).unwrap_err(),
            ProjectionError::StaleProjectionBasis
        );
        assert_eq!(current, original);
    }

    #[test]
    fn dynamic_interface_is_replaced_atomically_as_a_whole_node() {
        let mut current = projection(5, vec![port("a"), port("b")]);
        let next = projection(6, vec![port("c")]);
        let delta = GraphProjectionDelta::between(&current, &next).unwrap();

        assert_eq!(delta.node_replacements, next.nodes);
        assert_eq!(delta.node_replacements[0].ports, vec![port("c")]);
        let serialized = serde_json::to_value(&delta).unwrap();
        assert!(serialized.get("addedPins").is_none());

        current.apply_delta(delta).unwrap();
        assert_eq!(current, next);
    }

    #[test]
    fn editor_dto_does_not_serialize_protocol_ast() {
        let json = serde_json::to_string(&projection(1, vec![port("value")])).unwrap();

        for forbidden in [
            "execution",
            "interface",
            "typeConstraints",
            "valueType",
            "managedRole",
            "protocolFingerprint",
        ] {
            assert!(
                !json.contains(forbidden),
                "unexpected protocol field: {forbidden}"
            );
        }
    }
}
