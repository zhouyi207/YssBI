//! Ordered column choices preserve exact names, including unavailable entries.
use super::RelationalDraft;
use crate::workbench::details::{DetailsPanel, controls, parameters::ParameterDraft};
use gpui::{AnyElement, Context, Entity, IntoElement, div, prelude::*};
use gpui_component::{
    Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputState},
};
use yss_graph_editor::projection::EditorParameterConfiguration;

impl DetailsPanel {
    pub(super) fn render_columns(
        &self,
        index: usize,
        selected: &[String],
        manual: &[Entity<InputState>],
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let Some(EditorParameterConfiguration::ProjectColumns {
            schema_known,
            options,
            context_hint,
            ..
        }) = self.fields[index].model.configuration.as_ref()
        else {
            return div().into_any_element();
        };
        let mut content = div()
            .flex()
            .flex_col()
            .gap_2()
            .children(context_hint.as_deref().map(|text| controls::hint(text, cx)));
        if *schema_known {
            let names = options
                .iter()
                .map(|option| option.name.to_string())
                .collect::<Vec<_>>();
            content = content.child(
                div()
                    .id(("column-options", index))
                    .max_h(gpui::px(240.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(options.iter().enumerate().map(|(row, option)| {
                        let name = option.name.to_string();
                        let checked = selected.contains(&name);
                        Checkbox::new(gpui::SharedString::from(format!("column-{index}-{row}")))
                            .label(name.clone())
                            .checked(checked)
                            .disabled(busy)
                            .on_click(cx.listener(move |view, checked: &bool, _, cx| {
                                if !view.accepts_input(epoch, cx) {
                                    return;
                                }
                                if let ParameterDraft::Relational(RelationalDraft::Columns {
                                    selected,
                                    ..
                                }) = &mut view.fields[index].draft
                                {
                                    if *checked {
                                        selected.push(name.clone());
                                    } else {
                                        selected.retain(|value| value != &name);
                                    }
                                }
                                cx.notify();
                            }))
                    })),
            );
            for (row, name) in selected
                .iter()
                .filter(|name| !names.contains(name))
                .enumerate()
            {
                let name = name.clone();
                content = content.child(
                    Button::new(gpui::SharedString::from(format!(
                        "missing-column-{index}-{row}"
                    )))
                    .small()
                    .ghost()
                    .label(format!("{name} · 已不可用"))
                    .icon(IconName::Close)
                    .disabled(busy)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.accepts_input(epoch, cx) {
                            return;
                        }
                        if let ParameterDraft::Relational(RelationalDraft::Columns {
                            selected,
                            ..
                        }) = &mut view.fields[index].draft
                        {
                            selected.retain(|value| value != &name);
                        }
                        cx.notify();
                    })),
                );
            }
            if options.is_empty() {
                content = content.child(controls::hint("当前输入没有可选列", cx));
            }
            content = content.child(controls::hint("输出列顺序", cx)).children(
                selected.iter().enumerate().map(|(row, name)| {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .truncate()
                                .child(format!("{}. {name}", row + 1)),
                        )
                        .child(
                            Button::new(gpui::SharedString::from(format!(
                                "column-up-{index}-{row}"
                            )))
                            .small()
                            .ghost()
                            .icon(IconName::ChevronUp)
                            .tooltip("上移")
                            .disabled(busy || row == 0)
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    if view.accepts_input(epoch, cx) {
                                        view.move_column(index, row, -1, cx);
                                    }
                                },
                            )),
                        )
                        .child(
                            Button::new(gpui::SharedString::from(format!(
                                "column-down-{index}-{row}"
                            )))
                            .small()
                            .ghost()
                            .icon(IconName::ChevronDown)
                            .tooltip("下移")
                            .disabled(busy || row + 1 == selected.len())
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    if view.accepts_input(epoch, cx) {
                                        view.move_column(index, row, 1, cx);
                                    }
                                },
                            )),
                        )
                }),
            );
        } else {
            content = content
                .child(controls::hint("输入列结构尚未知，可先填写列名", cx))
                .children(manual.iter().enumerate().map(|(row, input)| {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(Input::new(input).small().flex_1().min_w_0().disabled(busy))
                        .child(
                            Button::new(gpui::SharedString::from(format!(
                                "manual-column-remove-{index}-{row}"
                            )))
                            .small()
                            .ghost()
                            .icon(IconName::Minus)
                            .disabled(busy)
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    if !view.accepts_input(epoch, cx) {
                                        return;
                                    }
                                    if let ParameterDraft::Relational(RelationalDraft::Columns {
                                        manual,
                                        ..
                                    }) = &mut view.fields[index].draft
                                        && row < manual.len()
                                    {
                                        manual.remove(row);
                                    }
                                    cx.notify();
                                },
                            )),
                        )
                }))
                .child(
                    Button::new(("manual-column-add", index))
                        .small()
                        .ghost()
                        .icon(IconName::Plus)
                        .label("添加列")
                        .disabled(busy)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            if !view.accepts_input(epoch, cx) {
                                return;
                            }
                            if let ParameterDraft::Relational(RelationalDraft::Columns {
                                manual,
                                ..
                            }) = &mut view.fields[index].draft
                            {
                                manual.push(
                                    cx.new(|cx| InputState::new(window, cx).placeholder("列名")),
                                );
                            }
                            cx.notify();
                        })),
                );
        }
        content
            .child(
                controls::apply(("columns-apply", index), busy)
                    .label("应用列选择")
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.apply_parameter(index, cx)
                        }
                    })),
            )
            .into_any_element()
    }

    fn move_column(&mut self, index: usize, row: usize, direction: isize, cx: &mut Context<Self>) {
        if let ParameterDraft::Relational(RelationalDraft::Columns { selected, .. }) =
            &mut self.fields[index].draft
            && let Some(target) = row.checked_add_signed(direction)
            && row < selected.len()
            && target < selected.len()
        {
            selected.swap(row, target);
            cx.notify();
        }
    }
}
