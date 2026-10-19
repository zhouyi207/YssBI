use gpui_kit::component::{Sizable, WindowExt, button::Button, notification::Notification};
use gpui_kit::{AnyWindowHandle, Context, SharedString, Window};
use std::rc::Rc;

use crate::lifecycle::LogReleased;

#[cfg(test)]
mod tests;

/// Routes feedback to the window that owns the operation, including callbacks without a Window.
pub struct Notifications {
    window: AnyWindowHandle,
}

impl Notifications {
    pub fn new(window: &Window) -> Self {
        Self {
            window: window.window_handle(),
        }
    }

    /// Replaces this owner's previous toast so repeated failures cannot accumulate.
    pub fn push<T: 'static>(&self, notification: Notification, cx: &mut Context<T>) {
        let window = self.window;
        let notification = notification.id1::<T>(("feedback", cx.entity_id()));
        let owner = cx.entity().downgrade();
        // Deferring also lets a newly created window install its Root before showing feedback.
        cx.defer(move |cx| {
            if owner.upgrade().is_none() {
                return;
            }
            window
                .update(cx, |_, window, cx| {
                    window.push_notification(notification, cx)
                })
                .log_released();
        });
    }

    /// Replaces a view's inline error, announcing only a newly reported one.
    pub fn replace_error<T: 'static>(
        &self,
        slot: &mut Option<String>,
        error: Option<String>,
        cx: &mut Context<T>,
    ) {
        if let Some(error) = &error {
            self.error(error.clone(), cx);
        }
        *slot = error;
    }

    pub fn error<T: 'static>(&self, message: impl Into<SharedString>, cx: &mut Context<T>) {
        self.push(Notification::error(message), cx);
    }

    pub fn error_with_retry<T: 'static>(
        &self,
        message: impl Into<SharedString>,
        cx: &mut Context<T>,
        retry: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) {
        let owner = cx.entity().downgrade();
        let retry = Rc::new(retry);
        self.push(
            Notification::error(message).action(move |_, _, cx| {
                let owner = owner.clone();
                let retry = retry.clone();
                Button::new("retry")
                    .small()
                    .label(crate::text::t("common.retry"))
                    .on_click(cx.listener(move |toast, _, window, cx| {
                        toast.dismiss(window, cx);
                        owner
                            .update(cx, |view, cx| retry(view, window, cx))
                            .log_released();
                    }))
            }),
            cx,
        );
    }
}
