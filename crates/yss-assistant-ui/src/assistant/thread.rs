//! The transcript is virtualized: ListState owns the reading position and tail following,
//! so only turns near the viewport are laid out on each frame.
use super::ConversationPanel;
use gpui_kit::assets::IconName;
use gpui_kit::component::{Sizable, button::Button, scroll::Scrollbar};
use gpui_kit::{
    AnyElement, Context, FollowMode, ListAlignment, ListScrollEvent, ListState, Window, div, list,
    prelude::*, px, rems,
};

pub(super) struct Viewport {
    list: ListState,
    turns: usize,
    session_output: bool,
    following: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        let list = ListState::new(0, ListAlignment::Top, px(1024.));
        list.set_follow_mode(FollowMode::Tail);
        Self {
            list,
            turns: 0,
            session_output: false,
            following: true,
        }
    }
}

impl Viewport {
    /// Items are the turns followed by the session-level output, when there is any.
    fn sync(&mut self, turns: usize, session_output: bool) {
        if turns < self.turns {
            self.list.reset(turns + usize::from(session_output));
        } else {
            if turns > self.turns {
                self.list.splice(self.turns..self.turns, turns - self.turns);
            }
            if session_output != self.session_output {
                self.list.splice(
                    turns..turns + usize::from(self.session_output),
                    usize::from(session_output),
                );
            }
        }
        self.turns = turns;
        self.session_output = session_output;
    }

    /// A replaced transcript invalidates every measured turn and the reading position.
    pub(super) fn reset(&mut self, turns: usize, session_output: bool) {
        self.list.reset(turns + usize::from(session_output));
        self.list.set_follow_mode(FollowMode::Tail);
        self.turns = turns;
        self.session_output = session_output;
        self.following = true;
    }

    fn jump_to_latest(&mut self) {
        self.list.set_follow_mode(FollowMode::Tail);
        self.following = true;
    }
}

impl ConversationPanel {
    pub(super) fn observe_scroll(&self, cx: &mut Context<Self>) {
        self.viewport.list.set_scroll_handler(cx.listener(
            |view, event: &ListScrollEvent, _, cx| {
                if view.viewport.following != event.is_following_tail {
                    view.viewport.following = event.is_following_tail;
                    cx.notify();
                }
            },
        ));
    }

    pub(super) fn render_thread(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let turns = self.transcript.turns.len();
        let session_output = !self.transcript.session_output.parts.is_empty();
        if turns == 0 && !session_output {
            return self.render_welcome(cx);
        }
        self.viewport.sync(turns, session_output);
        let state = self.viewport.list.clone();
        let show_latest = !self.viewport.following && state.is_scrolled_to_end() == Some(false);
        div()
            .id("assistant-transcript")
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(
                list(
                    state.clone(),
                    cx.processor(|view, ix: usize, _: &mut Window, cx| view.render_item(ix, cx)),
                )
                .size_full()
                .pb_4(),
            )
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::vertical(&state)
                        .id("assistant-transcript-scrollbar")
                        .viewport_from_layout(),
                ),
            )
            .when(show_latest, |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom_3()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            Button::new("assistant-latest")
                                .small()
                                .outline()
                                .rounded_full()
                                .icon(IconName::ArrowDown)
                                .label(yss_ui::text::t("panel.assistantScrollToBottom"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.viewport.jump_to_latest();
                                    cx.notify();
                                })),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_item(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let content = if let Some(turn) = self.transcript.turns.get(ix) {
            self.render_turn(turn, cx)
        } else if ix == self.transcript.turns.len()
            && !self.transcript.session_output.parts.is_empty()
        {
            self.render_output(&self.transcript.session_output, &[], self.running(), cx)
        } else {
            return gpui_kit::Empty.into_any_element();
        };
        div()
            .w_full()
            .flex()
            .justify_center()
            .px_4()
            .pt_5()
            .child(div().w_full().max_w(rems(48.)).min_w_0().child(content))
            .into_any_element()
    }
}
