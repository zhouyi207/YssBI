//! Selected-node presentation; binding and drafts remain with GraphDetails.
use super::GraphDetails;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon};
use gpui_kit::{Context, IntoElement, div, prelude::*};
use yss_graph_editor::projection::EditorNodeModel;

impl GraphDetails {
    pub(super) fn render_node_details(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled = self.graph().is_none_or(|graph| !graph.read(cx).can_edit());
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(yss_ui::appearance::HEADER_HEIGHT)
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Workflow)
                                    .size_4()
                                    .text_color(cx.theme().primary),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(
                                        node.display
                                            .user_label
                                            .as_deref()
                                            .unwrap_or(&node.display.title)
                                            .to_owned(),
                                    ),
                            ),
                    ),
            )
            .children(
                self.graph()
                    .and_then(|graph| graph.read(cx).command_error().map(str::to_owned))
                    .map(|error| {
                        div()
                            .id("node-command-error")
                            .role(gpui_kit::accesskit::Role::Alert)
                            .px_4()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(error)
                    }),
            )
            .child(self.parameters.clone())
            .child(self.render_diagnostics(node, cx))
            .child(self.description.clone())
            .child(self.render_ports(disabled, cx))
            .child(self.documentation.clone())
    }
}
