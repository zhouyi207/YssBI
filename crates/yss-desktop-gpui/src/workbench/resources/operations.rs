//! Graph resource commands capture the existing Project version before a menu/dialog is used.
use super::super::Workbench;
use gpui::{ClipboardItem, Context, PromptLevel, Window};
use yss_application::graph::open::OpenGraphRequest;
use yss_graph_document::GraphResourcePath;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

#[derive(Clone, Copy)]
pub(crate) enum GraphResourceAction {
    Rename,
    Duplicate,
    Delete,
    CopyPath,
}

#[derive(Clone)]
pub(super) struct GraphTarget {
    pub project: ProjectInstanceId,
    pub lifecycle: u64,
    pub path: GraphResourcePath,
    pub revision: ResourceRevision,
    pub name: String,
}

impl Workbench {
    pub(in crate::workbench) fn graph_resource_action(
        &mut self,
        path: String,
        action: GraphResourceAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let record = project
            .index
            .event_graphs
            .iter()
            .find(|record| record.path == path)
            .map(|record| (record.name.clone(), record.revision))
            .or_else(|| {
                project
                    .index
                    .function_graphs
                    .iter()
                    .find(|record| record.path == path)
                    .map(|record| (record.name.clone(), record.revision))
            });
        let Some((name, mut revision)) = record else {
            return;
        };
        if let Some(graph) = self.graphs.get(&path).and_then(gpui::WeakEntity::upgrade) {
            let graph = graph.read(cx);
            if graph.is_running()
                && matches!(
                    action,
                    GraphResourceAction::Rename | GraphResourceAction::Delete
                )
            {
                self.error = Some("请先停止该图的运行，再修改图资源。".into());
                cx.notify();
                return;
            }
            revision = graph.graph.editing.version.revision;
        }
        let Ok(path) = GraphResourcePath::new(path) else {
            return;
        };
        let target = GraphTarget {
            project: project.identity.clone(),
            lifecycle: self.lifecycle,
            path,
            revision,
            name,
        };
        match action {
            GraphResourceAction::Rename => self.rename_graph_dialog(target, window, cx),
            GraphResourceAction::Duplicate => {
                self.mutate_graph_resource(target, action, None, window, cx)
            }
            GraphResourceAction::CopyPath => {
                cx.write_to_clipboard(ClipboardItem::new_string(target.path.as_str().into()))
            }
            GraphResourceAction::Delete => {
                let prompt = window.prompt(
                    PromptLevel::Warning,
                    &format!("删除“{}”？", target.name),
                    Some("图文件和未保存的更改将被删除，调用或引用该图的节点需要重新配置。"),
                    &["删除", "取消"],
                    cx,
                );
                cx.spawn_in(window, async move |view, cx| {
                    if matches!(prompt.await, Ok(0)) {
                        let _ = view.update_in(cx, |view, window, cx| {
                            view.mutate_graph_resource(target, action, None, window, cx)
                        });
                    }
                })
                .detach();
            }
        }
    }

