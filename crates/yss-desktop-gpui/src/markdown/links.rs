//! Only remote web URLs reach the platform launcher.
use gpui_kit::component::{WindowExt, notification::Notification};
use gpui_kit::{App, SharedString, Window};

pub(crate) fn open_external(href: &SharedString, window: &mut Window, cx: &mut App) {
    let Ok(url) = url::Url::parse(href) else {
        return;
    };
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return;
    }
    let opening = cx
        .background_executor()
        .spawn(async move { open::that(url.as_str()) });
    window
        .spawn(cx, async move |cx| {
            if opening.await.is_err() {
                let _ = cx.update(|window, cx| {
                    window.push_notification(
                        Notification::error(crate::text::t("notifications.externalUrl.openFailed")),
                        cx,
                    );
                });
            }
        })
        .detach();
}
