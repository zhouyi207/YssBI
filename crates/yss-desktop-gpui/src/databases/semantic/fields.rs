//! Semantic form presentation reuses native inputs and renders only the current mapping page.
use super::{SemanticDialog, inputs::PAGE_VALUES};
use crate::text::translate as t;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{Context, Div, IntoElement, Render, Window, div, prelude::*};
use yss_data_contract::{ConversionDomain, NumericConstraints, SemanticType};

impl SemanticDialog {
    fn numeric_fields(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                Checkbox::new("numeric-integer")
                    .label(t("detail.data.integer"))
                    .checked(
                        self.draft
                            .numeric
                            .as_ref()
                            .is_some_and(|value| value.integer),
                    )
                    .disabled(!self.editable())
                    .on_click(cx.listener(|view, checked: &bool, _, cx| {
                        view.draft
                            .numeric
                            .get_or_insert_with(NumericConstraints::default)
                            .integer = *checked;
                        view.clear_feedback();
                        cx.notify();
                    })),
            )
            .child(div().text_xs().child(t("detail.data.minimum")))
            .child(Input::new(&self.minimum).disabled(!self.editable()))
            .child(div().text_xs().child(t("detail.data.maximum")))
            .child(Input::new(&self.maximum).disabled(!self.editable()))
    }

    fn mapping_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        self.mount_inputs(window, cx);
        let busy = !self.editable();
        let ordinal = self.draft.kind == SemanticType::Ordinal;
        let count = self.draft.values.len();
        let mut view = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t("detail.data.initialMappingHint")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(match self.draft.kind {
                        SemanticType::Ordinal => "detail.data.ordinalHint",
                        SemanticType::Binary => "detail.data.binaryHint",
                        _ => "detail.data.categoryHint",
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .text_xs()
                    .child(div().flex_1().child(t("detail.fields.value")))
                    .child(div().flex_1().child(t("detail.data.labelPlaceholder"))),
            );
        for index in self.page_range() {
            let input = &self.inputs[&index];
            let row = div()
                .flex()
                .items_center()
                .gap_1()
                .when(ordinal, |view| {
                    view.child(div().w_6().text_xs().child((index + 1).to_string()))
                })
                .child(
                    Input::new(&input.value)
                        .small()
                        .flex_1()
                        .min_w_0()
                        .disabled(busy),
                )
                .child(
                    Input::new(&input.label)
                        .small()
                        .flex_1()
                        .min_w_0()
                        .disabled(busy),
                )
                .when(ordinal, |view| {
                    view.child(
                        Button::new(("semantic-up", index))
                            .small()
                            .ghost()
                            .icon(IconName::ArrowUp)
                            .tooltip(crate::text::format(
                                "detail.parameterEditor.moveColumnUp",
                                &[("column", input.label.read(cx).value().to_string())],
                            ))
                            .disabled(busy || index == 0)
                            .on_click(
                                cx.listener(move |view, _, _, cx| view.reorder(index, -1, cx)),
                            ),
                    )
                    .child(
                        Button::new(("semantic-down", index))
                            .small()
                            .ghost()
                            .icon(IconName::ArrowDown)
                            .tooltip(crate::text::format(
                                "detail.parameterEditor.moveColumnDown",
                                &[("column", input.label.read(cx).value().to_string())],
                            ))
                            .disabled(busy || index + 1 == count)
                            .on_click(
                                cx.listener(move |view, _, _, cx| view.reorder(index, 1, cx)),
                            ),
                    )
                })
                .child(
                    Button::new(("semantic-remove", index))
                        .small()
                        .ghost()
                        .icon(IconName::X)
                        .tooltip(crate::text::format(
                            "detail.data.removeValue",
                            &[("index", (index + 1).to_string())],
                        ))
                        .disabled(busy)
                        .on_click(cx.listener(move |view, _, _, cx| view.remove_value(index, cx))),
                );
            view = view.child(row);
        }
        if count > PAGE_VALUES {
            view = view.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        Button::new("semantic-previous")
                            .small()
                            .ghost()
                            .icon(IconName::ChevronLeft)
                            .tooltip(t("conversion.previousValues"))
                            .disabled(busy || self.page == 0)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.show_page(view.page.saturating_sub(1), cx)
                            })),
                    )
                    .child(div().text_xs().child(format!(
                        "{} / {}",
                        self.page + 1,
                        count.div_ceil(PAGE_VALUES)
                    )))
                    .child(
                        Button::new("semantic-next")
                            .small()
                            .ghost()
                            .icon(IconName::ChevronRight)
                            .tooltip(t("conversion.nextValues"))
                            .disabled(busy || (self.page + 1) * PAGE_VALUES >= count)
                            .on_click(
                                cx.listener(|view, _, _, cx| view.show_page(view.page + 1, cx)),
                            ),
                    ),
            );
        }
        view.child(
            Button::new("semantic-add")
                .small()
                .outline()
                .icon(IconName::Plus)
                .label(t("detail.data.addValue"))
                .disabled(
                    busy || count >= ConversionDomain::MAX_VALUES
                        || (self.draft.kind == SemanticType::Binary && count >= 2),
                )
                .on_click(cx.listener(|view, _, _, cx| view.add_value(cx))),
        )
    }

    fn positive_field(&self, cx: &mut Context<Self>) -> Div {
        let choices = self
            .draft
            .values
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let (value, label) = self
                    .inputs
                    .get(&index)
                    .map(|input| {
                        (
                            input.value.read(cx).value().to_string(),
                            input.label.read(cx).value().to_string(),
                        )
                    })
                    .unwrap_or_else(|| (entry.value.clone(), entry.label.clone()));
                let caption = if !label.is_empty() {
                    label
                } else if !value.is_empty() {
                    value.clone()
                } else {
                    (index + 1).to_string()
                };
                (value, caption)
            })
            .collect::<Vec<_>>();
        let label = self
            .draft
            .positive_value
            .as_ref()
            .and_then(|value| {
                self.draft
                    .values
                    .iter()
                    .position(|entry| &entry.value == value)
            })
            .map(|index| choices[index].1.clone())
            .unwrap_or_else(|| t("detail.data.noPositive"));
        let owner = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_xs().child(t("detail.data.positive")))
            .child(
                Button::new("semantic-positive")
                    .small()
                    .outline()
                    .label(label)
                    .disabled(!self.editable())
                    .dropdown_menu(move |mut menu, _, _| {
                        for (value, label) in std::iter::once((None, t("detail.data.noPositive")))
                            .chain(
                                choices
                                    .iter()
                                    .map(|(value, label)| (Some(value.clone()), label.clone())),
                            )
                        {
                            let owner = owner.clone();
                            menu =
                                menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if !view.editable() {
                                            return;
                                        }
                                        view.flush_inputs(cx);
                                        if value.as_ref().is_none_or(|value| {
                                            view.draft
                                                .values
                                                .iter()
                                                .any(|entry| &entry.value == value)
                                        }) {
                                            view.draft.positive_value = value.clone();
                                            view.clear_feedback();
                                            cx.notify();
                                        }
                                    });
                                }));
                        }
                        menu
                    }),
            )
    }
}

