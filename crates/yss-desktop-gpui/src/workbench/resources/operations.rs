//! Graph resource commands capture the existing Project version before a menu/dialog is used.
use super::super::Workbench;
use super::super::name_form::NameForm;
use super::ResourceAction;
use gpui::{ClipboardItem, Context, Entity, Window};
use yss_graph_document::GraphResourcePath;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

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
        action: ResourceAction,
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
                && matches!(action, ResourceAction::Rename | ResourceAction::Delete)
            {
                self.error = Some(crate::text::t("native.workbench.stopGraphBeforeEditing").into());
                cx.notify();
                return;
            }
            revision = graph.resource_revision();
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
            ResourceAction::Rename => self.rename_graph_dialog(target, window, cx),
            ResourceAction::Duplicate => {
                self.mutate_graph_resource(target, action, None, None, window, cx)
            }
            ResourceAction::CopyPath => {
                cx.write_to_clipboard(ClipboardItem::new_string(target.path.as_str().into()))
            }
            ResourceAction::Delete => {
                let owner = cx.entity().downgrade();
                crate::modal_window::confirm(
                    crate::text::format(
                        "native.workbench.deleteResourceTitle",
                        &[("value0", target.name.to_string())],
                    ),
                    crate::text::t("native.workbench.deleteGraphMessage"),
                    crate::text::t("common.delete"),
                    crate::text::t("common.cancel"),
                    window,
                    cx,
                    move |_, window, cx| {
                        let _ = owner.update(cx, |view, cx| {
                            view.mutate_graph_resource(
                                target.clone(),
                                action,
                                None,
                                None,
                                window,
                                cx,
                            )
                        });
                        true
                    },
                );
            }
        }
    }

    pub(super) fn mutate_graph_resource(
        &mut self,
        target: GraphTarget,
        action: ResourceAction,
        name: Option<String>,
        form: Option<Entity<NameForm>>,
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
            && matches!(action, ResourceAction::Rename | ResourceAction::Delete)
        {
            self.error = Some(crate::text::t("native.workbench.stopGraphBeforeEditing").into());
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
                ResourceAction::Rename => application.rename_graph_resource(
                    request.project.clone(),
                    request.path.clone(),
                    request.revision,
                    name.unwrap_or_default(),
                    0,
                    OperationId::new(),
                )?,
                ResourceAction::Duplicate => application.duplicate_graph_resource(
                    request.project.clone(),
                    request.path.clone(),
                    request.revision,
                    OperationId::new(),
                    None,
                )?,
                ResourceAction::Delete => application.remove_graph_resource(
                    request.project.clone(),
                    request.path.clone(),
                    request.revision,
                    OperationId::new(),
                )?,
                ResourceAction::CopyPath => unreachable!(),
            };
            let path = match action {
                ResourceAction::Duplicate => {
                    receipt
                        .deltas
                        .iter()
                        .find_map(|delta| match &delta.payload {
                            yss_project_history::ResourceDocumentPatch::ResourceLifecycle(
                                patch,
                            ) if patch.before.is_none() => {
                                patch.after.as_ref().map(|after| after.path.to_string())
                            }
                            _ => None,
                        })
                }
                _ => None,
            };
            publisher.publish_resource(receipt.clone());
            Ok((receipt, path))
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
                    NameForm::fail(form.as_ref(), "native.workbench.resourceChanged", cx);
                    return;
                }
                view.busy = false;
                view.finish_resource_name(
                    form,
                    result
                        .is_err()
                        .then_some("native.workbench.graphResourceFailed"),
                    window,
                    cx,
                );
                match result {
                    Ok((receipt, path)) => {
                        view.accept_graph_resources(&receipt, window, cx);
                        if let Some(path) = path {
                            view.open_graph(path, window, cx);
                        }
                    }
                    Err(_) => view.refresh_project(window, cx),
                }
                view.persist_layout(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
