//! Output presents Execution failures for the active graph, independently of Logs and Problems.
mod failure;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, WeakEntity, Window,
    div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
};
use gpui_kit_assets::IconName;
use yss_application::graph::run::RunApplicationEventKind;
use yss_graph_execution::plan::PlanSourceIdentity;

use crate::canvas::{CanvasEvent, GraphCanvas};

pub enum OutputEvent {
    Locate(PlanSourceIdentity),
}

pub struct OutputPanel {
    focus: FocusHandle,
    graph: Option<WeakEntity<GraphCanvas>>,
    observation: Option<gpui::Subscription>,
}

impl OutputPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            graph: None,
            observation: None,
        }
    }

    pub fn set_graph(&mut self, graph: Option<WeakEntity<GraphCanvas>>, cx: &mut Context<Self>) {
        if self
            .graph
            .as_ref()
            .and_then(WeakEntity::upgrade)
            .map(|entity| entity.entity_id())
            == graph
                .as_ref()
                .and_then(WeakEntity::upgrade)
                .map(|entity| entity.entity_id())
        {
            return;
        }
        self.observation = graph.as_ref().and_then(WeakEntity::upgrade).map(|graph| {
            cx.subscribe(&graph, |_, _, event, cx| {
                if matches!(
                    event,
                    CanvasEvent::Projection { .. } | CanvasEvent::Execution
                ) {
                    cx.notify();
                }
            })
        });
        self.graph = graph;
        cx.notify();
    }
}

impl Render for OutputPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let graph = self.graph.as_ref().and_then(WeakEntity::upgrade);
        let event = graph
            .as_ref()
            .and_then(|graph| graph.read(cx).run_failure());
        let failure = event.and_then(|event| match event.kind() {
            RunApplicationEventKind::RunErrored { failure } => Some((event.identity(), failure)),
            _ => None,
        });
        let clear_run = graph
            .as_ref()
            .and_then(|graph| graph.read(cx).clearable_run())
            .cloned();
        let body = match failure {
            Some((run, failure)) => failure::render(run, failure, cx),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::t(if graph.is_some() {
                    "panel.outputEmpty"
                } else {
                    "panel.outputNoGraph"
                }))
                .into_any_element(),
        };
        div()
            .id("output")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_xs()
            .child(
                div()
                    .h_8()
                    .px_2()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().child(crate::text::t("panel.output")))
                    .when_some(graph.as_ref(), |view, graph| {
                        view.child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(graph.read(cx).run_status()),
                        )
                    })
                    .child(
                        Button::new("clear-output-run-notice")
                            .small()
                            .ghost()
                            .icon(IconName::Trash)
                            .tooltip(crate::text::t(if clear_run.is_some() {
                                "canvas.clearExecutionArtifacts"
                            } else {
                                "canvas.clearExecutionArtifactsDisabled"
                            }))
                            .disabled(clear_run.is_none())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if let Some(run) = &clear_run
                                    && let Some(graph) =
                                        view.graph.as_ref().and_then(WeakEntity::upgrade)
                                {
                                    graph.update(cx, |graph, cx| graph.clear_run_notice(run, cx));
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .id("output-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .child(body),
            )
    }
}

impl EventEmitter<PanelEvent> for OutputPanel {}
impl EventEmitter<OutputEvent> for OutputPanel {}
impl Focusable for OutputPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for OutputPanel {
    fn panel_name(&self) -> &'static str {
        "output"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for OutputPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::SquareTerminal).size_3())
            .child(crate::text::t("panel.output"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
