//! Output presents Execution failures for the active graph, independently of Logs and Problems.
mod failure;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Icon,
    dock::{BasePanel, Panel, PanelEvent},
};
use gpui_kit::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, WeakEntity, Window,
    div, prelude::*,
};
use yss_application::graph::run::RunApplicationEventKind;
use yss_graph_execution::plan::PlanSourceIdentity;
use yss_ui::{
    appearance,
    tool_panel::{self, ToolPanel},
};

use crate::canvas::{CanvasEvent, GraphCanvas};

pub enum OutputEvent {
    Locate(PlanSourceIdentity),
}

pub struct OutputPanel {
    focus: FocusHandle,
    graph: Option<WeakEntity<GraphCanvas>>,
    observation: Option<gpui_kit::Subscription>,
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
            Some((run, failure)) => div()
                .id("output-body")
                .size_full()
                .overflow_y_scroll()
                .overflow_x_scroll()
                .child(failure::render(run, failure, cx))
                .into_any_element(),
            None => appearance::empty_state(
                IconName::SquareTerminal,
                crate::text::t("panel.output"),
                crate::text::t(if graph.is_some() {
                    "panel.outputEmpty"
                } else {
                    "panel.outputNoGraph"
                }),
                cx,
            ),
        };
        div()
            .id("output")
            .track_focus(&self.focus)
            .size_full()
            .child(
                ToolPanel::new(crate::text::t("panel.output"), body).tool(
                    tool_panel::button("clear-output-run-notice")
                        .icon(IconName::Trash)
                        .accessibility_label(crate::text::t("canvas.clearExecutionArtifacts"))
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
