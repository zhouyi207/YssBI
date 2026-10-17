//! Platform window controls, sharing component icons and theme roles with the workbench.
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, TITLE_BAR_HEIGHT};
use gpui_kit::{
    App, ClickEvent, Decorations, Div, MouseButton, Stateful, Window, WindowControlArea, div,
    prelude::*,
};

pub(super) fn render(
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    modal: bool,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let controls = div()
        .id("native-window-controls")
        .flex()
        .flex_shrink_0()
        .h_full();
    if cfg!(target_os = "macos")
        || (cfg!(target_os = "linux")
            && !matches!(window.window_decorations(), Decorations::Client { .. }))
    {
        return controls;
    }
    let supported = window.window_controls();
    controls
        .when(!modal && supported.minimize, |controls| {
            controls.child(control(
                "window-minimize",
                IconName::WindowMinimize,
                "common.minimize",
                WindowControlArea::Min,
                |_, window, _| window.minimize_window(),
                cx,
            ))
        })
        .when(!modal && supported.maximize, |controls| {
            controls.child(control(
                "window-maximize",
                if window.is_maximized() {
                    IconName::WindowRestore
                } else {
                    IconName::WindowMaximize
                },
                if window.is_maximized() {
                    "common.restore"
                } else {
                    "common.maximize"
                },
                WindowControlArea::Max,
                |_, window, _| window.zoom_window(),
                cx,
            ))
        })
        .child(control(
            "window-close",
            IconName::WindowClose,
            "common.close",
            WindowControlArea::Close,
            on_close,
            cx,
        ))
}

fn control(
    id: &'static str,
    icon: IconName,
    label: &'static str,
    area: WindowControlArea,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let close = area == WindowControlArea::Close;
    let (hover, active, foreground) = if close {
        (
            cx.theme().danger,
            cx.theme().danger_active,
            cx.theme().danger_foreground,
        )
    } else {
        (
            cx.theme().secondary_hover,
            cx.theme().secondary_active,
            cx.theme().secondary_foreground,
        )
    };
    div()
        .id(id)
        .role(gpui_kit::accesskit::Role::Button)
        .aria_label(crate::text::t(label))
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(TITLE_BAR_HEIGHT)
        .min_w(crate::appearance::HEADER_HEIGHT)
        .h_full()
        .text_color(cx.theme().foreground)
        .hover(move |style| style.bg(hover).text_color(foreground))
        .active(move |style| style.bg(active).text_color(foreground))
        .when(cfg!(target_os = "windows"), |control| {
            control.window_control_area(area)
        })
        .when(cfg!(target_os = "linux"), |control| {
            control.on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
        })
        // Native caption hit testing owns pointer clicks on Windows. The callback
        // also gives accessibility activation the same command on every platform.
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .child(Icon::new(icon).small())
}
