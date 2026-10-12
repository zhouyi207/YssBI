//! Uncommitted text fields use the component's single-line or multiline control.
use gpui_kit::component::{
    Sizable,
    input::{Input, InputState, Textarea, TextareaState},
};
use gpui_kit::{
    AnyElement, App, AppContext, Entity, IntoElement, SharedString, Window, prelude::*,
};

pub enum TextField {
    Single(Entity<InputState>),
    Multiline(Entity<TextareaState>),
}

impl TextField {
    pub fn new(value: String, multiline: bool, window: &mut Window, cx: &mut App) -> Self {
        if multiline {
            Self::Multiline(cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(3, 10)
                    .default_value(value)
            }))
        } else {
            Self::Single(cx.new(|cx| InputState::new(window, cx).default_value(value)))
        }
    }

    pub fn value(&self, cx: &App) -> SharedString {
        match self {
            Self::Single(input) => input.read(cx).value(),
            Self::Multiline(input) => input.read(cx).value(),
        }
    }

    pub fn set_value(&self, value: String, window: &mut Window, cx: &mut App) {
        match self {
            Self::Single(input) => input.update(cx, |input, cx| input.set_value(value, window, cx)),
            Self::Multiline(input) => {
                input.update(cx, |input, cx| input.set_value(value, window, cx))
            }
        }
    }

    pub fn render(&self, disabled: bool) -> AnyElement {
        match self {
            Self::Single(input) => Input::new(input)
                .small()
                .flex_1()
                .min_w_0()
                .disabled(disabled)
                .into_any_element(),
            Self::Multiline(input) => Textarea::new(input)
                .small()
                .flex_1()
                .min_w_0()
                .disabled(disabled)
                .into_any_element(),
        }
    }
}

/// Parses a numeric draft into the JSON value submitted for a parameter or literal.
pub fn number(text: &str) -> Result<serde_json::Value, String> {
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
