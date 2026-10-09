//! Bounded series pages reuse the same draft for input, reordering and submission.
mod draft;
use super::{DetailsPanel, ParameterDraft, controls};
use crate::text::translate;
pub(super) use draft::ListDraft;
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit_assets::IconName;

impl DetailsPanel {
    pub(in crate::workbench::details::parameters) fn render_list_parameter(
        &self,
        index: usize,
        draft: &ListDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let page = draft.page;
        let pages = draft.pages();
        let values = div()
            .id(("series-values", index))
            .max_h(px(256.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1()
            .children(draft.visible().map(|(row, input)| {
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().text_xs().child((row + 1).to_string()))
                    .child(Input::new(input).small().flex_1().min_w_0().disabled(busy))
                    .children(
                        [
                            (-1, IconName::ChevronUp, "conversion.moveUp"),
                            (1, IconName::ChevronDown, "conversion.moveDown"),
                        ]
                        .into_iter()
                        .map(|(direction, icon, label)| {
                            Button::new(gpui::SharedString::from(format!(
                                "series-move-{index}-{row}-{direction}"
                            )))
                            .small()
                            .ghost()
                            .icon(icon)
                            .tooltip(translate(label))
                            .disabled(
                                busy || (direction < 0 && row == 0)
                                    || (direction > 0 && row + 1 == draft.len()),
                            )
                            .on_click(cx.listener(
                                move |view, _, window, cx| {
                                    if view.accepts_input(epoch, cx)
                                        && let ParameterDraft::List(draft) =
                                            &mut view.fields[index].draft
                                    {
                                        draft.move_row(row, direction, window, cx);
                                        cx.notify();
                                    }
                                },
                            ))
                        }),
                    )
                    .child(
                        Button::new(gpui::SharedString::from(format!(
                            "series-remove-{index}-{row}"
                        )))
                        .small()
                        .ghost()
                        .icon(IconName::Minus)
                        .tooltip(translate("conversion.removeValue"))
                        .disabled(busy)
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::List(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.remove(row, window, cx);
                                    cx.notify();
                                }
                            },
                        )),
                    )
            }));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(values)
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
                            .label(translate("conversion.addValue"))
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::List(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.add(window, cx);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        controls::apply(("list-apply", index), busy).on_click(cx.listener(
                            move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx) {
                                    view.apply_parameter(index, cx);
                                }
                            },
                        )),
                    )
                    .when(pages > 1, |bar| {
                        bar.child(
                            Button::new(("list-prev", index))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronLeft)
                                .tooltip(translate("conversion.previousValues"))
                                .disabled(busy || page == 0)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    if view.accepts_input(epoch, cx)
                                        && let ParameterDraft::List(draft) =
                                            &mut view.fields[index].draft
                                    {
                                        draft.show_page(page.saturating_sub(1), window, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(div().text_xs().child(format!("{} / {pages}", page + 1)))
                        .child(
                            Button::new(("list-next", index))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronRight)
                                .tooltip(translate("conversion.nextValues"))
                                .disabled(busy || page + 1 == pages)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    if view.accepts_input(epoch, cx)
                                        && let ParameterDraft::List(draft) =
                                            &mut view.fields[index].draft
                                    {
                                        draft.show_page(page + 1, window, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                    }),
            )
            .into_any_element()
    }
}
