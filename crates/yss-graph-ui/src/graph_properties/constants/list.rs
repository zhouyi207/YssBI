//! Bounded constant directory; row controls are created only for visited pages.
use super::*;
use gpui_kit::component::Disableable;
use gpui_kit::{AnyElement, IntoElement, div, prelude::*};
use yss_ui::inspector::{self, InspectorSection, Pager};

impl GraphProperties {
    pub(in crate::graph_properties) fn render_constants(
        &self,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self.generation;
        let page = self.constants_page;
        let pages = self.constants.len().div_ceil(PAGE_CONSTANTS).max(1);
        let mut section = InspectorSection::new(
            "graph-constants",
            yss_ui::text::translate("detail.constants.title"),
        )
        .count(self.constants.len())
        .collapsible(
            self.constants_open,
            cx.listener(|view, open: &bool, _, cx| {
                view.constants_open = *open;
                cx.notify();
            }),
        );
        if !self.constants_open {
            return section.into_any_element();
        }
        if self.constants.is_empty() {
            section = section.child(inspector::hint(
                yss_ui::text::t("native.workbench.constantsHint"),
                cx,
            ));
        }
        for (row, field) in self
            .constants
            .iter()
            .enumerate()
            .skip(page * PAGE_CONSTANTS)
            .take(PAGE_CONSTANTS)
        {
            section = section.child(self.render_constant(row, field, busy, cx));
        }
        section
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        inspector::add_button(
                            "add-graph-constant",
                            yss_ui::text::translate("detail.constants.add"),
                        )
                        .disabled(busy)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view.accepts_input(generation, cx) {
                                view.add_constant(cx);
                            }
                        })),
                    )
                    .child(div().flex_1())
                    .child(
                        Pager::new("constants-pager", page, pages).on_change(cx.listener(
                            |view, page: &usize, _, cx| {
                                view.constants_page = (*page)
                                    .min(view.constants.len().saturating_sub(1) / PAGE_CONSTANTS);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .into_any_element()
    }
}
