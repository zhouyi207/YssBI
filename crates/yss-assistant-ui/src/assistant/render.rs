use super::{
    CancelResponse, ConversationPanel,
    projection::{Turn, TurnState},
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    clipboard::Clipboard,
    marker::{Marker, MarkerContent, MarkerIcon},
    message::{Message, MessageContent, MessageFooter, MessageHeader},
};
use gpui_kit::{
    AnyElement, Context, IntoElement, Render, Role, SharedString, Window, div, prelude::*,
};
mod failure;
mod output;
mod plain;
mod task;

impl Render for ConversationPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_timing(window, cx);
        self.sync_input(window, cx);
        div()
            .id("assistant-conversation")
            .key_context("AssistantConversation")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &CancelResponse, _, cx| view.cancel(cx)))
            .child(self.render_thread(cx))
            .child(self.render_composer(window, cx))
    }
}
impl ConversationPanel {
    pub(super) fn render_turn(&self, turn: &Turn, cx: &mut Context<Self>) -> AnyElement {
        let id = &turn.id;
        let user =
            Message::new()
                .id(SharedString::from(format!("user-message-{id}")))
                .role(Role::ListItem)
                .p_3()
                .rounded(cx.theme().radius)
                .bg(cx.theme().muted)
                .header(
                    MessageHeader::new()
                        .content_inset(false)
                        .w_full()
                        .child(Icon::new(IconName::User).small())
                        .child(div().flex_1().child(yss_ui::text::t("panel.assistantYou")))
                        .when(!turn.user.is_empty(), |header| {
                            header.child(self.copy_action(turn, true, cx))
                        }),
                )
                .content(
                    MessageContent::new()
                        .child(plain::PlainText::new(
                            SharedString::from(format!("user-{id}")),
                            turn.user.clone(),
                            "panel.assistantYou",
                        ))
                        .child(self.reference_chips(
                            format!("history-references-{id}"),
                            turn.resources.iter().map(|reference| {
                                (&reference.resource, Some(reference.name.as_str()))
                            }),
                            false,
                            cx,
                        )),
                );
        let active = turn.state == TurnState::Running && self.timing_connected();
        let mut footer = MessageFooter::new()
            .content_inset(false)
            .w_full()
            .flex_wrap()
            .gap_2();
        let status = match turn.state {
            TurnState::Running if active => Some(("native.assistant.generating", None)),
            TurnState::Running => Some(("panel.assistantToolUnknown", Some(IconName::Clock))),
            TurnState::Failed if turn.output.failure().is_none() => Some((
                "panel.assistantReplyInterrupted",
                Some(IconName::CircleAlert),
            )),
            TurnState::Cancelled => Some(("panel.assistantReplyStopped", Some(IconName::Square))),
            _ => None,
        };
        if let Some((label, icon)) = status {
            footer = footer.child(
                Marker::new()
                    .id(SharedString::from(format!("turn-status-{id}")))
                    .role(Role::Status)
                    .flex_1()
                    .text_xs()
                    .loading(active)
                    .when_some(icon, |marker, icon| {
                        marker.icon(MarkerIcon::new().child(Icon::new(icon).small()))
                    })
                    .content(MarkerContent::new().text(yss_ui::text::t(label))),
            );
        }
        if turn.state != TurnState::Running && turn.output.has_text() {
            footer = footer.child(self.copy_action(turn, false, cx));
        }
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(user)
            .child(
                Message::new()
                    .id(SharedString::from(format!("assistant-message-{id}")))
                    .role(Role::ListItem)
                    .header(
                        MessageHeader::new()
                            .content_inset(false)
                            .child(self.execution_header(turn, cx)),
                    )
                    .content(
                        MessageContent::new()
                            .items_stretch()
                            .child(self.render_output(
                                &turn.output,
                                &turn.tasks,
                                turn.state == TurnState::Running,
                                cx,
                            )),
                    )
                    .when(status.is_some() || turn.output.has_text(), |message| {
                        message.footer(footer)
                    }),
            )
            .into_any_element()
    }

    fn copy_action(&self, turn: &Turn, user: bool, cx: &Context<Self>) -> Clipboard {
        let owner = cx.entity().downgrade();
        let id = turn.id.clone();
        let label = yss_ui::text::t(if user {
            "native.assistant.copyMessage"
        } else {
            "panel.assistantCopy"
        });
        Clipboard::new(SharedString::from(format!("copy-{user}-{id}")))
            .xsmall()
            .accessibility_label(label)
            .value_fn(move |_, cx| {
                owner
                    .upgrade()
                    .and_then(|owner| {
                        owner
                            .read(cx)
                            .transcript
                            .turns
                            .iter()
                            .find(|turn| turn.id == id)
                            .map(|turn| {
                                if user {
                                    turn.user.clone()
                                } else {
                                    turn.output.text()
                                }
                            })
                    })
                    .unwrap_or_default()
                    .into()
            })
    }
}
