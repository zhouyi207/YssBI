//! Modal content and actions; window ownership lives in the host.
use gpui::{AnyElement, App, ClickEvent, FocusHandle, IntoElement, SharedString, Window};
use gpui_component::button::ButtonVariant;
use std::rc::Rc;

pub(super) type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool>;
pub(super) type Body = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;
type Closed = Box<dyn FnOnce(&mut App)>;

pub(crate) struct ModalContent {
    pub(super) body: Body,
    pub(super) footer: Option<Body>,
    pub(super) confirm_text: Option<SharedString>,
    pub(super) confirm_variant: ButtonVariant,
    pub(super) cancel_text: Option<SharedString>,
    pub(super) confirm: Handler,
    pub(super) cancel: Handler,
    pub(super) closed: Option<Closed>,
    pub(super) focus: Option<FocusHandle>,
}

impl ModalContent {
    pub(crate) fn new<E: IntoElement>(body: impl Fn(&mut Window, &mut App) -> E + 'static) -> Self {
        Self {
            body: Box::new(move |window, cx| body(window, cx).into_any_element()),
            footer: None,
            confirm_text: Some(crate::text::t("common.confirm").into()),
            confirm_variant: ButtonVariant::Primary,
            cancel_text: None,
            confirm: Rc::new(|_, _, _| true),
            cancel: Rc::new(|_, _, _| true),
            closed: None,
            focus: None,
        }
    }

    pub(crate) fn footer<E: IntoElement>(
        mut self,
        footer: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.footer = Some(Box::new(move |window, cx| {
            footer(window, cx).into_any_element()
        }));
        self
    }

    /// Confirm runs in the originating window so asynchronous work survives modal closure.
    pub(crate) fn confirm(
        mut self,
        label: impl Into<SharedString>,
        action: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.confirm_text = Some(label.into());
        self.confirm = Rc::new(action);
        self
    }

    pub(crate) fn danger(mut self) -> Self {
        self.confirm_variant = ButtonVariant::Danger;
        self
    }

    pub(crate) fn cancel(mut self, label: impl Into<SharedString>) -> Self {
        self.cancel_text = Some(label.into());
        self
    }

    /// Cancel runs in the modal, including Escape and the native close request.
    pub(crate) fn on_cancel(
        mut self,
        action: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.cancel = Rc::new(action);
        self
    }

    pub(crate) fn on_closed(mut self, action: impl FnOnce(&mut App) + 'static) -> Self {
        self.closed = Some(Box::new(action));
        self
    }

    pub(crate) fn without_buttons(mut self) -> Self {
        self.confirm_text = None;
        self.cancel_text = None;
        self
    }

    pub(crate) fn focus(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }
}
