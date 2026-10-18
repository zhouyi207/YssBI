use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    label::Label,
    setting::{RenderOptions, SettingField, SettingItem},
};
use gpui_kit::{
    AnyElement, App, Axis, Context, IntoElement, SharedString, Window, div, prelude::*,
};
use std::rc::Rc;
use yss_ui::text::t;

pub(in crate::settings) type Reset = Rc<dyn Fn(&mut App)>;

/// A setting row whose reset command sits beside its title and appears only while the value
/// differs from its default. `render` returns that reset command, if any, and the control.
/// The layout mirrors the component's own setting item so both kinds of rows share one spine.
pub(in crate::settings) fn resettable_item(
    title: SharedString,
    description: Option<SharedString>,
    render: impl Fn(&RenderOptions, &mut Window, &mut App) -> (Option<Reset>, AnyElement) + 'static,
) -> SettingItem {
    // Custom rows are matched only by keywords, so the visible text must be among them.
    let keywords = std::iter::once(title.clone())
        .chain(description.clone())
        .collect::<Vec<_>>();
    SettingItem::render(move |options, window, cx| {
        let (reset, control) = render(options, window, cx);
        let horizontal = options.layout() == Axis::Horizontal;
        div()
            .w_full()
            .flex()
            .gap_3()
            .map(|row| {
                if horizontal {
                    row.items_center().justify_between()
                } else {
                    row.flex_col()
                }
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .map(|text| {
                        if horizontal {
                            text.flex_1().max_w_3_5()
                        } else {
                            text.w_full()
                        }
                    })
                    .child(
                        div()
                            .h_5()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(Label::new(title.clone()).text_sm())
                            .children(reset.map(|reset| {
                                Button::new("reset-setting")
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Undo2)
                                    .accessibility_label(t("preferences.reset"))
                                    .tooltip(t("preferences.reset"))
                                    .disabled(options.is_disabled())
                                    .on_click(move |_, _, cx| reset(cx))
                            })),
                    )
                    .children(description.clone().map(|description| {
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(description)
                    })),
            )
            .child(control)
    })
    .keywords(keywords)
}

impl crate::settings::SettingsPanel {
    pub(in crate::settings) fn render_field<E: IntoElement>(
        &self,
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        render: impl Fn(&Self, &mut Context<Self>) -> E + 'static,
        cx: &Context<Self>,
    ) -> SettingItem {
        let owner = cx.weak_entity();
        SettingItem::new(
            label,
            SettingField::render(move |options, _, cx| {
                let element = match owner.update(cx, |view, cx| render(view, cx).into_any_element())
                {
                    Ok(element) => element,
                    Err(_) => return div().into_any_element(),
                };
                div()
                    .w_64()
                    .max_w_full()
                    .when(options.layout() == Axis::Vertical, |field| field.w_full())
                    .child(element)
                    .into_any_element()
            }),
        )
        .description(description.into())
    }
}
