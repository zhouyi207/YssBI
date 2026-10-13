//! Import submission and index installation reuse the existing Application boundary.
use super::Workbench;
use crate::project::DesktopProject;
use gpui_kit::{AppContext, Context, WeakEntity, Window, px};
use yss_data_ui::imports::{
    IMPORT_FAILED, ImportDialog, ImportRequest, ImportScope, ImportSubmission,
    sample_import_failure,
};
use yss_project_identity::OperationId;
use yss_ui::lifecycle::LogReleased;

impl Workbench {
    pub(super) fn import_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let scope = ImportScope::new(project.identity.clone(), self.lifecycle);
        let services = self.services.data.clone();
        let owner = cx.entity().downgrade();
        let parent = window.window_handle();
        crate::modal_window::open(
            crate::text::t("importModal.title"),
            gpui_kit::size(px(780.), px(660.)),
            window,
            cx,
            move |window, cx| {
                let editor = cx.new(|cx| ImportDialog::new(services, scope, window, cx));
                let subscription =
                    cx.subscribe(&editor, move |dialog, submission: &ImportSubmission, cx| {
                        let (scope, request) = submission.clone().into_parts();
                        let dialog = dialog.downgrade();
                        let failure = dialog.clone();
                        let owner = owner.clone();
                        cx.defer(move |cx| {
                            let started = parent
                                .update(cx, |_, window, cx| {
                                    owner
                                        .update(cx, |view, cx| {
                                            view.start_database_import(
                                                scope, request, dialog, window, cx,
                                            )
                                        })
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false);
                            if !started {
                                failure
                                    .update(cx, |view, cx| view.reject_submission(cx))
                                    .log_released();
                            }
                        });
                    });
                let cancel = editor.clone();
                crate::modal_window::ModalContent::new(move |_, _| {
                    let _ = &subscription;
                    editor.clone()
                })
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
            || scope.lifecycle() != self.lifecycle
            || self
                .project
                .as_ref()
                .is_none_or(|project| &project.identity != scope.project())
        {
            return false;
        }
        self.busy = true;
        self.error = None;
        let expected = scope.clone();
        let publisher = self.services.tasks.clone();
        let samples = self.services.application.samples.clone();
        let job = self.services.tasks.run(move |application| {
            let result = match request {
                ImportRequest::Source { source, name } => application
                    .load_database_for_application(
                        scope.project().clone(),
                        OperationId::new(),
                        source,
                        name,
                    )
                    .map_err(|_| IMPORT_FAILED),
                ImportRequest::Sample { id, version } => application
                    .import_sample_dataset_for_application(
                        &samples,
                        scope.project().clone(),
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
            let project = application
                .query_project_index(scope.project().clone(), language, true)
                .ok()
                .map(|snapshot| DesktopProject::new(scope.project().clone(), snapshot, language));
            Ok((imported, project))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.lifecycle != expected.lifecycle()
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| &project.identity != expected.project())
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
                dialog
                    .update(cx, |dialog, cx| {
                        dialog.finished(imported.map(|_| ()), window, cx)
                    })
                    .log_released();
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
        true
    }
}
