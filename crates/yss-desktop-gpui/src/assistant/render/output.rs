//! Shared timeline rendering; folding never removes content from the projection.
use super::super::{
    ConversationPanel,
    projection::{Compaction, Content, Output, Part, Task, Usage},
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, IntoElement, SharedString, div, prelude::*, px};
use yss_harness_contract::ModelCallPurpose;

impl ConversationPanel {
    pub(in crate::assistant) fn render_output(
        &self,
        output: &Output,
        tasks: &[Task],
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = div().min_w_0().flex().flex_col().gap_2();
        for parts in output.parts.chunk_by(|left, right| {
            matches!(left.content, Content::Tool(_)) && matches!(right.content, Content::Tool(_))
        }) {
            let part = &parts[0];
            if matches!(part.content, Content::Tool(_)) {
                body = body.child(self.tool_group(
                    format!("tools-{}", part.id),
                    parts.iter().filter_map(|part| match &part.content {
                        Content::Tool(tool) => Some(tool),
                        _ => None,
                    }),
                    running,
                    cx,
                ));
            } else {
                body = body.child(self.render_part(
                    part,
                    tasks,
                    running,
                    output.parts.last().is_some_and(|last| last.id == part.id),
                    cx,
                ));
            }
        }
        body.into_any_element()
    }

    fn render_part(
        &self,
        part: &Part,
        tasks: &[Task],
        running: bool,
        last: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = part.id;
        match &part.content {
            Content::Text(text) => {
                let mut body = div().min_w_0().flex().flex_col().gap_2();
                if text.valid_bytes > 0 {
                    body = body.child(markdown(
                        format!("text-{id}"),
                        text.text[..text.valid_bytes].to_owned(),
                        running && self.timing_connected(),
                        cx,
                    ));
                }
                if text.valid_bytes < text.text.len() {
                    let key = format!("retracted-{id}");
                    let open = self.is_expanded(&key, false);
                    body = body.child(self.disclosure(
                        key,
                        crate::text::t("native.assistant.previousAttemptOutput").into(),
                        open,
                        cx,
                    ));
                    if open {
                        body = body.child(markdown(
                            format!("retracted-text-{id}"),
                            text.text[text.valid_bytes..].to_owned(),
                            false,
                            cx,
                        ));
                    }
                }
                body.into_any_element()
            }
            Content::Reasoning(text) => {
                let key = format!("reasoning-{id}");
                let open = self.is_expanded(&key, running);
                let label = if running && self.timing_connected() && last {
                    crate::text::t("native.assistant.thinking")
                } else {
                    crate::text::t("panel.assistantThinking")
                };
                let mut body = div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(self.disclosure(
                        key,
                        crate::text::format(
                            "native.assistant.characterCount",
                            &[
                                ("label", label.to_string()),
                                ("value0", text.chars().count().to_string()),
                            ],
                        ),
                        open,
                        cx,
                    ));
                if open {
                    body = body.child(
                        div()
                            .id(SharedString::from(format!("reasoning-scroll-{id}")))
                            .min_w_0()
                            .max_h(px(288.))
                            .overflow_y_scroll()
                            .pl_3()
                            .border_l_1()
                            .border_color(cx.theme().border)
                            .text_color(cx.theme().muted_foreground)
                            .child(super::plain::PlainText::new(
                                SharedString::from(format!("reasoning-text-{id}")),
                                text.clone(),
                                "panel.assistantThinking",
                            )),
                    );
                }
                body.into_any_element()
            }
            Content::Tool(_) => unreachable!("tool calls are grouped before rendering"),
            Content::Task(index) => self.task_card(id, &tasks[*index], running, cx),
            Content::Outcome(index) => self.outcome_card(id, &tasks[*index], cx),
            Content::Plan(plan) => self.plan_card(format!("plan-{id}"), plan, cx),
            Content::Citation(citation) => super::super::sources::card(
                format!("citation-{id}"),
                citation,
                cx.entity().downgrade(),
            )
            .into_any_element(),
            Content::Status { event, error } => div()
                .text_xs()
                .text_color(if *error {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                })
                .child(status_text(event))
                .into_any_element(),
            Content::Failure(code) => self.failure_card(id, code, cx),
            Content::Usage(usage) => self.usage_card(id, usage, cx),
            Content::Compaction(compaction) => self.compaction_card(id, compaction, running, cx),
        }
    }

    pub(super) fn is_expanded(&self, key: &str, default: bool) -> bool {
        self.expanded.get(key).copied().unwrap_or(default)
    }

