//! Filter choices consume the typed restrictions issued for the selected input column.
use super::FilterDraft;
use super::RelationalDraft;
use crate::workbench::details::{DetailsPanel, controls, parameters::ParameterDraft};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};
use gpui_component::{Disableable, Sizable, checkbox::Checkbox, input::Input};
use yss_graph_editor::projection::EditorFilterLiteralType;
use yss_graph_editor::projection::EditorParameterConfiguration;
use yss_node_protocol::dataframe::FilterOperator;
fn operator_label(operator: FilterOperator) -> &'static str {
    match operator {
        FilterOperator::Equal => "等于",
        FilterOperator::NotEqual => "不等于",
        FilterOperator::LessThan => "小于",
        FilterOperator::LessThanOrEqual => "小于或等于",
        FilterOperator::GreaterThan => "大于",
        FilterOperator::GreaterThanOrEqual => "大于或等于",
        FilterOperator::IsNull => "为空",
        FilterOperator::IsNotNull => "不为空",
    }
}

fn literal_label(kind: EditorFilterLiteralType) -> &'static str {
    match kind {
        EditorFilterLiteralType::Boolean => "布尔值",
        EditorFilterLiteralType::Integer => "整数",
        EditorFilterLiteralType::Decimal => "小数",
        EditorFilterLiteralType::String => "文本",
    }
}

impl DetailsPanel {
    pub(super) fn render_filter(
        &self,
        index: usize,
        draft: &FilterDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let Some(EditorParameterConfiguration::FilterPredicate {
            schema_known,
            columns,
            context_hint,
            ..
        }) = self.fields[index].model.configuration.as_ref()
        else {
            return div().into_any_element();
        };
        let column = draft.column.read(cx).value().to_string();
        let selected = columns.iter().find(|option| option.name.as_ref() == column);
        let operators = if *schema_known {
            selected
                .map(|option| option.operators.to_vec())
                .unwrap_or_default()
        } else {
            vec![
                FilterOperator::Equal,
                FilterOperator::NotEqual,
                FilterOperator::LessThan,
                FilterOperator::LessThanOrEqual,
                FilterOperator::GreaterThan,
                FilterOperator::GreaterThanOrEqual,
                FilterOperator::IsNull,
                FilterOperator::IsNotNull,
            ]
        };
        let types = if *schema_known {
            selected
                .map(|option| option.literal_types.to_vec())
                .unwrap_or_default()
        } else {
            vec![
                EditorFilterLiteralType::Boolean,
                EditorFilterLiteralType::Integer,
                EditorFilterLiteralType::Decimal,
                EditorFilterLiteralType::String,
            ]
        };
        let mut content = div()
            .flex()
            .flex_col()
            .gap_2()
            .children(context_hint.as_deref().map(|text| controls::hint(text, cx)));
        if *schema_known {
            content = content.child(controls::choice(
                ("filter-column", index),
                if column.is_empty() {
                    "选择列".into()
                } else {
                    column.clone()
                },
                Some(column),
                columns
                    .iter()
                    .map(|option| (option.name.to_string(), option.name.to_string()))
                    .collect(),
                busy,
                cx.listener(move |view, name: &String, window, cx| {
                    if !view.accepts_input(epoch, cx) {
                        return;
                    }
                    let field = &mut view.fields[index];
                    if let ParameterDraft::Relational(RelationalDraft::Filter(draft)) =
                        &mut field.draft
                    {
                        draft
                            .column
                            .update(cx, |input, cx| input.set_value(name.clone(), window, cx));
                        if let Some(EditorParameterConfiguration::FilterPredicate {
                            columns, ..
                        }) = &field.model.configuration
                            && let Some(option) =
                                columns.iter().find(|option| option.name.as_ref() == name)
                        {
                            if draft
                                .operator
                                .is_none_or(|operator| !option.operators.contains(&operator))
                            {
                                draft.operator = option.operators.first().copied();
                            }
                            if !option.literal_types.contains(&draft.literal_type)
                                && let Some(kind) = option.literal_types.first()
                            {
                                draft.literal_type = *kind;
                                draft.input.update(cx, |input, cx| {
                                    input.set_value(
                                        if *kind == EditorFilterLiteralType::Boolean {
                                            "false"
                                        } else {
                                            ""
                                        },
                                        window,
                                        cx,
                                    )
                                });
                            }
                        }
                    }
                    cx.notify();
                }),
            ));
            if selected.is_none() && !draft.column.read(cx).value().is_empty() {
                content = content.child(controls::hint("当前列已不可用，请重新选择", cx));
            }
        } else {
            content = content.child(Input::new(&draft.column).small().disabled(busy));
        }
        let chosen_operators = operators.clone();
        content = content.child(controls::choice(
            ("filter-operator", index),
            draft
                .operator
                .map(operator_label)
                .unwrap_or("比较方式")
                .into(),
            draft.operator.map(|value| format!("{value:?}")),
            operators
                .iter()
                .map(|value| (format!("{value:?}"), operator_label(*value).into()))
                .collect(),
            busy,
            cx.listener(move |view, value: &String, _, cx| {
                if !view.accepts_input(epoch, cx) {
                    return;
                }
                if let ParameterDraft::Relational(RelationalDraft::Filter(draft)) =
                    &mut view.fields[index].draft
                {
                    draft.operator = chosen_operators
                        .iter()
                        .find(|operator| format!("{operator:?}") == *value)
                        .copied();
                }
                cx.notify();
            }),
        ));
        if draft.operator.is_some_and(FilterOperator::requires_value) {
            let chosen_types = types.clone();
            content = content.child(controls::choice(
                ("filter-value-type", index),
                literal_label(draft.literal_type).into(),
                Some(literal_label(draft.literal_type).into()),
                types
                    .iter()
                    .map(|kind| (literal_label(*kind).into(), literal_label(*kind).into()))
                    .collect(),
                busy,
                cx.listener(move |view, value: &String, window, cx| {
                    if !view.accepts_input(epoch, cx) {
                        return;
                    }
                    if let ParameterDraft::Relational(RelationalDraft::Filter(draft)) =
                        &mut view.fields[index].draft
                        && let Some(kind) = chosen_types
                            .iter()
                            .find(|kind| literal_label(**kind) == value)
                    {
                        draft.literal_type = *kind;
                        draft.input.update(cx, |input, cx| {
                            input.set_value(
                                if *kind == EditorFilterLiteralType::Boolean {
                                    "false"
                                } else {
                                    ""
                                },
                                window,
                                cx,
                            )
                        });
                    }
                    cx.notify();
                }),
            ));
            if draft.literal_type == EditorFilterLiteralType::Boolean {
                content = content.child(
                    Checkbox::new(("filter-boolean", index))
                        .label("真")
                        .checked(draft.input.read(cx).value() == "true")
                        .disabled(busy)
                        .on_click(cx.listener(move |view, checked: &bool, window, cx| {
                            if !view.accepts_input(epoch, cx) {
                                return;
                            }
                            if let ParameterDraft::Relational(RelationalDraft::Filter(draft)) =
                                &mut view.fields[index].draft
                            {
                                draft.input.update(cx, |input, cx| {
                                    input.set_value(checked.to_string(), window, cx)
                                });
                            }
                            cx.notify();
                        })),
                );
            } else {
                content = content.child(Input::new(&draft.input).small().disabled(busy));
            }
        }
        content
            .child(
                controls::apply(("filter-apply", index), busy)
                    .label("应用筛选条件")
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.apply_parameter(index, cx)
                        }
                    })),
            )
            .into_any_element()
    }
}
