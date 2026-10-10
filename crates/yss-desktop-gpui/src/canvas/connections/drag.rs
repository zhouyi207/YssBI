//! The gesture owns its candidate query, so cancellation also cancels late delivery.
use std::collections::{BTreeMap, BTreeSet};

use gpui_kit::{Context, MouseButton, MouseDownEvent, Pixels, Point, Task, Window};
use yss_graph_document::{ConnectionId, NodeId, PortAddress};
use yss_graph_editor::{
    EditorGraphMutation,
    projection::{ConnectionDecision, ConnectionIntent},
};
use yss_node_protocol::PortDirection;
use yss_project::GraphEditVersion;

use crate::canvas::{Gesture, GraphCanvas, GraphCommand};

enum Candidates {
    Loading { _task: Task<()> },
    Ready(BTreeMap<PortAddress, ConnectionDecision>),
    Failed,
}

pub(in crate::canvas) struct ConnectionDrag {
    pub source: PortAddress,
    pub moving: bool,
    pub current: Point<Pixels>,
    pub press: Point<Pixels>,
    pub target: Option<PortAddress>,
    pub replaced: BTreeSet<ConnectionId>,
    pub dimmed_nodes: BTreeSet<NodeId>,
    version: GraphEditVersion,
    candidates: Candidates,
}

impl ConnectionDrag {
    pub fn decision(&self, port: &PortAddress) -> Option<&ConnectionDecision> {
        match &self.candidates {
            Candidates::Ready(candidates) => candidates.get(port),
            Candidates::Loading { .. } | Candidates::Failed => None,
        }
    }

    pub fn set_target(&mut self, target: Option<PortAddress>) -> bool {
        if self.target == target {
            return false;
        }
        self.target = target;
        self.update_replaced();
        true
    }

    fn update_replaced(&mut self) {
        self.replaced = match self
            .target
            .as_ref()
            .and_then(|target| self.decision(target))
        {
            Some(ConnectionDecision::Replace {
                displaced_connection_ids,
            }) => displaced_connection_ids.iter().copied().collect(),
            _ => BTreeSet::new(),
        };
    }

    pub fn feedback(&self) -> (u32, Option<String>) {
        use crate::{appearance, text};
        match self
            .target
            .as_ref()
            .and_then(|target| self.decision(target))
        {
            Some(ConnectionDecision::Append) => (appearance::GREEN, None),
            Some(ConnectionDecision::Replace { .. }) => (
                appearance::AMBER,
                Some(text::translate("canvas.connection.feedback.replace")),
            ),
            Some(ConnectionDecision::Invalid { reason }) => (
                appearance::RED,
                Some(text::translate(&format!(
                    "canvas.connection.errors.{reason}"
                ))),
            ),
            _ => (appearance::BLUE, None),
        }
    }
}

impl GraphCanvas {
    pub(in crate::canvas) fn connection_drag(&self) -> Option<&ConnectionDrag> {
        match &self.gesture {
            Some(Gesture::Connection(drag)) => Some(drag),
            _ => None,
        }
    }

    pub(in crate::canvas) fn connection_drag_mut(&mut self) -> Option<&mut ConnectionDrag> {
        match &mut self.gesture {
            Some(Gesture::Connection(drag)) => Some(drag),
            _ => None,
        }
    }

    pub(in crate::canvas) fn begin_port(
        &mut self,
        source: PortAddress,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if !self.can_edit() && event.button == MouseButton::Left {
            return;
        }
        self.cancel_gesture();
        self.located_port = None;
        self.selected_connections.clear();
        self.connection_click = None;
        self.palette = None;
        self.error = None;
        window.focus(&self.focus, cx);
        if event.modifiers.alt {
            self.submit(
                GraphCommand::Edit(EditorGraphMutation::DisconnectPort { address: source }),
                None,
                cx,
            );
            return;
        }
        let moving = event.modifiers.control || event.modifiers.platform;
        let allowed = self
            .port_details
            .as_ref()
            .and_then(|details| details.port(&source))
            .is_some_and(|port| {
                !port.orphan
                    && if moving {
                        port.connections.can_move
                    } else {
                        port.connections.can_append || port.connections.can_replace
                    }
            });
        if !allowed {
            cx.notify();
            return;
        }
        let version = self.graph.editing.version;
        let task = self.query_connections(source.clone(), moving, version, cx);
        self.gesture = Some(Gesture::Connection(ConnectionDrag {
            source,
            moving,
            version,
            current: event.position,
            press: event.position,
            target: None,
            replaced: BTreeSet::new(),
            dimmed_nodes: BTreeSet::new(),
            candidates: Candidates::Loading { _task: task },
        }));
        cx.notify();
    }

