//! Assemble declared, bound and derived ports for one protocol node.
use crate::concrete_interface::sort_concrete_ports;
use crate::derived_ports::{self, DerivedPortMember, derived_port_address, derived_port_members};
use crate::document_index::DocumentIndex;
use crate::port_projection::{
    BoundPortProjection, ConcretePortProjection, binding_kind, binding_matches_cardinality,
    binding_order, binding_origin, port_cardinality_kind, project_bound_port,
    project_concrete_port, project_declared_port, project_port_instance_additions,
};
use crate::schema_resolution::SchemaResolution;
use crate::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphPortBacking, GraphPortInstanceAdditionFact,
    GraphPortSemanticFact, GraphSchemaIssue, GraphSchemaState, graph_problem,
};
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{
    DynamicMemberLocator, DynamicPortBinding, GraphDocument, NodeId, PortAddress, PortRef,
};
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_protocol::{NodeProtocol, PortCardinality, PortDirection, PortKey, PortSpec};

pub(super) struct NodeInterface {
    pub ports: Box<[GraphPortSemanticFact]>,
    pub additions: Box<[GraphPortInstanceAdditionFact]>,
    pub unsupported_resolver: bool,
}

pub(super) fn project_node_interface(
    document: &GraphDocument,
    index: &DocumentIndex<'_>,
    node_id: NodeId,
    protocol: &NodeProtocol,
    resources: &ResourceCatalogSnapshot,
    resolved_schemas: &SchemaResolution,
    diagnostics: &mut Vec<GraphDiagnosticFact>,
) -> NodeInterface {
    let mut unsupported_resolver = false;
    let node_bindings = index.node_bindings(node_id);
    let mut ports = Vec::new();
    let mut derived_orders = std::collections::BTreeMap::new();
    let projection = InterfaceProjection {
        document,
        index,
        node_id,
        protocol,
        resources,
        resolved_schemas,
    };
    for spec in protocol.interface.ports.iter() {
        unsupported_resolver |= matches!(
            &spec.cardinality,
            PortCardinality::Derived { resolver } if !derived_ports::supports_resolver(resolver.as_str())
        );
        projection.project_template(spec, &mut ports, &mut derived_orders, diagnostics);
    }
    sort_concrete_ports(protocol, document, &mut ports, &derived_orders);
    number_variable_inputs(protocol, &mut ports);
    apply_resolved_schemas(&mut ports, resolved_schemas);
    for (address, _) in node_bindings
        .iter()
        .filter(|(address, _)| match &address.port {
            PortRef::Declared { key } | PortRef::Instance { template: key, .. } => {
                !protocol.interface.ports.iter().any(|spec| &spec.key == key)
            }
        })
    {
        diagnostics.push(graph_problem(
            GraphDiagnosticKind::PortUnknown,
            GraphDiagnosticLocation::Port((*address).clone()),
            [("port", address.to_string().into())],
        ));
    }
    let (port_instance_additions, minimum_instances_present) =
        project_port_instance_additions(node_id, protocol, node_bindings);
    if !minimum_instances_present {
        diagnostics.push(graph_problem(
            GraphDiagnosticKind::SemanticInvalid,
            GraphDiagnosticLocation::Node(node_id),
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
            diagnostics.push(graph_problem(
                GraphDiagnosticKind::InputUnbound,
                GraphDiagnosticLocation::Port(port.address.clone()),
                [("port", port.address.to_string().into())],
            ));
        }
    }

    NodeInterface {
        ports: ports.into_boxed_slice(),
        additions: port_instance_additions.into_boxed_slice(),
        unsupported_resolver,
    }
}

fn number_variable_inputs(protocol: &NodeProtocol, ports: &mut [GraphPortSemanticFact]) {
    let mut counts = BTreeMap::new();
    let templates = protocol
        .interface
        .ports
        .iter()
        .filter(|spec| {
            spec.direction == PortDirection::Input
                && matches!(spec.key.as_str(), "x" | "y")
                && matches!(spec.cardinality, PortCardinality::UserCreated { .. })
        })
        .map(|spec| &spec.key)
        .collect::<BTreeSet<_>>();
    for port in ports {
        let PortRef::Instance { template, .. } = &port.address.port else {
            continue;
        };
        if port.orphan || !templates.contains(template) {
            continue;
        }
        let count = counts.entry(template).or_insert(0);
        *count += 1;
        const DIGITS: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];
        let mut label = port.label.to_string();
        label.extend(
            count
                .to_string()
                .bytes()
                .map(|digit| DIGITS[(digit - b'0') as usize]),
        );
        port.instance_label = Some(label.into_boxed_str());
    }
}

struct InterfaceProjection<'a> {
    document: &'a GraphDocument,
    index: &'a DocumentIndex<'a>,
    node_id: NodeId,
    protocol: &'a NodeProtocol,
    resources: &'a ResourceCatalogSnapshot,
    resolved_schemas: &'a SchemaResolution,
}

