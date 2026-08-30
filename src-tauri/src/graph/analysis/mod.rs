use crate::graph::settings::GraphCompileSettings;
use yss_graph_analysis_contract::{
    CompilationBasis, DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity,
    ResourceVersionSet,
};
use yss_graph_document::GraphRevision;
use yss_graph_document::{ConnectionId, GraphDocument, NodeId, PortAddress, TypedValue};
use yss_graph_document::{DynamicPortBinding, PortRef};
use yss_graph_protocol::{
    ConnectionsPerPort, ParameterEditorSpec, ParameterKey, ParameterPresentation, PortDirection,
    PortEditorSpec, PortInstances, PortKey, PortKind, RelationalScalarType, ResolvedSchemaFact,
    SchemaExpr, TypeExpr,
};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::ResourceCatalogSnapshot;

pub(crate) mod result_category;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphNodeFact {
    pub node_id: NodeId,
    pub node_type: yss_graph_protocol::NodeTypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphProjectionFacts {
    nodes: Box<[GraphNodeProjectionFacts]>,
    diagnostics: Box<[GraphDiagnosticFact]>,
    outcome: GraphCompilationOutcome,
}

impl GraphProjectionFacts {
    pub fn new(
        nodes: impl IntoIterator<Item = GraphNodeProjectionFacts>,
        diagnostics: impl IntoIterator<Item = GraphDiagnosticFact>,
        outcome: GraphCompilationOutcome,
    ) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            diagnostics: diagnostics.into_iter().collect(),
            outcome,
        }
    }

    pub fn nodes(&self) -> &[GraphNodeProjectionFacts] {
        &self.nodes
    }

    pub fn diagnostics(&self) -> &[GraphDiagnosticFact] {
        &self.diagnostics
    }

    pub const fn outcome(&self) -> &GraphCompilationOutcome {
        &self.outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphNodeProjectionFacts {
    pub node_id: NodeId,
    pub node_type: yss_graph_protocol::NodeTypeId,
    pub instance_title: Option<Box<str>>,
    pub title: Box<str>,
    pub icon_id: Option<Box<str>>,
    pub style_id: Option<Box<str>>,
    pub managed: bool,
    pub parameters: Box<[GraphParameterFact]>,
    pub ports: Box<[GraphPortFact]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphParameterFact {
    pub key: ParameterKey,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
    pub editor: ParameterEditorSpec,
    pub presentation: ParameterPresentation,
    pub value_type: TypeExpr,
    pub inherited_value: Option<TypedValue>,
    pub value_source: Option<GraphParameterValueSource>,
    pub options: Box<[Box<str>]>,
    pub configuration: Option<GraphParameterConfigurationFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphParameterValueSource {
    Project,
    Node,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphParameterConfigurationFact {
    ProjectColumns {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        options: Box<[GraphColumnFact]>,
        value: Box<[Box<str>]>,
    },
    FilterPredicate {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        columns: Box<[GraphFilterColumnFact]>,
        value: Option<TypedValue>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphColumnFact {
    pub name: Box<str>,
    pub data_type: RelationalScalarType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFilterColumnFact {
    pub name: Box<str>,
    pub data_type: RelationalScalarType,
    pub operators: Box<[yss_graph_protocol::dataframe::FilterOperator]>,
    pub literal_types: Box<[GraphFilterLiteralType]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphFilterLiteralType {
    Boolean,
    Integer,
    Decimal,
    String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPortFact {
    pub address: PortAddress,
    pub template_key: PortKey,
    pub label: Box<str>,
    pub instance_label: Option<Box<str>>,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub instance_kind: GraphPortInstanceKind,
    pub orphan: bool,
    pub connections: GraphPortConnectionFacts,
    pub member_minimum: u16,
    pub member_instance_count: usize,
    pub member_complete: bool,
    pub editor: GraphPortEditorFact,
    pub protocol_default: Option<TypedValue>,
    pub value_type: TypeExpr,
    pub schema: Option<SchemaExpr>,
    pub resolved_schema: Option<ResolvedSchemaFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphPortInstanceKind {
    Declared,
    UserCreated,
    Derived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphPortConnectionFacts {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphPortEditorFact {
    Default,
    Hidden,
    InlineLiteral,
    SchemaColumns { allow_multiple: bool },
}

pub type GraphDiagnosticLocation = DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphDiagnosticFact {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub arguments: DiagnosticArguments,
    pub primary: GraphDiagnosticLocation,
    pub related: Box<[GraphDiagnosticLocation]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphCompilationOutcome {
    Complete,
    Incomplete,
    InternalFailure {
        stage: GraphCompilationStage,
        code: Box<str>,
        node_id: Option<NodeId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphCompilationStage {
    Analysis,
    Lowering,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphAnalysis {
    nodes: Box<[GraphNodeFact]>,
    registry_fingerprint: [u8; 32],
    graph_revision: u64,
    resource_versions: ResourceVersionSet,
    projection_facts: Option<GraphProjectionFacts>,
}

impl GraphAnalysis {
    pub fn nodes(&self) -> &[GraphNodeFact] {
        &self.nodes
    }

    pub fn registry_fingerprint(&self) -> &[u8; 32] {
        &self.registry_fingerprint
    }

    pub const fn graph_revision(&self) -> u64 {
        self.graph_revision
    }

    pub fn resource_versions(&self) -> &ResourceVersionSet {
        &self.resource_versions
    }

    pub fn projection_facts(&self) -> Option<&GraphProjectionFacts> {
        self.projection_facts.as_ref()
    }

    pub fn with_projection_facts(mut self, facts: GraphProjectionFacts) -> Self {
        self.projection_facts = Some(facts);
        self
    }
}

pub struct GraphAnalysisInput<'a> {
    pub document: &'a GraphDocument,
    pub catalog: &'a ResourceCatalogSnapshot,
    pub settings: &'a GraphCompileSettings,
    pub basis: &'a CompilationBasis<GraphRevision>,
}

pub fn analyze(input: GraphAnalysisInput<'_>) -> GraphAnalysis {
    let _ = (
        input.catalog.fingerprint(),
        input.settings.absolute_tolerance,
    );
    let nodes = input
        .document
        .nodes
        .values()
        .map(|node| GraphNodeFact {
            node_id: node.id,
            node_type: node.node_type.clone(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    GraphAnalysis {
        nodes,
        registry_fingerprint: *input.basis.registry_fingerprint.as_bytes(),
        graph_revision: input.basis.graph_revision.get(),
        resource_versions: input.basis.resource_versions.clone(),
        projection_facts: None,
    }
}

pub(crate) fn projection_facts(
    document: &GraphDocument,
    registry: &NodeRegistry,
) -> GraphProjectionFacts {
    let mut complete = true;
    let nodes = document
        .nodes
        .values()
        .map(|node| {
            let Some(protocol) = registry.protocol(&node.node_type) else {
                complete = false;
                return GraphNodeProjectionFacts {
                    node_id: node.id,
                    node_type: node.node_type.clone(),
                    instance_title: None,
                    title: node.node_type.as_str().into(),
                    icon_id: None,
                    style_id: None,
                    managed: false,
                    parameters: Box::new([]),
                    ports: Box::new([]),
                };
            };

            let mut ports = protocol
                .interface
                .ports
                .iter()
                .map(|spec| projection_port(document, node.id, protocol, spec, None))
                .collect::<Vec<_>>();
            for (address, binding) in &document.port_bindings {
                if address.node_id != node.id || ports.iter().any(|port| port.address == *address) {
                    continue;
                }
                let template = match &address.port {
                    PortRef::Declared { key } | PortRef::Instance { template: key, .. } => key,
                };
                let Some(spec) = protocol
                    .interface
                    .ports
                    .iter()
                    .find(|spec| &spec.key == template)
                else {
                    complete = false;
                    continue;
                };
                ports.push(projection_port(
                    document,
                    node.id,
                    protocol,
                    spec,
                    Some((address, binding)),
                ));
            }

            GraphNodeProjectionFacts {
                node_id: node.id,
                node_type: node.node_type.clone(),
                instance_title: None,
                title: protocol.catalog.title_key.as_str().into(),
                icon_id: Some(protocol.catalog.icon_id.as_str().into()),
                style_id: Some(protocol.catalog.style_id.as_str().into()),
                managed: protocol.managed_role.is_some(),
                parameters: protocol
                    .parameters
                    .parameters
                    .iter()
                    .map(|parameter| GraphParameterFact {
                        key: parameter.key.clone(),
                        title: parameter.title_key.as_str().into(),
                        description: parameter
                            .description_key
                            .as_ref()
                            .map(|key| key.as_str().into()),
                        editor: parameter.editor.clone(),
                        presentation: parameter.presentation,
                        value_type: parameter.value_type.clone(),
                        inherited_value: None,
                        value_source: node
                            .parameters
                            .contains_key(&parameter.key)
                            .then_some(GraphParameterValueSource::Node),
                        options: Box::new([]),
                        configuration: None,
                    })
                    .collect(),
                ports: ports.into_boxed_slice(),
            }
        })
        .collect::<Vec<_>>();
    GraphProjectionFacts::new(
        nodes,
        [],
        if complete {
            GraphCompilationOutcome::Complete
        } else {
            GraphCompilationOutcome::Incomplete
        },
    )
}

fn projection_port(
    document: &GraphDocument,
    node_id: NodeId,
    protocol: &yss_graph_protocol::NodeProtocol,
    spec: &yss_graph_protocol::PortSpec,
    dynamic: Option<(&PortAddress, &DynamicPortBinding)>,
) -> GraphPortFact {
    let address = dynamic
        .map(|(address, _)| address.clone())
        .unwrap_or_else(|| PortAddress::declared(node_id, spec.key.clone()));
    let (instance_kind, orphan, instance_label, value_type) = match dynamic {
        None => (
            graph_port_instance_kind(&spec.instances),
            false,
            None,
            spec.value_type.clone(),
        ),
        Some((_, DynamicPortBinding::UserCreated { .. })) => (
            GraphPortInstanceKind::UserCreated,
            false,
            None,
            spec.value_type.clone(),
        ),
        Some((_, DynamicPortBinding::Resolved { last_known, .. })) => (
            GraphPortInstanceKind::Derived,
            false,
            Some(last_known.label.clone().into_boxed_str()),
            last_known
                .value_type
                .clone()
                .unwrap_or_else(|| spec.value_type.clone()),
        ),
        Some((_, DynamicPortBinding::Orphan { last_known, .. })) => (
            GraphPortInstanceKind::Derived,
            true,
            Some(last_known.label.clone().into_boxed_str()),
            last_known
                .value_type
                .clone()
                .unwrap_or_else(|| spec.value_type.clone()),
        ),
    };
    let connections = document
        .connections
        .values()
        .filter(|connection| connection.input == address || connection.output == address)
        .count() as u32;
    let (maximum, ordered) = match spec.connections {
        ConnectionsPerPort::Single => (Some(1), false),
        ConnectionsPerPort::Multiple { max, ordered } => (max.map(u32::from), ordered),
    };
    let (member_minimum, member_instance_count, member_complete) = protocol
        .interface
        .member_group_for_template(&spec.key)
        .map(|group| {
            let member_instance_count = document
                .port_bindings
                .keys()
                .filter(|address| match &address.port {
                    PortRef::Declared { key } | PortRef::Instance { template: key, .. } => {
                        key == &spec.key
                    }
                })
                .count()
                .saturating_add(1);
            (
                group.min,
                member_instance_count,
                member_instance_count >= usize::from(group.min),
            )
        })
        .unwrap_or((0, 1, true));
    GraphPortFact {
        address,
        template_key: spec.key.clone(),
        label: instance_label.clone().unwrap_or_else(|| spec.title.clone()),
        instance_label,
        direction: spec.direction,
        kind: spec.kind,
        instance_kind,
        orphan,
        connections: GraphPortConnectionFacts {
            current: connections,
            maximum,
            ordered,
        },
        member_minimum,
        member_instance_count,
        member_complete,
        editor: graph_port_editor_fact(&spec.editor),
        protocol_default: spec.input_binding.as_ref().and_then(|binding| {
            binding
                .default_value
                .as_ref()
                .map(|value| yss_graph_protocol::protocol_value_to_json(&value.value))
        }),
        value_type,
        schema: spec.schema.clone(),
        resolved_schema: None,
    }
}

fn graph_port_instance_kind(instances: &PortInstances) -> GraphPortInstanceKind {
    match instances {
        PortInstances::Declared => GraphPortInstanceKind::Declared,
        PortInstances::UserCreated { .. } => GraphPortInstanceKind::UserCreated,
        PortInstances::Derived { .. } => GraphPortInstanceKind::Derived,
    }
}

fn graph_port_editor_fact(editor: &PortEditorSpec) -> GraphPortEditorFact {
    match editor {
        PortEditorSpec::Default => GraphPortEditorFact::Default,
        PortEditorSpec::Hidden => GraphPortEditorFact::Hidden,
        PortEditorSpec::InlineLiteral => GraphPortEditorFact::InlineLiteral,
        PortEditorSpec::SchemaColumns { allow_multiple } => GraphPortEditorFact::SchemaColumns {
            allow_multiple: *allow_multiple,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use yss_graph_analysis_contract::CompilationBasis;
    use yss_graph_registry::RegistryFingerprint;

    #[test]
    fn analysis_accepts_neutral_document_catalog_settings_and_basis() {
        let document = GraphDocument::default();
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            yss_graph_resource_contract::ResourceCatalogFingerprint::from_bytes([3; 32]),
        );
        let settings = GraphCompileSettings {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-9,
        };
        let basis = CompilationBasis {
            graph_revision: GraphRevision::new(1),
            registry_fingerprint: RegistryFingerprint::from_bytes([4; 32]),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        };
        let analysis = analyze(GraphAnalysisInput {
            document: &document,
            catalog: &catalog,
            settings: &settings,
            basis: &basis,
        });
        assert!(analysis.nodes().is_empty());
        assert_eq!(analysis.registry_fingerprint(), &[4; 32]);
    }
}
