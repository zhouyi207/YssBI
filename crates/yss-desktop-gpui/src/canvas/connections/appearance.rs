//! Recompute edge presentation on projection/run changes, never on pointer movement.
use std::collections::{BTreeMap, BTreeSet};

use yss_application::graph::run::RunApplicationEventKind;
use yss_graph_document::NodeId;
use yss_graph_execution::result::ConnectionCacheState;

use crate::canvas::GraphCanvas;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum State {
    #[default]
    Unexecuted,
    Running,
    Error,
    Valid,
    Stale,
}

impl State {
    pub fn label(self) -> &'static str {
        crate::text::t(match self {
            Self::Unexecuted => "canvas.graphState.unexecuted",
            Self::Running => "canvas.graphState.running",
            Self::Error => "canvas.graphState.error",
            Self::Valid => "canvas.graphState.valid",
            Self::Stale => "canvas.graphState.stale",
        })
    }

    pub fn dashes(self) -> &'static [f32] {
        match self {
            Self::Unexecuted => &[3., 6.],
            Self::Stale => &[8., 4., 2., 4.],
            Self::Error => &[2., 3.],
            Self::Running | Self::Valid => &[],
        }
    }

    pub fn opacity(self) -> f32 {
        if matches!(self, Self::Unexecuted | Self::Stale) {
            0.6
        } else {
            1.
        }
    }
}

impl GraphCanvas {
    pub(in crate::canvas) fn refresh_connection_states(&self) {
        let mut layer = self.connection_layer.borrow_mut();
        let results = &self.graph.results;
        let cache = results
            .connections
            .iter()
            .filter(|_| {
                results.semantic_input_hash == self.graph.projection.basis.semantic_input_hash
            })
            .map(|connection| {
                (
                    (connection.output.port().as_str(), connection.input.as_str()),
                    connection.state,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let running_ports = self
            .execution
            .running_outputs(&self.graph)
            .map(|output| output.port().as_str())
            .collect::<BTreeSet<_>>();
        let running_nodes = layer
            .anchors
            .values()
            .filter(|port| running_ports.contains(port.key.as_ref()))
            .map(|port| port.node_id)
            .collect::<BTreeSet<_>>();
        let pending = self
            .execution
            .pending_outputs(&self.graph)
            .map(|output| output.port().as_str())
            .collect::<BTreeSet<_>>();
        let failed = self
            .run_failure()
            .and_then(|event| match event.kind() {
                RunApplicationEventKind::RunErrored { failure } => failure.source.as_ref(),
                _ => None,
            })
            .filter(|source| source.graph().as_str() == self.path())
            .and_then(|source| source.node())
            .and_then(|node| uuid::Uuid::parse_str(node.as_str()).ok())
            .map(NodeId::from_uuid);

        for connection in &mut layer.connections {
            let output = &connection.output;
            let input = &connection.input;
            let cache = cache
                .get(&(output.key.as_ref(), input.key.as_ref()))
                .copied()
                .unwrap_or(ConnectionCacheState::New);
            connection.state = if connection.blocked
                || failed.is_some_and(|id| id == input.node_id || id == output.node_id)
            {
                State::Error
            } else if running_nodes.contains(&input.node_id) {
                State::Running
            } else {
                match cache {
                    ConnectionCacheState::New => State::Unexecuted,
                    ConnectionCacheState::Stale => State::Stale,
                    ConnectionCacheState::Valid if pending.contains(output.key.as_ref()) => {
                        State::Stale
                    }
                    ConnectionCacheState::Valid => State::Valid,
                }
            };
        }
    }
}
