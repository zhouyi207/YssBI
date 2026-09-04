//! Graph document analysis and editor projection facts.
//!
//! Serializable semantic-analysis contracts remain owned by
//! `yss-graph-analysis-contract`; this crate owns the executable analysis behavior.

#![deny(unused_must_use)]

use yss_graph_analysis_contract::{
    CompilationBasis, DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity,
    ResourceVersionSet,
};
use yss_graph_document::GraphRevision;
use yss_graph_document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, GraphDocument, NodeId, OrderKey,
    PortAddress, PortRef, TypedValue,
};
use yss_graph_document_edit::{port_member_group_state, user_created_port_instance_count};
use yss_graph_protocol::{
    ConnectionsPerPort, ParameterEditorSpec, ParameterKey, ParameterPresentation, PortDirection,
    PortEditorSpec, PortInstances, PortKey, RelationalScalarType, ResolvedSchemaFact, SchemaExpr,
    TypeExpr,
};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::ResourceCatalogSnapshot;
mod derived_ports;
mod result_category;
mod schema_resolution;

use derived_ports::{derived_port_address, derived_port_members};
use schema_resolution::resolve_editor_schemas;

pub use derived_ports::materialize_derived_port_bindings;

pub use result_category::{
    GraphPlotDataKind, GraphResultCategory, GraphStatisticalReportKind, result_category_for_node,
};

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
    pub port_instance_additions: Box<[GraphPortInstanceAdditionFact]>,
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
    pub label: Box<str>,
    pub instance_label: Option<Box<str>>,
    pub direction: PortDirection,
    pub backing: GraphPortBacking,
    pub orphan: bool,
    pub can_remove: bool,
    pub connections: GraphPortConnectionFacts,
    pub editor: GraphPortEditorFact,
    pub protocol_default: Option<TypedValue>,
    pub value_type: TypeExpr,
    pub schema: Option<SchemaExpr>,
    pub resolved_schema: Option<ResolvedSchemaFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphPortBacking {
    Declared,
    DocumentInstance,
    ProjectedDerived { origin: DynamicMemberLocator },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPortInstanceAdditionFact {
    pub template_key: PortKey,
    pub label: Box<str>,
    pub direction: PortDirection,
    pub can_add: bool,
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
    editor_projection_facts: Option<GraphProjectionFacts>,
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

    pub fn editor_projection_facts(&self) -> Option<&GraphProjectionFacts> {
        self.editor_projection_facts.as_ref()
    }

    pub fn with_editor_projection_facts(mut self, facts: GraphProjectionFacts) -> Self {
        self.editor_projection_facts = Some(facts);
        self
    }
}

pub struct GraphAnalysisInput<'a> {
    pub document: &'a GraphDocument,
    pub basis: &'a CompilationBasis<GraphRevision>,
}

pub fn analyze(input: GraphAnalysisInput<'_>) -> GraphAnalysis {
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
        editor_projection_facts: None,
    }
}

