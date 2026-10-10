//! Chart file menus capture the original typed resource path and Project revision.
use super::super::name_form::NameForm;
use super::{super::Workbench, ResourceAction};
use gpui_kit::{ClipboardItem, Context, Entity, Window};

use yss_chart_document::ChartResourcePath;
use yss_project_history::ResourceDocumentPatch;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

#[derive(Clone)]
struct ChartTarget {
    project: ProjectInstanceId,
    lifecycle: u64,
    path: ChartResourcePath,
    revision: ResourceRevision,
    name: String,
}
impl Workbench {
    pub(in crate::workbench) fn chart_resource_action(
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
        let Some(entry) = project
            .index
            .charts
            .iter()
            .find(|entry| entry.chart_path.as_str() == path)
        else {
            return;
        };
        let target = ChartTarget {
            project: project.identity.clone(),
            lifecycle: self.lifecycle,
            path: entry.chart_path.clone(),
            revision: entry.revision,
            name: entry.name.clone(),
        };
        if matches!(action, ResourceAction::CopyPath) {
            cx.write_to_clipboard(ClipboardItem::new_string(target.path.as_str().into()));
            return;
        }
        if self
            .charts
            .get(&path)
            .and_then(gpui_kit::WeakEntity::upgrade)
            .is_some_and(|chart| chart.read(cx).dirty())
        {
            self.error = Some(crate::text::t("native.workbench.saveChartBeforeEditing").into());
            cx.notify();
            return;
        }
        match action {
            ResourceAction::Rename => {
                self.resource_name_dialog(
                    crate::text::translate("contextMenu.dialog.renameChartTitle"),
                    target.name.clone(),
                    crate::text::translate("contextMenu.dialog.renameSubmit"),
                    window,
                    cx,
                    move |view, name, form, window, cx| {
                        view.mutate_chart_resource(
                            target.clone(),
                            action,
                            Some(name),
                            Some(form),
                            window,
                            cx,
                        );
                    },
                );
            }
            ResourceAction::Delete => {
                let owner = cx.entity().downgrade();
                crate::modal_window::confirm(
                    crate::text::format(
                        "native.workbench.deleteResourceTitle",
                        &[("value0", target.name.to_string())],
                    ),
                    crate::text::t("native.workbench.deleteChartMessage"),
                    crate::text::t("common.delete"),
                    crate::text::t("common.cancel"),
                    window,
                    cx,
                    move |_, window, cx| {
                        let _ = owner.update(cx, |view, cx| {
                            view.mutate_chart_resource(
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
            ResourceAction::Duplicate => {
                self.mutate_chart_resource(target, action, None, None, window, cx);
            }
            ResourceAction::CopyPath => {}
        }
    }
    fn mutate_chart_resource(
        &mut self,
        target: ChartTarget,
        action: ResourceAction,
        name: Option<String>,
        form: Option<Entity<NameForm>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_closing(cx)
            || self.lifecycle != target.lifecycle
            || self.project.as_ref().is_none_or(|project| {
                project.identity != target.project
                    || !project.index.charts.iter().any(|entry| {
                        entry.chart_path == target.path && entry.revision == target.revision
                    })
            })
            || self
                .charts
                .get(target.path.as_str())
                .and_then(gpui_kit::WeakEntity::upgrade)
                .is_some_and(|chart| chart.read(cx).dirty())
        {
            return false;
        }
        self.busy = true;
        self.error = None;
        let captured = target.clone();
        let owner = self.services.clone();
        let job = self.services.run(move |services| {
            let operation = OperationId::new();
            let mutation = match action {
                ResourceAction::Rename => services.application.rename_chart_resource(
                    captured.project,
                    operation,
                    captured.path.clone(),
                    captured.revision,
                    name.ok_or_else(|| anyhow::anyhow!("chart name missing"))?,
                    0,
                )?,
                ResourceAction::Duplicate => services.application.duplicate_chart_resource(
                    captured.project,
                    operation,
                    captured.path.clone(),
                    captured.revision,
                    None,
                )?,
                ResourceAction::Delete => services.application.remove_chart_resource(
                    captured.project,
                    operation,
                    captured.path.clone(),
                    captured.revision,
                )?,
                ResourceAction::CopyPath => unreachable!("copy does not mutate chart files"),
            };
            let path = mutation
                .moves
                .iter()
                .find(|moved| moved.from.as_ref() == captured.path.as_str())
                .map(|moved| moved.to.to_string())
                .or_else(|| {
                    mutation
                        .deltas
                        .iter()
                        .find_map(|delta| match &delta.payload {
                            ResourceDocumentPatch::ResourceLifecycle(patch)
                                if patch.before.is_none() =>
                            {
                                patch.after.as_ref().map(|after| after.path.to_string())
                            }
                            _ => None,
                        })
                });
            owner.publish_resource(mutation);
            Ok(path)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
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
                        .is_none()
                        .then_some("native.workbench.chartResourceFailed"),
                    window,
                    cx,
                );
                if let Some(path) = result {
                    if matches!(action, ResourceAction::Delete)
                        || (matches!(action, ResourceAction::Rename) && path.is_some())
                    {
                        view.retire_chart(target.path.as_str(), window, cx);
                    }
                    if let Some(path) = path {
                        view.open_chart(path, None, window, cx);
                    }
                }
                view.refresh_project(window, cx);
                view.persist_layout(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }
    fn retire_chart(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chart) = self.charts.remove(path).and_then(|chart| chart.upgrade()) else {
            return;
        };
        if self
            .details
            .read(cx)
            .chart()
            .is_some_and(|current| current.entity_id() == chart.entity_id())
        {
            self.details.update(cx, |details, cx| details.clear(cx));
        }
        self.dock
            .update(cx, |dock, cx| dock.remove_panel(chart, window, cx));
    }
}
