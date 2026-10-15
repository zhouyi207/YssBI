//! Bounded code/label controls share the existing parameter commit and restore paths.
mod draft;
mod positive;
use super::{
    ParameterForm,
    field::{
        ParameterDraft,
        list::{RowCommand, row_number},
    },
};
pub(super) use draft::DomainDraft;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, input::Input};
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*};
use yss_data_contract::ConversionDomain;
use yss_ui::appearance::HEADER_BUTTON_SIZE;
use yss_ui::inspector::{self, Pager};
use yss_ui::text::translate;

impl ParameterForm {
    /// Codes and labels form two aligned columns; the header row reserves the same lanes.
    pub(super) fn render_domain(
        &self,
        index: usize,
        draft: &DomainDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let len = draft.len();
        let mut rows = div()
            .id(("domain-values", index))
            .max_h_64()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (row, inputs) in draft.visible() {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(row_number(row, cx))
                    .child(
                        Input::new(&inputs.code)
                            .small()
                            .flex_1()
                            .min_w_0()
                            .disabled(busy),
                    )
                    .child(
                        Input::new(&inputs.label)
                            .small()
                            .flex_1()
                            .min_w_0()
                            .disabled(busy),
                    )
                    .child(self.row_menu(
                        ("domain-actions", inputs.code.entity_id().as_u64() as usize),
                        (row, len),
                        true,
                        busy,
                        cx,
                        move |view, command, window, cx| {
                            let Some(field) = view.fields.get_mut(index) else {
                                return;
                            };
                            let ParameterDraft::Domain(draft) = &mut field.draft else {
                                return;
                            };
                            match command {
                                RowCommand::Move(direction) => {
                                    draft.move_row(row, direction, window, cx)
                                }
                                RowCommand::Remove => draft.remove(row, window, cx),
                            }
                            field.error = None;
                            field.dirty = true;
                            cx.notify();
                        },
                    )),
            );
        }
        let columns = div()
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(div().w_6().flex_shrink_0())
            .child(div().flex_1().min_w_0().child(translate("conversion.code")))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(translate("conversion.label")),
            )
            .child(div().w(HEADER_BUTTON_SIZE).flex_shrink_0());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(inspector::hint(translate("conversion.domainHelp"), cx))
            .when(len > 0, |content| {
                content.child(div().flex().flex_col().gap_1().child(columns).child(rows))
            })
            .child(self.domain_positive(index, draft, busy, cx))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        inspector::add_button(
                            ("domain-add", index),
                            translate("conversion.addValue"),
                        )
                        .disabled(busy || len >= ConversionDomain::MAX_VALUES)
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::Domain(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.add(window, cx);
                                    view.fields[index].error = None;
                                    view.fields[index].dirty = true;
                                    cx.notify();
                                }
                            },
                        )),
                    )
                    .child(div().flex_1())
                    .child(
                        Pager::new(("domain-pager", index), draft.page, draft.pages())
                            .disabled(busy)
                            .on_change(cx.listener(move |view, page: &usize, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::Domain(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.show_page(*page, window, cx);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        inspector::commit_button(
                            ("domain-apply", index),
                            translate("native.workbench.applyMapping"),
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
