use gpui_kit::{App, Global};
use std::borrow::Cow;
use std::sync::Arc;
use yss_harness_contract::{HarnessMode, ReasoningEffort};
use yss_settings::UserSettings;

pub struct Preferences {
    settings: Arc<UserSettings>,
}
impl Global for Preferences {}

pub fn init(cx: &mut App) {
    cx.set_global(Preferences {
        settings: Arc::new(UserSettings::default()),
    });
}

pub fn current(cx: &App) -> &UserSettings {
    &cx.global::<Preferences>().settings
}

/// Name of an assistant mode, shared by its preference and the conversation picker.
pub fn mode_label(mode: HarnessMode) -> Cow<'static, str> {
    crate::text::t(match mode {
        HarnessMode::Ask => "panel.assistantModes.ask",
        HarnessMode::Write => "panel.assistantModes.write",
    })
}

/// Name of a reasoning effort, shared by its preference, model settings and the
/// conversation picker.
pub fn effort_label(effort: ReasoningEffort) -> Cow<'static, str> {
    crate::text::t(match effort {
        ReasoningEffort::Low => "panel.assistantEffort.low",
        ReasoningEffort::Medium => "panel.assistantEffort.medium",
        ReasoningEffort::High => "panel.assistantEffort.high",
    })
}

pub fn publish(settings: Arc<UserSettings>, cx: &mut App) -> anyhow::Result<()> {
    crate::keymap::validate_bindings(&settings.keybindings, cx)?;
    crate::text::set_locale(settings.language.locale());
    cx.set_global(Preferences { settings });
    crate::appearance::apply(cx);
    crate::keymap::apply_bindings(&current(cx).keybindings.clone(), cx)?;
    cx.refresh_windows();
    Ok(())
}
