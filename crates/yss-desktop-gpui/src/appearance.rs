//! Shared desktop colours and presentation primitives; no business or layout state.
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Theme, ThemeMode};
use gpui_kit::{App, IntoElement, Window, div, prelude::*, px, rgb, rgba};

// Zed One Dark's editor, panel and window-chrome roles share one palette.
pub const CANVAS: u32 = 0x282c33;
pub const SURFACE: u32 = 0x2f343e;
pub const SURFACE_RAISED: u32 = 0x363c46;
pub const CHROME: u32 = 0x3b414d;
pub const BORDER: u32 = 0x363c46;
pub const BORDER_STRONG: u32 = 0x464b57;
pub const SELECTED: u32 = 0x454a56;
pub const TEXT: u32 = 0xdce0e5;
pub const MUTED: u32 = 0xa9afbc;
pub const BLUE: u32 = 0x74ade8;
pub const GREEN: u32 = 0xa1c181;
pub const AMBER: u32 = 0xdec184;
pub const RED: u32 = 0xd07277;

pub fn install(window: &mut Window, cx: &mut App) {
    Theme::change(ThemeMode::Dark, Some(window), cx);
    Theme::update(cx, |theme| {
        theme.font_size = px(14.);
        theme.mono_font_size = px(12.);
        theme.radius = px(4.);
        theme.radius_lg = px(6.);
        let colors = &mut theme.colors;
        colors.background = rgb(SURFACE).into();
        colors.foreground = rgb(TEXT).into();
        colors.border = rgb(BORDER).into();
        colors.input = rgb(BORDER_STRONG).into();
        colors.muted = rgb(SURFACE_RAISED).into();
        colors.muted_foreground = rgb(MUTED).into();
        colors.accent = rgb(SURFACE_RAISED).into();
        colors.accent_foreground = rgb(TEXT).into();
        colors.primary = rgb(BLUE).into();
        colors.primary_hover = rgb(0x85c1ff).into();
        colors.primary_active = rgb(0x5b92cd).into();
        colors.primary_foreground = rgb(CANVAS).into();
        colors.ring = rgb(BLUE).into();
        colors.caret = rgb(BLUE).into();
        colors.selection = rgba(0x74ade83d).into();
        colors.link = rgb(BLUE).into();
        colors.link_hover = rgb(0x85c1ff).into();
        colors.link_active = rgb(BLUE).into();
        colors.danger = rgb(RED).into();
        colors.danger_hover = rgb(0xea858b).into();
        colors.danger_active = rgb(0xb85e65).into();
        colors.danger_foreground = rgb(CANVAS).into();
        colors.success = rgb(GREEN).into();
        colors.success_hover = rgb(0xaad581).into();
        colors.success_active = rgb(0x7f9f61).into();
        colors.success_foreground = rgb(CANVAS).into();
        colors.warning = rgb(AMBER).into();
        colors.warning_hover = rgb(0xffd885).into();
        colors.warning_active = rgb(0xb8985b).into();
        colors.warning_foreground = rgb(CANVAS).into();
        colors.info = rgb(BLUE).into();
        colors.info_hover = colors.primary_hover;
        colors.info_active = colors.primary_active;
        colors.info_foreground = rgb(CANVAS).into();
        colors.button = rgb(SURFACE_RAISED).into();
        colors.button_foreground = rgb(TEXT).into();
        colors.button_hover = rgb(SELECTED).into();
        colors.button_active = rgb(SELECTED).into();
        colors.group_box = rgb(SURFACE).into();
        colors.group_box_foreground = rgb(TEXT).into();
        colors.description_list_label = rgb(SURFACE).into();
        colors.description_list_label_foreground = rgb(MUTED).into();
        colors.secondary = rgb(SURFACE_RAISED).into();
        colors.secondary_foreground = rgb(TEXT).into();
        colors.secondary_hover = rgb(SELECTED).into();
        colors.secondary_active = rgb(SELECTED).into();
        // Button variants retain their own resolved tokens after the base roles change.
        colors.button_primary = colors.primary;
        colors.button_primary_hover = colors.primary_hover;
        colors.button_primary_active = colors.primary_active;
        colors.button_primary_foreground = colors.primary_foreground;
        colors.button_secondary = colors.secondary;
        colors.button_secondary_hover = colors.secondary_hover;
        colors.button_secondary_active = colors.secondary_active;
        colors.button_secondary_foreground = colors.secondary_foreground;
        colors.button_danger = colors.danger;
        colors.button_danger_hover = colors.danger_hover;
        colors.button_danger_active = colors.danger_active;
        colors.button_danger_foreground = colors.danger_foreground;
        colors.button_success = colors.success;
        colors.button_success_hover = colors.success_hover;
        colors.button_success_active = colors.success_active;
        colors.button_success_foreground = colors.success_foreground;
        colors.button_warning = colors.warning;
        colors.button_warning_hover = colors.warning_hover;
        colors.button_warning_active = colors.warning_active;
        colors.button_warning_foreground = colors.warning_foreground;
        colors.button_info = colors.info;
        colors.button_info_hover = colors.info_hover;
        colors.button_info_active = colors.info_active;
        colors.button_info_foreground = colors.info_foreground;
        colors.sidebar = rgb(SURFACE).into();
        colors.sidebar_foreground = rgb(TEXT).into();
        colors.sidebar_border = rgb(BORDER).into();
        colors.sidebar_accent = rgb(SELECTED).into();
        colors.sidebar_accent_foreground = rgb(TEXT).into();
        colors.sidebar_primary = rgb(BLUE).into();
        colors.sidebar_primary_foreground = rgb(CANVAS).into();
        colors.tab_bar = rgb(SURFACE).into();
        colors.tab = rgb(SURFACE).into();
        colors.tab_active = rgb(CANVAS).into();
        colors.tab_foreground = rgb(MUTED).into();
        colors.tab_active_foreground = rgb(TEXT).into();
        colors.title_bar = rgb(CHROME).into();
        colors.title_bar_border = rgb(BORDER).into();
        colors.status_bar = rgb(CHROME).into();
        colors.status_bar_border = rgb(BORDER).into();
        colors.window_border = rgb(BORDER).into();
        colors.overlay = rgba(0x282c33cc).into();
        colors.popover = rgb(SURFACE).into();
        colors.popover_foreground = rgb(TEXT).into();
        colors.list = rgb(SURFACE).into();
        colors.list_hover = rgb(SURFACE_RAISED).into();
        colors.list_active = rgb(SELECTED).into();
        colors.list_active_border = rgb(BORDER_STRONG).into();
        colors.table = rgb(CANVAS).into();
        colors.table_head = rgb(SURFACE).into();
        colors.table_head_foreground = rgb(MUTED).into();
        colors.table_even = rgb(0x2b3039).into();
        colors.table_hover = rgb(SURFACE_RAISED).into();
        colors.table_active = rgb(SELECTED).into();
        colors.table_active_border = rgb(BLUE).into();
        colors.table_row_border = rgb(BORDER).into();
        colors.table_foot = rgb(SURFACE).into();
        colors.table_foot_foreground = rgb(MUTED).into();
        colors.chart_1 = rgb(BLUE).into();
        colors.chart_2 = rgb(GREEN).into();
        colors.chart_3 = rgb(AMBER).into();
        colors.chart_4 = rgb(0x6eb4bf).into();
        colors.chart_5 = rgb(0xb477cf).into();
        colors.chart_grid = rgb(BORDER).into();
        colors.drag_border = rgb(BLUE).into();
        colors.drop_target = rgba(0x74ade826).into();
        colors.scrollbar = rgba(0x00000000).into();
        colors.scrollbar_thumb = rgba(0xc8ccd44c).into();
        colors.scrollbar_thumb_hover = rgba(0xc8ccd480).into();
    });
}

pub fn empty_state(
    icon: IconName,
    title: impl Into<String>,
    detail: impl Into<String>,
    cx: &App,
) -> gpui_kit::AnyElement {
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
            div().size_8().flex().items_center().justify_center().child(
                Icon::new(icon)
                    .size_5()
                    .text_color(cx.theme().muted_foreground),
            ),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
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