    pub(in crate::canvas) fn end_port(
        &mut self,
        target: PortAddress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_port(target, cx);
        self.commit_blurred_port_inputs(window, cx);
    }

    fn finish_port(&mut self, target: PortAddress, cx: &mut Context<Self>) {
        if self.connection_drag().is_none() {
            return;
        }
        let Some(Gesture::Connection(drag)) = self.gesture.take() else {
            return;
        };
        cx.stop_propagation();
        if drag.source == target {
            cx.notify();
            return;
        }
        if let Some(ConnectionDecision::Invalid { reason }) = drag.decision(&target) {
            self.error = Some(crate::text::translate(&format!(
                "canvas.connection.errors.{reason}"
            )));
        } else {
            // Missing preview decisions are neutral, including failed reads.
            // The mutation still validates the captured version and connection.
            let mutation = if drag.moving {
                EditorGraphMutation::MoveConnections {
                    source: drag.source,
                    target,
                }
            } else {
                let output = self.connection_layer.borrow().port_direction(&drag.source)
                    == Some(PortDirection::Output);
                let (output, input) = if output {
                    (drag.source, target)
                } else {
                    (target, drag.source)
                };
                EditorGraphMutation::Connect {
                    output,
                    input,
                    order: None,
                }
            };
            self.submit(GraphCommand::Edit(mutation), Some(drag.version), cx);
        }
        cx.notify();
    }

    fn query_connections(
        &self,
        source: PortAddress,
        moving: bool,
        version: GraphEditVersion,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let project = self.graph.project.clone();
        let path = self.graph.projection.graph_path.clone();
        let query = source.clone();
        let task = self.services.run(move |services| {
            Ok(services.application.graph_connection_candidates(
                &project,
                &path,
                version,
                &query,
                if moving {
                    ConnectionIntent::MoveConnections
                } else {
                    ConnectionIntent::Connect
                },
            )?)
        });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                if view.graph.editing.version != version {
                    return;
                }
                let candidates = result.map(|candidates| {
                    candidates
                        .candidates
                        .into_iter()
                        .map(|candidate| (candidate.port, candidate.decision))
                        .collect::<BTreeMap<_, _>>()
                });
                let dimmed_nodes = candidates
                    .as_ref()
                    .map(|candidates| {
                        view.graph
                            .projection
                            .nodes
                            .iter()
                            .filter(|node| {
                                node.node_id != source.node_id
                                    && node.ports.iter().all(|port| {
                                        matches!(
                                            candidates.get(&port.address),
                                            Some(ConnectionDecision::Invalid { .. })
                                        )
                                    })
                            })
                            .map(|node| node.node_id)
                            .collect()
                    })
                    .unwrap_or_default();
                let Some(drag) = view.connection_drag_mut().filter(|drag| {
                    drag.source == source && drag.moving == moving && drag.version == version
                }) else {
                    return;
                };
                drag.dimmed_nodes = dimmed_nodes;
                drag.candidates = match candidates {
                    Ok(candidates) => Candidates::Ready(candidates),
                    Err(_) => {
                        tracing::debug!(
                            code = "native_connection_candidates_rejected",
                            "Native connection candidates rejected"
                        );
                        Candidates::Failed
                    }
                };
                drag.update_replaced();
                cx.notify();
            });
        })
    }
}
