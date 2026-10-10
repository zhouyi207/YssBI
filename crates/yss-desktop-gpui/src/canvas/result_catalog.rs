//! Canvas search and the results panel share labels and current output identities.
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use yss_graph_document::PortAddress;
use yss_graph_execution::{
    plan::PlanOutputRef,
    result::{ResultCacheState, ResultReference},
};

use super::GraphCanvas;

#[derive(PartialEq, Eq)]
pub(crate) struct ResultEntry {
    pub title: String,
    pub search_label: String,
    pub address: Option<PortAddress>,
    pub output: PlanOutputRef,
    pub reference: ResultReference,
    pub stale: bool,
    pub waiting: bool,
}

impl ResultEntry {
    pub(super) fn current(&self) -> bool {
        !self.stale && !self.waiting && self.address.is_some()
    }
}

impl GraphCanvas {
    pub(crate) fn result_entries(&self) -> &Rc<[ResultEntry]> {
        &self.presentation.results
    }

    pub(super) fn project_result_entries(&self, pending: &BTreeSet<&str>) -> Rc<[ResultEntry]> {
        let graph = &self.graph;
        let labels: BTreeMap<_, _> = graph
            .projection
            .nodes
            .iter()
            .flat_map(|node| {
                node.ports.iter().map(move |port| {
                    let node_title = node
                        .display
                        .user_label
                        .as_deref()
                        .unwrap_or(&node.display.title);
                    let pin_title = port
                        .display
                        .instance_label
                        .as_deref()
                        .unwrap_or(&port.display.label);
                    (
                        port.address.to_string(),
                        (
                            port.address.clone(),
                            format!("{node_title} · {pin_title}"),
                            format!("{node_title} {pin_title} {}", node.display.title)
                                .to_lowercase(),
                        ),
                    )
                })
            })
            .collect();
        let current =
            graph.results.semantic_input_hash == graph.projection.basis.semantic_input_hash;
        let entries: Vec<_> = graph
            .results
            .outputs
            .iter()
            .filter_map(|(output, state)| {
                let (result_id, stale) = match state {
                    ResultCacheState::Valid { result_id } => (*result_id, false),
                    ResultCacheState::Stale { result_id } => (*result_id, true),
                    ResultCacheState::Missing => return None,
                };
                let label = labels.get(output.port().as_str());
                Some(ResultEntry {
                    title: label
                        .map_or_else(|| output.port().to_string(), |(_, title, _)| title.clone()),
                    search_label: label.map_or_else(
                        || output.port().as_str().to_lowercase(),
                        |(_, _, search)| search.clone(),
                    ),
                    address: label.map(|(address, _, _)| address.clone()),
                    output: output.clone(),
                    reference: ResultReference {
                        execution_session_id: graph.results.execution_session_id,
                        result_id,
                    },
                    stale,
                    waiting: !current || pending.contains(output.port().as_str()),
                })
            })
            .collect();
        if self.result_entries().as_ref() == entries.as_slice() {
            self.result_entries().clone()
        } else {
            entries.into()
        }
    }
}
