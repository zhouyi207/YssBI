//! Bounded constant directory; row controls are created only for visited pages.
use super::*;
use crate::workbench::controls;
use gpui::{AnyElement, IntoElement, div, prelude::*};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
};
use gpui_kit_assets::IconName;

impl GraphProperties {
    pub(in crate::workbench::graph_properties) fn render_constants(
        &self,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self.generation;
        let page = self.constants_page;
        let pages = self.constants.len().div_ceil(PAGE_CONSTANTS).max(1);
        Collapsible::new()
            .open(self.constants_open)
            .child(
                Button::new("constants-toggle")
                    .small()
                    .ghost()
                    .w_full()
                    .label(crate::text::translate("detail.constants.title"))
                    .icon(if self.constants_open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.constants_open = !view.constants_open;
                        cx.notify();
                    })),
            )
            .when(self.constants_open, |section| {
                section.content(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div().flex().items_center().gap_1().justify_end().child(
                                Button::new("add-graph-constant")
                                    .small()
                                    .ghost()
                                    .icon(IconName::Plus)
                                    .label(crate::text::translate("detail.constants.add"))
                                    .tooltip(crate::text::translate(
                                        "panel.assistantToolNames.create_constants",
                                    ))
                                    .disabled(busy)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.accepts_input(generation, cx) {
                                            view.add_constant(cx);
                                        }
                                    })),
                            ),
                        )
                        .when(self.constants.is_empty(), |view| {
                            view.child(controls::hint(
                                crate::text::translate("native.workbench.constantsHint"),
                                cx,
                            ))
                        })
                        .when(pages > 1, |view| {
                            view.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        Button::new("constants-previous")
                                            .small()
                                            .ghost()
                                            .icon(IconName::ChevronLeft)
                                            .tooltip(crate::text::translate(
                                                "databaseEditor.previousPage",
                                            ))
                                            .disabled(page == 0)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.constants_page =
                                                    view.constants_page.saturating_sub(1);
                                                cx.notify();
                                            })),
                                    )
                                    .child(div().text_xs().child(format!("{} / {pages}", page + 1)))
                                    .child(
                                        Button::new("constants-next")
                                            .small()
                                            .ghost()
                                            .icon(IconName::ChevronRight)
                                            .tooltip(crate::text::translate(
                                                "databaseEditor.nextPage",
                                            ))
                                            .disabled(page + 1 >= pages)
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.constants_page = (view.constants_page + 1)
                                                    .min(
                                                        view.constants.len().saturating_sub(1)
                                                            / PAGE_CONSTANTS,
                                                    );
                                                cx.notify();
                                            })),
                                    ),
                            )
                        })
                        .children(
                            self.constants
                                .iter()
                                .enumerate()
                                .skip(page * PAGE_CONSTANTS)
                                .take(PAGE_CONSTANTS)
                                .map(|(row, field)| self.render_constant(row, field, busy, cx)),
                        ),
                )
            })
            .into_any_element()
    }
}
