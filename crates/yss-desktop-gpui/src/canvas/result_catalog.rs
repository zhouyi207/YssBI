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
    pub address: PortAddress,
    pub output: PlanOutputRef,
    pub reference: ResultReference,
}

impl GraphCanvas {
    pub(crate) fn result_entries(&self) -> &Rc<[ResultEntry]> {
        &self.presentation.results
    }

    pub(super) fn project_result_entries(&self, pending: &BTreeSet<&str>) -> Rc<[ResultEntry]> {
        let graph = &self.graph;
        if graph.results.semantic_input_hash != graph.projection.basis.semantic_input_hash {
            return if self.result_entries().is_empty() {
                self.result_entries().clone()
            } else {
                Rc::default()
            };
        }
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
        let entries: Vec<_> = graph
            .results
            .outputs
            .iter()
            .filter_map(|(output, state)| {
                let ResultCacheState::Valid { result_id } = state else {
                    return None;
                };
                if pending.contains(output.port().as_str()) {
                    return None;
                }
                let (address, title, search_label) = labels.get(output.port().as_str())?;
                Some(ResultEntry {
                    title: title.clone(),
                    search_label: search_label.clone(),
                    address: address.clone(),
                    output: output.clone(),
                    reference: ResultReference {
                        execution_session_id: graph.results.execution_session_id,
                        result_id: *result_id,
                    },
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
