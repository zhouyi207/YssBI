//! Application theme policy. Window observers never own or mutate saved preferences.
use std::collections::HashMap;

use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Theme, ThemeMode};
use gpui_kit::{
    App, Global, Hsla, IntoElement, SharedString, Subscription, Window, WindowAppearance, WindowId,
    div, prelude::*, px, rgb,
};
use yss_settings::AppearanceSettings;

struct Appearance {
    system: WindowAppearance,
    default_ui_font: SharedString,
    default_mono_font: SharedString,
    applied: Option<(AppearanceSettings, ThemeMode)>,
    windows: HashMap<WindowId, Subscription>,
    _closed: Subscription,
}

impl Global for Appearance {}

fn init(cx: &mut App) {
    if cx.has_global::<Appearance>() {
        return;
    }
    let closed = cx.on_window_closed(|cx, id| {
        cx.global_mut::<Appearance>().windows.remove(&id);
    });
    cx.set_global(Appearance {
        system: cx.window_appearance(),
        default_ui_font: cx.theme().font_family.clone(),
        default_mono_font: cx.theme().mono_font_family.clone(),
        applied: None,
        windows: HashMap::new(),
        _closed: closed,
    });
}

/// Applies saved appearance without replacing views, inputs, focus, or drafts.
pub fn apply(cx: &mut App) {
    init(cx);
    #[cfg(target_os = "macos")]
    {
        let preference = crate::preferences::current(cx).appearance.mode;
        if cx
            .global::<Appearance>()
            .applied
            .as_ref()
            .is_none_or(|(previous, _)| previous.mode != preference)
        {
            cx.set_window_appearance(match preference {
                yss_settings::ThemeMode::System => None,
                yss_settings::ThemeMode::Light => Some(WindowAppearance::Light),
                yss_settings::ThemeMode::Dark => Some(WindowAppearance::Dark),
            });
            if preference == yss_settings::ThemeMode::System {
                // Clearing the native override restores the actual OS appearance.
                let system = cx.window_appearance();
                cx.global_mut::<Appearance>().system = system;
            }
        }
    }
    let settings = &crate::preferences::current(cx).appearance;
    let runtime = cx.global::<Appearance>();
    let mode = match settings.mode {
        yss_settings::ThemeMode::System => ThemeMode::from(runtime.system),
        yss_settings::ThemeMode::Light => ThemeMode::Light,
        yss_settings::ThemeMode::Dark => ThemeMode::Dark,
    };
    if runtime
        .applied
        .as_ref()
        .is_some_and(|(previous, previous_mode)| previous == settings && *previous_mode == mode)
    {
        return;
    }
    let settings = settings.clone();
    let ui_font = settings.ui_font_family.as_ref().map_or_else(
        || runtime.default_ui_font.clone(),
        |family| SharedString::from(family.clone()),
    );
    let mono_font = settings.mono_font_family.as_ref().map_or_else(
        || runtime.default_mono_font.clone(),
        |family| SharedString::from(family.clone()),
    );
    // Load a complete light/dark theme before setting typography: changing mode
    // reloads the component theme, including its default font values.
    if cx.theme().mode != mode {
        Theme::change(mode, None, cx);
    }
    Theme::update(cx, |theme| {
        theme.font_family = ui_font;
        theme.font_size = px(settings.ui_font_size);
        theme.mono_font_family = mono_font;
        theme.mono_font_size = px(settings.mono_font_size);
    });
    cx.global_mut::<Appearance>().applied = Some((settings, mode));
}

pub fn install(window: &mut Window, cx: &mut App) {
    init(cx);
    let id = window.window_handle().window_id();
    if !cx.global::<Appearance>().windows.contains_key(&id) {
        // Retain one subscription per window; closing a window drops it. Use
        // the window's appearance because Linux's app-level value can lag.
        let observer = window.observe_window_appearance(|window, cx| {
            cx.global_mut::<Appearance>().system = window.appearance();
            if crate::preferences::current(cx).appearance.mode == yss_settings::ThemeMode::System {
                apply(cx);
            }
        });
        let runtime = cx.global_mut::<Appearance>();
        runtime.system = window.appearance();
        runtime.windows.insert(id, observer);
    }
    apply(cx);
    // Root also sets this on every frame; install covers measurements performed
    // while constructing a new window, before its first Root render.
    window.set_rem_size(cx.theme().font_size);
}

/// Adapts domain-supplied category colors to the active canvas contrast.
pub(crate) fn data_color(color: u32, cx: &App) -> Hsla {
    let mut color: Hsla = rgb(color).into();
    if !cx.theme().is_dark() {
        color.l = color.l.min(0.34);
    }
    color
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
