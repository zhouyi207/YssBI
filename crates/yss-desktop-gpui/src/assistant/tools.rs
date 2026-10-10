//! Tool groups borrow event facts; individual visible cards own their disclosure and reads.
use super::{
    ConversationPanel,
    inspect::ToolCard,
    projection::{Tool, ToolState},
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    spinner::Spinner,
};
use gpui_kit::{AnyElement, Context, Empty, IntoElement, SharedString, div, prelude::*};

impl ConversationPanel {
    pub(super) fn tool_group<'a>(
        &self,
        id: String,
        tools: impl Iterator<Item = &'a Tool> + Clone,
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (mut count, mut pending, mut failed, mut current) = (0, 0, 0, None);
        for tool in tools.clone() {
            count += 1;
            if tool.running() {
                pending += 1;
                current.get_or_insert(tool.kind);
            } else if tool.state.failed() {
                failed += 1;
            }
        }
        if count == 0 {
            return Empty.into_any_element();
        }
        let connected = running && self.timing_connected();
        let active = pending > 0 && connected;
        let open = self.expanded.get(&id).copied().unwrap_or(active);
        let label = crate::text::format(
            if active {
                "panel.assistantToolsRunning"
            } else if pending > 0 {
                "panel.assistantToolsUnconfirmed"
            } else {
                "panel.assistantToolsFinished"
            },
            &[
                ("completed", (count - pending).to_string()),
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
                    .child(crate::text::format(
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
            body = body.children(tools.map(|tool| ToolCard {
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
            .content(body)
            .into_any_element()
    }
}

impl ToolState {
    pub(super) fn failed(self) -> bool {
        !matches!(self, Self::Running | Self::Completed | Self::GraphSucceeded)
    }

    pub(super) fn label(self, connected: bool) -> &'static str {
        crate::text::t(match self {
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
