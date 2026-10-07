//! A drag is a graph-scoped intent; its preview has no document or mutation authority.
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, Icon, IconName};
use yss_graph_document::{ConstantId, GraphResourcePath};
use yss_project::GraphEditVersion;
use yss_project_identity::ProjectInstanceId;

#[derive(Clone)]
pub(crate) struct ConstantDrag {
    pub project: ProjectInstanceId,
    pub path: GraphResourcePath,
    pub version: GraphEditVersion,
    pub id: ConstantId,
    pub name: String,
}

impl Render for ConstantDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .child(Icon::new(IconName::Asterisk))
            .child(self.name.clone())
    }
}
