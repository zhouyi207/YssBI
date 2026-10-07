//! Import submission and index installation reuse the existing Application boundary.
use super::Workbench;
use crate::{
    imports::{ImportDialog, ImportRequest, ImportScope},
    project::DesktopProject,
};
use gpui::{AppContext, Context, WeakEntity, Window, prelude::*, px};
use gpui_component::WindowExt;
use yss_project_identity::OperationId;

impl Workbench {
    pub(super) fn import_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let scope = ImportScope {
            project: project.identity.clone(),
            lifecycle: self.lifecycle,
        };
        let services = self.services.clone();
        let owner = cx.entity().downgrade();
        let editor = cx.new(|cx| ImportDialog::new(services, owner, scope, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            let cancel = editor.clone();
            dialog
                .title("导入数据")
                .width(px(740.))
                .close_button(false)
                .overlay_closable(false)
                .footer(gpui::div())
                .child(editor.clone())
                .on_cancel(move |_, window, cx| {
                    cancel.update(cx, |view, cx| view.cancel(window, cx))
                })
        });
    }
    pub(crate) fn start_database_import(
        &mut self,
        scope: ImportScope,
        request: ImportRequest,
        dialog: WeakEntity<ImportDialog>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_closing(cx)
            || scope.lifecycle != self.lifecycle
            || self
                .project
                .as_ref()
                .is_none_or(|project| project.identity != scope.project)
        {
            return false;
        }
        self.busy = true;
        self.error = None;
        let expected = scope.clone();
        let publisher = self.services.clone();
        let job = self.services.run(move |services| {
            let result = match request {
                ImportRequest::Source { source, name } => services
                    .application
                    .load_database_for_application(
                        scope.project.clone(),
                        OperationId::new(),
                        source,
                        name,
                    )
                    .map_err(anyhow::Error::from),
                ImportRequest::Sample { id, version } => services
                    .application
                    .import_sample_dataset_for_application(
                        &services.samples,
                        scope.project.clone(),
                        OperationId::new(),
                        &id,
                        version,
                    )
                    .map_err(anyhow::Error::from),
            };
            let id = result.ok().map(|receipt| {
                publisher.publish_resource(receipt.mutation);
                receipt.data.id
            });
            let project = services
                .application
                .query_project_index(scope.project.clone(), "zh-CN", true)
                .ok()
                .map(|snapshot| DesktopProject::new(scope.project, snapshot));
            Ok((id, project))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != expected.lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != expected.project)
                {
                    return;
                }
                view.busy = false;
                let (id, index) = result.unwrap_or((None, None));
                if let Some(index) = index {
                    view.install_project_index(index, window, cx);
                }
                // Import can commit before a failed session refresh; re-query the actual owners in either case.
                view.rebind_session(window, cx);
                let failed = id.is_none();
                if let Some(id) = id {
                    view.open_database(id, None, window, cx);
                } else {
                    view.error = Some("导入未完成，请检查来源与项目目录。".into());
                }
                let _ = dialog.update(cx, |dialog, cx| dialog.finished(failed, window, cx));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }
}
