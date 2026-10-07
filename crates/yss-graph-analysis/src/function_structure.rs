//! Structural dispatch and ABI member identity, independent of display labels.
use crate::{GraphNodeImplementation, GraphPortBacking, GraphPortSemanticFact};
use yss_graph_document::{DocumentNode, DynamicMemberLocator, GraphDocument, GraphResourcePath};
use yss_node_protocol::PortDirection;
use yss_node_registry::{RegisteredNode, StructuralNodeRole};

/// Concrete result addresses, including unclaimed derived members, for runtime schema feedback.
pub fn function_output_addresses(
    document: &GraphDocument,
    registry: &yss_node_registry::NodeRegistry,
    resources: &yss_graph_resource_contract::ResourceCatalogSnapshot,
) -> Vec<yss_graph_document::PortAddress> {
    let index = crate::document_index::DocumentIndex::new(document);
    function_output_addresses_with_index(document, &index, registry, resources)
}

pub(crate) fn function_output_addresses_with_index(
    document: &GraphDocument,
    index: &crate::document_index::DocumentIndex<'_>,
    registry: &yss_node_registry::NodeRegistry,
    resources: &yss_graph_resource_contract::ResourceCatalogSnapshot,
) -> Vec<yss_graph_document::PortAddress> {
    let mut outputs = Vec::new();
    for node in document.nodes.values() {
        let Some(registered) = registry.get(&node.node_type) else {
            continue;
        };
        if !registered
            .structural_role()
            .is_some_and(StructuralNodeRole::calls_function)
        {
            continue;
        }
        for spec in &registered.protocol().interface.ports {
            if spec.direction != PortDirection::Output {
                continue;
            }
            if spec.cardinality == yss_node_protocol::PortCardinality::Declared {
                outputs.push(yss_graph_document::PortAddress::declared(
                    node.id,
                    spec.key.clone(),
                ));
                continue;
            }
            let yss_node_protocol::PortCardinality::Derived { resolver } = &spec.cardinality else {
                continue;
            };
            for member in crate::derived_ports::derived_port_members(
                document,
                node.id,
                registered.protocol(),
                resolver.as_str(),
                &Default::default(),
                resources,
            ) {
                let bound = index.node_bindings(node.id).iter().find(|(address, binding)| {
                    matches!(&address.port, yss_graph_document::PortRef::Instance { template, .. } if template == &spec.key)
                        && crate::port_projection::binding_origin(binding) == Some(&member.locator)
                });
                outputs.push(
                    bound
                        .map(|(address, _)| (*address).clone())
                        .unwrap_or_else(|| {
                            crate::derived_ports::derived_port_address(
                                document,
                                node.id,
                                &spec.key,
                                &member.locator,
                            )
                        }),
                );
            }
        }
    }
    outputs
}

pub(crate) fn implementation(
    document: &GraphDocument,
    node: &DocumentNode,
    registered: &RegisteredNode,
    ports: &[GraphPortSemanticFact],
) -> Option<GraphNodeImplementation> {
    Some(match registered.structural_role() {
        Some(StructuralNodeRole::FunctionEntry) => GraphNodeImplementation::FunctionEntry,
        Some(StructuralNodeRole::FunctionReturn) => GraphNodeImplementation::FunctionReturn,
        Some(role @ (StructuralNodeRole::GroupApply | StructuralNodeRole::GroupTransform)) => {
            GraphNodeImplementation::GroupMap {
                target: GraphResourcePath::new(registered.function_reference(&node.parameters)?)
                    .ok()?,
                transform: role == StructuralNodeRole::GroupTransform,
            }
        }
        Some(StructuralNodeRole::Call) => {
            let target =
                GraphResourcePath::new(registered.function_reference(&node.parameters)?).ok()?;
            let arguments = ports
                .iter()
                .filter(|port| port.direction == PortDirection::Input && !port.orphan)
                .map(|port| {
                    let origin = match &port.backing {
                        GraphPortBacking::ProjectedDerived { origin } => Some(origin),
                        _ => document
                            .port_bindings
                            .get(&port.address)
                            .and_then(crate::port_projection::binding_origin),
                    };
                    match origin {
                        Some(DynamicMemberLocator::FunctionParameter {
                            function,
                            parameter,
                        }) if function == &target => {
                            Some((parameter.clone(), port.address.clone()))
                        }
                        _ => None,
                    }
                })
                .collect::<Option<Box<[_]>>>()?;
            GraphNodeImplementation::FunctionCall { target, arguments }
        }
        None => GraphNodeImplementation::Kernel(
            registered
                .implementation()
                .map_or(node.node_type.as_str(), |implementation| {
                    implementation.implementation_identity()
                })
                .into(),
        ),
    })
}

pub(crate) fn validate_group_signatures(
    document: &GraphDocument,
    registry: &yss_node_registry::NodeRegistry,
    resources: &yss_graph_resource_contract::ResourceCatalogSnapshot,
) -> Vec<crate::GraphDiagnosticFact> {
    document
        .nodes
        .values()
        .filter_map(|node| {
            let registered = registry.get(&node.node_type)?;
            if !matches!(
                registered.structural_role(),
                Some(StructuralNodeRole::GroupApply | StructuralNodeRole::GroupTransform)
            ) {
                return None;
            }
            let target =
                GraphResourcePath::new(registered.function_reference(&node.parameters)?).ok()?;
            let signature = resources.function_signature(&target)?;
            let valid = signature.parameters().len() == 1
                && signature.parameters()[0].data_type()
                    == &yss_data_contract::ValueType::DataFrame
                && signature.result() == Some(&yss_data_contract::ValueType::DataFrame);
            (!valid).then(|| {
                crate::graph_problem(
                    yss_graph_diagnostics::GraphDiagnosticKind::FunctionAbiMismatch,
                    crate::GraphDiagnosticLocation::Node(node.id),
                    [("resource_key", target.as_str().into())],
                )
            })
        })
        .collect()
}