pub fn editor_projection_facts(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
) -> GraphProjectionFacts {
    let mut complete = true;
    let resolved_schemas = resolve_editor_schemas(document, registry, resources);
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
                    port_instance_additions: Box::new([]),
                };
            };

            let node_bindings = document
                .port_bindings
                .iter()
                .filter(|(address, _)| address.node_id == node.id)
                .collect::<Vec<_>>();
            let mut ports = Vec::new();
            for spec in protocol.interface.ports.iter() {
                if matches!(spec.instances, PortInstances::Declared) {
                    let address = PortAddress::declared(node.id, spec.key.clone());
                    ports.push(project_declared_port(
                        document,
                        address.clone(),
                        spec,
                        resolved_schemas.get(&address),
                    ));
                    continue;
                }

                let mut bindings = node_bindings
                    .iter()
                    .copied()
                    .filter(|(address, _)| {
                        matches!(
                            &address.port,
                            PortRef::Instance { template, .. } if template == &spec.key
                        )
                    })
                    .collect::<Vec<_>>();
                bindings.sort_by(
                    |(left_address, left_binding), (right_address, right_binding)| {
                        binding_order(left_binding)
                            .cmp(binding_order(right_binding))
                            .then_with(|| left_address.cmp(right_address))
                    },
                );
                for (address, binding) in bindings {
                    if !binding_matches_policy(binding, &spec.instances) {
                        complete = false;
                        continue;
                    }
                    ports.push(project_bound_port(
                        document,
                        protocol,
                        spec,
                        address,
                        binding,
                        &node_bindings,
                        resolved_schemas.get(address),
                    ));
                }
                if let PortInstances::Derived { resolver } = &spec.instances {
                    for member in derived_port_members(
                        document,
                        node.id,
                        resolver.as_str(),
                        &resolved_schemas,
                        resources,
                    ) {
                        if node_bindings.iter().any(|(_, binding)| {
                            binding_origin(binding).is_some_and(|origin| origin == &member.locator)
                        }) {
                            continue;
                        }
                        let address =
                            derived_port_address(document, node.id, &spec.key, &member.locator);
                        ports.push(project_concrete_port(
                            document,
                            spec,
                            ConcretePortProjection {
                                address,
                                backing: GraphPortBacking::ProjectedDerived {
                                    origin: member.locator,
                                },
                                orphan: false,
                                can_remove: false,
                                instance_label: Some(member.label),
                                value_type: member.value_type,
                                resolved_schema: None,
                            },
                        ));
                    }
                }
            }
            if node_bindings
                .iter()
                .any(|(address, _)| match &address.port {
                    PortRef::Declared { key } | PortRef::Instance { template: key, .. } => {
                        !protocol.interface.ports.iter().any(|spec| &spec.key == key)
                    }
                })
            {
                complete = false;
            }
            let (port_instance_additions, minimum_instances_present) =
                project_port_instance_additions(node.id, protocol, &node_bindings);
            if !minimum_instances_present {
                complete = false;
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
                port_instance_additions: port_instance_additions.into_boxed_slice(),
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

fn binding_order(binding: &DynamicPortBinding) -> &OrderKey {
    match binding {
        DynamicPortBinding::UserCreated { order }
        | DynamicPortBinding::Resolved { order, .. }
        | DynamicPortBinding::Orphan { order, .. } => order,
    }
}

fn binding_origin(binding: &DynamicPortBinding) -> Option<&DynamicMemberLocator> {
    match binding {
        DynamicPortBinding::UserCreated { .. } => None,
        DynamicPortBinding::Resolved { origin, .. } | DynamicPortBinding::Orphan { origin, .. } => {
            Some(origin)
        }
    }
}

fn binding_matches_policy(binding: &DynamicPortBinding, policy: &PortInstances) -> bool {
    matches!(
        (binding, policy),
        (
            DynamicPortBinding::UserCreated { .. },
            PortInstances::UserCreated { .. }
        ) | (
            DynamicPortBinding::Resolved { .. } | DynamicPortBinding::Orphan { .. },
            PortInstances::Derived { .. }
        )
    )
}

fn project_declared_port(
    document: &GraphDocument,
    address: PortAddress,
    spec: &yss_graph_protocol::PortSpec,
    resolved_schema: Option<&ResolvedSchemaFact>,
) -> GraphPortFact {
    debug_assert!(matches!(spec.instances, PortInstances::Declared));
    project_concrete_port(
        document,
        spec,
        ConcretePortProjection {
            address,
            backing: GraphPortBacking::Declared,
            orphan: false,
            can_remove: false,
            instance_label: None,
            value_type: spec.value_type.clone(),
            resolved_schema: resolved_schema.cloned(),
        },
    )
}

fn project_bound_port(
    document: &GraphDocument,
    protocol: &yss_graph_protocol::NodeProtocol,
    spec: &yss_graph_protocol::PortSpec,
    address: &PortAddress,
    binding: &DynamicPortBinding,
    node_bindings: &[(&PortAddress, &DynamicPortBinding)],
    resolved_schema: Option<&ResolvedSchemaFact>,
) -> GraphPortFact {
    let (orphan, can_remove, instance_label, value_type) = match binding {
        DynamicPortBinding::UserCreated { .. } => (
            false,
            can_remove_user_created_port(address.node_id, protocol, spec, address, node_bindings),
            None,
            spec.value_type.clone(),
        ),
        DynamicPortBinding::Resolved { last_known, .. } => (
            false,
            false,
            Some(last_known.label.clone().into_boxed_str()),
            last_known
                .value_type
                .clone()
                .unwrap_or_else(|| spec.value_type.clone()),
        ),
        DynamicPortBinding::Orphan { last_known, .. } => (
            true,
            true,
            Some(last_known.label.clone().into_boxed_str()),
            last_known
                .value_type
                .clone()
                .unwrap_or_else(|| spec.value_type.clone()),
        ),
    };
    project_concrete_port(
        document,
        spec,
        ConcretePortProjection {
            address: address.clone(),
            backing: GraphPortBacking::DocumentInstance,
            orphan,
            can_remove,
            instance_label,
            value_type,
            resolved_schema: resolved_schema.cloned(),
        },
    )
}

struct ConcretePortProjection {
    address: PortAddress,
    backing: GraphPortBacking,
    orphan: bool,
    can_remove: bool,
    instance_label: Option<Box<str>>,
    value_type: TypeExpr,
    resolved_schema: Option<ResolvedSchemaFact>,
}

fn project_concrete_port(
    document: &GraphDocument,
    spec: &yss_graph_protocol::PortSpec,
    projection: ConcretePortProjection,
) -> GraphPortFact {
    let ConcretePortProjection {
        address,
        backing,
        orphan,
        can_remove,
        instance_label,
        value_type,
        resolved_schema,
    } = projection;
    let connections = document
        .connections
        .values()
        .filter(|connection| connection.input == address || connection.output == address)
        .count() as u32;
    let (maximum, ordered) = match spec.connections {
        ConnectionsPerPort::Single => (Some(1), false),
        ConnectionsPerPort::Multiple { max, ordered } => (max.map(u32::from), ordered),
    };
    GraphPortFact {
        address,
        label: instance_label.clone().unwrap_or_else(|| spec.title.clone()),
        instance_label,
        direction: spec.direction,
        backing,
        orphan,
        can_remove,
        connections: GraphPortConnectionFacts {
            current: connections,
            maximum,
            ordered,
        },
        editor: graph_port_editor_fact(&spec.editor),
        protocol_default: spec.input_binding.as_ref().and_then(|binding| {
            binding
                .default_value
                .as_ref()
                .map(|value| yss_graph_protocol::protocol_value_to_json(&value.value))
        }),
        value_type,
        schema: spec.schema.clone(),
        resolved_schema,
    }
}

fn can_remove_user_created_port(
    node_id: NodeId,
    protocol: &yss_graph_protocol::NodeProtocol,
    spec: &yss_graph_protocol::PortSpec,
    address: &PortAddress,
    node_bindings: &[(&PortAddress, &DynamicPortBinding)],
) -> bool {
    let PortRef::Instance { instance_id, .. } = address.port else {
        return false;
    };
    if let Some(group) = protocol.interface.member_group_for_template(&spec.key) {
        let state = port_member_group_state(node_id, group, node_bindings.iter().copied());
        return !state.is_complete(instance_id) || state.complete_count() > usize::from(group.min);
    }
    let PortInstances::UserCreated { min, .. } = spec.instances else {
        return false;
    };
    user_created_port_instance_count(node_id, &spec.key, node_bindings.iter().copied())
        > usize::from(min)
}

fn project_port_instance_additions(
    node_id: NodeId,
    protocol: &yss_graph_protocol::NodeProtocol,
    node_bindings: &[(&PortAddress, &DynamicPortBinding)],
) -> (Vec<GraphPortInstanceAdditionFact>, bool) {
    let mut minimum_instances_present = true;
    let additions = protocol
        .interface
        .ports
        .iter()
        .filter_map(|spec| {
            let PortInstances::UserCreated { min, max } = spec.instances else {
                return None;
            };
            let (minimum_instances, maximum_instances, current_instances) =
                if let Some(group) = protocol.interface.member_group_for_template(&spec.key) {
                    if group.templates.first() != Some(&spec.key) {
                        return None;
                    }
                    (
                        group.min,
                        group.max,
                        port_member_group_state(node_id, group, node_bindings.iter().copied())
                            .complete_count(),
                    )
                } else {
                    (
                        min,
                        max,
                        user_created_port_instance_count(
                            node_id,
                            &spec.key,
                            node_bindings.iter().copied(),
                        ),
                    )
                };
            minimum_instances_present &= current_instances >= usize::from(minimum_instances);
            Some(GraphPortInstanceAdditionFact {
                template_key: spec.key.clone(),
                label: spec.title.clone(),
                direction: spec.direction,
                can_add: maximum_instances
                    .is_none_or(|maximum| current_instances < usize::from(maximum)),
            })
        })
        .collect();
    (additions, minimum_instances_present)
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
    fn analysis_accepts_neutral_document_and_basis() {
        let document = GraphDocument::default();
        let basis = CompilationBasis {
            graph_revision: GraphRevision::new(1),
            registry_fingerprint: RegistryFingerprint::from_bytes([4; 32]),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        };
        let analysis = analyze(GraphAnalysisInput {
            document: &document,
            basis: &basis,
        });
        assert!(analysis.nodes().is_empty());
        assert_eq!(analysis.registry_fingerprint(), &[4; 32]);
    }
}
