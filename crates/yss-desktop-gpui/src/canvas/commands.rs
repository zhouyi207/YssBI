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
    Edit(EditorGraphMutation),
    Undo,
    Redo,
    Save,
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
        self.submit_command(command, version, None, cx);
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
        self.submit_command(GraphCommand::Edit(mutation), Some(version), creation, cx);
    }

    fn submit_command(
        &mut self,
        command: GraphCommand,
        version: Option<GraphEditVersion>,
        creation: Option<gpui::Entity<super::palette::NodePalette>>,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.gesture = None;
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
        let task = self.services.run(move |services| {
            let application = &services.application;
            let response = match command {
                GraphCommand::Edit(mutation) => application.edit_graph(request, mutation)?,
                GraphCommand::Undo => application.change_graph_history(request, false)?,
                GraphCommand::Redo => application.change_graph_history(request, true)?,
                GraphCommand::Save => application.save_current_graph(request)?.graph,
                GraphCommand::UpdateConstant {
                    id,
                    name,
                    data_type,
                    value,
                } => {
                    let document = application.current_graph_document(
                        &request.project_instance_id,
                        &request.graph_path,
                        request.version,
                    )?;
                    let mut constant = document
                        .constants
                        .get(&id)
                        .ok_or_else(|| anyhow::anyhow!("constant disappeared"))?
                        .clone();
                    constant.name = name;
                    constant.data_type = data_type;
                    if let Some(value) = value {
                        constant.data_value =
                            super::authoring::parse_constant_input(value, &constant.data_type)?;
                        constant.tabular = None;
                    }
                    application.edit_graph(
                        request,
                        EditorGraphMutation::SetConstant {
                            id,
                            constant: Some(constant),
                        },
                    )?
                }
                GraphCommand::SetParameter {
                    node_id,
                    key,
                    value,
                } => application.edit_graph(
                    request,
                    EditorGraphMutation::SetParameters {
                        node_id,
                        parameters: [(key, value)].into_iter().collect(),
                    },
                )?,
            };
            Ok(response)
        });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(response) => {
                        view.install_response(response, cx);
                        if let Some(creation) = &creation {
                            creation.update(cx, |creation, cx| creation.creation_succeeded(cx));
                        }
                    }
                    Err(error) => {
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
            });
        })
        .detach();
        cx.notify();
    }

    pub fn install_response(&mut self, response: GraphEditResponse, cx: &mut Context<Self>) {
        if let Err(_error) = self.graph.install_edit(response) {
            self.error = Some("无法更新图状态，请重新打开图。".into());
            return;
        }
        *self.connection_layer.borrow_mut() =
            super::connections::ConnectionLayer::new(&self.graph.projection);
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
