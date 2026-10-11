//! Aggregate result facts for nodes, ports and connections before runtime/diagnostic precedence.
use super::state::{CacheCount, State};
use std::collections::{BTreeMap, BTreeSet};
use yss_application::graph::results::GraphResultState;
use yss_graph_document::{NodeId, PortAddress};
use yss_graph_execution::result::{ConnectionCacheState, ResultCacheState};

#[derive(Default)]
pub(super) struct CacheStates<'a> {
    pub nodes: BTreeMap<NodeId, CacheCount>,
    pub ports: BTreeMap<&'a PortAddress, State>,
    pub connections: BTreeMap<(&'a str, &'a str), State>,
}

impl<'a> CacheStates<'a> {
    pub fn new(
        results: Option<&'a GraphResultState>,
        addresses: &BTreeMap<&str, &'a PortAddress>,
        pending: &BTreeSet<&str>,
    ) -> Self {
        let mut nodes = BTreeMap::<NodeId, CacheCount>::new();
        let mut output_nodes = BTreeSet::new();
        let mut cache = BTreeMap::new();
        for (output, state) in results.into_iter().flat_map(|results| &results.outputs) {
            let Some(address) = addresses.get(output.port().as_str()).copied() else {
                continue;
            };
            let state = if pending.contains(output.port().as_str()) {
                State::Unexecuted
            } else {
                match state {
                    ResultCacheState::Missing => State::Unexecuted,
                    ResultCacheState::Valid { .. } => State::Valid,
                }
            };
            cache.insert(address, state);
            output_nodes.insert(address.node_id);
            nodes.entry(address.node_id).or_default().add(state);
        }
        let mut edge_cache = BTreeMap::new();
        for connection in results.into_iter().flat_map(|results| &results.connections) {
            let state = match connection.state {
                ConnectionCacheState::New => State::Unexecuted,
                ConnectionCacheState::Stale => State::Stale,
                ConnectionCacheState::Valid
                    if pending.contains(connection.output.port().as_str()) =>
                {
                    State::Stale
                }
                ConnectionCacheState::Valid => State::Valid,
            };
            edge_cache.insert(
                (connection.output.port().as_str(), connection.input.as_str()),
                state,
            );
            let Some(address) = addresses.get(connection.input.as_str()).copied() else {
                continue;
            };
            cache
                .entry(address)
                .and_modify(|previous| {
                    *previous = if *previous == State::Stale || state == State::Stale {
                        State::Stale
                    } else if *previous == State::Unexecuted || state == State::Unexecuted {
                        State::Unexecuted
                    } else {
                        State::Valid
                    };
                })
                .or_insert(state);
            // Observer nodes have no output cache; their input connections are the result facts.
            if !output_nodes.contains(&address.node_id) {
                nodes.entry(address.node_id).or_default().add(state);
            }
        }
        Self {
            nodes,
            ports: cache,
            connections: edge_cache,
        }
    }
}
