//! Resolve current or explicitly previous output references without running the graph.
use crate::canvas::{CanvasEvent, GraphCanvas};
use gpui::Context;
use std::sync::Arc;
use yss_application::graph::results::{ResultPinQuery, ResultQueryApplicationError};
use yss_graph_document::PortAddress;
use yss_graph_editor::projection::EditorPortModel;
use yss_graph_execution::{
    plan::{PlanGraphId, PlanOutputRef, PlanPortAddress},
    result::{ResultCacheState, ResultReference, ResultRetentionError},
};
use yss_node_protocol::PortDirection;

impl GraphCanvas {
    pub(super) fn port_result_outputs(&self, port: &EditorPortModel) -> Vec<PortAddress> {
        if port.direction == PortDirection::Output {
            return vec![port.address.clone()];
        }
        self.graph
            .projection
            .connections
            .iter()
            .filter(|edge| edge.input == port.address)
            .map(|edge| edge.output.clone())
            .collect()
    }

    pub(super) fn previous_port_results(&self, outputs: &[PortAddress]) -> Vec<ResultReference> {
        if self.graph.results.semantic_input_hash != self.graph.projection.basis.semantic_input_hash
        {
            return vec![];
        }
        outputs
            .iter()
            .filter_map(|address| {
                let output = self.plan_output(address);
                match self.graph.results.outputs.get(&output) {
                    Some(ResultCacheState::Stale { result_id }) => Some(ResultReference {
                        execution_session_id: self.graph.results.execution_session_id,
                        result_id: *result_id,
                    }),
                    _ => None,
                }
            })
            .collect()
    }

    fn plan_output(&self, address: &PortAddress) -> PlanOutputRef {
        PlanOutputRef::new(
            PlanGraphId::from_existing(self.path().to_owned().into_boxed_str()),
            PlanPortAddress::from_existing(address.to_string().into_boxed_str()),
        )
    }

    pub(in crate::canvas) fn inspect_port_result(
        &mut self,
        address: &PortAddress,
        previous: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(port) = self
            .graph
            .projection
            .nodes
            .iter()
            .find(|node| node.node_id == address.node_id)
            .and_then(|node| node.ports.iter().find(|port| port.address == *address))
        else {
            return;
        };
        let outputs = self.port_result_outputs(port);
        let targets = if previous {
            self.previous_port_results(&outputs)
                .into_iter()
                .map(Target::Previous)
                .collect::<Vec<_>>()
        } else {
            outputs
                .into_iter()
                .filter(|output| !self.result_waiting(&self.plan_output(output)))
                .map(Target::Current)
                .collect()
        };
        self.cancel_gesture();
        self.error = None;
        let projection = self.graph.projection.clone();
        let project = self.graph.project.clone();
        let version = self.graph.editing.version;
        let session = self.graph.results.execution_session_id;
        let path = projection.graph_path.clone();
        let task = self.services.run(move |services| {
            let app = &services.application;
            app.current_graph_document(&project, &path, version)?;
            for target in targets {
                let (reference, output) = match target {
                    Target::Previous(reference) => (reference, None),
                    Target::Current(output) => {
                        let Some(result) =
                            app.query_pin_result(ResultPinQuery::new(path.clone(), output))?
                        else {
                            continue;
                        };
                        (
                            result.provenance().reference(),
                            Some(result.output().clone()),
                        )
                    }
                };
                if reference.execution_session_id != session {
                    return Err(ResultQueryApplicationError::SessionChanged.into());
                }
                match app.retain_owned_result(reference) {
                    Ok((lease, _)) => {
                        app.current_graph_document(&project, &path, version)?;
                        return Ok(Some((Arc::new(lease), output)));
                    }
                    Err(ResultQueryApplicationError::Retention(
                        ResultRetentionError::Unavailable,
                    )) => continue,
                    Err(error) => return Err(error.into()),
                }
            }
            Ok(None)
        });
        self.read_task = Some(cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.read_task = None;
                if !view.can_edit()
                    || view.graph.editing.version != version
                    || view.graph.results.execution_session_id != session
                    || !Arc::ptr_eq(&view.graph.projection, &projection)
                {
                    return;
                }
                match result {
                    Ok(Some((lease, output))) => {
                        let current = output.as_ref().is_none_or(|output| {
                            view.result_entries().iter().any(|entry| {
                                entry.current()
                                    && entry.output == *output
                                    && entry.reference == lease.reference()
                            })
                        });
                        if current {
                            cx.emit(CanvasEvent::InspectResult(lease));
                        } else {
                            view.error = Some(crate::text::translate("native.results.unavailable"));
                        }
                    }
                    Ok(None) | Err(_) => {
                        view.error = Some(crate::text::translate("native.results.unavailable"))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

// Only the explicit previous-value action bypasses the current output query.
enum Target {
    Current(PortAddress),
    Previous(ResultReference),
}