    pub(super) fn disclosure(
        &self,
        key: String,
        label: String,
        open: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(SharedString::from(key.clone()))
            .self_start()
            .max_w_full()
            .small()
            .ghost()
            .icon(if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .accessibility_label(label.clone())
            .label(label)
            .tooltip(if open {
                crate::text::t("native.assistant.collapse")
            } else {
                crate::text::t("native.assistant.expandFullOutput")
            })
            .on_click(cx.listener(move |view, _, _, cx| {
                view.expanded.insert(key.clone(), !open);
                cx.notify();
            }))
    }

    fn usage_card(&self, id: u64, usage: &Usage, cx: &mut Context<Self>) -> AnyElement {
        let key = format!("usage-{id}");
        let open = self.is_expanded(&key, false);
        let count = |value: Option<u64>| {
            value
                .map(|n| n.to_string())
                .unwrap_or_else(|| crate::text::t("native.assistant.notReturned").into())
        };
        let title = match usage.purpose {
            ModelCallPurpose::Response => crate::text::t("native.assistant.modelCallCompleted"),
            ModelCallPurpose::Compaction => crate::text::t("native.assistant.summaryCallCompleted"),
        };
        let tokens = &usage.tokens;
        let mut body = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .text_color(cx.theme().muted_foreground)
            .child(self.disclosure(
                key,
                crate::text::format(
                    "native.assistant.tokenSummary",
                    &[
                        ("title", title.to_string()),
                        ("value0", count(tokens.input_tokens).to_string()),
                        ("value1", count(tokens.output_tokens).to_string()),
                    ],
                ),
                open,
                cx,
            ));
        if open {
            body = body.child(div().text_xs().child(crate::text::format(
                "native.assistant.tokenDetails",
                &[
                    ("value0", count(tokens.cached_input_tokens).to_string()),
                    (
                        "value1",
                        count(tokens.cache_creation_input_tokens).to_string(),
                    ),
                    ("value2", count(tokens.reasoning_tokens).to_string()),
                    (
                        "value3",
                        count(usage.context_window.map(u64::from)).to_string(),
                    ),
                ],
            )));
        }
        body.into_any_element()
    }

    fn compaction_card(
        &self,
        id: u64,
        compaction: &Compaction,
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = running && !compaction.finished && !compaction.interrupted;
        let key = format!("compaction-{id}");
        let open = self.is_expanded(&key, active);
        let state = if compaction.finished {
            crate::text::t("common.completed")
        } else if active && !self.timing_connected() {
            crate::text::t("panel.assistantToolUnknown")
        } else if active {
            crate::text::t("native.assistant.inProgress")
        } else {
            crate::text::t("panel.assistantToolInterrupted")
        };
        let mut body = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.disclosure(
                key,
                crate::text::format(
                    "native.assistant.compactionStatus",
                    &[
                        ("state", state.to_string()),
                        ("value0", compaction.completed_bytes.to_string()),
                        ("value1", compaction.total_bytes.to_string()),
                    ],
                ),
                open,
                cx,
            ));
        if open {
            body = body.child(
                div()
                    .min_w_0()
                    .pl_3()
                    .border_l_1()
                    .border_color(cx.theme().border)
                    .child(self.render_output(&compaction.output, &[], active, cx)),
            );
        }
        body.into_any_element()
    }
}

pub(super) fn markdown(
    id: String,
    text: String,
    streaming: bool,
    cx: &mut Context<ConversationPanel>,
) -> AnyElement {
    super::super::markdown::view(
        SharedString::from(id),
        text,
        streaming,
        cx.entity().downgrade(),
    )
    .into_any_element()
}

/// Localize status events at presentation time, preserving all authored/model text verbatim.
pub(super) fn status_text(event: &yss_harness_contract::AssistantEventKind) -> String {
    use crate::text::{format, t};
    use yss_harness_contract::{AgentRuntimePhase, AssistantEventKind as Event};
    let (key, run_id, step_id, retriable) = match event {
        Event::RuntimeStatus { phase, attempt } => {
            let label = t(match phase {
                AgentRuntimePhase::CheckingDelivery => "native.assistant.inspectDelivery",
                AgentRuntimePhase::Reconnecting => "panel.assistantReloadConversations",
                AgentRuntimePhase::Compacting => "native.assistant.compactContext",
            });
            return if *attempt == 0 {
                label.to_owned()
            } else {
                format(
                    "native.assistant.attempt",
                    &[
                        ("label", label.to_owned()),
                        ("attempt", attempt.to_string()),
                    ],
                )
            };
        }
        Event::ContextCompactionProgress {
            completed_bytes,
            total_bytes,
        } => {
            return format(
                "native.assistant.compactionProgress",
                &[
                    ("completed_bytes", completed_bytes.to_string()),
                    ("total_bytes", total_bytes.to_string()),
                ],
            );
        }
        Event::WorkflowPlanned { run_id } => {
            ("native.assistant.workflowPlanned", run_id, None, false)
        }
        Event::WorkflowStarted { run_id } => {
            ("native.assistant.workflowStarted", run_id, None, false)
        }
        Event::WorkflowStepStarted { run_id, step_id } => (
            "native.assistant.workflowStepStarted",
            run_id,
            Some(step_id),
            false,
        ),
        Event::WorkflowStepCompleted { run_id, step_id } => (
            "native.assistant.workflowStepCompleted",
            run_id,
            Some(step_id),
            false,
        ),
        Event::WorkflowStepFailed {
            run_id,
            step_id,
            retriable,
        } => (
            "native.assistant.workflowStepFailed",
            run_id,
            Some(step_id),
            *retriable,
        ),
        Event::WorkflowCompleted { run_id } => {
            ("native.assistant.workflowCompleted", run_id, None, false)
        }
        Event::WorkflowPaused { run_id } => {
            ("native.assistant.workflowPaused", run_id, None, false)
        }
        Event::WorkflowResumed { run_id } => {
            ("native.assistant.workflowResumed", run_id, None, false)
        }
        Event::WorkflowCancelled { run_id } => {
            ("native.assistant.workflowCancelled", run_id, None, false)
        }
        _ => unreachable!("only status events are stored as presentation notices"),
    };
    format(
        key,
        &[
            ("run_id", run_id.to_string()),
            (
                "step_id",
                step_id.map(ToString::to_string).unwrap_or_default(),
            ),
            (
                "value0",
                if retriable {
                    t("native.assistant.retryable")
                } else {
                    ""
                }
                .to_owned(),
            ),
        ],
    )
}
