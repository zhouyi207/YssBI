//! Shared confirmation form in native modal windows.
use super::ModalContent;
use gpui_kit::{App, ClickEvent, SharedString, Window, div, prelude::*, px, size};

pub(crate) fn confirm(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    accept: impl Into<SharedString>,
    cancel: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
    action: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
) {
    let message = message.into();
    let accept = accept.into();
    let cancel = cancel.into();
    super::open(title, size(px(500.), px(260.)), window, cx, move |_, _| {
        ModalContent::new(move |_, _| div().text_sm().child(message.clone()))
            .confirm(accept, action)
            .danger()
            .cancel(cancel)
    });
}
