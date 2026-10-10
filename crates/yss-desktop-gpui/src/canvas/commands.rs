mod worker;

use gpui::{Context, actions};
use yss_application::graph::editing::{GraphEditRequest, GraphEditResponse};
use yss_graph_editor::EditorGraphMutation;
use yss_project::GraphEditVersion;
use yss_project_identity::OperationId;

use super::{CanvasEvent, GraphCanvas};

actions!(
    graph_canvas,
    [
        SaveGraph,
        UndoGraph,
        RedoGraph,
        DeleteSelection,
        DuplicateSelection,
        SelectAll,
        CancelGesture,
        FrameGraph,
        RunWholeGraph,
        CancelRun,
        RunCurrentNode,
        RunToNode,
        RefreshRunState
    ]
);

pub enum GraphCommand {
    CommitPortInputs,
    RunAfterPortInputs(yss_application::graph::run::RunDemand),
    Edit(EditorGraphMutation),
    Undo,
    Redo,
    Save,
    Paste {
        source: String,
        anchor: yss_graph_document::NodePosition,
    },
    SetParameter {
        node_id: yss_graph_document::NodeId,
        key: yss_node_protocol::ParameterKey,
        value: serde_json::Value,
    },
    UpdateConstant {
        id: yss_graph_document::ConstantId,
        name: String,
        data_type: yss_data_contract::ValueType,
        value: Option<super::ConstantValueInput>,
    },
}

impl GraphCanvas {
    pub(crate) fn submit(
        &mut self,
        command: GraphCommand,
        version: Option<GraphEditVersion>,
        cx: &mut Context<Self>,
    ) {
        if let Some(task) = self.submit_command(command, version, None, cx) {
            task.detach();
        }
    }

    pub(super) fn submit_creation(
        &mut self,
        mutation: EditorGraphMutation,
        version: GraphEditVersion,
        cx: &mut Context<Self>,
    ) {
        let creation = self.palette.as_ref().map(|palette| palette.view.clone());
        if self.busy {
            if let Some(creation) = creation {
                creation.update(cx, |creation, cx| creation.creation_failed(cx));
            }
            return;
        }
        if let Some(task) =
            self.submit_command(GraphCommand::Edit(mutation), Some(version), creation, cx)
        {
            task.detach();
        }
    }

