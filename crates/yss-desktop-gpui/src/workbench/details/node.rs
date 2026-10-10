//! Selected-node presentation; binding and drafts remain with DetailsPanel.
use super::DetailsPanel;
use crate::{appearance, canvas::GraphCommand};
use gpui::{Context, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit_assets::IconName;
use yss_graph_editor::{EditorGraphMutation, projection::EditorNodeModel};

impl DetailsPanel {
    pub(super) fn render_node_details(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled = self.graph().is_none_or(|graph| !graph.read(cx).can_edit());
        let node_id = node.node_id;
        let epoch = self.epoch;
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Workflow)
                                    .size_4()
                                    .text_color(gpui::rgb(appearance::BLUE)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(
                                        node.display
                                            .user_label
                                            .as_deref()
                                            .unwrap_or(&node.display.title)
                                            .to_owned(),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::text::t("settings.models.displayName")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Input::new(&self.label)
                                    .small()
                                    .flex_1()
                                    .min_w_0()
                                    .disabled(disabled),
                            )
                            .child(
                                Button::new("node-label")
                                    .small()
                                    .ghost()
                                    .icon(IconName::Check)
                                    .tooltip(crate::text::t("native.workbench.applyName"))
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if !view.accepts_input(epoch, cx) {
                                            return;
                                        }
                                        let label = view.label.read(cx).value().to_string();
                                        view.submit(
                                            GraphCommand::Edit(EditorGraphMutation::SetNodeLabel {
                                                node_id,
                                                label: (!label.is_empty()).then_some(label),
                                            }),
                                            cx,
                                        );
                                    })),
                            ),
                    ),
            )
            .children(
                self.graph()
                    .and_then(|graph| graph.read(cx).command_error().map(str::to_owned))
                    .map(|error| {
                        div()
                            .id("node-command-error")
                            .role(gpui::accesskit::Role::Alert)
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
