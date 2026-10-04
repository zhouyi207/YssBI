//! Discardable type-resolution cache and its canonical input/output identities.
use crate::{GraphInputCoercion, GraphNodeSemanticFact, GraphPortSemanticFact};
use std::collections::BTreeMap;
use yss_graph_document::{NodeId, PortAddress};
use yss_node_protocol::TypeState;

#[derive(Clone, Default)]
pub struct GraphSemanticCache {
    pub(crate) schemas: crate::schema_resolution::SchemaCache,
    pub(super) nodes: BTreeMap<NodeId, CachedNodeResolution>,
    pub(super) reused_nodes: usize,
}

impl GraphSemanticCache {
    #[cfg(test)]
    pub(crate) const fn reused_nodes(&self) -> usize {
        self.reused_nodes
    }
}

#[derive(Clone)]
pub(super) struct CachedNodeResolution {
    pub(super) input_fingerprint: [u8; 32],
    pub(super) output_states: BTreeMap<PortAddress, TypeState>,
    pub(super) coercions: Box<[GraphInputCoercion]>,
}

pub(super) fn node_input_fingerprint(
    document_node: &yss_graph_document::DocumentNode,
    protocol_fingerprint: Option<&yss_node_registry::ProtocolFingerprint>,
    ports: &[GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    constant_type: Option<&yss_data_contract::ValueType>,
) -> [u8; 32] {
    let ports = ports
        .iter()
        .map(|port| {
            (
                &port.address,
                port.direction,
                &port.accepted_type,
                port.orphan,
                states.get(&port.address),
                &port.schema_state,
            )
        })
        .collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-node-semantic-input.v1",
        &(
            &document_node.node_type,
            &document_node.parameters,
            protocol_fingerprint,
            constant_type,
            ports,
        ),
    )
    .expect("node semantic inputs are canonically serializable")
}

pub(super) fn semantic_fingerprint(
    document_node: &yss_graph_document::DocumentNode,
    node: &GraphNodeSemanticFact,
) -> [u8; 32] {
    let ports = node
        .ports
        .iter()
        .map(|port| {
            (
                &port.address,
                &port.accepted_type,
                &port.type_state,
                // Observing this output must not invalidate the value that supplied it.
                // Consumer input fields remain part of that consumer's fingerprint.
                match &port.schema_state {
                    crate::GraphSchemaState::Observed { .. }
                        if port.direction == yss_node_protocol::PortDirection::Output =>
                    {
                        &crate::GraphSchemaState::Deferred
                    }
                    state => state,
                },
            )
        })
        .collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-node-semantics.v1",
        &(
            &document_node.node_type,
            &document_node.parameters,
            node.constant
                .as_ref()
                .map(|constant| (&constant.data_type, &constant.data_value, &constant.tabular)),
            ports,
        ),
    )
    .expect("node semantic facts are canonically serializable")
}

pub(super) fn semantic_fingerprint_without_document(node: &GraphNodeSemanticFact) -> [u8; 32] {
    let ports = node
        .ports
        .iter()
        .map(|port| (&port.address, &port.accepted_type, &port.type_state))
        .collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-node-semantics.unavailable.v1",
        &(&node.node_type, ports),
    )
    .expect("node semantic facts are canonically serializable")
}
