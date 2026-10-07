//! Current output catalogue and result-panel composition, without owning result payloads.
use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
    div, prelude::*, uniform_list,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, DockPlacement, Panel, PanelEvent},
};
use yss_graph_execution::result::{ResultCacheState, ResultReference};

use super::Workbench;
use crate::{
    appearance,
    assets::NativeIcon,
    canvas::GraphCanvas,
    results::{ResultEvent, ResultPanel},
};

struct ResultEntry {
    title: String,
    reference: ResultReference,
    stale: bool,
    waiting: bool,
}

pub struct ResultsPanel {
    focus: FocusHandle,
    entries: Vec<ResultEntry>,
}
pub enum ResultsEvent {
    Open(ResultReference),
}

impl ResultsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            entries: vec![],
        }
    }
    pub fn set_graph(&mut self, canvas: Option<&GraphCanvas>) {
        self.entries = canvas
            .map(|canvas| {
                let graph = &canvas.graph;
                let labels: std::collections::BTreeMap<String, String> = graph
                    .projection
                    .nodes
                    .iter()
                    .flat_map(|node| {
                        node.ports.iter().map(move |port| {
                            (
                                port.address.to_string(),
                                format!(
                                    "{} · {}",
                                    node.display
                                        .user_label
                                        .as_ref()
                                        .unwrap_or(&node.display.title),
                                    port.display.label
                                ),
                            )
                        })
                    })
                    .collect();
                graph
                    .results
                    .outputs
                    .iter()
                    .filter_map(|(output, state)| {
                        let (result_id, stale) = match state {
                            ResultCacheState::Valid { result_id } => (*result_id, false),
                            ResultCacheState::Stale { result_id } => (*result_id, true),
                            ResultCacheState::Missing => return None,
                        };
                        Some(ResultEntry {
                            title: labels
                                .get(output.port().as_str())
                                .cloned()
                                .unwrap_or_else(|| output.port().to_string()),
                            reference: ResultReference {
                                execution_session_id: graph.results.execution_session_id,
                                result_id,
                            },
                            stale,
                            waiting: canvas.result_waiting(output),
                        })
                    })
                    .collect()
            })
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
                    "查看结果",
                    "运行图后，在这里选择要查看的数据",
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
                                    div()
                                        .h_8()
                                        .flex()
                                        .items_center()
                                        .px_2()
                                        .gap_2()
                                        .child(
                                            Button::new(("open-result", index))
                                                .small()
                                                .ghost()
                                                .icon(NativeIcon::Table)
                                                .label(entry.title.clone())
                                                .disabled(entry.waiting)
                                                .on_click(cx.listener(move |_, _, _, cx| {
                                                    cx.emit(ResultsEvent::Open(reference))
                                                })),
                                        )
                                        .when(entry.stale, |view| {
                                            view.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("上次结果 · 已过期"),
                                            )
                                        })
                                        .when(entry.waiting, |view| view.child("正在同步结果…"))
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
            .child(Icon::new(NativeIcon::Table).size_3())
            .child("结果")
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
            .and_then(gpui::WeakEntity::upgrade)
            && panel.read(cx).available()
        {
            self.dock.update(cx, |dock, cx| {
                dock.add_panel_view(
                    gpui_component::dock::panel_handle(panel.clone()),
                    DockPlacement::Center,
                    None,
                    window,
                    cx,
                )
            });
            if let Some(id) = intent {
                if panel.read(cx).loaded() {
                    self.finish_intent(&id, true, window, cx);
                } else {
                    self.observe_result_intent(&panel, id, window, cx);
                    panel.update(cx, |panel, cx| panel.load(window, cx));
                }
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
        self.subscriptions.push(
            cx.subscribe_in(&panel, window, move |view, _, event, _, cx| {
                if view.lifecycle == lifecycle && matches!(event, ResultEvent::Activated) {
                    view.clear_graph_context(cx);
                }
            }),
        );
        self.result_panels.insert(key, panel.downgrade());
        self.dock.update(cx, |dock, cx| {
            dock.add_panel_view(
                gpui_component::dock::panel_handle(panel.clone()),
                DockPlacement::Center,
                None,
                window,
                cx,
            )
        });
        if let Some(id) = intent {
            self.observe_result_intent(&panel, id, window, cx);
        }
        panel.update(cx, |panel, cx| panel.load(window, cx));
    }

    fn observe_result_intent(
        &mut self,
        panel: &gpui::Entity<ResultPanel>,
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
                    ResultEvent::Activated => return,
                };
                if view.lifecycle == lifecycle
                    && let Some(id) = pending.take()
                {
                    view.finish_intent(&id, applied, window, cx);
                }
            },
        ));
    }

    fn clear_graph_context(&mut self, cx: &mut Context<Self>) {
        for panel in self
            .activities
            .values()
            .filter_map(gpui::WeakEntity::upgrade)
        {
            panel.update(cx, |panel, cx| panel.set_active_graph(None, cx));
        }
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
