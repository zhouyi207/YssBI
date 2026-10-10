mod worker;
pub(crate) use worker::{GraphCommandOutcome, GraphCommandRequest};

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
        FrameSelection,
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
        if !self.can_edit() {
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
        let Some(request) = self.capture_command(command, version, cx) else {
            if let Some(creation) = creation {
                creation.update(cx, |creation, cx| creation.creation_failed(cx));
            }
            return None;
        };
        let reroute_selection = if matches!(
            &request.command,
            GraphCommand::Edit(EditorGraphMutation::InsertReroute { .. })
        ) {
            self.connection_click.take()
        } else {
            self.connection_click = None;
            None
        };
        let insertion_selection = matches!(
            &request.command,
            GraphCommand::Paste { .. }
                | GraphCommand::Edit(EditorGraphMutation::DuplicateSubgraph { .. })
        )
        .then(|| self.selected.clone());
        if creation.is_none() {
            self.palette = None;
        }
        self.begin_command(cx);
        let run_after = if let GraphCommand::RunAfterPortInputs(demand) = &request.command {
            Some(demand.clone())
        } else {
            None
        };
        let task = self
            .services
            .run(move |services| Ok(request.commit(&services.application)));
        let task = cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update(cx, |view, cx| {
                let outcome = result.unwrap_or_else(|error| GraphCommandOutcome {
                    error: Some(error),
                    ..Default::default()
                });
                if outcome.error.is_none() {
                    if let Some(response) = &outcome.response
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
                                yss_graph_document::GraphDocumentOperation::InsertNode { node } => {
                                    Some(node.id)
                                }
                                _ => None,
                            })
                            .collect();
                        view.selected_connections.clear();
                    }
                } else if let Some(selection) = reroute_selection {
                    view.restore_connection_click(selection, cx);
                }
                let completed = view.finish_command(outcome, cx);
                if completed && let Some(demand) = run_after {
                    view.run_graph(demand, cx);
                }
                if let Some(creation) = &creation {
                    creation.update(cx, |creation, cx| {
                        if completed {
                            creation.creation_succeeded(cx);
                        } else {
                            creation.creation_failed(cx);
                        }
                    });
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

    fn capture_command(
        &mut self,
        command: GraphCommand,
        version: Option<GraphEditVersion>,
        cx: &mut Context<Self>,
    ) -> Option<GraphCommandRequest> {
        if !self.can_edit() {
            return None;
        }
        let inputs = self.prepare_port_edits(version, cx)?;
        if inputs.is_empty() && matches!(command, GraphCommand::CommitPortInputs) {
            return None;
        }
        Some(GraphCommandRequest {
            request: GraphEditRequest {
                project_instance_id: self.graph.project.clone(),
                graph_path: self.graph.projection.graph_path.clone(),
                version: version.unwrap_or(self.graph.editing.version),
                operation_id: OperationId::new(),
                locale: crate::text::locale().into(),
            },
            inputs,
            command,
        })
    }

    pub(super) fn begin_command(&mut self, cx: &mut Context<Self>) {
        if self.refresh_task.take().is_some() {
            self.refresh_pending = true;
        }
        self.read_task = None;
        self.context_menu = None;
        self.hovered_connection = None;
        if self.gesture.is_some() {
            self.cancel_gesture();
            self.emit_selection(cx);
        }
        self.busy = true;
        self.error = None;
        cx.notify();
    }

    pub(crate) fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<GraphCommandRequest> {
        let request = self.capture_command(GraphCommand::Save, None, cx)?;
        self.palette = None;
        self.connection_click = None;
        self.begin_command(cx);
        Some(request)
    }

    pub(crate) fn finish_command(
        &mut self,
        outcome: GraphCommandOutcome,
        cx: &mut Context<Self>,
    ) -> bool {
        self.busy = false;
        self.accept_port_edits(&outcome.inputs[..outcome.applied_inputs]);
        self.cancel_port_edits();
        self.refresh_pending |= outcome.language != crate::text::locale();
        let mut completed = outcome.error.is_none();
        if let Some(response) = outcome.response {
            completed &= self.install_response(response, cx);
        }
        if let Some(error) = outcome.error {
            tracing::error!(
                code = "native_graph_command_failed",
                "Native graph command failed"
            );
            self.error = Some(crate::text::translate(
                error
                    .downcast_ref::<crate::constant_values::InputError>()
                    .map_or("native.canvas.commandFailed", |error| error.0),
            ));
        }
        if !completed {
            self.preview.clear();
            self.refresh_pending = true;
        }
        if self.refresh_pending {
            self.refresh(cx);
        }
        cx.notify();
        completed
    }

    pub(crate) fn cancel_prepared_save(&mut self, cx: &mut Context<Self>) {
        self.busy = false;
        self.cancel_port_edits();
        if self.refresh_pending {
            self.refresh(cx);
        }
        cx.notify();
    }

    pub(crate) fn fail_prepared_save(&mut self, cx: &mut Context<Self>) {
        self.finish_command(
            GraphCommandOutcome {
                error: Some(anyhow::anyhow!("graph save worker failed")),
                ..Default::default()
            },
            cx,
        );
    }

    fn install_response(&mut self, response: GraphEditResponse, cx: &mut Context<Self>) -> bool {
        if let Err(_error) = self.graph.install_edit(response) {
            self.error = Some("无法更新图状态，请重新打开图。".into());
            return false;
        }
        self.refresh_task = None;
        self.refresh_failed = false;
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
        true
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
