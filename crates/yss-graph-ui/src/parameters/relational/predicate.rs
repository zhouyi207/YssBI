//! Filter controls consume the selected column's issued restrictions.
mod choices;
mod draft;

use super::RelationalDraft;
use crate::parameters::{ParameterForm, field::ParameterDraft};
use choices::FilterMenu;
pub(in crate::parameters) use draft::FilterDraft;
use gpui_kit::component::{Disableable, Sizable, checkbox::Checkbox, input::Input};
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*};
use yss_graph_editor::projection::{EditorFilterLiteralType, EditorParameterConfiguration};
use yss_node_protocol::dataframe::FilterOperator;
use yss_ui::inspector::{self, InlineMessage};
use yss_ui::text::translate;

fn operator_label(operator: FilterOperator) -> String {
    translate(match operator {
        FilterOperator::Equal => "native.workbench.equal",
        FilterOperator::NotEqual => "native.workbench.notEqual",
        FilterOperator::LessThan => "native.workbench.lessThan",
        FilterOperator::LessThanOrEqual => "native.workbench.lessOrEqual",
        FilterOperator::GreaterThan => "native.workbench.greaterThan",
        FilterOperator::GreaterThanOrEqual => "native.workbench.greaterOrEqual",
        FilterOperator::IsNull => "native.workbench.isNull",
        FilterOperator::IsNotNull => "native.workbench.isNotNull",
    })
}

fn literal_label(kind: EditorFilterLiteralType) -> String {
    translate(match kind {
        EditorFilterLiteralType::Boolean => "native.workbench.boolean",
        EditorFilterLiteralType::Integer => "conversion.integer",
        EditorFilterLiteralType::Decimal => "native.workbench.decimal",
        EditorFilterLiteralType::String => "conversion.text",
    })
}

impl ParameterForm {
    pub(in crate::parameters::relational) fn render_filter(
        &self,
        index: usize,
        draft: &FilterDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let Some(
            configuration @ EditorParameterConfiguration::FilterPredicate {
                schema_known,
                columns,
                context_hint,
                ..
            },
        ) = self.fields[index].model.configuration.as_ref()
        else {
            return div().into_any_element();
        };
        let column = if *schema_known {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.filter_choice(index, FilterMenu::Column, busy, cx))
                .when(
                    draft.selected(configuration, cx).is_none()
                        && !draft.column.read(cx).value().is_empty(),
                    |column| {
                        column.child(InlineMessage::warning(translate(
                            "detail.parameterEditor.unavailableColumn",
                        )))
                    },
                )
                .when(columns.is_empty(), |column| {
                    column.child(inspector::hint(
                        translate("detail.parameterEditor.noColumns"),
                        cx,
                    ))
                })
                .into_any_element()
        } else {
            Input::new(&draft.column)
                .small()
                .disabled(busy)
                .into_any_element()
        };
        let mut content = div()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                context_hint
                    .as_deref()
                    .map(|text| inspector::hint(text, cx)),
            )
            .child(inspector::sub_field(
                translate("detail.parameterEditor.column"),
                column,
                cx,
            ))
            .child(inspector::sub_field(
                translate("detail.parameterEditor.operator"),
                self.filter_choice(index, FilterMenu::Operator, busy, cx),
                cx,
            ));
        if draft.operator.is_some_and(FilterOperator::requires_value) {
            let types = draft.literal_types(configuration, cx);
            if types.len() > 1 || !types.contains(&draft.literal_type) {
                content = content.child(inspector::sub_field(
                    translate("detail.parameterEditor.valueType"),
                    self.filter_choice(index, FilterMenu::LiteralType, busy, cx),
                    cx,
                ));
            }
            let value = if draft.literal_type == EditorFilterLiteralType::Boolean {
                Checkbox::new(("filter-boolean", index))
                    .label(translate("native.workbench.trueValue"))
                    .checked(draft.input.read(cx).value() == "true")
                    .disabled(busy)
                    .on_click(cx.listener(move |view, checked: &bool, window, cx| {
                        if !view.accepts_input(epoch, cx) {
                            return;
                        }
                        let field = &mut view.fields[index];
                        if let ParameterDraft::Relational(RelationalDraft::Filter(draft)) =
                            &mut field.draft
                        {
                            draft.input.update(cx, |input, cx| {
                                input.set_value(if *checked { "true" } else { "false" }, window, cx)
                            });
                            field.error = None;
                            field.dirty = true;
                            cx.notify();
                        }
                    }))
                    .into_any_element()
            } else {
                Input::new(&draft.input)
                    .small()
                    .disabled(busy)
                    .into_any_element()
            };
            content = content.child(inspector::sub_field(
                translate("detail.fields.value"),
                value,
                cx,
            ));
        }
        content
            .child(
                div().flex().justify_end().child(
                    inspector::commit_button(
                        ("filter-apply", index),
                        translate("native.workbench.applyFilter"),
                        busy,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.apply_parameter(index, cx);
                        }
                    })),
                ),
            )
            .into_any_element()
    }
}
