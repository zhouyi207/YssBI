//! One column draft follows either issued choices or the shared editable text list.
use super::RelationalDraft;
use crate::parameters::{
    ParameterForm,
    field::{
        ParameterDraft,
        list::{ListDraft, RowCommand, row_number},
    },
};
use gpui_kit::component::{ActiveTheme, Disableable, checkbox::Checkbox};
use gpui_kit::{AnyElement, Context, IntoElement, SharedString, Window, div, prelude::*};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_editor::projection::EditorParameterConfiguration;
use yss_node_protocol::{ParameterKey, RelationalScalarType};
use yss_ui::inspector::{self, Pager};
use yss_ui::text::translate;

const PAGE_SIZE: usize = 50;

pub(in crate::parameters) enum ColumnsDraft {
    Known { selected: Vec<String>, page: usize },
    Manual(ListDraft),
}

impl ColumnsDraft {
    pub(super) fn new(
        key: &ParameterKey,
        known: bool,
        values: &[Box<str>],
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        let values = values.iter().map(ToString::to_string);
        if known {
            Self::Known {
                selected: values.collect(),
                page: 0,
            }
        } else {
            Self::Manual(ListDraft::text(key, values, window, cx))
        }
    }

    pub(super) fn value(&self, allow_empty: bool, cx: &gpui_kit::App) -> Result<Value, String> {
        let value = match self {
            Self::Known { selected, .. } => serde_json::json!(selected),
            Self::Manual(draft) => draft.value(cx)?,
        };
        Ok(
            if !allow_empty && value.as_array().is_some_and(Vec::is_empty) {
                Value::Null
            } else {
                value
            },
        )
    }
}

impl ParameterForm {
    /// Selected columns carry their order and reorder menu in a trailing lane that unselected
    /// rows keep empty, so names and types stay on the same columns.
    pub(super) fn render_columns(
        &self,
        index: usize,
        draft: &ColumnsDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(EditorParameterConfiguration::ProjectColumns {
            options,
            context_hint,
            ..
        }) = self.fields[index].model.configuration.as_ref()
        else {
            return div().into_any_element();
        };
        let content = div().flex().flex_col().gap_2().children(
            context_hint
                .as_deref()
                .map(|text| inspector::hint(text, cx)),
        );
        let (selected, page) = match draft {
            ColumnsDraft::Known { selected, page } => (selected, *page),
            ColumnsDraft::Manual(draft) => {
                return content
                    .child(self.render_list_parameter(
                        index,
                        draft,
                        busy,
                        "detail.parameterEditor.addColumn",
                        cx,
                    ))
                    .into_any_element();
            }
        };
        let epoch = self.epoch;
        let eligible = options
            .iter()
            .map(|option| option.name.as_ref())
            .collect::<BTreeSet<_>>();
        let positions = selected
            .iter()
            .enumerate()
            .map(|(row, name)| (name.as_str(), row))
            .collect::<BTreeMap<_, _>>();
        let missing = selected
            .iter()
            .filter(|name| !eligible.contains(name.as_str()));
        let count = options.len() + missing.clone().count();
        let pages = count.div_ceil(PAGE_SIZE).max(1);
        let page = page.min(pages - 1);
        let rows = options
            .iter()
            .map(|option| (option.name.as_ref(), Some(option.data_type)))
            .chain(missing.map(|name| (name.as_str(), None)));
        let mut list = div()
            .id(("column-options", index))
            .max_h_64()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (row, (name, data_type)) in rows.enumerate().skip(page * PAGE_SIZE).take(PAGE_SIZE) {
            let position = positions.get(name).copied();
            let toggle_name = name.to_owned();
            let order = position.map(|position| {
                let name = name.to_owned();
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(row_number(position, cx))
                    .child(self.row_menu(
                        SharedString::from(format!("column-actions-{index}-{name}")),
                        (position, selected.len()),
                        false,
                        busy,
                        cx,
                        move |view, command, _, cx| {
                            if let RowCommand::Move(direction) = command
                                && let Some(field) = view.fields.get_mut(index)
                                && let ParameterDraft::Relational(RelationalDraft::Columns(
                                    ColumnsDraft::Known { selected, .. },
                                )) = &mut field.draft
                                && let Some(row) = selected.iter().position(|value| value == &name)
                                && let Some(target) = row.checked_add_signed(direction)
                                && target < selected.len()
                            {
                                selected.swap(row, target);
                                field.error = None;
                                field.dirty = true;
                                cx.notify();
                            }
                        },
                    ))
            });
            list = list.child(
                div()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Checkbox::new(SharedString::from(format!("column-{index}-{row}")))
                            .label(name.to_owned())
                            .flex_1()
                            .min_w_0()
                            .checked(position.is_some())
                            .disabled(busy)
                            .on_click(cx.listener(move |view, checked: &bool, _, cx| {
                                if !view.accepts_input(epoch, cx) {
                                    return;
                                }
                                if let ParameterDraft::Relational(RelationalDraft::Columns(
                                    ColumnsDraft::Known { selected, .. },
                                )) = &mut view.fields[index].draft
                                {
                                    if *checked {
                                        if !selected.contains(&toggle_name) {
                                            selected.push(toggle_name.clone());
                                        }
                                    } else {
                                        selected.retain(|name| name != &toggle_name);
                                    }
                                    view.fields[index].error = None;
                                    view.fields[index].dirty = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(if data_type.is_none() {
                                cx.theme().danger
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(match data_type {
                                Some(RelationalScalarType::Known(kind)) => kind.to_string(),
                                Some(RelationalScalarType::Unknown) => "?".into(),
                                None => translate("detail.parameterEditor.unavailableColumn"),
                            }),
                    )
                    .child(
                        div()
                            .w_12()
                            .flex_shrink_0()
                            .flex()
                            .justify_end()
                            .children(order),
                    ),
            );
        }
        content
            .when(count > 0, |content| content.child(list))
            .when(options.is_empty(), |content| {
                content.child(inspector::hint(
                    translate("detail.parameterEditor.noColumns"),
                    cx,
                ))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(
                        Pager::new(("column-pager", index), page, pages)
                            .disabled(busy)
                            .on_change(cx.listener(move |view, page: &usize, _, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::Relational(RelationalDraft::Columns(
                                        ColumnsDraft::Known { page: next, .. },
                                    )) = &mut view.fields[index].draft
                                {
                                    *next = *page;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        inspector::commit_button(
                            ("columns-apply", index),
                            translate("native.workbench.applyColumns"),
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