    pub(super) fn mutate_graph_resource(
        &mut self,
        target: GraphTarget,
        action: GraphResourceAction,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx)
            || self.lifecycle != target.lifecycle
            || self
                .project
                .as_ref()
                .is_none_or(|project| project.identity != target.project)
        {
            return;
        }
        if let Some(graph) = self
            .graphs
            .get(target.path.as_str())
            .and_then(gpui::WeakEntity::upgrade)
            && graph.read(cx).is_running()
            && matches!(
                action,
                GraphResourceAction::Rename | GraphResourceAction::Delete
            )
        {
            self.error = Some("请先停止该图的运行，再修改图资源。".into());
            cx.notify();
            return;
        }
        self.busy = true;
        self.error = None;
        window.focus(&self.focus, cx);
        let publisher = self.services.clone();
        let request = target.clone();
        let job = self.services.run(move |services| {
            let application = &services.application;
            let receipt = match action {
                GraphResourceAction::Rename => application.rename_graph_resource(
                    request.project.clone(),
                    request.path.clone(),
                    request.revision,
                    name.unwrap_or_default(),
                    request.lifecycle,
                    OperationId::new(),
                )?,
                GraphResourceAction::Duplicate => application.duplicate_graph_resource(
                    request.project.clone(),
                    request.path.clone(),
                    request.revision,
                    OperationId::new(),
                    None,
                )?,
                GraphResourceAction::Delete => application.remove_graph_resource(
                    request.project.clone(),
                    request.path.clone(),
                    request.revision,
                    OperationId::new(),
                )?,
                GraphResourceAction::CopyPath => unreachable!(),
            };
            let path = receipt.deltas.iter().find_map(|delta| {
                if let yss_project_history::ResourceDocumentPatch::ResourceLifecycle(patch) = &delta.payload
                    && match action {
                        GraphResourceAction::Rename => patch.before.as_ref().is_some_and(|before| before.path.as_ref() == request.path.as_str()),
                        GraphResourceAction::Duplicate => patch.before.is_none(),
                        _ => false,
                    }
                {
                    patch
                        .after
                        .as_ref()
                        .filter(|after| after.path.as_ref() != request.path.as_str())
                        .map(|after| after.path.to_string())
                } else {
                    None
                }
            });
            publisher.publish_resource(receipt);
            let projection = if matches!(action, GraphResourceAction::Rename) {
                path.as_ref()
                    .map(|path| {
                        application
                            .open_graph(OpenGraphRequest::new(
                                request.project.clone(),
                                GraphResourcePath::new(path.clone())?,
                                request.lifecycle,
                                "zh-CN",
                            ))
                            .map(crate::project::OpenedGraph::from_open)
                            .map_err(anyhow::Error::from)
                    })
                    .transpose()
            } else {
                Ok(None)
            };
            // Publication succeeded even if its replacement view could not be read.
            Ok((path, projection))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != target.lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != target.project)
                {
                    return;
                }
                view.busy = false;
                match result {
                    Ok((path, projection)) => match action {
                        GraphResourceAction::Rename => {
                            let canvas = view
                                .graphs
                                .remove(target.path.as_str())
                                .and_then(|canvas| canvas.upgrade());
                            match (path, projection) {
                                (Some(path), Ok(Some(graph))) => {
                                    if let Some(canvas) = canvas {
                                        canvas.update(cx, |canvas, cx| {
                                            canvas.install_projection(graph, cx)
                                        });
                                        view.graphs.insert(path, canvas.downgrade());
                                    } else {
                                        view.install_graph(graph, window, cx);
                                    }
                                }
                                (path, _) => {
                                    if let Some(canvas) = canvas {
                                        view.retire_graph(canvas, window, cx);
                                    }
                                    if let Some(path) = path {
                                        view.open_graph(path, window, cx);
                                    }
                                    view.error = Some(
                                        "图已重命名，视图未能恢复，请从项目目录重新打开。".into(),
                                    );
                                }
                            }
                        }
                        GraphResourceAction::Duplicate => {
                            if let Some(path) = path {
                                view.open_graph(path, window, cx);
                            }
                        }
                        GraphResourceAction::Delete => {
                            if let Some(canvas) = view
                                .graphs
                                .remove(target.path.as_str())
                                .and_then(|canvas| canvas.upgrade())
                            {
                                view.retire_graph(canvas, window, cx);
                            }
                        }
                        GraphResourceAction::CopyPath => {}
                    },
                    Err(_) => {
                        view.error =
                            Some("图资源未能修改，资源可能已变化，请刷新目录后重试。".into());
                        view.refresh_project(window, cx);
                    }
                }
                view.persist_layout(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn retire_graph(
        &mut self,
        canvas: gpui::Entity<crate::canvas::GraphCanvas>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .details
            .read(cx)
            .graph()
            .is_some_and(|current| current.entity_id() == canvas.entity_id())
        {
            self.details.update(cx, |details, cx| details.clear(cx));
            self.problems.update(cx, |problems, cx| problems.clear(cx));
            self.output
                .update(cx, |output, cx| output.set_graph(None, cx));
            self.results.update(cx, |results, cx| {
                results.set_graph(None);
                cx.notify();
            });
        }
        self.dock
            .update(cx, |dock, cx| dock.remove_panel(canvas, window, cx));
    }
}
