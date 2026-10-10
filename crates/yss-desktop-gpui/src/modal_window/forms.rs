//! Shared confirmation form in native modal windows.
use super::ModalContent;
use gpui::{App, ClickEvent, SharedString, Window, div, prelude::*, px, size};

pub(crate) fn confirm(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    accept: &'static str,
    cancel: &'static str,
    window: &mut Window,
    cx: &mut App,
    action: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
) {
    let message = message.into();
    super::open(title, size(px(500.), px(260.)), window, cx, move |_, _| {
        ModalContent::new(move |_, _| div().text_sm().child(message.clone()))
            .confirm(accept, action)
            .danger()
            .cancel(cancel)
    });
}
