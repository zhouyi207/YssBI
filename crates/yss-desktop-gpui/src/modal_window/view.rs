//! Shared native frame and modal actions, built from the existing component controls.
use super::{ModalContent, state};
use crate::window_chrome;
use gpui_kit::base::actions::{Cancel, Confirm};
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{
    AnyWindowHandle, App, ClickEvent, Context, FocusHandle, IntoElement, Render, SharedString,
    Window, div, prelude::*, px, size,
};
use std::{cell::RefCell, rc::Rc};

pub(super) struct ModalWindow {
    pub(super) title: SharedString,
    parent: AnyWindowHandle,
    pub(super) origin: AnyWindowHandle,
    content: ModalContent,
    focus: FocusHandle,
    confirming: bool,
    closing: bool,
}

impl ModalWindow {
    pub(super) fn new(
        title: SharedString,
        parent: AnyWindowHandle,
        origin: AnyWindowHandle,
        previous_focus: Option<FocusHandle>,
        content: ModalContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let handle = window.window_handle();
        let modal = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = modal.update(cx, |modal, cx| {
                modal.cancel(&ClickEvent::default(), window, cx)
            });
            false
        });
        cx.on_release(move |modal, cx| {
            if let Some(closed) = modal.content.closed.take() {
                closed(cx);
            }
            cx.defer(move |cx| {
                let _ = parent.update(cx, |_, window, cx| {
                    let restore_focus = state(window, cx).update(cx, |state, _| {
                        if state.child == Some(handle) {
                            state.child = None;
                        }
                        state.child.is_none()
                    });
                    if restore_focus {
                        window.activate_window();
                        if let Some(focus) = previous_focus {
                            window.focus(&focus, cx);
                        }
                    }
                });
            });
        })
        .detach();
        let focus = cx.focus_handle();
        let initial_focus = content.focus.clone().unwrap_or_else(|| focus.clone());
        cx.defer_in(window, move |_, window, cx| {
            window.focus(&initial_focus, cx)
        });
        Self {
            title,
            parent,
            origin,
            content,
            focus,
            confirming: false,
            closing: false,
        }
    }

    pub(super) fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        let handle = window.window_handle();
        let parent = self.parent;
        cx.defer(move |cx| {
            let _ = parent.update(cx, |_, window, cx| {
                state(window, cx).update(cx, |state, _| {
                    if state.child == Some(handle) {
                        state.child = None;
                    }
                })
            });
        });
        window.remove_window();
    }

    fn cancel(&mut self, event: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || self.confirming || state(window, cx).read(cx).child.is_some() {
            return;
        }
        if (self.content.cancel)(event, window, cx) {
            self.finish(window, cx);
        }
    }

    fn confirm(&mut self, event: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || self.confirming || self.content.confirm_text.is_none() {
            return;
        }
        self.confirming = true;
        let action = self.content.confirm.clone();
        let event = event.clone();
        let origin = self.origin;
        let handle = window.window_handle();
        let modal = cx.weak_entity();
        cx.defer(move |cx| {
            let close = origin
                .update(cx, |_, window, cx| action(&event, window, cx))
                .unwrap_or(true);
            let _ = handle.update(cx, |_, window, cx| {
                let _ = modal.update(cx, |modal, cx| {
                    modal.confirming = false;
                    if close {
                        modal.finish(window, cx);
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
}

impl Render for ModalWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = (self.content.body)(window, cx);
        let footer = if let Some(footer) = &self.content.footer {
            Some(footer(window, cx))
        } else {
            self.content.confirm_text.clone().map(|label| {
                div()
                    .flex()
                    .justify_end()
                    .items_center()
                    .gap_2()
                    .when_some(self.content.cancel_text.clone(), |view, label| {
                        view.child(
                            Button::new("modal-cancel")
                                .label(label)
                                .disabled(self.confirming)
                                .on_click(cx.listener(|view, event, window, cx| {
                                    view.cancel(event, window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("modal-confirm")
                            .label(label)
                            .with_variant(self.content.confirm_variant)
                            .disabled(self.confirming)
                            .on_click(cx.listener(|view, event, window, cx| {
                                view.confirm(event, window, cx)
                            })),
                    )
                    .into_any_element()
            })
        };
        div()
            .key_context("NativeModal")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|view, _: &Cancel, window, cx| {
                cx.stop_propagation();
                view.cancel(&ClickEvent::default(), window, cx);
            }))
            .on_action(cx.listener(|view, _: &Confirm, window, cx| {
                cx.stop_propagation();
                view.confirm(&ClickEvent::default(), window, cx);
            }))
            .child(window_chrome::modal_title_bar(
                cx.listener(|view, event, window, cx| view.cancel(event, window, cx)),
                div().text_sm().child(self.title.clone()),
                window,
                cx,
            ))
            .child(
                div()
                    .id("native-modal-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .child(body),
            )
            .when_some(footer, |view, footer| {
                view.child(
                    div()
                        .p_3()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(footer),
                )
            })
    }
}

pub(super) fn open_prompt(
    parent: AnyWindowHandle,
    title: SharedString,
    message: SharedString,
    buttons: Vec<SharedString>,
    sender: tokio::sync::oneshot::Sender<usize>,
    cx: &mut App,
) {
    let child = parent
        .update(cx, |_, window, cx| state(window, cx).read(cx).child)
        .ok()
        .flatten();
    if let Some(child) = child {
        open_prompt(child, title, message, buttons, sender, cx);
        return;
    }
    let _ = parent.update(cx, |_, window, cx| {
        super::open(title, size(px(540.), px(280.)), window, cx, move |_, _| {
            let sender = Rc::new(RefCell::new(Some(sender)));
            let enter = sender.clone();
            ModalContent::new(move |_, _| div().text_sm().child(message.clone()))
                .confirm(crate::text::t("common.confirm"), move |_, _, cx| {
                    if let Some(sender) = enter.borrow_mut().take() {
                        cx.defer(move |_| {
                            let _ = sender.send(0);
                        });
                    }
                    true
                })
                .footer(move |_, _| {
                    div().flex().justify_end().flex_wrap().gap_2().children(
                        buttons.iter().enumerate().map(|(index, label)| {
                            let sender = sender.clone();
                            Button::new(("prompt-answer", index))
                                .label(label.clone())
                                .when(index == 0, |button| button.primary())
                                .on_click(move |_, window, cx| {
                                    super::close(window, cx);
                                    if let Some(sender) = sender.borrow_mut().take() {
                                        cx.defer(move |_| {
                                            let _ = sender.send(index);
                                        });
                                    }
                                })
                        }),
                    )
                })
        });
    });
}
