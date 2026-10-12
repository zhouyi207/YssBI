use gpui_kit::accesskit::Role;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon};
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, IntoElement, RenderOnce, SharedString, Window, div,
    prelude::*, relative, rems,
};

/// One editable property: a label lane with trailing actions, help, the control, then messages.
/// Labels take the secondary color so values carry the emphasis; CJK fonts often lack the
/// intermediate weights that would otherwise separate labels from section titles.
#[derive(IntoElement)]
pub struct PropertyField {
    label: SharedString,
    description: Option<SharedString>,
    actions: Vec<AnyElement>,
    control: Option<AnyElement>,
    messages: Vec<AnyElement>,
    inline: bool,
}

impl PropertyField {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            description: None,
            actions: Vec::new(),
            control: None,
            messages: Vec::new(),
            inline: false,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// Places a compact control, such as a switch, at the end of the label row.
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    pub fn message(mut self, message: impl IntoElement) -> Self {
        self.messages.push(message.into_any_element());
        self
    }
}

impl RenderOnce for PropertyField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = div()
            .flex_1()
            .min_w_0()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(self.label);
        let actions = div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_0p5()
            .children(self.actions);
        let row = div()
            .min_w_0()
            .flex()
            .items_center()
            .gap_1()
            .child(label)
            .child(actions);
        let (row, control) = if self.inline {
            (row.gap_2().children(self.control), None)
        } else {
            (row, self.control)
        };
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(row)
            .when_some(self.description, |field, description| {
                field.child(super::hint(description, cx))
            })
            .children(control)
            .children(self.messages)
    }
}

/// Severity of an inline message; its icon carries the meaning alongside the color.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Information,
}

/// A status line attached to a field, row or section.
#[derive(IntoElement)]
pub struct InlineMessage {
    id: Option<ElementId>,
    severity: Severity,
    subject: Option<SharedString>,
    text: SharedString,
}

impl InlineMessage {
    pub fn new(severity: Severity, text: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            severity,
            subject: None,
            text: text.into(),
        }
    }

    pub fn error(text: impl Into<SharedString>) -> Self {
        Self::new(Severity::Error, text)
    }

    pub fn warning(text: impl Into<SharedString>) -> Self {
        Self::new(Severity::Warning, text)
    }

    /// A stable identity lets assistive technology announce errors as alerts.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// The object the message is about, such as a parameter or a port.
    pub fn subject(mut self, subject: impl Into<SharedString>) -> Self {
        self.subject = Some(subject.into());
        self
    }
}

impl RenderOnce for InlineMessage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (icon, icon_color, text_color) = match self.severity {
            Severity::Error => (IconName::CircleX, theme.danger, theme.danger),
            Severity::Warning => (IconName::TriangleAlert, theme.warning, theme.foreground),
            Severity::Information => (
                IconName::Info,
                theme.muted_foreground,
                theme.muted_foreground,
            ),
        };
        // The icon slot and the text line resolve from the same rem height, so the icon stays
        // centered on the first line at every zoom level.
        let content = div()
            .min_w_0()
            .flex()
            .items_start()
            .gap_1p5()
            .text_xs()
            .line_height(relative(1.5))
            .child(
                div()
                    .flex_shrink_0()
                    .h(rems(1.125))
                    .flex()
                    .items_center()
                    .child(Icon::new(icon).size_3().text_color(icon_color)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(text_color)
                    .when_some(self.subject, |text, subject| {
                        text.child(
                            div()
                                .text_color(theme.foreground)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(subject),
                        )
                    })
                    .child(self.text),
            );
        match self.id {
            Some(id) => content
                .id(id)
                .when(self.severity == Severity::Error, |content| {
                    content.role(Role::Alert)
                })
                .into_any_element(),
            None => content.into_any_element(),
        }
    }
}
