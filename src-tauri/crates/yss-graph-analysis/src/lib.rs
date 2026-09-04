//! Graph document analysis and authoritative semantic snapshots.
//!
//! Serializable semantic-analysis contracts remain owned by
//! `yss-graph-analysis-contract`; this crate owns the executable analysis behavior.

#![deny(unused_must_use)]

use yss_graph_analysis_contract::{
    CompilationBasis, DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity,
    ResourceVersionSet,
};
use yss_graph_compiler_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, GraphDocument, GraphResourcePath,
    NodeId, OrderKey, PortAddress, PortRef,
};
use yss_graph_document_edit::{port_member_group_state, user_created_port_instance_count};
use yss_graph_protocol::{
    ConnectionsPerPort, InputCoercionKind, ParameterEditorSpec, ParameterIssueKind, ParameterKey,
    ParameterPresentation, PortCardinality, PortDirection, PortEditorSpec, PortKey,
    RelationalScalarType, ResolvedSchemaFact, ResolvedType, ResourceDisplayKind, SchemaExpr,
    TypeDomain, TypeExpr, TypeId, TypeState, TypeUnknownReason, TypeValidationContext, TypedValue,
    validate_parameter_values,
};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::{GraphResourceId, ResourceCatalogSnapshot};
mod derived_ports;
mod result_category;
mod schema_resolution;
mod type_resolution;

use derived_ports::{derived_port_address, derived_port_members};
use schema_resolution::resolve_graph_schemas;

pub use derived_ports::materialize_derived_port_bindings;

pub use result_category::{
    GraphPlotDataKind, GraphResultCategory, GraphStatisticalReportKind, result_category_for_node,
};
pub use type_resolution::GraphSemanticCache;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphSemanticSnapshot {
    nodes: Box<[GraphNodeSemanticFact]>,
    diagnostics: Box<[GraphDiagnosticFact]>,
    outcome: GraphCompilationOutcome,
}

