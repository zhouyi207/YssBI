//! Rename the captured session; each form owns its draft and asynchronous outcome.
use super::{Workbench, principal};
use crate::workbench::name_form::NameForm;
use gpui::{Context, Entity, Focusable, ParentElement, Window, px};
use gpui_component::WindowExt;
use yss_application::harness::HarnessSessionError;
use yss_harness_contract::HarnessSessionId;
use yss_harness_core::HarnessError;

impl Workbench {
    pub(in crate::workbench) fn rename_conversation(
        &mut self,
        id: String,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let Ok(session) = HarnessSessionId::try_new(id) else {
            return;
        };
        let owner = cx.entity().downgrade();
        let lifecycle = self.lifecycle;
        let form = NameForm::new(title, window, cx);
        let focus = form.focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let session = session.clone();
            let form = form.clone();
            let cancel = form.clone();
            dialog
                .title(crate::text::translate("native.workbench.renameSession"))
                .width(px(440.))
                .close_button(false)
                .overlay_closable(false)
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text(crate::text::translate("contextMenu.dialog.renameSubmit"))
                        .show_cancel(true)
                        .cancel_text(crate::text::translate("common.cancel")),
                )
                .child(form.clone())
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy())
                .on_ok(move |_, window, cx| {
                    let Some(title) = form.read(cx).value(cx) else {
                        return false;
                    };
                    let _ = owner.update(cx, |view, cx| {
                        if view.lifecycle != lifecycle {
                            NameForm::fail(Some(&form), "native.assistant.sessionChanged", cx);
                        } else if !view.is_closing(cx) {
                            view.commit_conversation_rename(
                                session.clone(),
                                title,
                                form.clone(),
                                window,
                                cx,
                            );
                        }
                    });
                    false
                })
        });
        window.focus(&focus, cx);
    }

    fn commit_conversation_rename(
        &mut self,
        session: HarnessSessionId,
        title: String,
        form: Entity<NameForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.assistant_busy = true;
        form.update(cx, |form, cx| form.submitting(cx));
        let lifecycle = self.lifecycle;
        let id = session.to_string();
        let services = self.services.clone();
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .rename_harness_session(
                    &services.application.harness.host,
                    &principal(),
                    &session,
                    title,
                )
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    NameForm::fail(Some(&form), "native.assistant.sessionChanged", cx);
                    return;
                }
                view.assistant_busy = false;
                match result {
                    Ok(Ok(session)) => {
                        if let Some(panel) = view.conversations.get(&id) {
                            panel.update(cx, |panel, cx| {
                                panel.update_session(session);
                                cx.notify();
                            });
                        }
                        form.update(cx, |form, cx| form.finish(None, cx));
                        window.close_dialog(cx);
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
                view.invalidate_assistant_directory(window, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
