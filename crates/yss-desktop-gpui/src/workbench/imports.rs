//! Import submission and index installation reuse the existing Application boundary.
use super::Workbench;
use crate::{
    imports::{IMPORT_FAILED, ImportDialog, ImportRequest, ImportScope, sample_import_failure},
    project::DesktopProject,
};
use gpui::{AppContext, Context, WeakEntity, Window, px};
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
        crate::modal_window::open(
            crate::text::t("importModal.title"),
            gpui::size(px(780.), px(660.)),
            window,
            cx,
            move |window, cx| {
                let editor = cx.new(|cx| ImportDialog::new(services, owner, scope, window, cx));
                let cancel = editor.clone();
                crate::modal_window::ModalContent::new(move |_, _| editor.clone())
                    .without_buttons()
                    .on_cancel(move |_, window, cx| {
                        cancel.update(cx, |view, cx| view.cancel(window, cx))
                    })
            },
        );
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
                    .map_err(|_| IMPORT_FAILED),
                ImportRequest::Sample { id, version } => services
                    .application
                    .import_sample_dataset_for_application(
                        &services.samples,
                        scope.project.clone(),
                        OperationId::new(),
                        &id,
                        version,
                    )
                    .map_err(|error| sample_import_failure(&error)),
            };
            let imported = result.map(|receipt| {
                publisher.publish_resource(receipt.mutation);
                receipt.data.id
            });
            let language = crate::text::locale();
            let project = services
                .application
                .query_project_index(scope.project.clone(), language, true)
                .ok()
                .map(|snapshot| DesktopProject::new(scope.project, snapshot, language));
            Ok((imported, project))
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
                let (imported, index) = result.unwrap_or((Err(IMPORT_FAILED), None));
                if let Some(index) = index {
                    view.install_project_index(index, window, cx);
                }
                // Import can commit before a failed session refresh; re-query the actual owners in either case.
                view.rebind_session(window, cx);
                match &imported {
                    Ok(id) => {
                        view.expand_project_category("project.data", cx);
                        view.open_database(id.clone(), None, window, cx);
                    }
                    Err(key) => view.error = Some(crate::text::translate(key)),
                }
                let _ = dialog.update(cx, |dialog, cx| {
                    dialog.finished(imported.map(|_| ()), window, cx)
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }
}