impl Render for SemanticDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut view =
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(div().text_sm().child(crate::text::format(
                    "detail.data.confirmSemanticMessage",
                    &[
                        ("column", self.column.clone()),
                        ("kind", self.draft.kind.to_string()),
                    ],
                )));
        if self.values_ready {
            if self.draft.kind == SemanticType::Numeric {
                view = view.child(self.numeric_fields(cx));
            } else if self.domain() {
                view = view.child(self.mapping_fields(window, cx));
                if self.draft.kind == SemanticType::Binary && self.draft.values.len() == 2 {
                    view = view.child(self.positive_field(cx));
                }
            }
        }
        view.when(self.loading || self.saving, |view| {
            view.child(div().text_xs().child(t("common.loading")))
        })
        .when_some(self.error, |view, key| {
            view.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(crate::text::format(
                        key,
                        &[("count", self.draft.values.len().to_string())],
                    )),
            )
        })
        .when(
            self.error == Some("detail.data.mappingLoadFailed"),
            |view| {
                view.child(
                    Button::new("semantic-retry")
                        .small()
                        .outline()
                        .label(t("common.retry"))
                        .disabled(self.loading || self.saving)
                        .on_click(cx.listener(|view, _, window, cx| view.read_values(window, cx))),
                )
            },
        )
    }
}
