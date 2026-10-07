use super::Workbench;
use gpui::{Context, Window};
use gpui_component::dock::DockPlacement;
use yss_project_identity::ProjectResourceKind;
use yss_ui_contract::{UiIntent, UiIntentReceipt, UiIntentStatus, UiPanel};

impl Workbench {
    pub(super) fn enqueue_intent(
        &mut self,
        receipt: UiIntentReceipt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if receipt.status != UiIntentStatus::Pending
            || self
                .intent_queue
                .iter()
                .any(|queued| queued.id == receipt.id)
        {
            return;
        }
        if self.intent_queue.len() >= 128 {
            self.intent_resync = true;
            return;
        }
        self.intent_queue.push_back(receipt);
        self.apply_next_intent(window, cx);
    }

    pub(super) fn resync_intents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(binding) = &self.ui_binding else {
            return;
        };
        match binding.pending() {
            Ok(receipts) => {
                self.intent_resync = false;
                for receipt in receipts {
                    self.enqueue_intent(receipt, window, cx);
                }
            }
            Err(_error) => tracing::debug!(
                code = "native_ui_intent_recovery_rejected",
                "Native UI intent recovery rejected"
            ),
        }
    }

    fn apply_next_intent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.intent_busy || self.busy || self.closing {
            return;
        }
        let Some(receipt) = self.intent_queue.pop_front() else {
            if self.intent_resync {
                self.resync_intents(window, cx);
            }
            return;
        };
        let claimed = self.ui_binding.as_ref().is_some_and(|binding| {
            binding
                .settle(&receipt.id, UiIntentStatus::Claimed)
                .unwrap_or(false)
        });
        if !claimed {
            self.apply_next_intent(window, cx);
            return;
        }
        self.intent_busy = true;
        match receipt.intent {
            UiIntent::OpenResource { resource, node_id }
                if matches!(
                    resource.kind,
                    ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph
                ) =>
            {
                self.open_graph_intent(resource.id, node_id, receipt.id, window, cx);
            }
            UiIntent::ShowPanel { panel } => {
                let applied = match panel {
                    UiPanel::Project | UiPanel::Nodes => {
                        let key = if panel == UiPanel::Project {
                            "project"
                        } else {
                            "nodes"
                        };
                        if let Some(panel) =
                            self.activities.get(key).and_then(gpui::WeakEntity::upgrade)
                        {
                            self.dock.update(cx, |dock, cx| {
                                dock.add_panel_view(
                                    gpui_component::dock::panel_handle(panel),
                                    DockPlacement::Left,
                                    None,
                                    window,
                                    cx,
                                )
                            });
                            true
                        } else {
                            false
                        }
                    }
                    UiPanel::Details => {
                        self.dock.update(cx, |dock, cx| {
                            dock.add_panel_view(
                                gpui_component::dock::panel_handle(self.details.clone()),
                                DockPlacement::Right,
                                None,
                                window,
                                cx,
                            )
                        });
                        true
                    }
                    UiPanel::Problems => {
                        self.dock.update(cx, |dock, cx| {
                            dock.add_panel_view(
                                gpui_component::dock::panel_handle(self.problems.clone()),
                                DockPlacement::Bottom,
                                None,
                                window,
                                cx,
                            )
                        });
                        true
                    }
                    UiPanel::Logs => {
                        self.dock.update(cx, |dock, cx| {
                            dock.add_panel_view(
                                gpui_component::dock::panel_handle(self.logs.clone()),
                                DockPlacement::Bottom,
                                None,
                                window,
                                cx,
                            )
                        });
                        true
                    }
                    UiPanel::Output => {
                        self.dock.update(cx, |dock, cx| {
                            dock.add_panel_view(
                                gpui_component::dock::panel_handle(self.output.clone()),
                                DockPlacement::Bottom,
                                None,
                                window,
                                cx,
                            )
                        });
                        true
                    }
                    _ => false,
                };
                self.finish_intent(&receipt.id, applied, window, cx);
            }
            UiIntent::OpenResult { source } => {
                let reference = source.validate().ok().and_then(|_| {
                    Some(yss_graph_execution::result::ResultReference {
                        execution_session_id:
                            yss_graph_execution::identity::ExecutionSessionId::new(
                                source.execution_session_id.parse().ok()?,
                            ),
                        result_id: yss_graph_execution::result::ResultId::from_existing(
                            source.result_id.parse().ok()?,
                        ),
                    })
                });
                if let Some(reference) = reference {
                    self.open_result(reference, Some(receipt.id), window, cx);
                } else {
                    self.finish_intent(&receipt.id, false, window, cx);
                }
            }
            _ => self.finish_intent(&receipt.id, false, window, cx),
        }
    }

    pub(super) fn finish_intent(
        &mut self,
        id: &str,
        applied: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(binding) = &self.ui_binding
            && let Err(_error) = binding.settle(
                id,
                if applied {
                    UiIntentStatus::Applied
                } else {
                    UiIntentStatus::Failed
                },
            )
        {
            tracing::debug!(
                code = "native_ui_intent_settlement_rejected",
                "Native UI intent settlement rejected"
            );
        }
        self.intent_busy = false;
        // Defer the next claim so a recovered batch cannot recurse through the whole queue.
        cx.defer_in(window, |view, window, cx| {
            view.apply_next_intent(window, cx)
        });
    }
}
