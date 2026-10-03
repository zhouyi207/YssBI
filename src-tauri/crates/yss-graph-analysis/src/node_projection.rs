//! Assemble node facts from protocols, parameter values and resolved port schemas.
mod interface;

use crate::document_index::DocumentIndex;
use crate::parameter_projection::{
    project_node_parameters, project_parameter_groups, validate_node_parameters,
};
use crate::schema_resolution::SchemaResolution;
use crate::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphNodeSemanticFact, graph_problem,
    referenced_constant,
};
use interface::project_node_interface;
use std::{collections::BTreeMap, sync::Arc};
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{DocumentNode, GraphDocument, NodeId};
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_registry::NodeRegistry;

pub(crate) fn project_nodes(
    document: &GraphDocument,
    index: &DocumentIndex<'_>,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    schemas: &SchemaResolution,
    diagnostics: &mut Vec<GraphDiagnosticFact>,
) -> (Vec<GraphNodeSemanticFact>, Option<NodeId>) {
    let mut internal_interface_node = None;
    let mut constants = BTreeMap::new();
    let nodes = document
        .nodes
        .values()
        .map(|node| {
            let Some(protocol) = registry.protocol(&node.node_type) else {
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::NodeUnknown,
                    GraphDiagnosticLocation::Node(node.id),
                    [("node_type", node.node_type.as_str().into())],
                ));
                return unknown_node(node);
            };
            let parameters = project_node_parameters(node, protocol);
            validate_node_parameters(
                node,
                protocol,
                &parameters,
                registry,
                resources,
                diagnostics,
            );
            let interface = project_node_interface(
                document,
                index,
                node.id,
                protocol,
                resources,
                schemas,
                diagnostics,
            );
            if interface.unsupported_resolver {
                internal_interface_node = Some(node.id);
            }
            let constant = referenced_constant(document, node, protocol);
            GraphNodeSemanticFact {
                constant: constant.map(|constant| {
                    constants
                        .entry(constant.id)
                        .or_insert_with(|| Arc::new(constant.clone()))
                        .clone()
                }),
                node_id: node.id,
                node_type: node.node_type.clone(),
                instance_title: constant.map(|constant| constant.name.clone().into_boxed_str()),
                title: protocol.catalog.title_key.as_str().into(),
                icon_id: Some(protocol.catalog.icon_id.as_str().into()),
                style_id: Some(protocol.catalog.style_id.as_str().into()),
                managed: protocol.managed_role.is_some(),
                parameter_groups: project_parameter_groups(protocol),
                parameters,
                inputs: Box::new([]),
                ports: interface.ports,
                port_instance_additions: interface.additions,
                specialization: None,
                semantic_fingerprint: [0; 32],
            }
        })
        .collect();
    (nodes, internal_interface_node)
}

fn unknown_node(node: &DocumentNode) -> GraphNodeSemanticFact {
    GraphNodeSemanticFact {
        constant: None,
        node_id: node.id,
        node_type: node.node_type.clone(),
        instance_title: None,
        title: node.node_type.as_str().into(),
        icon_id: None,
        style_id: None,
        managed: false,
        parameter_groups: Box::new([]),
        parameters: Box::new([]),
        inputs: Box::new([]),
        ports: Box::new([]),
        port_instance_additions: Box::new([]),
        specialization: None,
        semantic_fingerprint: [0; 32],
    }
}
