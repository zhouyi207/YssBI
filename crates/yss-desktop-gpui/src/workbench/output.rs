//! Output presents Execution failures for the active graph, independently of Logs and Problems.
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, WeakEntity, Window,
    div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
};
use yss_application::graph::run::RunApplicationEventKind;
use yss_graph_execution::{
    error::{RunFailureCode, RunPhase},
    plan::PlanSourceIdentity,
};

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
        let failure = event.as_ref().and_then(|event| match event.kind() {
            RunApplicationEventKind::RunErrored { failure } => Some(failure),
            _ => None,
        });
        div()
            .id("output")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .px_3()
            .py_2()
            .bg(cx.theme().background)
            .text_sm()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        graph
                            .as_ref()
                            .map(|graph| graph.read(cx).run_status())
                            .unwrap_or("请打开图以查看运行状态"),
                    )
                    .child(
                        Button::new("clear-run-failure")
                            .small()
                            .ghost()
                            .icon(IconName::Delete)
                            .label("清除运行错误")
                            .on_click(cx.listener(|view, _, _, cx| {
                                if let Some(graph) =
                                    view.graph.as_ref().and_then(WeakEntity::upgrade)
                                {
                                    graph.update(cx, |graph, cx| graph.clear_run_failure(cx));
                                }
                            })),
                    ),
            )
            .when_some(failure, |view, failure| {
                let source = failure.source.clone();
                view.child(div().py_2().text_color(cx.theme().danger).child(
                    crate::text::translate(&format!(
                        "runFailure.causes.{}",
                        failure_key(failure.code)
                    )),
                ))
                .child(div().child(crate::text::translate(&format!(
                    "runFailure.phases.{}",
                    phase_key(failure.phase)
                ))))
                .children(failure.groups.iter().map(|group| {
                    div().py_1().child(match group.ordinal {
                        Some(ordinal) => format!("第 {ordinal} 组 · {}", group.function.as_str()),
                        None => format!("空输入结构探测 · {}", group.function.as_str()),
                    })
                }))
                .when_some(source, |view, source| {
                    view.child(
                        Button::new("locate-run-failure")
                            .small()
                            .ghost()
                            .label("定位失败节点")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(OutputEvent::Locate(source.clone()))
                            })),
                    )
                })
            })
            .when(failure.is_none(), |view| {
                view.child(
                    div()
                        .py_2()
                        .text_color(cx.theme().muted_foreground)
                        .child("当前图暂无运行错误"),
                )
            })
    }
}

fn failure_key(code: RunFailureCode) -> &'static str {
    match code {
        RunFailureCode::KernelFailed => "kernelFailed",
        RunFailureCode::KernelNotFound => "kernelNotFound",
        RunFailureCode::InvalidNumericInput => "invalidNumericInput",
        RunFailureCode::ShapeMismatch => "shapeMismatch",
        RunFailureCode::GroupSchemaMismatch => "groupSchemaMismatch",
        RunFailureCode::GroupKeyCollision => "groupKeyCollision",
        RunFailureCode::InvalidParameter => "invalidParameter",
        RunFailureCode::UnalignedSeries => "unalignedSeries",
        RunFailureCode::BudgetExceeded => "budgetExceeded",
        RunFailureCode::InputLayoutMismatch => "inputLayoutMismatch",
        RunFailureCode::OutputContractMismatch => "outputContractMismatch",
        RunFailureCode::ScientificFailure => "scientificFailure",
        RunFailureCode::DivisionByZero => "divisionByZero",
        RunFailureCode::NonFiniteResult => "nonFiniteResult",
        RunFailureCode::DeadlineExceeded => "deadlineExceeded",
        RunFailureCode::ResourceUnavailable => "resourceUnavailable",
        RunFailureCode::InputResultUnavailable => "inputResultUnavailable",
        RunFailureCode::FinalizationFailed => "finalizationFailed",
    }
}

fn phase_key(phase: RunPhase) -> &'static str {
    match phase {
        RunPhase::Admission => "admission",
        RunPhase::PlanValidation => "planValidation",
        RunPhase::ResourcePreparation => "resourcePreparation",
        RunPhase::Execution => "execution",
        RunPhase::Finalization => "finalization",
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
            .child("运行")
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
