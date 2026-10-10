//! Stateless controls shared by native graph property and node editors.
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{App, ElementId, div, prelude::*};

pub(super) fn hint(text: impl Into<String>, cx: &App) -> gpui_kit::Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub(super) fn apply(id: impl Into<ElementId>, disabled: bool) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .icon(IconName::Check)
        .tooltip(crate::text::t("native.workbench.applyChanges"))
        .disabled(disabled)
}

pub(crate) fn number(text: &str) -> Result<serde_json::Value, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(crate::text::t("notifications.parameter.enterNumber").into());
    }
    // Integer drafts stay exact in Rust; never fall back to a rounded float on overflow.
    if !text.contains(['.', 'e', 'E']) {
        if let Ok(value) = text.parse::<i64>() {
            return Ok(value.into());
        }
        return text
            .parse::<u64>()
            .map(serde_json::Value::from)
            .map_err(|_| crate::text::t("native.workbench.integerOutOfRange").into());
    }
    text.parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(serde_json::Value::Number)
        .ok_or_else(|| crate::text::t("native.workbench.finiteNumberRequired").into())
}
