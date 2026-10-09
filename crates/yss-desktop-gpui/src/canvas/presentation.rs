//! Derive canvas decoration once per projection/run change, independently of pointer movement.
mod cache;
mod state;

use state::CacheCount;
pub(super) use state::State;
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use yss_application::graph::run::RunApplicationEventKind;
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_graph_document::{ConnectionId, NodeId, PortAddress};
use yss_graph_execution::error::RunFailureCode;
use yss_node_protocol::PortDirection;

use super::GraphCanvas;

#[derive(Clone, Copy, Default)]
pub(super) struct NodeAppearance {
    pub state: State,
    pub cache: CacheCount,
    pub failure: Option<RunFailureCode>,
}

#[derive(Default)]
pub(super) struct Presentation {
    pub nodes: BTreeMap<NodeId, NodeAppearance>,
    pub ports: BTreeMap<PortAddress, State>,
    connections: BTreeMap<ConnectionId, State>,
}

impl Presentation {
    pub fn connection(&self, id: ConnectionId) -> State {
        self.connections.get(&id).copied().unwrap_or_default()
    }
}

impl GraphCanvas {
    pub(super) fn refresh_presentation(&mut self) {
        self.node_contents.refresh(&self.graph.projection);
        let layer = self.connection_layer.borrow();
        let addresses = layer.addresses().collect::<BTreeMap<_, _>>();
        let keys = addresses
            .iter()
            .map(|(key, address)| (*address, *key))
            .collect::<BTreeMap<_, _>>();
        let graph = &self.graph;
        let results = &graph.results;
        let current = results.semantic_input_hash == graph.projection.basis.semantic_input_hash;
        let pending = self
            .execution
            .pending_outputs(graph)
            .map(|output| output.port().as_str())
            .collect::<BTreeSet<_>>();
        let running = self
            .execution
            .running_outputs(graph)
            .filter_map(|output| addresses.get(output.port().as_str()))
            .map(|address| address.node_id)
            .collect::<BTreeSet<_>>();
        let failure = self.run_failure().and_then(|event| match event.kind() {
            RunApplicationEventKind::RunErrored { failure } => {
                let source = failure.source.as_ref()?;
                if source.graph().as_str() != self.path() {
                    return None;
                }
                let node = uuid::Uuid::parse_str(source.node()?.as_str()).ok()?;
                Some((NodeId::from_uuid(node), failure.code))
            }
            _ => None,
        });
        let failed = |id| {
            failure
                .filter(|(node, _)| *node == id)
                .map(|(_, code)| code)
        };
        let mut view = Presentation::default();
        let cache = cache::CacheStates::new(current.then_some(results), &addresses, &pending);
        let mut blocked_ports = BTreeSet::new();
        let mut blocked_connections = BTreeSet::new();
        for diagnostic in graph.projection.diagnostics.iter().filter(|d| d.blocking) {
            match &diagnostic.location {
                DiagnosticLocation::Port(address) => {
                    blocked_ports.insert(address);
                }
                DiagnosticLocation::Connection(id) => {
                    blocked_connections.insert(*id);
                }
                _ => {}
            }
        }
        let mut orphan_ports = BTreeSet::new();
        for node in &graph.projection.nodes {
            let mut appearance = NodeAppearance {
                cache: cache.nodes.get(&node.node_id).copied().unwrap_or_default(),
                ..Default::default()
            };
            appearance.failure = failed(node.node_id);
            appearance.state = State::resolve(
                appearance.cache.state(),
                running.contains(&node.node_id),
                appearance.failure.is_some() || node.diagnostics.iter().any(|d| d.blocking),
            );
            for port in &node.ports {
                if port.orphan {
                    orphan_ports.insert(&port.address);
                }
                let state = cache.ports.get(&port.address).copied().unwrap_or_else(|| {
                    if port.direction == PortDirection::Input {
                        if appearance.cache.valid > 0 {
                            State::Valid
                        } else if appearance.cache.stale > 0 {
                            State::Stale
                        } else {
                            State::Unexecuted
                        }
                    } else {
                        State::Unexecuted
                    }
                });
                view.ports.insert(
                    port.address.clone(),
                    State::resolve(
                        state,
                        running.contains(&node.node_id),
                        appearance.failure.is_some() || blocked_ports.contains(&port.address),
                    ),
                );
            }
            view.nodes.insert(node.node_id, appearance);
        }
        for connection in &graph.projection.connections {
            let cache = keys
                .get(&connection.output)
                .zip(keys.get(&connection.input))
                .and_then(|(output, input)| cache.connections.get(&(*output, *input)))
                .copied()
                .unwrap_or_default();
            let error = blocked_connections.contains(&connection.connection_id)
                || orphan_ports.contains(&connection.output)
                || orphan_ports.contains(&connection.input)
                || failed(connection.output.node_id).is_some()
                || failed(connection.input.node_id).is_some();
            view.connections.insert(
                connection.connection_id,
                State::resolve(cache, running.contains(&connection.input.node_id), error),
            );
        }
        self.presentation = Rc::new(view);
    }
}
