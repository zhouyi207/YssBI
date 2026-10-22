//! Failure presentation borrows the current execution fact; callbacks retain only its identity.
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, FontWeight, IntoElement, div, prelude::*};
use yss_application::graph::run::RunIdentity;
use yss_graph_execution::error::{RunFailure, RunPhase};
use yss_graph_ui::feedback;

use super::OutputPanel;
use crate::text;

pub(super) fn render(
    run: &RunIdentity,
    failure: &RunFailure,
    node_title: Option<String>,
    cx: &Context<OutputPanel>,
) -> AnyElement {
    let code = feedback::run_failure_key(failure.code);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .border_b_1()
        .border_color(cx.theme().danger.opacity(0.3))
        .bg(cx.theme().danger.opacity(0.05))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().danger)
                .child(text::t("runFailure.title")),
        )
        .child(feedback::run_failure(failure.code))
        .children(failure.groups.iter().map(|group| {
            div()
                .text_color(cx.theme().muted_foreground)
                .child(match group.ordinal {
                    Some(ordinal) => text::format(
                        "runFailure.group",
                        &[
                            ("ordinal", ordinal.to_string()),
                            ("function", group.function.as_str().to_owned()),
                        ],
                    ),
                    None => text::format(
                        "runFailure.emptyGroupProbe",
                        &[("function", group.function.as_str().to_owned())],
                    ),
                })
        }))
        .when_some(
            failure
                .source
                .as_ref()
                .filter(|source| source.node().is_some()),
            |view, source| {
                let run = run.run_id();
                let source = source.clone();
                let node = source
                    .node()
                    .map(|node| node.as_str().to_owned())
                    .unwrap_or_default();
                view.child(
                    Button::new(("locate-run-failure", run.get()))
                        .small()
                        .link()
                        .self_start()
                        .justify_start()
                        .label(text::format(
                            "runFailure.node",
                            &[("name", node_title.unwrap_or_else(|| node.clone()))],
                        ))
                        .tooltip(format!("{} · {node}", source.graph().as_str()))
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.locate(run, source.clone(), cx)),
                        ),
                )
            },
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x_3()
                .gap_y_1()
                .text_color(cx.theme().muted_foreground)
                .child(text::translate(&format!(
                    "runFailure.phases.{}",
                    phase_key(failure.phase)
                )))
                .child(text::format(
                    "runFailure.run",
                    &[("id", run.run_id().get().to_string())],
                ))
                .child(text::format(
                    "runFailure.code",
                    &[("code", code.to_owned())],
                )),
        )
        .into_any_element()
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
