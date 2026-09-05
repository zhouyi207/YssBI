use crate::{GraphNodeSemanticFact, GraphPortBacking, GraphPortSemanticFact};
use std::collections::BTreeMap;
use yss_graph_document::{
    DynamicMemberLocator, DynamicPortBinding, GraphDocument, PortAddress, PortRef,
};
use yss_graph_protocol::{NodeProtocol, PortKey};

/// A borrowed address index over the snapshot's sole concrete port facts.
pub struct ConcreteGraphInterface<'a> {
    pub(crate) nodes: &'a [GraphNodeSemanticFact],
}

impl<'a> ConcreteGraphInterface<'a> {
    pub fn port(&self, address: &PortAddress) -> Option<&'a GraphPortSemanticFact> {
        self.nodes
            .iter()
            .find(|node| node.node_id == address.node_id)?
            .ports
            .iter()
            .find(|port| &port.address == address)
    }

    pub fn nodes(&self) -> &'a [GraphNodeSemanticFact] {
        self.nodes
    }
}

pub(crate) fn sort_concrete_ports(
    protocol: &NodeProtocol,
    document: &GraphDocument,
    ports: &mut [GraphPortSemanticFact],
    derived_orders: &BTreeMap<(PortKey, DynamicMemberLocator), usize>,
) {
    ports.sort_by_cached_key(|port| {
        let (template, instance) = match &port.address.port {
            PortRef::Declared { key } => (key, false),
            PortRef::Instance { template, .. } => (template, true),
        };
        let group = protocol.interface.member_group_for_template(template);
        let first_template = group
            .and_then(|group| group.templates.first())
            .unwrap_or(template);
        let slot = protocol
            .interface
            .ports
            .iter()
            .position(|spec| &spec.key == first_template)
            .unwrap_or(usize::MAX);
        let member_slot = group
            .and_then(|group| group.templates.iter().position(|key| key == template))
            .unwrap_or(0);
        let binding = document.port_bindings.get(&port.address);
        let origin = match &port.backing {
            GraphPortBacking::ProjectedDerived { origin } => Some(origin),
            _ => binding.and_then(|binding| match binding {
                DynamicPortBinding::Resolved { origin, .. }
                | DynamicPortBinding::Orphan { origin, .. } => Some(origin),
                _ => None,
            }),
        };
        let order = if let Some(origin) = origin {
            derived_orders
                .get(&(template.clone(), origin.clone()))
                .map_or_else(|| "~orphan".to_owned(), |index| format!("{index:010}"))
        } else if instance {
            binding
                .map(|binding| crate::binding_order(binding).as_str().to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };
        (slot, order, member_slot, port.address.clone())
    });
}
