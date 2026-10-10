//! Native documentation presentation; queries and selection stay with the owner.
use super::{DocumentationState, NodeDocumentation};
use crate::text::translate;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    text::{TextView, TextViewStyle},
};
use gpui_kit::{Context, IntoElement, Render, Window, div, prelude::*, px, rems};
use yss_node_protocol::NodeTypeId;

impl Render for NodeDocumentation {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = crate::text::locale();
        if let Some(target) = &self.target
            && target.language != language
        {
            let mut target = target.clone();
            target.language = language;
            self.set_target(Some(target), cx);
        }
        if matches!(self.state, DocumentationState::Missing) {
            return div().into_any_element();
        }
        Collapsible::new()
            .w_full()
            .min_w_0()
            .open(self.expanded)
            .child(
                Button::new("node-documentation-toggle")
                    .small()
                    .ghost()
                    .w_full()
                    .h_7()
                    .px_2()
                    .rounded_none()
                    .justify_start()
                    .gap_1p5()
                    .bg(cx.theme().muted.opacity(0.6))
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.expanded = !view.expanded;
                        cx.notify();
                    }))
                    .icon(
                        Icon::new(if self.expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size_3(),
                    )
                    .label(translate("detail.nodeDoc.documentation")),
            )
            .when(self.expanded, |section| {
                section.content(
                    div()
                        .w_full()
                        .min_w_0()
                        .py_1p5()
                        .px_2()
                        .text_size(px(13.))
                        .child(self.render_body(cx)),
                )
            })
            .into_any_element()
    }
}

impl NodeDocumentation {
    fn render_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        match &self.state {
            DocumentationState::Missing => div().into_any_element(),
            DocumentationState::Loading => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(translate("native.workbench.loadingNodeDocumentation"))
                .into_any_element(),
            DocumentationState::Ready(markdown) => {
                let mut table = gpui_kit::StyleRefinement::default();
                table.overflow.x = Some(gpui_kit::Overflow::Scroll);
                TextView::new(markdown)
                    .plugin(crate::markdown::Markdown::default())
                    .scrollable(false)
                    .style(
                        TextViewStyle::default()
                            .paragraph_gap(rems(0.5))
                            .heading_font_size(|level, _| {
                                px(match level {
                                    1 => 20.,
                                    2 => 17.,
                                    3 => 15.,
                                    _ => 14.,
                                })
                            })
                            .table(table),
                    )
                    .into_any_element()
            }
            DocumentationState::Failed => div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .child(
                    div()
                        .flex_1()
                        .child(translate("native.workbench.nodeDocumentationFailed")),
                )
                .child(
                    Button::new("retry-node-documentation")
                        .small()
                        .ghost()
                        .label(translate("common.retry"))
                        .on_click(cx.listener(|view, _, _, cx| view.read(cx))),
                )
                .into_any_element(),
        }
    }
}

impl crate::workbench::details::DetailsPanel {
    pub(in crate::workbench::details) fn render_node_definition(
        &self,
        node_type: &NodeTypeId,
        cx: &gpui_kit::App,
    ) -> impl IntoElement + use<> {
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .min_h_7()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .w(gpui_kit::relative(0.4))
                            .child(translate("detail.fields.type")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child(node_type.as_str().to_owned()),
                    ),
            )
            .child(self.documentation.clone())
    }
}
