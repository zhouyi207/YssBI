//! Shared native window chrome and gestures, composed with the component frame, icons and theme.
mod controls;
#[cfg(target_os = "linux")]
mod input_region;

use gpui_kit::base::{GlobalState, RootPlugin};
use gpui_kit::component::{
    ActiveTheme, InteractiveElementExt, Root, TITLE_BAR_HEIGHT, window_paddings,
};
use gpui_kit::{
    AnyElement, App, Bounds, ClickEvent, Context, Decorations, Div, Edges, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ResizeEdge, Size, Stateful,
    Window, WindowControlArea, div, point, prelude::*, px, size,
};

// Match the component frame's default resize band, measured from its public window paddings.
const RESIZE_HIT_SIZE: Pixels = px(4.);

pub fn init(cx: &mut App) {
    // Register after component initialization so capture wraps its frame as well as the content.
    Root::register_plugin::<WindowChrome>(cx, |_, _| WindowChrome::default());
}

#[derive(Default)]
struct WindowChrome {
    pending_move: bool,
    #[cfg(target_os = "linux")]
    input_region: input_region::WindowInputRegion,
}

impl Render for WindowChrome {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl RootPlugin for WindowChrome {
    #[cfg(target_os = "linux")]
    fn prepare(&mut self, window: &mut Window, _: &mut Context<Self>) {
        // window_bounds() describes restore geometry, not the maximized or fullscreen surface.
        let input_region = match window.window_decorations() {
            Decorations::Client { .. } => Some(window_input_bounds(
                window.bounds().size,
                window_paddings(window),
                window.is_resizable(),
            )),
            Decorations::Server => None,
        };
        if let Err(error) = self.input_region.update(window, input_region) {
            tracing::error!(%error, "Failed to update the native window input region");
        }
    }

    fn decorate(
        &self,
        surface: AnyElement,
        root: &Root,
        window: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        let state = root.plugin::<Self>().expect("window chrome is registered");
        div()
            .id("native-window-gestures")
            .size_full()
            .capture_any_mouse_down(window.listener_for(
                &state,
                |state, event: &MouseDownEvent, window, cx| {
                    state.pending_move = false;
                    if event.button == MouseButton::Left
                        && let Some(edge) = resize_edge(event.position, window)
                    {
                        // The platform can keep the release after a resize; do not arm a title-bar move.
                        window.prevent_default();
                        cx.stop_propagation();
                        window.start_window_resize(edge);
                    }
                },
            ))
            .child(surface)
    }
}

pub fn title_bar(
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    content: impl IntoElement,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    title_bar_with_controls(on_close, content, false, window, cx)
}

pub fn modal_title_bar(
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    content: impl IntoElement,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    title_bar_with_controls(on_close, content, true, window, cx)
}

fn title_bar_with_controls(
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    content: impl IntoElement,
    modal: bool,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let state = Root::read(window, cx)
        .plugin::<WindowChrome>()
        .expect("window chrome is registered");
    let client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });

    div()
        .id("native-title-bar")
        .flex()
        .items_center()
        .flex_shrink_0()
        .h(TITLE_BAR_HEIGHT)
        .bg(cx.theme().title_bar)
        .border_b_1()
        .border_color(cx.theme().title_bar_border)
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            // Window selection accepts presses outside text; this press belongs to the title bar.
            GlobalState::suppress_text_selection(cx);
        })
        .child(
            div()
                .id("native-title-bar-content")
                .flex()
                .items_center()
                .flex_1()
                .min_w_0()
                .h_full()
                .pl(if cfg!(target_os = "macos") && !window.is_fullscreen() {
                    px(80.)
                } else {
                    px(12.)
                })
                .window_control_area(WindowControlArea::Drag)
                .on_mouse_down(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| state.pending_move = true),
                )
                .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                    state.pending_move = false;
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| state.pending_move = false),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| state.pending_move = false),
                )
                .on_mouse_exit(window.listener_for(&state, |state, _, _, _| {
                    state.pending_move = false;
                }))
                .on_mouse_move(window.listener_for(
                    &state,
                    |state, event: &MouseMoveEvent, window, _| {
                        let pending_move = std::mem::take(&mut state.pending_move);
                        if pending_move && event.pressed_button == Some(MouseButton::Left) {
                            window.start_window_move();
                        }
                    },
                ))
                .when(
                    !modal && (cfg!(target_os = "linux") || cfg!(target_os = "macos")),
                    |bar| {
                        bar.on_double_click(|_, window, _| {
                            if cfg!(target_os = "macos") {
                                window.titlebar_double_click();
                            } else {
                                window.zoom_window();
                            }
                        })
                    },
                )
                .when(cfg!(target_os = "linux") && client_decorated, |bar| {
                    bar.on_mouse_down(MouseButton::Right, |event, window, _| {
                        window.show_window_menu(event.position);
                    })
                })
                .child(content),
        )
        .child(controls::render(on_close, modal, window, cx))
}

