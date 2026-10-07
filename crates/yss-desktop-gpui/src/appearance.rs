//! Shared desktop colours and presentation primitives; no business or layout state.
use gpui::{App, IntoElement, Window, div, prelude::*, px, rgb};
use gpui_component::{ActiveTheme, Icon, IconName, Theme, ThemeMode};

pub const CANVAS: u32 = 0x141820;
pub const SURFACE: u32 = 0x1c212b;
pub const SURFACE_RAISED: u32 = 0x232a36;
pub const BORDER: u32 = 0x303846;
pub const TEXT: u32 = 0xdde3ed;
pub const MUTED: u32 = 0x8b97aa;
pub const BLUE: u32 = 0x89b4fa;
pub const GREEN: u32 = 0x89c5aa;
pub const AMBER: u32 = 0xdfba82;

pub fn install(window: &mut Window, cx: &mut App) {
    Theme::change(ThemeMode::Dark, Some(window), cx);
    let theme = Theme::global_mut(cx);
    theme.font_size = px(14.);
    theme.mono_font_size = px(12.);
    theme.radius = px(5.);
    theme.radius_lg = px(8.);
    let colors = &mut theme.colors;
    colors.background = rgb(SURFACE).into();
    colors.foreground = rgb(TEXT).into();
    colors.border = rgb(BORDER).into();
    colors.input = rgb(BORDER).into();
    colors.muted = rgb(SURFACE_RAISED).into();
    colors.muted_foreground = rgb(MUTED).into();
    colors.accent = rgb(0x2b384d).into();
    colors.accent_foreground = rgb(TEXT).into();
    colors.primary = rgb(0x6e9fdd).into();
    colors.primary_hover = rgb(0x82b1ed).into();
    colors.primary_active = rgb(0x5e90cd).into();
    colors.primary_foreground = rgb(0x101925).into();
    colors.ring = rgb(BLUE).into();
    colors.caret = rgb(BLUE).into();
    colors.secondary = rgb(SURFACE_RAISED).into();
    colors.secondary_foreground = rgb(TEXT).into();
    colors.secondary_hover = rgb(0x2c3543).into();
    colors.sidebar = rgb(0x1a1f28).into();
    colors.sidebar_foreground = rgb(TEXT).into();
    colors.sidebar_border = rgb(BORDER).into();
    colors.sidebar_accent = rgb(0x283449).into();
    colors.sidebar_accent_foreground = rgb(TEXT).into();
    colors.tab_bar = rgb(0x191e27).into();
    colors.tab = rgb(0x191e27).into();
    colors.tab_active = rgb(SURFACE).into();
    colors.tab_foreground = rgb(MUTED).into();
    colors.tab_active_foreground = rgb(TEXT).into();
    colors.title_bar = rgb(0x191e27).into();
    colors.title_bar_border = rgb(BORDER).into();
    colors.popover = rgb(SURFACE_RAISED).into();
    colors.popover_foreground = rgb(TEXT).into();
    colors.list = rgb(SURFACE).into();
    colors.list_hover = rgb(SURFACE_RAISED).into();
    colors.list_active = rgb(0x283449).into();
    colors.list_active_border = rgb(0x425675).into();
    colors.table = rgb(SURFACE).into();
    colors.table_head = rgb(SURFACE_RAISED).into();
    colors.table_head_foreground = rgb(MUTED).into();
    colors.table_even = rgb(0x1f2530).into();
    colors.table_hover = rgb(0x2a3443).into();
    colors.table_row_border = rgb(0x29313d).into();
    colors.scrollbar_thumb = rgb(0x465165).into();
    colors.scrollbar_thumb_hover = rgb(0x66758c).into();
}

pub fn empty_state(
    icon: IconName,
    title: impl Into<String>,
    detail: impl Into<String>,
    cx: &App,
) -> gpui::AnyElement {
    div()
        .size_full()
        .min_h_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .px_5()
        .child(
            div()
                .size_10()
                .rounded_lg()
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .size_5()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(title.into()),
        )
        .child(
            div()
                .text_xs()
                .text_center()
                .text_color(cx.theme().muted_foreground)
                .child(detail.into()),
        )
        .into_any_element()
}
