use super::Workbench;
use gpui::{Context, Window};
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
            Err(yss_application::presentation::UiError::Session) => self.rebind_session(window, cx),
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
            UiIntent::OpenResource {
                resource,
                node_id: None,
            } if resource.kind == ProjectResourceKind::Doc => {
                self.open_document(resource.id, Some(receipt.id), window, cx);
            }
            UiIntent::OpenResource {
                resource,
                node_id: None,
            } if resource.kind == ProjectResourceKind::Mind => {
                self.open_mind(resource.id, Some(receipt.id), window, cx);
            }
            UiIntent::OpenResource {
                resource,
                node_id: None,
            } if resource.kind == ProjectResourceKind::Database => {
                self.open_database(resource.id, Some(receipt.id), window, cx);
            }
            UiIntent::OpenResource {
                resource,
                node_id: None,
            } if resource.kind == ProjectResourceKind::Chart => {
                self.open_chart(resource.id, Some(receipt.id), window, cx);
            }
            UiIntent::ShowPanel {
                panel: UiPanel::Assistant,
            } => {
                self.assistant_intent = Some(receipt.id);
                self.show_assistant(window, cx);
            }
            UiIntent::ShowPanel { panel } => {
                let target = match panel {
                    UiPanel::Project => Some(super::menus::WorkbenchPanel::Project),
                    UiPanel::Nodes => Some(super::menus::WorkbenchPanel::Nodes),
                    UiPanel::Details => Some(super::menus::WorkbenchPanel::Details),
                    UiPanel::Problems => Some(super::menus::WorkbenchPanel::Problems),
                    UiPanel::Output => Some(super::menus::WorkbenchPanel::Output),
                    UiPanel::Logs => Some(super::menus::WorkbenchPanel::Logs),
                    UiPanel::Assistant => Some(super::menus::WorkbenchPanel::Assistant),
                    _ => None,
                };
                let applied = target.is_some_and(|panel| self.show_panel(panel, window, cx));
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
                    self.open_result(reference, Some(receipt.id), None, window, cx);
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
