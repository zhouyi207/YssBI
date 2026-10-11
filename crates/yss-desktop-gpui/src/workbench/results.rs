//! Current output catalogue and result-panel composition, without owning result payloads.
pub(super) mod window;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, DockPlacement, Panel, PanelEvent},
};
use gpui_kit::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
    div, prelude::*, uniform_list,
};
use std::rc::Rc;
use yss_graph_execution::result::ResultReference;

use super::Workbench;
use crate::{
    appearance,
    canvas::{GraphCanvas, ResultEntry},
    results::{ResultEvent, ResultPanel},
};

pub struct ResultsPanel {
    focus: FocusHandle,
    entries: Rc<[ResultEntry]>,
}
pub enum ResultsEvent {
    Open(ResultReference),
}

impl ResultsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            entries: Rc::default(),
        }
    }
    pub fn set_graph(&mut self, canvas: Option<&GraphCanvas>) {
        self.entries = canvas
            .map(|canvas| canvas.result_entries().clone())
            .unwrap_or_default();
    }
}
impl Render for ResultsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("current-results")
            .track_focus(&self.focus)
            .size_full()
            .bg(cx.theme().background)
            .when(self.entries.is_empty(), |view| {
                view.child(appearance::empty_state(
                    IconName::Inbox,
                    crate::text::t("panel.assistantToolNames.inspect_result"),
                    crate::text::t("native.workbench.resultsHint"),
                    cx,
                ))
            })
            .when(!self.entries.is_empty(), |view| {
                view.child(
                    uniform_list(
                        "result-entries",
                        self.entries.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| {
                                    let entry = &view.entries[index];
                                    let reference = entry.reference;
                                    div().h_8().flex().items_center().px_2().gap_2().child(
                                        Button::new(("open-result", reference.result_id.get()))
                                            .small()
                                            .ghost()
                                            .icon(IconName::Table)
                                            .label(entry.title.clone())
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                if view
                                                    .entries
                                                    .iter()
                                                    .any(|entry| entry.reference == reference)
                                                {
                                                    cx.emit(ResultsEvent::Open(reference));
                                                }
                                            })),
                                    )
                                })
                                .collect()
                        }),
                    )
                    .size_full(),
                )
            })
    }
}
impl EventEmitter<PanelEvent> for ResultsPanel {}
impl EventEmitter<ResultsEvent> for ResultsPanel {}
impl Focusable for ResultsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ResultsPanel {
    fn panel_name(&self) -> &'static str {
        "results"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for ResultsPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::Table).size_3())
            .child(crate::text::t("detail.description.result"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}

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
        let services = self.services.clone();
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
        self.mark_project_resource(None, cx);
        self.details.update(cx, |panel, cx| panel.clear(cx));
        self.problems.update(cx, |panel, cx| panel.clear(cx));
        self.output
            .update(cx, |panel, cx| panel.set_graph(None, cx));
        self.results.update(cx, |panel, cx| {
            panel.set_graph(None);
            cx.notify();
        });
    }
}
