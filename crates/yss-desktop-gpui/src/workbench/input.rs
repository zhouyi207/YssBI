//! Uncommitted text fields use the component's single-line or multiline control.
use gpui::{AnyElement, App, AppContext, Entity, IntoElement, SharedString, Window, prelude::*};
use gpui_component::{
    Disableable, Sizable,
    input::{Input, InputState, Textarea, TextareaState},
};

pub(super) enum TextField {
    Single(Entity<InputState>),
    Multiline(Entity<TextareaState>),
}

impl TextField {
    pub fn new(value: String, multiline: bool, window: &mut Window, cx: &mut App) -> Self {
        if multiline {
            Self::Multiline(cx.new(|cx| {
                TextareaState::new(window, cx).auto_grow(3, 10).default_value(value)
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
            Self::Multiline(input) => input.update(cx, |input, cx| input.set_value(value, window, cx)),
        }
    }

    pub fn render(&self, disabled: bool) -> AnyElement {
        match self {
            Self::Single(input) => Input::new(input).small().flex_1().min_w_0().disabled(disabled).into_any_element(),
            Self::Multiline(input) => Textarea::new(input).small().flex_1().min_w_0().disabled(disabled).into_any_element(),
        }
    }
}
