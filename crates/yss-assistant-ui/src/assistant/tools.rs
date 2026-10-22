//! Tool groups borrow event facts; individual visible cards own their disclosure and reads.
use super::{
    ConversationPanel,
    inspect::ToolCard,
    projection::{Approval, Tool, ToolState},
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    spinner::Spinner,
};
use gpui_kit::{AnyElement, Context, Empty, IntoElement, SharedString, div, prelude::*};
use yss_harness_contract::ApprovalDecision;

impl ConversationPanel {
    pub(super) fn tool_group<'a>(
        &self,
        id: String,
        tools: impl Iterator<Item = &'a Tool> + Clone,
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (mut count, mut pending, mut failed, mut uncertain, mut current) = (0, 0, 0, 0, None);
        for tool in tools.clone() {
            count += 1;
            if tool.running() {
                pending += 1;
                current.get_or_insert(&tool.kind);
            } else if tool.state.failed() {
                failed += 1;
            } else if matches!(tool.state, ToolState::Unknown | ToolState::Interrupted) {
                uncertain += 1;
            }
        }
        if count == 0 {
            return Empty.into_any_element();
        }
        let connected = running && self.timing_connected();
        let active = pending > 0 && connected;
        let open = self.expanded.get(&id).copied().unwrap_or(active);
        let label = yss_ui::text::format(
            if active {
                "panel.assistantToolsRunning"
            } else if pending > 0 {
                "panel.assistantToolsUnconfirmed"
            } else if uncertain > 0 {
                "panel.assistantToolsUncertain"
            } else {
                "panel.assistantToolsFinished"
            },
            &[
                ("completed", (count - pending - uncertain).to_string()),
                ("count", count.to_string()),
            ],
        );
        let mut row = div()
            .min_w_0()
            .flex_1()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2();
        row = if active {
            row.child(Spinner::new().small().icon(IconName::LoaderCircle))
        } else {
            row.child(Icon::new(IconName::Wrench).small())
        };
        row = row.child(label.clone());
        if failed > 0 {
            row = row.child(
                div()
                    .text_color(cx.theme().danger)
                    .child(yss_ui::text::format(
                        "panel.assistantToolsFailed",
                        &[("count", failed.to_string())],
                    )),
            );
        }
        if active && let Some(kind) = current {
            row = row.child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(super::activity::tool_name(kind)),
            );
        }
        let mut body = div().min_w_0().flex().flex_col().gap_1();
        if open {
            body = body.children(tools.clone().map(|tool| ToolCard {
                tool: tool.clone(),
                owner: cx.entity().downgrade(),
                generation: self.generation,
                catalog: self.resource_catalog.clone(),
                connected,
            }));
        }
        Collapsible::new()
            .open(open)
            .min_w_0()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(
                Button::new(SharedString::from(id.clone()))
                    .small()
                    .ghost()
                    .w_full()
                    .h_auto()
                    .icon(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .accessibility_label(label)
                    .child(row)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.expanded.insert(id.clone(), !open);
                        cx.notify();
                    })),
            )
            .when(!open, |group| {
                group.child(
                    div()
                        .min_w_0()
                        .px_2()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .children(tools.take(3).map(|tool| {
                            div()
                                .min_w_0()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .child(Icon::new(tool.state.icon(connected)).small())
                                .child(super::activity::tool_name(&tool.kind))
                                .child(
                                    div()
                                        .text_color(if tool.state.failed() {
                                            cx.theme().danger
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .child(tool.state.label(connected)),
                                )
                        })),
                )
            })
            .content(body)
            .into_any_element()
    }
}

const APPROVAL_ARGUMENT_PREVIEW: usize = 400;

