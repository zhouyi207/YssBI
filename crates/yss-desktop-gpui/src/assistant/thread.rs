//! The loaded history range and reading position belong to this conversation view.
use super::ConversationPanel;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, Pixels, ScrollHandle, div, prelude::*, px};
use std::cell::Cell;

const PAGE_TURNS: usize = 20;
const BOTTOM_THRESHOLD: Pixels = px(32.);

pub(super) struct Viewport {
    scroll: ScrollHandle,
    first: Option<usize>,
    follow_latest: bool,
    // Keep a visible turn stable when history or asynchronous Markdown changes
    // the layout. ScrollHandle remains the actual offset owner.
    anchor: Cell<Option<ReadingPosition>>,
}

#[derive(Clone, Copy, Debug)]
struct ReadingPosition {
    turn: usize,
    top: Pixels,
    viewport_top: Pixels,
    offset: Pixels,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            first: None,
            follow_latest: true,
            anchor: Cell::new(None),
        }
    }
}

impl Viewport {
    fn first_turn(&mut self, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        let first = self.first.get_or_insert(count.saturating_sub(PAGE_TURNS));
        *first = (*first).min(count - 1);
        *first
    }

    fn capture_anchor(&self) {
        let Some(first) = self.first else {
            return;
        };
        // Child zero is always the history control, including when it is empty.
        let item = self.scroll.top_item().max(1);
        self.anchor.set(
            self.scroll
                .bounds_for_item(item)
                .map(|bounds| ReadingPosition {
                    turn: first + item - 1,
                    top: bounds.top(),
                    viewport_top: self.scroll.bounds().top(),
                    offset: self.scroll.offset().y,
                }),
        );
    }

    fn at_latest(&self) -> bool {
        self.scroll.offset().y + self.scroll.max_offset().y <= BOTTOM_THRESHOLD
    }

    fn scrolled(&mut self) -> bool {
        let following = self.at_latest();
        let changed = self.follow_latest != following;
        self.follow_latest = following;
        if following {
            self.anchor.set(None);
        } else {
            self.capture_anchor();
        }
        changed
    }

    fn load_earlier(&mut self) {
        self.capture_anchor();
        self.first = self.first.map(|first| first.saturating_sub(PAGE_TURNS));
        self.follow_latest = false;
    }

    fn jump_to_latest(&mut self) {
        self.follow_latest = true;
        self.anchor.set(None);
        self.scroll.scroll_to_bottom();
    }

    fn finish_layout(&self) -> bool {
        let mut offset = self.scroll.offset();
        let target = if self.follow_latest {
            -self.scroll.max_offset().y
        } else if let Some(mut position) = self.anchor.get() {
            let Some(item) = self
                .first
                .and_then(|first| position.turn.checked_sub(first))
            else {
                return false;
            };
            let Some(bounds) = self.scroll.bounds_for_item(item + 1) else {
                return false;
            };
            let viewport_top = self.scroll.bounds().top();
            let shift = viewport_top - position.viewport_top - (bounds.top() - position.top);
            // Scrolling (including momentum) changes the offset without changing
            // these layout coordinates. Only layout changes need correction.
            let target = if shift == px(0.) {
                offset.y
            } else {
                position.offset + shift
            };
            position.top = bounds.top();
            position.viewport_top = viewport_top;
            position.offset = target.clamp(-self.scroll.max_offset().y, px(0.));
            self.anchor.set(Some(position));
            target
        } else {
            return false;
        }
        .clamp(-self.scroll.max_offset().y, px(0.));
        if (offset.y - target).abs() <= px(0.5) {
            return false;
        }
        offset.y = target;
        self.scroll.set_offset(offset);
        true
    }
}

impl ConversationPanel {
    pub(super) fn render_thread(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.transcript.turns.is_empty() && self.transcript.session_output.parts.is_empty() {
            return gpui_kit::Empty.into_any_element();
        }
        let first = self.viewport.first_turn(self.transcript.turns.len());
        let owner = cx.entity().downgrade();
        let mut body = div()
            .on_children_prepainted(move |_, window, cx| {
                if owner
                    .upgrade()
                    .is_some_and(|view| view.read(cx).viewport.finish_layout())
                {
                    let owner = owner.clone();
                    window.on_next_frame(move |_, cx| {
                        let _ = owner.update(cx, |_, cx| cx.notify());
                    });
                }
            })
            .id("assistant-transcript")
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .track_scroll(&self.viewport.scroll)
            .px_3()
            .py_3()
            .flex()
            .flex_col()
            .gap_4()
            .on_scroll_wheel(cx.listener(|_, _, window, cx| {
                // Observe the offset after GPUI has handled the wheel event.
                cx.defer_in(window, |view, _, cx| {
                    if view.viewport.scrolled() {
                        cx.notify();
                    }
                });
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .self_center()
                    .when(first > 0, |control| {
                        control.child(
                            Button::new("assistant-earlier")
                                .small()
                                .ghost()
                                .label(crate::text::format(
                                    "panel.assistantLoadEarlier",
                                    &[("count", (first * 2).to_string())],
                                ))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.viewport.load_earlier();
                                    cx.notify();
                                })),
                        )
                    }),
            );
        for turn in &self.transcript.turns[first..] {
            body = body.child(
                div()
                    .w_full()
                    .max_w(px(768.))
                    .min_w_0()
                    .flex_shrink_0()
                    .self_center()
                    .child(self.render_turn(turn, cx)),
            );
        }
        if !self.transcript.session_output.parts.is_empty() {
            body = body.child(
                div()
                    .w_full()
                    .max_w(px(768.))
                    .min_w_0()
                    .flex_shrink_0()
                    .self_center()
                    .child(self.render_output(
                        &self.transcript.session_output,
                        &[],
                        self.running(),
                        cx,
                    )),
            );
        }
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(body)
            .when(
                !self.viewport.follow_latest && !self.viewport.at_latest(),
                |view| {
                    view.child(
                        div().absolute().right_3().bottom_2().child(
                            Button::new("assistant-latest")
                                .small()
                                .icon(IconName::ArrowDown)
                                .tooltip(crate::text::t("panel.assistantScrollToBottom"))
                                .accessibility_label(crate::text::t(
                                    "panel.assistantScrollToBottom",
                                ))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.viewport.jump_to_latest();
                                    cx.notify();
                                })),
                        ),
                    )
                },
            )
            .into_any_element()
    }
}
