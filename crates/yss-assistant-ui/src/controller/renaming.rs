//! Rename the captured session; each form owns its draft and asynchronous outcome.
use super::{AssistantController, AssistantEventKind, principal};
use gpui_kit::{Context, Entity, Focusable, Window, px};
use yss_application::harness::HarnessSessionError;
use yss_harness_contract::HarnessSessionId;
use yss_harness_core::HarnessError;
use yss_ui::lifecycle::LogReleased;
use yss_ui::name_form::NameForm;

impl AssistantController {
    pub fn rename_conversation(
        &mut self,
        id: String,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.has_pending_submission(cx) {
            return;
        }
        let Ok(session) = HarnessSessionId::try_new(id) else {
            return;
        };
        let owner = cx.entity().downgrade();
        let lifecycle = self.epoch;
        yss_ui::modal_window::open(
            yss_ui::text::t("native.workbench.renameSession"),
            gpui_kit::size(px(480.), px(270.)),
            window,
            cx,
            move |window, cx| {
                let form = NameForm::new(title, window, cx);
                let focus = form.focus_handle(cx);
                let body = form.clone();
                let cancel = form.clone();
                yss_ui::modal_window::ModalContent::new(move |_, _| body.clone())
                    .focus(focus)
                    .cancel(yss_ui::text::t("common.cancel"))
                    .on_cancel(move |_, _, cx| !cancel.read(cx).busy())
                    .confirm(
                        yss_ui::text::t("contextMenu.dialog.renameSubmit"),
                        move |_, window, cx| {
                            let Some(title) = form.read(cx).value(cx) else {
                                return false;
                            };
                            owner
                                .update(cx, |view, cx| {
                                    if view.epoch != lifecycle {
                                        NameForm::fail(
                                            Some(&form),
                                            "native.assistant.sessionChanged",
                                            cx,
                                        );
                                    } else if !view.unavailable(cx) {
                                        view.commit_conversation_rename(
                                            session.clone(),
                                            title,
                                            form.clone(),
                                            window,
                                            cx,
                                        );
                                    }
                                })
                                .log_released();
                            false
                        },
                    )
            },
        );
    }

    fn commit_conversation_rename(
        &mut self,
        session: HarnessSessionId,
        title: String,
        form: Entity<NameForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        form.update(cx, |form, cx| form.submitting(cx));
        let lifecycle = self.epoch;
        let id = session.to_string();
        let services = self.services.clone();
        let job = self.services.tasks.executor().spawn(async move {
            services
                .tasks
                .application()
                .rename_harness_session(&services.host, &principal(), &session, title)
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await;
            view.update_in(cx, |view, window, cx| {
                if view.epoch != lifecycle {
                    NameForm::fail(Some(&form), "native.assistant.sessionChanged", cx);
                    return;
                }
                view.busy = false;
                match result {
                    Ok(Ok(session)) => {
                        if let Some(panel) = view.conversations.get(&id) {
                            panel.update(cx, |panel, cx| {
                                panel.update_session(session);
                                cx.notify();
                            });
                        }
                        view.emit(AssistantEventKind::MetadataChanged, cx);
                        form.update(cx, |form, cx| form.finish(None, cx));
                        yss_ui::modal_window::close_child(window, cx);
                    }
                    result => {
                        let key = match result {
                            Ok(Err(HarnessSessionError::Host(HarnessError::ConcurrentTurn))) => {
                                "native.assistant.alreadyRunning"
                            }
                            Ok(Err(HarnessSessionError::Host(HarnessError::InvalidMessage))) => {
                                "native.workbench.conversationNameInvalid"
                            }
                            Ok(Err(
                                HarnessSessionError::Changed
                                | HarnessSessionError::SessionCapture(_),
                            )) => "native.assistant.sessionChanged",
                            Ok(Err(HarnessSessionError::Host(
                                HarnessError::SessionNotFound | HarnessError::SessionNotActive,
                            ))) => "native.workbench.conversationUnavailable",
                            _ => "native.workbench.renameSessionFailed",
                        };
                        NameForm::fail(Some(&form), key, cx);
                    }
                }
                view.invalidate_directory(window, cx);
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }
}