impl ConversationPanel {
    pub(super) fn approval_card(
        &self,
        approval: &Approval,
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut arguments = approval.arguments.to_string();
        if let Some((end, _)) = arguments.char_indices().nth(APPROVAL_ARGUMENT_PREVIEW) {
            arguments.truncate(end);
            arguments.push('…');
        }
        let header = div()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::SquareTerminal).small())
            .child(div().min_w_0().truncate().child(yss_ui::text::format(
                "panel.assistantApprovalTitle",
                &[("tool", super::activity::tool_name(&approval.kind))],
            )));
        let mut card = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(header)
            .child(
                div()
                    .min_w_0()
                    .text_xs()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(cx.theme().muted_foreground)
                    .child(arguments),
            );
        let settled = match approval.decision {
            Some(ApprovalDecision::AllowOnce | ApprovalDecision::AllowForProject) => {
                Some("panel.assistantApprovalAllowed")
            }
            Some(ApprovalDecision::Deny) => Some("panel.assistantApprovalDenied"),
            None if !running => Some("panel.assistantApprovalExpired"),
            None => None,
        };
        if let Some(key) = settled {
            return card
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(yss_ui::text::t(key)),
                )
                .into_any_element();
        }
        card = card
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(yss_ui::text::t("panel.assistantApprovalHint")),
            )
            .child(
                div().flex().flex_wrap().gap_2().children(
                    [
                        (
                            ApprovalDecision::AllowOnce,
                            "panel.assistantApprovalAllowOnce",
                        ),
                        (
                            ApprovalDecision::AllowForProject,
                            "panel.assistantApprovalAllowProject",
                        ),
                        (ApprovalDecision::Deny, "panel.assistantApprovalDeny"),
                    ]
                    .map(|(decision, key)| {
                        let id = approval.id.clone();
                        let button = Button::new(SharedString::from(format!(
                            "approval-{}-{key}",
                            approval.id
                        )))
                        .small()
                        .label(yss_ui::text::t(key))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.resolve_approval(&id, decision, cx)
                        }));
                        match decision {
                            ApprovalDecision::AllowOnce => button.primary(),
                            ApprovalDecision::AllowForProject => button.outline(),
                            ApprovalDecision::Deny => button.ghost(),
                        }
                    }),
                ),
            );
        card.into_any_element()
    }
}

impl ToolState {
    pub(super) fn failed(self) -> bool {
        matches!(self, Self::Failed | Self::TimedOut | Self::GraphFailed)
    }

    pub(super) fn icon(self, connected: bool) -> IconName {
        match self {
            Self::Completed | Self::GraphSucceeded => IconName::Check,
            Self::Failed | Self::TimedOut | Self::GraphFailed => IconName::CircleAlert,
            Self::Running if connected => IconName::LoaderCircle,
            Self::Running | Self::Unknown | Self::Interrupted | Self::Cancelled => IconName::Clock,
        }
    }

    pub(super) fn label(self, connected: bool) -> std::borrow::Cow<'static, str> {
        yss_ui::text::t(match self {
            Self::Running if !connected => "panel.assistantToolUnknown",
            Self::Running => "panel.assistantToolRunning",
            Self::Completed => "panel.assistantToolCompleted",
            Self::Failed => "panel.assistantToolFailed",
            Self::Cancelled => "panel.assistantToolCancelled",
            Self::TimedOut => "panel.assistantToolTimedOut",
            Self::Interrupted => "panel.assistantToolInterrupted",
            Self::Unknown => "panel.assistantToolUnknown",
            Self::GraphSucceeded => "panel.assistantGraphSucceeded",
            Self::GraphFailed => "panel.assistantGraphFailed",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ToolState;

    #[test]
    fn failure_counts_do_not_conflate_unknown_or_cancelled_outcomes() {
        for state in [
            ToolState::Failed,
            ToolState::TimedOut,
            ToolState::GraphFailed,
        ] {
            assert!(state.failed());
        }
        for state in [
            ToolState::Running,
            ToolState::Completed,
            ToolState::GraphSucceeded,
            ToolState::Unknown,
            ToolState::Interrupted,
            ToolState::Cancelled,
        ] {
            assert!(!state.failed());
        }
    }
}
