//! Native documentation presentation; queries and selection stay with the owner.
use super::{DocumentationState, NodeDocumentation};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme,
    text::{TextView, TextViewStyle},
};
use gpui_kit::{Context, IntoElement, Render, Window, div, prelude::*, rems};
use yss_node_protocol::NodeTypeId;
use yss_ui::inspector::{self, InspectorHeader, InspectorSection};
use yss_ui::text::translate;

impl Render for NodeDocumentation {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = yss_ui::text::locale();
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
        InspectorSection::new(
            "node-documentation",
            translate("detail.nodeDoc.documentation"),
        )
        .collapsible(
            self.expanded,
            cx.listener(|view, open: &bool, _, cx| {
                view.expanded = *open;
                cx.notify();
            }),
        )
        .when(self.expanded, |section| section.child(self.render_body(cx)))
        .into_any_element()
    }
}

impl NodeDocumentation {
    fn render_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        match &self.state {
            DocumentationState::Missing => div().into_any_element(),
            DocumentationState::Loading => {
                inspector::hint(translate("native.workbench.loadingNodeDocumentation"), cx)
                    .into_any_element()
            }
            DocumentationState::Ready(markdown) => {
                let mut table = gpui_kit::StyleRefinement::default();
                table.overflow.x = Some(gpui_kit::Overflow::Scroll);
                // Headings step down from the UI font, so documentation never outranks the
                // inspector's own titles.
                let base = cx.theme().font_size;
                div()
                    .w_full()
                    .min_w_0()
                    .text_sm()
                    .child(
                        TextView::new(markdown)
                            .plugin(yss_ui::markdown::Markdown::default())
                            .scrollable(false)
                            .style(
                                TextViewStyle::default()
                                    .paragraph_gap(rems(0.5))
                                    .heading_font_size(move |level, _| {
                                        base * match level {
                                            1 => 1.,
                                            2 => 0.9375,
                                            _ => 0.875,
                                        }
                                    })
                                    .table(table),
                            ),
                    )
                    .into_any_element()
            }
            DocumentationState::Failed => div().into_any_element(),
        }
    }
}

impl crate::details::GraphDetails {
    pub(in crate::details) fn render_node_definition(
        &self,
        node_type: &NodeTypeId,
        _: &gpui_kit::App,
    ) -> impl IntoElement + use<> {
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(InspectorHeader::new(
                IconName::BookOpen,
                node_type.as_str().to_owned(),
            ))
            .child(self.documentation.clone())
    }
}
