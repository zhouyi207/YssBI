//! Inspector presentation for the Details panel. Owners keep drafts, disclosure and paging state
//! and pass current values and intents; these elements own the shared spine, rhythm and hierarchy.
mod field;
mod pager;
mod section;
#[cfg(test)]
mod tests;

pub use field::{InlineMessage, PropertyField, Severity};
pub use pager::Pager;
pub use section::{InspectorHeader, InspectorSection};

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{
    App, ClickEvent, Context, Div, ElementId, IntoElement, Rems, SharedString, WeakEntity, Window,
    div, prelude::*, rems,
};

/// Where content under a [`disclosure`] row starts: past its chevron, under its label.
pub const DISCLOSURE_INDENT: Rems = rems(1.125);

/// Supporting text: field help, empty collections and status lines.
pub fn hint(text: impl Into<String>, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

/// A full-width status line between sections, such as a loading notice or a command error.
pub fn status_band(content: impl IntoElement, cx: &App) -> Div {
    div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(content)
}

/// One item of a collection whose fields edit and apply together, such as a constant.
pub fn record(cx: &App) -> Div {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .p_2()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
}

/// A labeled part of a composite editor, subordinate to its field's own label.
pub fn sub_field(label: impl Into<String>, control: impl IntoElement, cx: &App) -> Div {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(hint(label, cx))
        .child(control)
}

/// Read-only label and value; values share the trailing column so numbers align across rows.
pub fn property_row(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &App,
) -> Div {
    div()
        .min_w_0()
        .flex()
        .items_start()
        .gap_3()
        .text_xs()
        .child(
            div()
                .flex_shrink_0()
                .text_color(cx.theme().muted_foreground)
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_right()
                .truncate()
                .child(value.into()),
        )
}

/// Stacks read-only rows at one table rhythm, tighter than the gap between fields.
pub fn property_list(rows: impl IntoIterator<Item = impl IntoElement>) -> Div {
    div().min_w_0().flex().flex_col().gap_0p5().children(rows)
}

/// Select-style trigger for an owner-built dropdown menu: current choice leading, caret trailing.
pub fn choice_trigger(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .small()
        .w_full()
        .label(label)
        .dropdown_caret(true)
}

/// Commit for a single-line draft that applies explicitly; it matches a small input's height.
pub fn apply_button(id: impl Into<ElementId>, disabled: bool) -> Button {
    let label = crate::text::translate("native.workbench.applyChanges");
    Button::new(id)
        .small()
        .ghost()
        .icon(IconName::Check)
        .tooltip(label.clone())
        .accessibility_label(label)
        .disabled(disabled)
}

/// Labeled commit for a composite draft such as a list, mapping, column set or predicate.
pub fn commit_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    disabled: bool,
) -> Button {
    Button::new(id)
        .small()
        .outline()
        .icon(IconName::Check)
        .label(label)
        .disabled(disabled)
}

/// Appends an item to the collection above it. The negative margin puts the icon on the spine.
pub fn add_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .ml_neg_2()
        .icon(IconName::Plus)
        .label(label)
}

/// Compact icon command for a field, row or header lane; its tooltip is also its accessible name.
pub fn action_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    crate::tool_panel::button(id)
        .icon(icon)
        .tooltip(label.clone())
        .accessibility_label(label)
}

/// Trigger for secondary row commands; attach the menu with a trailing anchor.
pub fn overflow_button(id: impl Into<ElementId>) -> Button {
    action_button(
        id,
        IconName::Ellipsis,
        crate::text::translate("native.inspector.moreActions"),
    )
}

/// Binds a menu item to its retained owner; a closed owner has nothing left to update.
pub fn owner_command<T: 'static>(
    owner: WeakEntity<T>,
    command: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        if let Err(error) = owner.update(cx, |owner, cx| command(owner, window, cx)) {
            tracing::debug!("inspector command target closed: {error}");
        }
    }
}

/// Nested disclosure row for a flex column inside a section. It stretches across the column and
/// its negative margins let the hover surface bleed past the spine while the chevron stays on it.
/// The chevron slot and gap add up to [`DISCLOSURE_INDENT`].
pub fn disclosure(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    open: bool,
    cx: &App,
) -> Button {
    let label = label.into();
    Button::new(id)
        .small()
        .ghost()
        .mx_neg_2()
        .accessibility_label(label.clone())
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_1p5()
                .child(
                    Icon::new(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .xsmall()
                    .text_color(cx.theme().muted_foreground),
                )
                .child(div().flex_1().min_w_0().truncate().child(label))
                .when_some(detail, |row, detail| {
                    row.child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(detail),
                    )
                }),
        )
}
