//! Selected-node presentation; binding and drafts remain with GraphDetails.
use super::GraphDetails;
use gpui_kit::assets::IconName;
use gpui_kit::{Context, IntoElement, div, prelude::*};
use yss_graph_editor::projection::EditorNodeModel;
use yss_ui::inspector::InspectorHeader;

impl GraphDetails {
    pub(super) fn render_node_details(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled = self.graph().is_none_or(|graph| !graph.read(cx).can_edit());
        let header = match node
            .display
            .user_label
            .as_deref()
            .filter(|label| *label != &*node.display.title)
        {
            Some(label) => InspectorHeader::new(IconName::Workflow, label.to_owned())
                .detail(node.display.title.to_string()),
            None => InspectorHeader::new(IconName::Workflow, node.display.title.to_string()),
        };
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(header)
            .child(self.render_diagnostic_summary(node, cx))
            .child(self.parameters.clone())
            .child(self.render_ports(disabled, cx))
            .child(self.render_diagnostics(node, cx))
            .child(self.description.clone())
            .child(self.documentation.clone())
            .child(self.render_technical_details(node, cx))
    }
}
