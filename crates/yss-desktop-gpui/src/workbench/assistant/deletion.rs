//! Confirm deletion of a captured conversation, then discard its native views after commit.
use super::{Workbench, principal};
use gpui::{Context, Window};
use yss_application::harness::HarnessSessionError;
use yss_harness_contract::HarnessSessionId;
use yss_harness_core::HarnessError;

impl Workbench {
    pub(in crate::workbench) fn delete_conversation(
        &mut self,
        id: String,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || self.assistant_busy {
            return;
        }
        let Ok(session) = HarnessSessionId::try_new(id) else {
            return;
        };
        let title = if title.is_empty() {
            crate::text::t("panel.assistantNewConversation")
        } else {
            &title
        };
        let owner = cx.entity().downgrade();
        let lifecycle = self.lifecycle;
        crate::modal_window::confirm(
            crate::text::format(
                "native.workbench.deleteConversationTitle",
                &[("title", title.to_string())],
            ),
            crate::text::t("native.workbench.deleteConversationMessage"),
            crate::text::t("native.workbench.deleteConversation"),
            crate::text::t("common.cancel"),
            window,
            cx,
            move |_, window, cx| {
                let _ = owner.update(cx, |view, cx| {
                    if view.lifecycle == lifecycle && !view.is_closing(cx) && !view.assistant_busy {
                        view.commit_conversation_deletion(session.clone(), window, cx);
                    }
                });
                true
            },
        );
    }

    fn commit_conversation_deletion(
        &mut self,
        session: HarnessSessionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.assistant_busy = true;
        self.error = None;
        let lifecycle = self.lifecycle;
        let id = session.to_string();
        let services = self.services.clone();
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .delete_harness_session(&services.application.harness.host, &principal(), &session)
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.assistant_busy = false;
                match result {
                    Ok(Ok(())) => {
                        if let Some(panel) = view.conversations.remove(&id) {
                            view.dock
                                .update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
                        }
                        view.sync_active_conversation(cx);
                    }
                    Ok(Err(HarnessSessionError::Host(HarnessError::ConcurrentTurn))) => {
                        view.error =
                            Some(crate::text::t("native.workbench.conversationRunning").into());
                    }
                    Ok(Err(HarnessSessionError::Host(HarnessError::ConcurrentWorkflow))) => {
                        view.error = Some(
                            crate::text::t("native.workbench.conversationWorkflowActive").into(),
                        );
                    }
                    _ => {
                        view.error = Some(
                            crate::text::t("native.workbench.conversationDeleteFailed").into(),
                        );
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
