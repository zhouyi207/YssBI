//! Bounded code/label controls share the existing parameter commit and restore paths.
mod draft;
mod positive;
use super::{ParameterForm, controls, field::ParameterDraft};
use crate::text::translate;
pub(super) use draft::DomainDraft;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*, px};
use yss_data_contract::ConversionDomain;

impl ParameterForm {
    pub(super) fn render_domain(
        &self,
        index: usize,
        draft: &DomainDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let pages = draft.pages();
        let page = draft.page;
        let duplicates = draft.has_duplicates(cx);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(controls::hint(translate("conversion.domainHelp"), cx))
            .child(
                div()
                    .id(("domain-values", index))
                    .max_h(px(256.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(draft.visible().map(|(row, inputs)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_2()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_md()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(div().flex_1().text_xs().child((row + 1).to_string()))
                                    .children(
                                        [
                                            (-1, IconName::ChevronUp, "conversion.moveUp"),
                                            (1, IconName::ChevronDown, "conversion.moveDown"),
                                        ]
                                        .into_iter()
                                        .map(
                                            |(direction, icon, label)| {
                                                Button::new(gpui_kit::SharedString::from(format!(
                                                    "domain-move-{index}-{row}-{direction}"
                                                )))
                                                .small()
                                                .ghost()
                                                .icon(icon)
                                                .tooltip(translate(label))
                                                .disabled(
                                                    busy || (direction < 0 && row == 0)
                                                        || (direction > 0
                                                            && row + 1 == draft.len()),
                                                )
                                                .on_click(cx.listener(
                                                    move |view, _, window, cx| {
                                                        if view.accepts_input(epoch, cx)
                                                            && let ParameterDraft::Domain(draft) =
                                                                &mut view.fields[index].draft
                                                        {
                                                            draft.move_row(
                                                                row, direction, window, cx,
                                                            );
                                                            view.fields[index].error = None;
                                                            view.fields[index].dirty = true;
                                                            cx.notify();
                                                        }
                                                    },
                                                ))
                                            },
                                        ),
                                    )
                                    .child(
                                        Button::new(gpui_kit::SharedString::from(format!(
                                            "domain-remove-{index}-{row}"
                                        )))
                                        .small()
                                        .ghost()
                                        .icon(IconName::Minus)
                                        .tooltip(translate("conversion.removeValue"))
                                        .disabled(busy)
                                        .on_click(
                                            cx.listener(move |view, _, window, cx| {
                                                if view.accepts_input(epoch, cx)
                                                    && let ParameterDraft::Domain(draft) =
                                                        &mut view.fields[index].draft
                                                {
                                                    draft.remove(row, window, cx);
                                                    view.fields[index].error = None;
                                                    view.fields[index].dirty = true;
                                                    cx.notify();
                                                }
                                            }),
                                        ),
                                    ),
                            )
                            .child(controls::hint(translate("conversion.code"), cx))
                            .child(Input::new(&inputs.code).small().disabled(busy))
                            .child(controls::hint(translate("conversion.label"), cx))
                            .child(Input::new(&inputs.label).small().disabled(busy))
                    })),
            )
            .when(
                duplicates && self.fields[index].error.is_none(),
                |content| {
                    content.child(
                        div()
                            .id(("domain-duplicates", index))
                            .role(gpui_kit::accesskit::Role::Alert)
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(translate("conversion.duplicateValues")),
                    )
                },
            )
            .child(self.domain_positive(index, draft, busy, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new(("domain-add", index))
                            .small()
                            .ghost()
                            .icon(IconName::Plus)
                            .label(translate("conversion.addValue"))
                            .disabled(busy || draft.len() >= ConversionDomain::MAX_VALUES)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::Domain(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.add(window, cx);
                                    view.fields[index].error = None;
                                    view.fields[index].dirty = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        controls::apply(("domain-apply", index), busy || duplicates)
                            .tooltip(translate("native.workbench.applyMapping"))
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
                                Button::new(gpui_kit::SharedString::from(format!(
                                    "domain-page-{index}-{direction}"
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
                                    move |view, _, window, cx| {
                                        if view.accepts_input(epoch, cx)
                                            && let ParameterDraft::Domain(draft) =
                                                &mut view.fields[index].draft
                                        {
                                            draft.show_page(
                                                page.saturating_add_signed(direction),
                                                window,
                                                cx,
                                            );
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