impl GraphSemanticSnapshot {
    pub fn new(
        nodes: impl IntoIterator<Item = GraphNodeSemanticFact>,
        diagnostics: impl IntoIterator<Item = GraphDiagnosticFact>,
        outcome: GraphCompilationOutcome,
    ) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            diagnostics: diagnostics.into_iter().collect(),
            outcome,
        }
    }

    pub fn nodes(&self) -> &[GraphNodeSemanticFact] {
        &self.nodes
    }

    pub fn diagnostics(&self) -> &[GraphDiagnosticFact] {
        &self.diagnostics
    }

    pub const fn outcome(&self) -> &GraphCompilationOutcome {
        &self.outcome
    }

    pub fn node(&self, node_id: NodeId) -> Option<&GraphNodeSemanticFact> {
        self.nodes.iter().find(|node| node.node_id == node_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphNodeSemanticFact {
    pub node_id: NodeId,
    pub node_type: yss_graph_protocol::NodeTypeId,
    pub instance_title: Option<Box<str>>,
    pub title: Box<str>,
    pub icon_id: Option<Box<str>>,
    pub style_id: Option<Box<str>>,
    pub managed: bool,
    pub parameters: Box<[GraphParameterFact]>,
    pub ports: Box<[GraphPortSemanticFact]>,
    pub port_instance_additions: Box<[GraphPortInstanceAdditionFact]>,
    pub specialization: Option<GraphKernelSpecialization>,
    pub semantic_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphParameterFact {
    pub key: ParameterKey,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
    pub editor: ParameterEditorSpec,
    pub presentation: ParameterPresentation,
    pub value_type: TypeExpr,
    pub inherited_value: Option<serde_json::Value>,
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
        value: Option<serde_json::Value>,
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
pub struct GraphPortSemanticFact {
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
    pub accepted_type: TypeExpr,
    pub accepted_domain: Option<TypeDomain>,
    pub type_state: TypeState,
    pub schema: Option<SchemaExpr>,
    pub resolved_schema: Option<ResolvedSchemaFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphKernelSpecialization {
    pub implementation: Box<str>,
    pub input_types: Box<[GraphPortTypeBinding]>,
    pub output_types: Box<[GraphPortTypeBinding]>,
    pub coercions: Box<[GraphInputCoercion]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPortTypeBinding {
    pub address: PortAddress,
    pub value_type: ResolvedType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphInputCoercion {
    pub address: PortAddress,
    pub kind: InputCoercionKind,
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

fn graph_problem(
    kind: GraphDiagnosticKind,
    primary: GraphDiagnosticLocation,
    arguments: impl IntoIterator<Item = (&'static str, Box<str>)>,
) -> GraphDiagnosticFact {
    GraphDiagnosticFact {
        code: DiagnosticCode::new(kind.code()),
        severity: kind.default_severity(),
        arguments: arguments
            .into_iter()
            .map(|(key, value)| (Box::<str>::from(key), value))
            .collect(),
        primary,
        related: Box::new([]),
    }
}

struct SkipTypeValidation;

impl TypeValidationContext for SkipTypeValidation {
    fn validate_nominal_parameter(
        &self,
        _type_id: &TypeId,
        _value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        None
    }
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
    registry_fingerprint: [u8; 32],
    resource_versions: ResourceVersionSet,
    semantic_snapshot: GraphSemanticSnapshot,
}

impl GraphAnalysis {
    pub fn registry_fingerprint(&self) -> &[u8; 32] {
        &self.registry_fingerprint
    }

    pub fn resource_versions(&self) -> &ResourceVersionSet {
        &self.resource_versions
    }

    pub fn semantic_snapshot(&self) -> &GraphSemanticSnapshot {
        &self.semantic_snapshot
    }

    pub fn with_semantic_snapshot(mut self, snapshot: GraphSemanticSnapshot) -> Self {
        self.semantic_snapshot = snapshot;
        self
    }
}

pub fn analyze(
    basis: &CompilationBasis,
    semantic_snapshot: GraphSemanticSnapshot,
) -> GraphAnalysis {
    GraphAnalysis {
        registry_fingerprint: *basis.registry_fingerprint.as_bytes(),
        resource_versions: basis.resource_versions.clone(),
        semantic_snapshot,
    }
}

pub fn resolve_graph_semantics(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
) -> GraphSemanticSnapshot {
    resolve_graph_semantics_with_cache(
        document,
        registry,
        resources,
        &mut GraphSemanticCache::default(),
    )
}

pub fn resolve_graph_semantics_with_cache(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    cache: &mut GraphSemanticCache,
) -> GraphSemanticSnapshot {
    let mut complete = true;
    let mut diagnostics = Vec::new();
    let resolved_schemas = resolve_graph_schemas(document, registry, resources);
    let mut nodes = document
        .nodes
        .values()
        .map(|node| {
            let Some(protocol) = registry.protocol(&node.node_type) else {
                complete = false;
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::NodeUnknown,
                    GraphDiagnosticLocation::Node(node.id),
                    [("node_type", node.node_type.as_str().into())],
                ));
                return GraphNodeSemanticFact {
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
                    specialization: None,
                    semantic_fingerprint: [0; 32],
                };
            };

            for issue in validate_parameter_values(protocol, &node.parameters, &SkipTypeValidation)
            {
                complete = false;
                let kind = match issue.kind {
                    ParameterIssueKind::Unknown => GraphDiagnosticKind::ParameterUnknown,
                    ParameterIssueKind::Required => GraphDiagnosticKind::ParameterRequired,
                    ParameterIssueKind::InvalidType
                    | ParameterIssueKind::Constraint
                    | ParameterIssueKind::InvalidNominal(_)
                    | ParameterIssueKind::InvalidResourceId => {
                        GraphDiagnosticKind::ParameterInvalid
                    }
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
            for parameter in protocol.parameters.parameters.iter() {
                let ParameterEditorSpec::Resource { kind } = &parameter.editor else {
                    continue;
                };
                let Some(identity) = node
                    .parameters
                    .get(&parameter.key)
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                if resource_exists(resources, *kind, identity) {
                    continue;
                }
                complete = false;
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::ResourceResolutionFailed,
                    GraphDiagnosticLocation::Resource(identity.into()),
                    [("resource_key", identity.into())],
                ));
            }

            let node_bindings = document
                .port_bindings
                .iter()
                .filter(|(address, _)| address.node_id == node.id)
                .collect::<Vec<_>>();
            let mut ports = Vec::new();
            for spec in protocol.interface.ports.iter() {
                if matches!(spec.cardinality, PortCardinality::Declared) {
                    let address = PortAddress::declared(node.id, spec.key.clone());
                    ports.push(project_declared_port(
                        document,
                        address.clone(),
                        spec,
                        resolved_schemas.get(&address),
                    ));
                    continue;
                }

                let derived_members = match &spec.cardinality {
                    PortCardinality::Derived { resolver } => derived_port_members(
                        document,
                        node.id,
                        resolver.as_str(),
                        &resolved_schemas,
                        resources,
                    ),
                    PortCardinality::Declared | PortCardinality::UserCreated { .. } => Vec::new(),
                };
                let derived_by_origin = derived_members
                    .iter()
                    .map(|member| (&member.locator, member))
                    .collect::<std::collections::BTreeMap<_, _>>();

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
                    if !binding_matches_cardinality(binding, &spec.cardinality) {
                        complete = false;
                        diagnostics.push(graph_problem(
                            GraphDiagnosticKind::PortBindingKindMismatch,
                            GraphDiagnosticLocation::Port(address.clone()),
                            [
                                (
                                    "expected_kind",
                                    port_cardinality_kind(&spec.cardinality).into(),
                                ),
                                ("actual_kind", binding_kind(binding).into()),
                            ],
                        ));
                        continue;
                    }
                    let projected = project_bound_port(
                        document,
                        protocol,
                        spec,
                        &node_bindings,
                        BoundPortProjection {
                            address,
                            binding,
                            current_member: binding_origin(binding)
                                .and_then(|origin| derived_by_origin.get(origin).copied()),
                            resolved_schema: resolved_schemas.get(address),
                        },
                    );
                    if projected.orphan {
                        complete = false;
                        diagnostics.push(graph_problem(
                            GraphDiagnosticKind::PortOrphan,
                            GraphDiagnosticLocation::Port(address.clone()),
                            [("port", address.to_string().into())],
                        ));
                    }
                    ports.push(projected);
                }
                if matches!(spec.cardinality, PortCardinality::Derived { .. }) {
                    for member in derived_members {
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
            for (address, _) in node_bindings
                .iter()
                .filter(|(address, _)| match &address.port {
                    PortRef::Declared { key } | PortRef::Instance { template: key, .. } => {
                        !protocol.interface.ports.iter().any(|spec| &spec.key == key)
                    }
                })
            {
                complete = false;
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::PortUnknown,
                    GraphDiagnosticLocation::Port((*address).clone()),
                    [("port", address.to_string().into())],
                ));
            }
            let (port_instance_additions, minimum_instances_present) =
                project_port_instance_additions(node.id, protocol, &node_bindings);
            if !minimum_instances_present {
                complete = false;
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::SemanticInvalid,
                    GraphDiagnosticLocation::Node(node.id),
                    std::iter::empty(),
                ));
            }
            for port in &ports {
                if port.direction == PortDirection::Input
                    && port.connections.current == 0
                    && port.protocol_default.is_none()
                    && document
                        .input_states
                        .get(&port.address)
                        .is_none_or(|state| state.literal_override.is_none())
                {
                    complete = false;
                    diagnostics.push(graph_problem(
                        GraphDiagnosticKind::InputUnbound,
                        GraphDiagnosticLocation::Port(port.address.clone()),
                        [("port", port.address.to_string().into())],
                    ));
                }
                if port.schema.is_some() && port.resolved_schema.is_none() {
                    complete = false;
                    diagnostics.push(graph_problem(
                        GraphDiagnosticKind::InterfaceSchemaDependencyUnresolved,
                        GraphDiagnosticLocation::Port(port.address.clone()),
                        std::iter::empty(),
                    ));
                }
            }

            GraphNodeSemanticFact {
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
                specialization: None,
                semantic_fingerprint: [0; 32],
            }
        })
        .collect::<Vec<_>>();
    let type_diagnostics =
        type_resolution::resolve_node_types(document, registry, resources, &mut nodes, cache);
    if type_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_blocking())
    {
        complete = false;
    }
    diagnostics.extend(type_diagnostics);
    if contains_value_dependency_cycle(document) {
        complete = false;
        diagnostics.push(graph_problem(
            GraphDiagnosticKind::DependencyValueCycle,
            GraphDiagnosticLocation::Graph,
            std::iter::empty(),
        ));
    }
    GraphSemanticSnapshot::new(
        nodes,
        diagnostics,
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

fn resource_exists(
    resources: &ResourceCatalogSnapshot,
    kind: ResourceDisplayKind,
    identity: &str,
) -> bool {
    match kind {
        ResourceDisplayKind::Function => GraphResourcePath::new(identity)
            .ok()
            .is_some_and(|path| resources.function_signature(&path).is_some()),
        ResourceDisplayKind::Variable => resources
            .variable_contract(&GraphResourceId::new(identity))
            .is_some(),
        ResourceDisplayKind::Database => resources
            .database_schema(&GraphResourceId::new(identity))
            .is_some(),
    }
}

pub fn contains_value_dependency_cycle(document: &GraphDocument) -> bool {
    let mut remaining = document
        .nodes
        .keys()
        .map(|node_id| (*node_id, 0_usize))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut dependents = std::collections::BTreeMap::<NodeId, Vec<NodeId>>::new();
    for connection in document.connections.values() {
        let Some(input) = remaining.get_mut(&connection.input.node_id) else {
            continue;
        };
        let Some(next) = input.checked_add(1) else {
            return true;
        };
        *input = next;
        dependents
            .entry(connection.output.node_id)
            .or_default()
            .push(connection.input.node_id);
    }
    let mut ready = remaining
        .iter()
        .filter_map(|(node_id, count)| (*count == 0).then_some(*node_id))
        .collect::<std::collections::VecDeque<_>>();
    let mut visited = 0_usize;
    while let Some(node_id) = ready.pop_front() {
        let Some(next_visited) = visited.checked_add(1) else {
            return true;
        };
        visited = next_visited;
        for dependent in dependents.get(&node_id).into_iter().flatten() {
            let Some(count) = remaining.get_mut(dependent) else {
                continue;
            };
            let Some(next) = count.checked_sub(1) else {
                return true;
            };
            *count = next;
            if *count == 0 {
                ready.push_back(*dependent);
            }
        }
    }
    visited != document.nodes.len()
}

fn binding_origin(binding: &DynamicPortBinding) -> Option<&DynamicMemberLocator> {
    match binding {
        DynamicPortBinding::UserCreated { .. } => None,
        DynamicPortBinding::Resolved { origin, .. } | DynamicPortBinding::Orphan { origin, .. } => {
            Some(origin)
        }
    }
}

fn binding_matches_cardinality(
    binding: &DynamicPortBinding,
    cardinality: &PortCardinality,
) -> bool {
    matches!(
        (binding, cardinality),
        (
            DynamicPortBinding::UserCreated { .. },
            PortCardinality::UserCreated { .. }
        ) | (
            DynamicPortBinding::Resolved { .. } | DynamicPortBinding::Orphan { .. },
            PortCardinality::Derived { .. }
        )
    )
}

fn binding_kind(binding: &DynamicPortBinding) -> &'static str {
    match binding {
        DynamicPortBinding::UserCreated { .. } => "user_created",
        DynamicPortBinding::Resolved { .. } => "resolved",
        DynamicPortBinding::Orphan { .. } => "orphan",
    }
}

fn port_cardinality_kind(cardinality: &PortCardinality) -> &'static str {
    match cardinality {
        PortCardinality::Declared => "declared",
        PortCardinality::UserCreated { .. } => "user_created",
        PortCardinality::Derived { .. } => "derived",
    }
}

fn project_declared_port(
    document: &GraphDocument,
    address: PortAddress,
    spec: &yss_graph_protocol::PortSpec,
    resolved_schema: Option<&ResolvedSchemaFact>,
) -> GraphPortSemanticFact {
    debug_assert!(matches!(spec.cardinality, PortCardinality::Declared));
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
    node_bindings: &[(&PortAddress, &DynamicPortBinding)],
    projection: BoundPortProjection<'_>,
) -> GraphPortSemanticFact {
    let BoundPortProjection {
        address,
        binding,
        current_member,
        resolved_schema,
    } = projection;
    let (orphan, can_remove, instance_label, value_type) = match binding {
        DynamicPortBinding::UserCreated { .. } => (
            false,
            can_remove_user_created_port(address.node_id, protocol, spec, address, node_bindings),
            None,
            spec.value_type.clone(),
        ),
        DynamicPortBinding::Resolved { last_known, .. } => current_member.map_or_else(
            || {
                (
                    true,
                    true,
                    Some(last_known.label.clone().into_boxed_str()),
                    last_known
                        .value_type
                        .clone()
                        .unwrap_or_else(|| spec.value_type.clone()),
                )
            },
            |member| {
                (
                    false,
                    false,
                    Some(member.label.clone()),
                    member.value_type.clone(),
                )
            },
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

struct BoundPortProjection<'a> {
    address: &'a PortAddress,
    binding: &'a DynamicPortBinding,
    current_member: Option<&'a derived_ports::DerivedPortMember>,
    resolved_schema: Option<&'a ResolvedSchemaFact>,
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
) -> GraphPortSemanticFact {
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
    GraphPortSemanticFact {
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
        protocol_default: spec
            .input_binding
            .as_ref()
            .and_then(|binding| binding.default_value.clone()),
        accepted_type: value_type,
        accepted_domain: None,
        type_state: TypeState::Unknown(TypeUnknownReason::UnsupportedDeclaration),
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
    let PortCardinality::UserCreated { min, .. } = spec.cardinality else {
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
            let PortCardinality::UserCreated { min, max } = spec.cardinality else {
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
    use yss_data_contract::DataType;
    use yss_graph_analysis_contract::CompilationBasis;
    use yss_graph_catalog::build_builtin_node_system;
    use yss_graph_document::{DocumentConnection, DocumentNode, NodePosition, ParameterValues};
    use yss_graph_protocol::{NodeTypeId, ParameterConstraint};
    use yss_graph_registry::RegistryFingerprint;
    use yss_graph_resource_contract::{
        ResourceCatalogFingerprint, ResourceCatalogSnapshot, VariableValueContract,
    };

    fn empty_resources() -> ResourceCatalogSnapshot {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    }

    fn resolved_scalar(id: &str) -> ResolvedType {
        ResolvedType::Nominal(TypeId::new(id).expect("fixture type ID is valid"))
    }

    fn resolved_series(element: &str) -> ResolvedType {
        ResolvedType::Applied {
            constructor: yss_graph_protocol::TypeConstructorId::new(
                yss_graph_protocol::DATA_SERIES_CONSTRUCTOR_ID,
            )
            .expect("fixture constructor ID is valid"),
            arguments: Box::new([resolved_scalar(element)]),
        }
    }

    fn add_result_type(source_types: &[(&str, &str)], reverse_order: bool) -> TypeState {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let add_id = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            add_id,
            DocumentNode {
                id: add_id,
                node_type: NodeTypeId::new("yssbi.numeric.add")
                    .expect("built-in Add node type is valid"),
                position: NodePosition { x: 200.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        for (index, (node_type, output_key)) in source_types.iter().enumerate() {
            let source_id = NodeId::new();
            document.nodes.insert(
                source_id,
                DocumentNode {
                    id: source_id,
                    node_type: NodeTypeId::new(*node_type)
                        .expect("fixture source node type is valid"),
                    position: NodePosition {
                        x: 0.0,
                        y: index as f64 * 100.0,
                    },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
            let instance = PortAddress::instance(
                add_id,
                PortKey::new("operands").unwrap(),
                yss_graph_document::PortInstanceId::new(),
            );
            let order = if reverse_order {
                source_types.len() - index
            } else {
                index
            };
            document.port_bindings.insert(
                instance.clone(),
                DynamicPortBinding::UserCreated {
                    order: OrderKey::new(format!("{order:05}")),
                },
            );
            let connection_id = ConnectionId::new();
            document.connections.insert(
                connection_id,
                DocumentConnection {
                    id: connection_id,
                    output: PortAddress::declared(source_id, PortKey::new(*output_key).unwrap()),
                    input: instance,
                    order: None,
                },
            );
        }

        let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
        facts
            .node(add_id)
            .and_then(|node| {
                node.ports.iter().find(|port| {
                    matches!(
                        &port.address.port,
                        PortRef::Declared { key } if key.as_str() == "result"
                    )
                })
            })
            .map(|port| port.type_state.clone())
            .expect("Add result semantic fact is present")
    }

    #[test]
    fn add_resolver_promotes_shape_and_element_independently_of_operand_order() {
        let scalar_int_float = [
            ("yssbi.constant.int64", "value"),
            ("yssbi.constant.float64", "value"),
        ];
        assert_eq!(
            add_result_type(&scalar_int_float, false),
            TypeState::Exact(resolved_scalar("core.float64"))
        );
        assert_eq!(
            add_result_type(&scalar_int_float, true),
            TypeState::Exact(resolved_scalar("core.float64"))
        );
        assert_eq!(
            add_result_type(
                &[
                    ("yssbi.constant.int64", "value"),
                    ("yssbi.constant.int64", "value"),
                ],
                false,
            ),
            TypeState::Exact(resolved_scalar("core.int64"))
        );
        assert_eq!(
            add_result_type(
                &[
                    ("yssbi.data_series.convert.string_to_int64", "output"),
                    ("yssbi.constant.float64", "value"),
                ],
                false,
            ),
            TypeState::Exact(resolved_series("core.float64"))
        );
        assert_eq!(
            add_result_type(
                &[
                    ("yssbi.data_series.convert.string_to_int64", "output"),
                    ("yssbi.data_series.convert.int64_to_float64", "output"),
                ],
                false,
            ),
            TypeState::Exact(resolved_series("core.float64"))
        );

        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let add = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            add,
            DocumentNode {
                id: add,
                node_type: NodeTypeId::new("yssbi.numeric.add").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        for index in 0..2 {
            document.port_bindings.insert(
                PortAddress::instance(
                    add,
                    PortKey::new("operands").unwrap(),
                    yss_graph_document::PortInstanceId::new(),
                ),
                DynamicPortBinding::UserCreated {
                    order: OrderKey::new(format!("{index:05}")),
                },
            );
        }
        let snapshot = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
        let result = snapshot
            .node(add)
            .unwrap()
            .ports
            .iter()
            .find(|port| {
                port.address == PortAddress::declared(add, PortKey::new("result").unwrap())
            })
            .unwrap();
        assert!(matches!(result.type_state, TypeState::Constrained(_)));
        assert!(result.type_state.exact().is_none());
    }

    #[test]
    fn incremental_semantics_equal_full_resolution_and_stop_at_unchanged_output_types() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let source = NodeId::new();
        let view = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            source,
            DocumentNode {
                id: source,
                node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::from([(
                    ParameterKey::new("value").unwrap(),
                    serde_json::json!(1),
                )]),
                user_label: None,
            },
        );
        document.nodes.insert(
            view,
            DocumentNode {
                id: view,
                node_type: NodeTypeId::new("yssbi.debug.view").unwrap(),
                position: NodePosition { x: 200.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let connection_id = ConnectionId::new();
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(source, PortKey::new("value").unwrap()),
                input: PortAddress::declared(view, PortKey::new("data").unwrap()),
                order: None,
            },
        );
        let resources = empty_resources();
        let mut cache = GraphSemanticCache::default();
        resolve_graph_semantics_with_cache(&document, &builtin.registry, &resources, &mut cache);
        assert_eq!(cache.reused_nodes(), 0);

        document
            .nodes
            .get_mut(&source)
            .unwrap()
            .parameters
            .insert(ParameterKey::new("value").unwrap(), serde_json::json!(2));
        let incremental = resolve_graph_semantics_with_cache(
            &document,
            &builtin.registry,
            &resources,
            &mut cache,
        );
        let full = resolve_graph_semantics(&document, &builtin.registry, &resources);

        assert_eq!(cache.reused_nodes(), 1);
        assert_eq!(incremental, full);
    }

    #[test]
    fn semantic_cache_invalidates_a_variable_when_its_resource_type_changes() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let variable = NodeId::new();
        let resource = GraphResourceId::new("variables/cache-type");
        let mut document = GraphDocument::default();
        document.nodes.insert(
            variable,
            DocumentNode {
                id: variable,
                node_type: NodeTypeId::new("yssbi.project.variable.get").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::from([(
                    ParameterKey::new("variable").unwrap(),
                    serde_json::json!(resource.as_str()),
                )]),
                user_label: None,
            },
        );
        let catalog = |data_type, fingerprint| {
            ResourceCatalogSnapshot::new(
                BTreeMap::new(),
                BTreeMap::from([(resource.clone(), VariableValueContract::new(data_type))]),
                BTreeMap::new(),
                ResourceCatalogFingerprint::from_bytes([fingerprint; 32]),
            )
        };
        let mut cache = GraphSemanticCache::default();
        let integer = resolve_graph_semantics_with_cache(
            &document,
            &builtin.registry,
            &catalog(DataType::Int64, 1),
            &mut cache,
        );
        let float = resolve_graph_semantics_with_cache(
            &document,
            &builtin.registry,
            &catalog(DataType::Float64, 2),
            &mut cache,
        );
        let output_type = |snapshot: &GraphSemanticSnapshot| {
            snapshot
                .node(variable)
                .unwrap()
                .ports
                .iter()
                .find(|port| {
                    port.address == PortAddress::declared(variable, PortKey::new("value").unwrap())
                })
                .unwrap()
                .type_state
                .clone()
        };

        assert_eq!(
            output_type(&integer),
            TypeState::Exact(resolved_scalar("core.int64"))
        );
        assert_eq!(
            output_type(&float),
            TypeState::Exact(resolved_scalar("core.float64"))
        );
        assert_eq!(cache.reused_nodes(), 0);
    }

    #[test]
    fn widened_output_preserves_existing_connection_and_reports_the_mismatch() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let left = NodeId::new();
        let right = NodeId::new();
        let add = NodeId::new();
        let consumer = NodeId::new();
        let mut document = GraphDocument::default();
        for (node_id, node_type, x) in [
            (left, "yssbi.constant.int64", 0.0),
            (right, "yssbi.constant.int64", 0.0),
            (add, "yssbi.numeric.add", 200.0),
            (consumer, "yssbi.dataframe.series.int_range", 400.0),
        ] {
            document.nodes.insert(
                node_id,
                DocumentNode {
                    id: node_id,
                    node_type: NodeTypeId::new(node_type).unwrap(),
                    position: NodePosition { x, y: 0.0 },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
        }

        for (index, source) in [left, right].into_iter().enumerate() {
            let operand = PortAddress::instance(
                add,
                PortKey::new("operands").unwrap(),
                yss_graph_document::PortInstanceId::new(),
            );
            document.port_bindings.insert(
                operand.clone(),
                DynamicPortBinding::UserCreated {
                    order: OrderKey::new(format!("{index:05}")),
                },
            );
            let id = ConnectionId::new();
            document.connections.insert(
                id,
                DocumentConnection {
                    id,
                    output: PortAddress::declared(source, PortKey::new("value").unwrap()),
                    input: operand,
                    order: None,
                },
            );
        }
        let downstream_connection = ConnectionId::new();
        document.connections.insert(
            downstream_connection,
            DocumentConnection {
                id: downstream_connection,
                output: PortAddress::declared(add, PortKey::new("result").unwrap()),
                input: PortAddress::declared(consumer, PortKey::new("start").unwrap()),
                order: None,
            },
        );

        let compatible = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
        assert!(!compatible.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_str() == GraphDiagnosticKind::TypeConnectionMismatch.code()
                && diagnostic.primary == GraphDiagnosticLocation::Connection(downstream_connection)
        }));

        document.nodes.get_mut(&right).unwrap().node_type =
            NodeTypeId::new("yssbi.constant.float64").unwrap();
        let widened = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

        assert!(document.connections.contains_key(&downstream_connection));
        assert!(widened.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_str() == GraphDiagnosticKind::TypeConnectionMismatch.code()
                && diagnostic.primary == GraphDiagnosticLocation::Connection(downstream_connection)
        }));
        assert_eq!(
            widened
                .node(add)
                .unwrap()
                .ports
                .iter()
                .find(|port| port.address
                    == PortAddress::declared(add, PortKey::new("result").unwrap()))
                .map(|port| &port.type_state),
            Some(&TypeState::Exact(resolved_scalar("core.float64")))
        );
    }

    #[test]
    fn exact_int_to_float_assignment_is_recorded_in_the_node_specialization() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let source = NodeId::new();
        let target = NodeId::new();
        let mut document = GraphDocument::default();
        for (node_id, node_type) in [
            (source, "yssbi.constant.int64"),
            (target, "yssbi.distribution.normal.sample"),
        ] {
            document.nodes.insert(
                node_id,
                DocumentNode {
                    id: node_id,
                    node_type: NodeTypeId::new(node_type).unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
        }
        let mean = PortAddress::declared(target, PortKey::new("mean").unwrap());
        let connection_id = ConnectionId::new();
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(source, PortKey::new("value").unwrap()),
                input: mean.clone(),
                order: None,
            },
        );

        let snapshot = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
        let specialization = snapshot
            .node(target)
            .and_then(|node| node.specialization.as_ref())
            .expect("the exact target node is specialized");

        assert_eq!(
            specialization.coercions.as_ref(),
            [GraphInputCoercion {
                address: mean,
                kind: InputCoercionKind::WidenInt64ToFloat64,
            }]
        );
    }

    #[test]
    fn analysis_accepts_neutral_document_and_basis() {
        let basis = CompilationBasis {
            registry_fingerprint: RegistryFingerprint::from_bytes([4; 32]),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        };
        let analysis = analyze(
            &basis,
            GraphSemanticSnapshot::new([], [], GraphCompilationOutcome::Complete),
        );
        assert!(analysis.semantic_snapshot().nodes().is_empty());
        assert_eq!(analysis.registry_fingerprint(), &[4; 32]);
    }

    #[test]
    fn editor_projection_reports_an_unbound_required_input() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let node_id = NodeId::new();
        let node_type = NodeTypeId::new("yssbi.debug.view").expect("built-in node type is valid");
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type,
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );

        let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

        assert_eq!(facts.outcome(), &GraphCompilationOutcome::Incomplete);
        assert!(facts.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_str() == GraphDiagnosticKind::InputUnbound.code()
                && matches!(
                    &diagnostic.primary,
                    GraphDiagnosticLocation::Port(address) if address.node_id == node_id
                )
        }));
    }

    #[test]
    fn editor_projection_reports_required_parameters_from_the_protocol() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let (node_type, parameter_key) = builtin
            .registry
            .iter()
            .find_map(|(node_type, _)| {
                builtin
                    .registry
                    .protocol(node_type)
                    .into_iter()
                    .flat_map(|protocol| protocol.parameters.parameters.iter())
                    .find(|parameter| {
                        parameter.default_value.is_none()
                            && parameter
                                .constraints
                                .contains(&ParameterConstraint::Required)
                    })
                    .map(|parameter| (node_type.clone(), parameter.key.clone()))
            })
            .expect("built-ins include a required parameter");
        let node_id = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type,
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );

        let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

        assert!(facts.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_str() == GraphDiagnosticKind::ParameterRequired.code()
                && matches!(
                    &diagnostic.primary,
                    GraphDiagnosticLocation::Parameter { node_id: owner, key }
                        if *owner == node_id && key == &parameter_key
                )
        }));
    }

    #[test]
    fn editor_projection_reports_missing_resources_at_resource_location() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let (node_type, parameter_key, resource_kind) = builtin
            .registry
            .iter()
            .find_map(|(node_type, _)| {
                builtin
                    .registry
                    .protocol(node_type)
                    .into_iter()
                    .flat_map(|protocol| protocol.parameters.parameters.iter())
                    .find_map(|parameter| match &parameter.editor {
                        ParameterEditorSpec::Resource { kind } => {
                            Some((node_type.clone(), parameter.key.clone(), *kind))
                        }
                        _ => None,
                    })
            })
            .expect("built-ins include a resource parameter");
        let identity = match resource_kind {
            ResourceDisplayKind::Function => "functions/missing.yssbi-function",
            ResourceDisplayKind::Variable => "variables/missing",
            ResourceDisplayKind::Database => "databases/missing",
        };
        let node_id = NodeId::new();
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type,
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::from([(
                    parameter_key,
                    serde_json::Value::String(identity.to_owned()),
                )]),
                user_label: None,
            },
        );

        let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

        assert!(facts.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_str() == GraphDiagnosticKind::ResourceResolutionFailed.code()
                && matches!(
                    &diagnostic.primary,
                    GraphDiagnosticLocation::Resource(resource) if resource.as_ref() == identity
                )
        }));
    }

    #[test]
    fn editor_projection_reports_a_graph_level_value_cycle() {
        let builtin = build_builtin_node_system().expect("built-in node system is valid");
        let left = NodeId::new();
        let right = NodeId::new();
        let mut document = GraphDocument::default();
        for (node_id, x) in [(left, 0.0), (right, 200.0)] {
            document.nodes.insert(
                node_id,
                DocumentNode {
                    id: node_id,
                    node_type: NodeTypeId::new("yssbi.value.convert")
                        .expect("built-in node type is valid"),
                    position: NodePosition { x, y: 0.0 },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
        }
        for (source, target) in [(left, right), (right, left)] {
            let connection_id = ConnectionId::new();
            document.connections.insert(
                connection_id,
                DocumentConnection {
                    id: connection_id,
                    output: PortAddress::declared(
                        source,
                        PortKey::new("output").expect("built-in port key is valid"),
                    ),
                    input: PortAddress::declared(
                        target,
                        PortKey::new("input").expect("built-in port key is valid"),
                    ),
                    order: None,
                },
            );
        }

        let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

        assert!(facts.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_str() == GraphDiagnosticKind::DependencyValueCycle.code()
                && diagnostic.primary == GraphDiagnosticLocation::Graph
        }));
    }
}
