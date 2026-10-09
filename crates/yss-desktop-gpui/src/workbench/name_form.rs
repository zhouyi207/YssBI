//! Shared name input owns only its draft, submission state and localized error.
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Window, div,
    prelude::*,
};
use gpui_component::{
    ActiveTheme,
    input::{Input, InputEvent, InputState},
};

pub(super) struct NameForm {
    input: Entity<InputState>,
    busy: bool,
    error: Option<&'static str>,
    _subscription: gpui::Subscription,
}

impl NameForm {
    pub(super) fn new(initial: String, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
        cx.new(|cx| {
            let subscription = cx.subscribe(&input, |form: &mut Self, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    form.error = None;
                    cx.notify();
                }
            });
            Self {
                input,
                busy: false,
                error: None,
                _subscription: subscription,
            }
        })
    }

    pub(super) fn value(&self, cx: &App) -> Option<String> {
        if self.busy {
            return None;
        }
        let value = self.input.read(cx).value().trim().to_owned();
        (!value.is_empty()).then_some(value)
    }

    pub(super) fn busy(&self) -> bool {
        self.busy
    }

    pub(super) fn submitting(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
    }

    pub(super) fn finish(&mut self, error: Option<&'static str>, cx: &mut Context<Self>) {
        self.busy = false;
        self.error = error;
        cx.notify();
    }

    pub(super) fn fail(form: Option<&Entity<Self>>, error: &'static str, cx: &mut App) {
        if let Some(form) = form {
            form.update(cx, |form, cx| form.finish(Some(error), cx));
        }
    }
}

impl Focusable for NameForm {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for NameForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Input::new(&self.input).disabled(self.busy))
            .when_some(self.error, |body, error| {
                body.child(
                    div()
                        .id("name-form-error")
                        .role(gpui::accesskit::Role::Alert)
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(crate::text::translate(error)),
                )
            })
    }
}