    pub(in crate::canvas) fn submit_command(
        &mut self,
        command: GraphCommand,
        version: Option<GraphEditVersion>,
        creation: Option<gpui::Entity<super::palette::NodePalette>>,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Task<Option<GraphEditVersion>>> {
        if self.busy {
            return None;
        }
        let Some(inputs) = self.prepare_port_edits(version, cx) else {
            if let Some(creation) = creation {
                creation.update(cx, |creation, cx| creation.creation_failed(cx));
            }
            return None;
        };
        if inputs.is_empty() && matches!(command, GraphCommand::CommitPortInputs) {
            return None;
        }
        let reroute_selection = if matches!(
            &command,
            GraphCommand::Edit(EditorGraphMutation::InsertReroute { .. })
        ) {
            self.connection_click.take()
        } else {
            self.connection_click = None;
            None
        };
        let insertion_selection = matches!(
            &command,
            GraphCommand::Paste { .. }
                | GraphCommand::Edit(EditorGraphMutation::DuplicateSubgraph { .. })
        )
        .then(|| self.selected.clone());
        self.read_task = None;
        self.context_menu = None;
        self.hovered_connection = None;
        if self.gesture.is_some() {
            self.cancel_gesture();
            self.emit_selection(cx);
        }
        if creation.is_none() {
            self.palette = None;
        }
        self.busy = true;
        self.error = None;
        let request = GraphEditRequest {
            project_instance_id: self.graph.project.clone(),
            graph_path: self.graph.projection.graph_path.clone(),
            version: version.unwrap_or(self.graph.editing.version),
            operation_id: OperationId::new(),
            locale: "zh-CN".into(),
        };
        let run_after = if let GraphCommand::RunAfterPortInputs(demand) = &command {
            Some(demand.clone())
        } else {
            None
        };
        let submitted_inputs = inputs.clone();
        let task = self.services.run(move |services| {
            Ok(worker::apply(
                &services.application,
                request,
                &inputs,
                command,
            ))
        });
        let task = cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update(cx, |view, cx| {
                view.busy = false;
                let mut outcome = result.unwrap_or_else(|error| worker::Outcome {
                    error: Some(error),
                    ..Default::default()
                });
                let completed = outcome.error.is_none();
                view.accept_port_edits(&submitted_inputs[..outcome.applied_inputs]);
                if outcome.error.is_some()
                    && let Some(response) = outcome.response.take()
                {
                    view.install_response(response, cx);
                }
                let result = match outcome.error {
                    Some(error) => Err(error),
                    None => Ok(outcome.response),
                };
                match result {
                    Ok(response) => {
                        if let Some(response) = &response
                            && insertion_selection
                                .as_ref()
                                .is_some_and(|selection| *selection == view.selected)
                        {
                            view.selected = response
                                .update
                                .patch
                                .operations
                                .iter()
                                .filter_map(|operation| match operation {
                                    yss_graph_document::GraphDocumentOperation::InsertNode {
                                        node,
                                    } => Some(node.id),
                                    _ => None,
                                })
                                .collect();
                            view.selected_connections.clear();
                        }
                        if let Some(response) = response {
                            view.install_response(response, cx);
                        }
                        if let Some(demand) = run_after {
                            view.run_graph(demand, cx);
                        }
                        if let Some(creation) = &creation {
                            creation.update(cx, |creation, cx| creation.creation_succeeded(cx));
                        }
                    }
                    Err(error) => {
                        if let Some(selection) = reroute_selection {
                            view.restore_connection_click(selection, cx);
                        }
                        tracing::error!(
                            code = "native_graph_command_failed",
                            "Native graph command failed"
                        );
                        view.error = Some(crate::text::translate(
                            error
                                .downcast_ref::<crate::constant_values::InputError>()
                                .map_or("native.canvas.commandFailed", |error| error.0),
                        ));
                        if let Some(creation) = &creation {
                            creation.update(cx, |creation, cx| creation.creation_failed(cx));
                        }
                        view.preview.clear();
                        view.refresh_pending = true;
                    }
                }
                if view.refresh_pending {
                    view.refresh(cx);
                }
                cx.notify();
                completed.then_some(view.graph.editing.version)
            })
            .ok()
            .flatten()
        });
        cx.notify();
        Some(task)
    }

    pub fn install_response(&mut self, response: GraphEditResponse, cx: &mut Context<Self>) {
        if let Err(_error) = self.graph.install_edit(response) {
            self.error = Some("无法更新图状态，请重新打开图。".into());
            return;
        }
        *self.connection_layer.borrow_mut() =
            super::connections::ConnectionLayer::new(&self.graph.projection);
        self.refresh_presentation();
        self.retain_located();
        self.selected.retain(|id| {
            self.graph
                .projection
                .nodes
                .iter()
                .any(|node| node.node_id == *id)
        });
        self.cancel_gesture();
        cx.emit(CanvasEvent::Projection {
            nodes: self.selected.iter().copied().collect(),
            projection: self.graph.projection.clone(),
        });
        cx.emit(CanvasEvent::Edited);
        cx.emit(gpui_component::dock::PanelEvent::LayoutChanged);
        if self.refresh_pending {
            self.refresh(cx);
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.refreshing {
            self.refresh_pending = true;
            return;
        }
        self.refreshing = true;
        self.refresh_pending = false;
        let version = self.graph.editing.version;
        let path = self.graph.projection.graph_path.clone();
        let request = yss_application::graph::open::OpenGraphRequest::new(
            self.graph.project.clone(),
            self.graph.projection.graph_path.clone(),
            0,
            "zh-CN",
        );
        let task = self.services.run(move |services| {
            Ok(crate::project::OpenedGraph::from_open(
                services.application.open_graph(request)?,
            ))
        });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.refreshing = false;
                if view.graph.projection.graph_path != path {
                    return;
                }
                if view.busy || view.graph.editing.version != version {
                    view.refresh_pending = true;
                } else {
                    match result {
                        Ok(graph) => {
                            view.install_projection(graph, cx);
                        }
                        Err(_error) => tracing::warn!(
                            code = "native_graph_refresh_failed",
                            "Native graph refresh failed"
                        ),
                    }
                }
                if view.refresh_pending && !view.busy {
                    view.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn install_projection(
        &mut self,
        graph: crate::project::OpenedGraph,
        cx: &mut Context<Self>,
    ) {
        self.graph.replace(graph);
        *self.connection_layer.borrow_mut() =
            super::connections::ConnectionLayer::new(&self.graph.projection);
        self.refresh_presentation();
        self.cancel_gesture();
        self.retain_located();
        self.selected.retain(|id| {
            self.graph
                .projection
                .nodes
                .iter()
                .any(|node| node.node_id == *id)
        });
        self.resync_execution(cx);
        cx.emit(CanvasEvent::Projection {
            nodes: self.selected.iter().copied().collect(),
            projection: self.graph.projection.clone(),
        });
        cx.emit(gpui_component::dock::PanelEvent::LayoutChanged);
        cx.notify();
    }

    pub fn create_node(
        &mut self,
        descriptor: yss_node_catalog::NodeCreation,
        cx: &mut Context<Self>,
    ) {
        self.create_node_at(descriptor, self.world(self.bounds.get().center()), cx);
    }

    pub(super) fn create_node_at(
        &mut self,
        descriptor: yss_node_catalog::NodeCreation,
        position: yss_graph_document::NodePosition,
        cx: &mut Context<Self>,
    ) {
        self.submit(
            GraphCommand::Edit(EditorGraphMutation::CreateNode {
                descriptor,
                position,
                connect_from: None,
                parameters: Default::default(),
                port_counts: Default::default(),
                user_label: None,
            }),
            Some(self.graph.editing.version),
            cx,
        );
    }
}
