//! One column draft follows either issued choices or the shared editable text list.
use super::RelationalDraft;
use crate::{
    text::translate,
    workbench::parameters::{
        ParameterForm, controls,
        field::{ParameterDraft, list::ListDraft},
    },
};
use gpui::{AnyElement, Context, IntoElement, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
};
use gpui_kit_assets::IconName;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_editor::projection::EditorParameterConfiguration;
use yss_node_protocol::{ParameterKey, RelationalScalarType};

const PAGE_SIZE: usize = 50;

pub(in crate::workbench::parameters) enum ColumnsDraft {
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

    pub(super) fn value(&self, allow_empty: bool, cx: &gpui::App) -> Result<Value, String> {
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
        let content = div()
            .flex()
            .flex_col()
            .gap_2()
            .children(context_hint.as_deref().map(|text| controls::hint(text, cx)));
        let ColumnsDraft::Known { selected, page } = draft else {
            let ColumnsDraft::Manual(draft) = draft else {
                unreachable!()
            };
            return content
                .child(self.render_list_parameter(
                    index,
                    draft,
                    busy,
                    "detail.parameterEditor.addColumn",
                    cx,
                ))
                .into_any_element();
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
        let page = (*page).min(pages - 1);
        let rows = options
            .iter()
            .map(|option| (option.name.as_ref(), Some(option.data_type)))
            .chain(missing.map(|name| (name.as_str(), None)));
        content
            .child(
                div()
                    .id(("column-options", index))
                    .max_h(gpui::px(256.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(rows.enumerate().skip(page * PAGE_SIZE).take(PAGE_SIZE).map(
                        |(row, (name, data_type))| {
                            let position = positions.get(name).copied();
                            let name = name.to_owned();
                            let toggle_name = name.clone();
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Checkbox::new(gpui::SharedString::from(format!(
                                        "column-{index}-{row}"
                                    )))
                                    .label(name.clone())
                                    .flex_1()
                                    .min_w_0()
                                    .checked(position.is_some())
                                    .disabled(busy)
                                    .on_click(cx.listener(move |view, checked: &bool, _, cx| {
                                        if !view.accepts_input(epoch, cx) {
                                            return;
                                        }
                                        if let ParameterDraft::Relational(
                                            RelationalDraft::Columns(ColumnsDraft::Known {
                                                selected,
                                                ..
                                            }),
                                        ) = &mut view.fields[index].draft
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
                                        .text_xs()
                                        .text_color(if data_type.is_none() {
                                            cx.theme().danger
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .child(match data_type {
                                            Some(RelationalScalarType::Known(kind)) => {
                                                kind.to_string()
                                            }
                                            Some(RelationalScalarType::Unknown) => "?".into(),
                                            None => translate(
                                                "detail.parameterEditor.unavailableColumn",
                                            ),
                                        }),
                                )
                                .child(
                                    div().w(gpui::px(24.)).text_xs().text_right().child(
                                        position
                                            .map(|row| (row + 1).to_string())
                                            .unwrap_or_default(),
                                    ),
                                )
                                .children(
                                    [
                                        (-1, IconName::ChevronUp, "conversion.moveUp"),
                                        (1, IconName::ChevronDown, "conversion.moveDown"),
                                    ]
                                    .into_iter()
                                    .map(
                                        |(direction, icon, label)| {
                                            let name = name.clone();
                                            Button::new(gpui::SharedString::from(format!(
                                                "column-move-{index}-{row}-{direction}"
                                            )))
                                            .small()
                                            .ghost()
                                            .icon(icon)
                                            .tooltip(translate(label))
                                            .disabled(
                                                busy || position.is_none_or(|row| {
                                                    (direction < 0 && row == 0)
                                                        || (direction > 0
                                                            && row + 1 == selected.len())
                                                }),
                                            )
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                if !view.accepts_input(epoch, cx) {
                                                    return;
                                                }
                                                if let ParameterDraft::Relational(
                                                    RelationalDraft::Columns(ColumnsDraft::Known {
                                                        selected,
                                                        ..
                                                    }),
                                                ) = &mut view.fields[index].draft
                                                    && let Some(row) = selected
                                                        .iter()
                                                        .position(|value| value == &name)
                                                    && let Some(target) =
                                                        row.checked_add_signed(direction)
                                                    && target < selected.len()
                                                {
                                                    selected.swap(row, target);
                                                    view.fields[index].error = None;
                                                    view.fields[index].dirty = true;
                                                    cx.notify();
                                                }
                                            }))
                                        },
                                    ),
                                )
                        },
                    )),
            )
            .when(options.is_empty(), |content| {
                content.child(controls::hint(
                    translate("detail.parameterEditor.noColumns"),
                    cx,
                ))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        controls::apply(("columns-apply", index), busy)
                            .label(translate("native.workbench.applyColumns"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx) {
                                    view.apply_parameter(index, cx);
                                }
                            })),
                    )
                    .when(pages > 1, |bar| {
                        bar.children(
                            [
                                (-1, IconName::ChevronLeft, "conversion.previousValues"),
                                (1, IconName::ChevronRight, "conversion.nextValues"),
                            ]
                            .into_iter()
                            .map(|(direction, icon, label)| {
                                Button::new(gpui::SharedString::from(format!(
                                    "column-page-{index}-{direction}"
                                )))
                                .small()
                                .ghost()
                                .icon(icon)
                                .tooltip(translate(label))
                                .disabled(
                                    busy || (direction < 0 && page == 0)
                                        || (direction > 0 && page + 1 == pages),
                                )
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        if view.accepts_input(epoch, cx)
                                            && let ParameterDraft::Relational(
                                                RelationalDraft::Columns(ColumnsDraft::Known {
                                                    page: next,
                                                    ..
                                                }),
                                            ) = &mut view.fields[index].draft
                                        {
                                            *next = page.saturating_add_signed(direction);
                                            cx.notify();
                                        }
                                    },
                                ))
                            }),
                        )
                        .child(div().text_xs().child(format!("{} / {pages}", page + 1)))
                    }),
            )
            .into_any_element()
    }
}
