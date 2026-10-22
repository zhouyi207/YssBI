//! Shared timeline rendering; folding never removes content from the projection.
use super::super::{
    ConversationPanel,
    projection::{Compaction, Content, Output, Part, Task, Usage},
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    marker::{Marker, MarkerContent, MarkerIcon},
    progress::Progress,
};
use gpui_kit::{AnyElement, Context, IntoElement, Role, SharedString, div, prelude::*, rems};
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
                        yss_ui::text::t("native.assistant.previousAttemptOutput").into(),
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
                let active = running && self.timing_connected() && last;
                let open = self.is_expanded(&key, false);
                let label = yss_ui::text::t(if active {
                    "native.assistant.thinking"
                } else {
                    "panel.assistantThinking"
                });
                let mut body = div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Marker::new()
                                    .id(SharedString::from(format!("reasoning-status-{id}")))
                                    .role(Role::Status)
                                    .w_auto()
                                    .loading(active)
                                    .when(!active, |marker| {
                                        marker.icon(
                                            MarkerIcon::new()
                                                .child(Icon::new(IconName::Lightbulb).small()),
                                        )
                                    }),
                            )
                            .child(self.disclosure(key, label.into_owned(), open, cx)),
                    );
                if open {
                    body = body.child(
                        div()
                            .id(SharedString::from(format!("reasoning-scroll-{id}")))
                            .min_w_0()
                            .max_h(rems(18.))
                            .overflow_y_scroll()
                            .text_color(cx.theme().muted_foreground)
                            .child(markdown(
                                format!("reasoning-text-{id}"),
                                text.clone(),
                                active,
                                cx,
                            )),
                    );
                } else if let Some(preview) = reasoning_preview(text, active) {
                    body = body.child(
                        div()
                            .min_w_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(preview),
                    );
                }
                body.into_any_element()
            }
            Content::Tool(_) => unreachable!("tool calls are grouped before rendering"),
            Content::Task(index) => self.task_card(id, &tasks[*index], running, cx),
            Content::Outcome(index) => self.outcome_card(id, &tasks[*index], cx),
            Content::Plan(plan) => self.plan_card(format!("plan-{id}"), plan, cx),
            Content::Approval(approval) => self.approval_card(approval, running, cx),
            Content::Citation(citation) => super::super::sources::card(
                format!("citation-{id}"),
                citation,
                cx.entity().downgrade(),
            )
            .into_any_element(),
            Content::Status { phase, attempt } => Marker::new()
                .id(SharedString::from(format!("event-status-{id}")))
                .role(Role::Status)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .icon(MarkerIcon::new().child(Icon::new(IconName::Info).small()))
                .content(MarkerContent::new().text(status_text(*phase, *attempt)))
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
                yss_ui::text::t("native.assistant.collapse")
            } else {
                yss_ui::text::t("native.assistant.expandFullOutput")
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
                .unwrap_or_else(|| yss_ui::text::t("native.assistant.notReturned").into())
        };
        let title = match usage.purpose {
            ModelCallPurpose::Response => yss_ui::text::t("native.assistant.modelCallCompleted"),
            ModelCallPurpose::Compaction => {
                yss_ui::text::t("native.assistant.summaryCallCompleted")
            }
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
                yss_ui::text::format(
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
            body = body.child(div().text_xs().child(yss_ui::text::format(
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
            yss_ui::text::t("common.completed")
        } else if active && !self.timing_connected() {
            yss_ui::text::t("panel.assistantToolUnknown")
        } else if active {
            yss_ui::text::t("native.assistant.inProgress")
        } else {
            yss_ui::text::t("panel.assistantToolInterrupted")
        };
        let mut body = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.disclosure(
                key,
                yss_ui::text::format(
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
        if compaction.total_bytes > 0 {
            body = body.child(
                Progress::new(SharedString::from(format!("compaction-progress-{id}")))
                    .xsmall()
                    .accessibility_label(yss_ui::text::t("native.assistant.compactContext"))
                    .value(
                        (compaction.completed_bytes as f64 / compaction.total_bytes as f64 * 100.)
                            .clamp(0., 100.) as f32,
                    ),
            );
        }
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

fn reasoning_preview(text: &str, active: bool) -> Option<String> {
    // A bounded excerpt keeps streaming reasoning from displacing the answer;
    // expanding always reveals the original, unmodified Markdown.
    let line = if active {
        text.lines().rev().find(|line| !line.trim().is_empty())
    } else {
        text.lines().find(|line| !line.trim().is_empty())
    }?;
    let mut characters = line.trim().chars();
    let mut preview: String = characters.by_ref().take(160).collect();
    if characters.next().is_some() {
        preview.push('…');
    }
    Some(preview)
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

/// Localize runtime status at presentation time.
pub(super) fn status_text(phase: yss_harness_contract::AgentRuntimePhase, attempt: u32) -> String {
    use yss_harness_contract::AgentRuntimePhase;
    use yss_ui::text::{format, t};
    let label = t(match phase {
        AgentRuntimePhase::CheckingDelivery => "native.assistant.inspectDelivery",
        AgentRuntimePhase::Reconnecting => "panel.assistantReloadConversations",
        AgentRuntimePhase::Compacting => "native.assistant.compactContext",
    });
    if attempt == 0 {
        label.into_owned()
    } else {
        format(
            "native.assistant.attempt",
            &[
                ("label", label.into_owned()),
                ("attempt", attempt.to_string()),
            ],
        )
    }
}
