//! Resolve current output identities without running the graph.
use crate::canvas::{CanvasEvent, GraphCanvas};
use gpui_kit::Context;
use std::sync::Arc;
use yss_application::graph::results::{ResultPinQuery, ResultQueryApplicationError};
use yss_graph_document::PortAddress;
use yss_graph_editor::projection::EditorPortModel;
use yss_graph_execution::result::{ResultReference, ResultRetentionError};
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

    pub(crate) fn inspect_result(&mut self, reference: ResultReference, cx: &mut Context<Self>) {
        let Some(address) = self
            .result_entries()
            .iter()
            .find(|entry| entry.reference == reference)
            .map(|entry| entry.address.clone())
        else {
            return;
        };
        self.inspect_port_result(&address, cx);
    }

    pub(in crate::canvas) fn inspect_port_result(
        &mut self,
        address: &PortAddress,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit()
            || self.graph.results.semantic_input_hash
                != self.graph.projection.basis.semantic_input_hash
        {
            return;
        }
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
        let targets = self
            .port_result_outputs(port)
            .into_iter()
            .filter_map(|address| {
                let entry = self
                    .result_entries()
                    .iter()
                    .find(|entry| entry.address == address)?;
                (!self.result_waiting(&entry.output)).then_some((address, entry.reference))
            })
            .collect::<Vec<_>>();
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
            for (address, expected) in targets {
                let Some(result) =
                    app.query_pin_result(ResultPinQuery::new(path.clone(), address.clone()))?
                else {
                    continue;
                };
                let reference = result.provenance().reference();
                if reference != expected {
                    continue;
                }
                let output = result.output().clone();
                if reference.execution_session_id != session {
                    return Err(ResultQueryApplicationError::SessionChanged.into());
                }
                match app.retain_owned_result(reference) {
                    Ok((lease, _)) => {
                        app.current_graph_document(&project, &path, version)?;
                        if app
                            .query_pin_result(ResultPinQuery::new(path.clone(), address))?
                            .is_none_or(|result| result.provenance().reference() != reference)
                        {
                            continue;
                        }
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
                        let current = view.graph.results.semantic_input_hash
                            == view.graph.projection.basis.semantic_input_hash
                            && !view.result_waiting(&output)
                            && view.result_entries().iter().any(|entry| {
                                entry.output == output && entry.reference == lease.reference()
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