fn resize_edge(position: Point<Pixels>, window: &Window) -> Option<ResizeEdge> {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return None;
    };
    let frame = frame_bounds(window.bounds().size, window_paddings(window));
    if !frame.dilate(RESIZE_HIT_SIZE).contains(&position) {
        return None;
    }
    let left = !tiling.left && position.x <= frame.left() + RESIZE_HIT_SIZE;
    let right = !tiling.right && position.x >= frame.right() - RESIZE_HIT_SIZE;
    let top = !tiling.top && position.y <= frame.top() + RESIZE_HIT_SIZE;
    let bottom = !tiling.bottom && position.y >= frame.bottom() - RESIZE_HIT_SIZE;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeEdge::TopLeft),
        (_, true, true, _) => Some(ResizeEdge::TopRight),
        (true, _, _, true) => Some(ResizeEdge::BottomLeft),
        (_, true, _, true) => Some(ResizeEdge::BottomRight),
        (true, _, _, _) => Some(ResizeEdge::Left),
        (_, true, _, _) => Some(ResizeEdge::Right),
        (_, _, true, _) => Some(ResizeEdge::Top),
        (_, _, _, true) => Some(ResizeEdge::Bottom),
        _ => None,
    }
}

fn frame_bounds(window_size: Size<Pixels>, insets: Edges<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(insets.left, insets.top),
        size(
            window_size.width - insets.left - insets.right,
            window_size.height - insets.top - insets.bottom,
        ),
    )
}

#[cfg(any(target_os = "linux", test))]
fn window_input_bounds(
    window_size: Size<Pixels>,
    insets: Edges<Pixels>,
    resizable: bool,
) -> Bounds<Pixels> {
    let resize_band = if resizable { RESIZE_HIT_SIZE } else { px(0.) };
    frame_bounds(
        window_size,
        insets.map(|inset| (*inset - resize_band).max(px(0.))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_passes_through_but_frame_and_resize_band_receive_input() {
        let bounds = window_input_bounds(size(px(1000.), px(700.)), Edges::all(px(20.)), true);
        assert_eq!(
            bounds,
            Bounds::new(point(px(16.), px(16.)), size(px(968.), px(668.)))
        );
        for position in [
            point(px(10.), px(350.)),
            point(px(990.), px(350.)),
            point(px(500.), px(10.)),
            point(px(500.), px(690.)),
            point(px(10.), px(10.)),
        ] {
            assert!(!bounds.contains(&position));
        }
        for position in [
            point(px(17.), px(350.)),
            point(px(983.), px(350.)),
            point(px(500.), px(17.)),
            point(px(500.), px(683.)),
            point(px(17.), px(17.)),
            point(px(500.), px(350.)),
        ] {
            assert!(bounds.contains(&position));
        }
    }

    #[test]
    fn tiled_sides_and_maximized_windows_keep_input_to_the_surface_edge() {
        let window_size = size(px(800.), px(600.));
        let insets = Edges {
            top: px(0.),
            bottom: px(20.),
            left: px(0.),
            right: px(20.),
        };
        assert_eq!(
            window_input_bounds(window_size, insets, true),
            Bounds::new(point(px(0.), px(0.)), size(px(784.), px(584.))),
        );
        assert_eq!(
            window_input_bounds(window_size, Edges::all(px(0.)), true),
            Bounds::new(Point::default(), window_size),
        );
    }

    #[test]
    fn fixed_window_does_not_reserve_an_invisible_resize_band() {
        assert_eq!(
            window_input_bounds(size(px(500.), px(300.)), Edges::all(px(20.)), false),
            Bounds::new(point(px(20.), px(20.)), size(px(460.), px(260.))),
        );
    }
}
