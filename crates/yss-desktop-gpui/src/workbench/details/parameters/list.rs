//! Bounded pages for text and numeric series parameter drafts.
use super::{DetailsPanel, ParameterDraft, controls};
use gpui::{AnyElement, Context, Entity, IntoElement, div, prelude::*};
use gpui_component::{
    Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};

impl DetailsPanel {
    pub(super) fn render_list_parameter(
        &self,
        index: usize,
        rows: &[Entity<InputState>],
        page: &usize,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let page = (*page).min(rows.len().saturating_sub(1) / 25);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                rows.iter()
                    .enumerate()
                    .skip(page * 25)
                    .take(25)
                    .map(|(row, input)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(Input::new(input).small().flex_1().min_w_0().disabled(busy))
                            .child(
                                Button::new(gpui::SharedString::from(format!(
                                    "list-up-{index}-{row}"
                                )))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronUp)
                                .tooltip("上移")
                                .disabled(busy || row == 0)
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        if view.accepts_input(epoch, cx) {
                                            view.move_list_value(index, row, -1, cx);
                                        }
                                    },
                                )),
                            )
                            .child(
                                Button::new(gpui::SharedString::from(format!(
                                    "list-down-{index}-{row}"
                                )))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronDown)
                                .tooltip("下移")
                                .disabled(busy || row + 1 == rows.len())
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        if view.accepts_input(epoch, cx) {
                                            view.move_list_value(index, row, 1, cx);
                                        }
                                    },
                                )),
                            )
                            .child(
                                Button::new(gpui::SharedString::from(format!(
                                    "list-remove-{index}-{row}"
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
                                        if let ParameterDraft::List { rows, .. } =
                                            &mut view.fields[index].draft
                                            && row < rows.len()
                                        {
                                            rows.remove(row);
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(("list-add", index))
                            .small()
                            .ghost()
                            .icon(IconName::Plus)
                            .label("添加值")
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if !view.accepts_input(epoch, cx) {
                                    return;
                                }
                                if let ParameterDraft::List { rows, page, .. } =
                                    &mut view.fields[index].draft
                                {
                                    rows.push(cx.new(|cx| InputState::new(window, cx)));
                                    *page = rows.len().saturating_sub(1) / 25;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        controls::apply(("list-apply", index), busy).on_click(cx.listener(
                            move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx) {
                                    view.apply_parameter(index, cx)
                                }
                            },
                        )),
                    )
                    .when(page > 0, |view| {
                        view.child(
                            Button::new(("list-prev", index))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronLeft)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if view.accepts_input(epoch, cx)
                                        && let ParameterDraft::List { page, .. } =
                                            &mut view.fields[index].draft
                                    {
                                        *page = page.saturating_sub(1);
                                        cx.notify();
                                    }
                                })),
                        )
                    })
                    .when((page + 1) * 25 < rows.len(), |view| {
                        view.child(
                            Button::new(("list-next", index))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronRight)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if view.accepts_input(epoch, cx)
                                        && let ParameterDraft::List { page, .. } =
                                            &mut view.fields[index].draft
                                    {
                                        *page += 1;
                                        cx.notify();
                                    }
                                })),
                        )
                    }),
            )
            .into_any_element()
    }

    fn move_list_value(
        &mut self,
        index: usize,
        row: usize,
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        if let ParameterDraft::List { rows, page, .. } = &mut self.fields[index].draft
            && let Some(target) = row.checked_add_signed(direction)
            && row < rows.len()
            && target < rows.len()
        {
            rows.swap(row, target);
            *page = target / 25;
            cx.notify();
        }
    }
}
