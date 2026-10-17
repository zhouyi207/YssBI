use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Window, div,
    prelude::*,
};

pub struct NameForm {
    input: Entity<InputState>,
    busy: bool,
    notifications: crate::notifications::Notifications,
}

impl NameForm {
    pub fn new(initial: String, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
        cx.new(|_| Self {
            input,
            busy: false,
            notifications: crate::notifications::Notifications::new(window),
        })
    }

    pub fn value(&self, cx: &App) -> Option<String> {
        if self.busy {
            return None;
        }
        let value = self.input.read(cx).value().trim().to_owned();
        (!value.is_empty()).then_some(value)
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn submitting(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        cx.notify();
    }

    pub fn finish(&mut self, error: Option<&'static str>, cx: &mut Context<Self>) {
        self.busy = false;
        if let Some(error) = error {
            self.notifications.error(crate::text::translate(error), cx);
        }
        cx.notify();
    }

    pub fn fail(form: Option<&Entity<Self>>, error: &'static str, cx: &mut App) {
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
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Input::new(&self.input).disabled(self.busy))
    }
}
