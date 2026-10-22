//! Current output catalogue and result-panel composition, without owning result payloads.
pub(super) mod window;

use super::Workbench;
use gpui_kit::component::WindowExt;
use gpui_kit::component::dock::DockPlacement;
use gpui_kit::{AppContext, Context, Window};
use yss_graph_execution::result::ResultReference;
use yss_results_ui::results::{ResultEvent, ResultPanel};
use yss_ui::lifecycle::LogReleased;

impl Workbench {
    pub(super) fn open_result(
        &mut self,
        reference: ResultReference,
        intent: Option<String>,
        handoff: Option<std::sync::Arc<yss_application::graph::results::ResultLease>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = (
            reference.execution_session_id.as_uuid(),
            reference.result_id.get(),
        );
        if let Some(panel) = self
            .result_panels
            .get(&key)
            .and_then(gpui_kit::WeakEntity::upgrade)
            && panel.read(cx).available()
        {
            self.present_panel(
                gpui_kit::component::dock::panel_handle(panel.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
            if let Some(id) = intent {
                if panel.read(cx).loaded() {
                    self.finish_intent(&id, true, window, cx);
                } else {
                    self.observe_result_intent(&panel, id, window, cx);
                    panel.update(cx, |panel, cx| panel.load_retained(handoff, window, cx));
                }
            } else if handoff.is_some() && !panel.read(cx).loaded() {
                panel.update(cx, |panel, cx| panel.load_retained(handoff, window, cx));
            }
            return;
        }
        self.result_panels.retain(|_, panel| {
            panel
                .upgrade()
                .is_some_and(|panel| panel.read(cx).available())
        });
        let services = self.services.tasks.clone();
        let panel = cx.new(|cx| ResultPanel::new(services, reference, cx));
        let lifecycle = self.lifecycle;
        self.subscriptions.push(cx.subscribe_in(
            &panel,
            window,
            move |view, panel, event, window, cx| {
                if matches!(event, ResultEvent::OpenWindow) {
                    view.open_result_window(panel, window, cx);
                    return;
                }
                if view.lifecycle != lifecycle {
                    return;
                }
                match event {
                    ResultEvent::Activated => view.clear_graph_context(cx),
                    ResultEvent::Replaced { previous, current } => {
                        let previous = (
                            previous.execution_session_id.as_uuid(),
                            previous.result_id.get(),
                        );
                        if view
                            .result_panels
                            .get(&previous)
                            .and_then(gpui_kit::WeakEntity::upgrade)
                            .is_some_and(|held| held == *panel)
                        {
                            view.result_panels.remove(&previous);
                        }
                        view.result_panels.insert(
                            (
                                current.execution_session_id.as_uuid(),
                                current.result_id.get(),
                            ),
                            panel.downgrade(),
                        );
                    }
                    _ => {}
                }
            },
        ));
        self.result_panels.insert(key, panel.downgrade());
        self.present_panel(
            gpui_kit::component::dock::panel_handle(panel.clone()),
            DockPlacement::Center,
            window,
            cx,
        );
        if let Some(id) = intent {
            self.observe_result_intent(&panel, id, window, cx);
        }
        panel.update(cx, |panel, cx| panel.load_retained(handoff, window, cx));
    }

    /// Computes an uncomputed pin of any project graph, then opens its result.
    pub(super) fn compute_result(
        &mut self,
        graph: yss_graph_document::GraphResourcePath,
        output: yss_graph_execution::plan::PlanOutputRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(overview) = &self.overview else {
            return;
        };
        let task = overview.read(cx).compute(graph, output);
        let lifecycle = self.lifecycle;
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                match result {
                    Ok(Some(reference)) => view.open_result(reference, None, None, window, cx),
                    // A failed or cancelled run publishes nothing; Output shows why.
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            code = "native_compute_rejected",
                            "Result computation was rejected"
                        );
                        window.push_notification(
                            gpui_kit::component::notification::Notification::error(
                                yss_graph_ui::feedback::run_rejection(&error),
                            ),
                            cx,
                        );
                    }
                }
            })
            .log_released();
        })
        .detach();
    }

    fn observe_result_intent(
        &mut self,
        panel: &gpui_kit::Entity<ResultPanel>,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let lifecycle = self.lifecycle;
        let mut pending = Some(id);
        self.subscriptions.push(cx.subscribe_in(
            panel,
            window,
            move |view, _, event, window, cx| {
                let applied = match event {
                    ResultEvent::Loaded(applied) => *applied,
                    ResultEvent::Closed => false,
                    ResultEvent::Activated
                    | ResultEvent::Replaced { .. }
                    | ResultEvent::OpenWindow => return,
                };
                if view.lifecycle == lifecycle
                    && let Some(id) = pending.take()
                {
                    view.finish_intent(&id, applied, window, cx);
                }
            },
        ));
    }

    pub(super) fn clear_graph_context(&mut self, cx: &mut Context<Self>) {
        self.active_editor = None;
        cx.notify();
        self.mark_project_resource(None, cx);
        self.details.update(cx, |panel, cx| panel.clear(cx));
    }
}