impl InterfaceProjection<'_> {
    fn project_template(
        &self,
        spec: &PortSpec,
        ports: &mut Vec<GraphPortSemanticFact>,
        derived_orders: &mut BTreeMap<(PortKey, DynamicMemberLocator), usize>,
        diagnostics: &mut Vec<GraphDiagnosticFact>,
    ) {
        let Self {
            document,
            index,
            node_id,
            resources,
            resolved_schemas,
            ..
        } = *self;
        let node_bindings = index.node_bindings(node_id);
        if matches!(spec.cardinality, PortCardinality::Declared) {
            let address = PortAddress::declared(node_id, spec.key.clone());
            ports.push(project_declared_port(
                document,
                index,
                address.clone(),
                spec,
                resolved_schemas.get(&address),
            ));
            return;
        }
        let derived_members = match &spec.cardinality {
            PortCardinality::Derived { resolver } => derived_port_members(
                document,
                node_id,
                self.protocol,
                resolver.as_str(),
                resolved_schemas,
                resources,
            ),
            PortCardinality::Declared | PortCardinality::UserCreated { .. } => Vec::new(),
        };
        let derived_by_origin = derived_members
            .iter()
            .map(|member| (&member.locator, member))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (index, member) in derived_members.iter().enumerate() {
            derived_orders.insert((spec.key.clone(), member.locator.clone()), index);
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

        // Index claimed origins once per template instead of scanning every binding for every member.
        let bound_origins = bindings
            .iter()
            .filter_map(|(_, binding)| binding_origin(binding))
            .collect::<BTreeSet<_>>();
        self.project_bindings(spec, &bindings, &derived_by_origin, ports, diagnostics);
        if matches!(spec.cardinality, PortCardinality::Derived { .. }) {
            for member in derived_members {
                if bound_origins.contains(&member.locator) {
                    continue;
                }
                let address = derived_port_address(document, node_id, &spec.key, &member.locator);
                ports.push(project_concrete_port(
                    document,
                    index,
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
                        resolved_schema: member.schema,
                    },
                ));
            }
        }
    }

    fn project_bindings(
        &self,
        spec: &PortSpec,
        bindings: &[(&PortAddress, &DynamicPortBinding)],
        derived_by_origin: &BTreeMap<&DynamicMemberLocator, &DerivedPortMember>,
        ports: &mut Vec<GraphPortSemanticFact>,
        diagnostics: &mut Vec<GraphDiagnosticFact>,
    ) {
        let Self {
            document,
            index,
            node_id,
            protocol,
            resolved_schemas,
            ..
        } = *self;
        let node_bindings = index.node_bindings(node_id);
        for &(address, binding) in bindings {
            if binding_origin(binding).is_some_and(|origin| !derived_by_origin.contains_key(origin))
                && !self.port_is_referenced(address)
            {
                continue;
            }
            if !binding_matches_cardinality(binding, &spec.cardinality) {
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
                index,
                protocol,
                spec,
                node_bindings,
                BoundPortProjection {
                    address,
                    binding,
                    current_member: binding_origin(binding)
                        .and_then(|origin| derived_by_origin.get(origin).copied()),
                    resolved_schema: resolved_schemas.get(address),
                },
            );
            if projected.orphan {
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::PortOrphan,
                    GraphDiagnosticLocation::Port(address.clone()),
                    [("port", address.to_string().into())],
                ));
            }
            ports.push(projected);
        }
    }

    fn port_is_referenced(&self, address: &PortAddress) -> bool {
        self.index.connection_count(address) != 0
            || self
                .document
                .input_states
                .get(address)
                .is_some_and(|state| state.literal_override.is_some())
    }
}

fn apply_resolved_schemas(
    ports: &mut [GraphPortSemanticFact],
    resolved_schemas: &SchemaResolution,
) {
    for port in ports {
        // Derived columns acquire their field schema when their concrete interface resolves.
        if port.schema_state.exact().is_some() {
            continue;
        }
        port.schema_state = resolved_schemas
            .state(&port.address)
            .cloned()
            .unwrap_or_else(|| {
                if port.schema.is_some() {
                    GraphSchemaState::Pending(GraphSchemaIssue::UnresolvedUpstream)
                } else {
                    port.schema_state.clone()
                }
            });
        if matches!(port.schema_state, GraphSchemaState::NotApplicable) && port.schema.is_some() {
            port.schema_state = GraphSchemaState::Pending(GraphSchemaIssue::UnresolvedUpstream);
        } else if port.schema.is_none() && matches!(port.schema_state, GraphSchemaState::Pending(_))
        {
            port.schema_state = GraphSchemaState::NotApplicable;
        }
    }
}
