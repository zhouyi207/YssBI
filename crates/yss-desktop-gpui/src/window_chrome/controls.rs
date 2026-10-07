//! Platform window controls, sharing component icons and theme roles with the workbench.
use gpui::{
    App, ClickEvent, Decorations, Div, MouseButton, Stateful, Window, WindowControlArea, div,
    prelude::*,
};
use gpui_component::{ActiveTheme, Icon, IconName, Sizable, TITLE_BAR_HEIGHT};

pub(super) fn render(
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
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
        .when(supported.minimize, |controls| {
            controls.child(control(
                "window-minimize",
                IconName::WindowMinimize,
                WindowControlArea::Min,
                |_, window, _| window.minimize_window(),
                cx,
            ))
        })
        .when(supported.maximize, |controls| {
            controls.child(control(
                "window-maximize",
                if window.is_maximized() {
                    IconName::WindowRestore
                } else {
                    IconName::WindowMaximize
                },
                WindowControlArea::Max,
                |_, window, _| window.zoom_window(),
                cx,
            ))
        })
        .child(control(
            "window-close",
            IconName::WindowClose,
            WindowControlArea::Close,
            on_close,
            cx,
        ))
}

fn control(
    id: &'static str,
    icon: IconName,
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
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(TITLE_BAR_HEIGHT)
        .h_full()
        .text_color(cx.theme().foreground)
        .hover(move |style| style.bg(hover).text_color(foreground))
        .active(move |style| style.bg(active).text_color(foreground))
        .when(cfg!(target_os = "windows"), |control| {
            control.window_control_area(area)
        })
        .when(cfg!(target_os = "linux"), |control| {
            control
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_click(event, window, cx);
                })
        })
        .child(Icon::new(icon).small())
}
