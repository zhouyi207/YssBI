//! Confirm deletion of a captured conversation, then discard its native views after commit.
use super::{AssistantController, AssistantEventKind, principal};
use gpui_kit::{Context, Window};
use yss_application::harness::HarnessSessionError;
use yss_harness_contract::HarnessSessionId;
use yss_harness_core::HarnessError;
use yss_ui::lifecycle::LogReleased;

impl AssistantController {
    pub fn delete_conversation(
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
        let title = if title.is_empty() {
            yss_ui::text::t("panel.assistantNewConversation")
        } else {
            title.as_str().into()
        };
        let owner = cx.entity().downgrade();
        let lifecycle = self.epoch;
        yss_ui::modal_window::confirm(
            yss_ui::text::format(
                "native.workbench.deleteConversationTitle",
                &[("title", title.to_string())],
            ),
            yss_ui::text::t("native.workbench.deleteConversationMessage"),
            yss_ui::text::t("native.workbench.deleteConversation"),
            yss_ui::text::t("common.cancel"),
            window,
            cx,
            move |_, window, cx| {
                owner
                    .update(cx, |view, cx| {
                        if view.epoch == lifecycle && !view.unavailable(cx) {
                            view.commit_conversation_deletion(session.clone(), window, cx);
                        }
                    })
                    .log_released();
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
        self.busy = true;
        self.emit(AssistantEventKind::Error(None), cx);
        let lifecycle = self.epoch;
        let id = session.to_string();
        let services = self.services.clone();
        let job = self.services.tasks.executor().spawn(async move {
            services
                .tasks
                .application()
                .delete_harness_session(&services.host, &principal(), &session)
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await;
            view.update_in(cx, |view, window, cx| {
                if view.epoch != lifecycle {
                    return;
                }
                view.busy = false;
                match result {
                    Ok(Ok(())) => {
                        view.subscriptions.remove(&id);
                        if let Some(panel) = view.conversations.remove(&id) {
                            view.emit(AssistantEventKind::RemoveConversation(panel), cx);
                        }
                    }
                    Ok(Err(HarnessSessionError::Host(HarnessError::ConcurrentTurn))) => {
                        view.emit(
                            AssistantEventKind::Error(Some(
                                yss_ui::text::t("native.workbench.conversationRunning").into(),
                            )),
                            cx,
                        );
                    }
                    Ok(Err(HarnessSessionError::Host(HarnessError::ConcurrentWorkflow))) => {
                        view.emit(
                            AssistantEventKind::Error(Some(
                                yss_ui::text::t("native.workbench.conversationWorkflowActive")
                                    .into(),
                            )),
                            cx,
                        );
                    }
                    _ => {
                        view.emit(
                            AssistantEventKind::Error(Some(
                                yss_ui::text::t("native.workbench.conversationDeleteFailed").into(),
                            )),
                            cx,
                        );
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
