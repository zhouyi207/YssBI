//! Read-only node diagnostics from the current editor projection.
use super::GraphDetails;
use crate::feedback::diagnostic_message;
use crate::problems::location;
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*};
use yss_graph_editor::projection::EditorNodeModel;
use yss_ui::inspector::{InspectorSection, Pager};
use yss_ui::text::translate;

pub(super) const PAGE_DIAGNOSTICS: usize = 50;

impl GraphDetails {
    pub(super) fn render_diagnostics(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(projection) = self
            .projection
            .as_ref()
            .filter(|_| !node.diagnostics.is_empty())
        else {
            return div().into_any_element();
        };
        let epoch = self.epoch;
        let pages = node.diagnostics.len().div_ceil(PAGE_DIAGNOSTICS);
        let rows = node
            .diagnostics
            .iter()
            .skip(self.diagnostics_page * PAGE_DIAGNOSTICS)
            .take(PAGE_DIAGNOSTICS)
            .map(|diagnostic| {
                diagnostic_message(diagnostic).when_some(
                    location::label(&diagnostic.location, projection, Some(node))
                        .filter(|label| label != location::node_title(node)),
                    |message, label| message.subject(label),
                )
            });
        InspectorSection::new("node-diagnostics", translate("detail.sections.diagnostics"))
            .count(node.diagnostics.len())
            .collapsible(
                self.diagnostics_open,
                cx.listener(move |view, open: &bool, _, cx| {
                    if view.epoch == epoch {
                        view.diagnostics_open = *open;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("node-diagnostic-rows")
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(pages > 1, |rows| {
                        rows.max_h_80()
                            .overflow_y_scroll()
                            .track_scroll(&self.diagnostics_scroll)
                    })
                    .children(rows),
            )
            .when(pages > 1, |section| {
                section.child(div().flex().justify_end().child(
                    Pager::new("node-diagnostics-pager", self.diagnostics_page, pages).on_change(
                        cx.listener(move |view, page: &usize, _, cx| {
                            if view.epoch == epoch {
                                view.diagnostics_page = *page;
                                view.diagnostics_scroll
                                    .set_offset(gpui_kit::Point::default());
                                cx.notify();
                            }
                        }),
                    ),
                ))
            })
            .into_any_element()
    }
}
