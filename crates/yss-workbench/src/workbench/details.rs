//! Dock and cross-editor selection routing; graph drafts stay in the retained graph owner.
#[cfg(test)]
pub(super) mod tests;
use super::EditorPanel;
use super::files::{FilePanel, WeakFilePanel};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon,
    dock::{BasePanel, Panel, PanelEvent},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, WeakEntity, Window, div, prelude::*,
};
use std::sync::Arc;
use yss_data_ui::databases::DatabaseEditor;
use yss_graph_document::NodeId;
use yss_graph_editor::projection::EditorProjectionModel;
use yss_graph_ui::{GraphServices, canvas::GraphCanvas, details::GraphDetails};
use yss_node_protocol::{NodeTypeId, ParameterKey};
use yss_project::GraphEditVersion;
use yss_project_identity::ProjectInstanceId;

/// An editor that renders its own details; graph selections use the graph inspector instead.
#[derive(Clone)]
pub(crate) enum DetailsTarget {
    File(WeakFilePanel),
    Database(WeakEntity<DatabaseEditor>),
}

impl DetailsTarget {
    fn entity_id(&self) -> EntityId {
        match self {
            Self::File(file) => file.entity_id(),
            Self::Database(database) => database.entity_id(),
        }
    }

    pub(in crate::workbench) fn upgrade(&self) -> Option<EditorPanel> {
        Some(match self {
            Self::File(file) => EditorPanel::File(file.upgrade()?),
            Self::Database(database) => EditorPanel::Database(database.upgrade()?),
        })
    }
}

pub struct DetailsPanel {
    graph: Entity<GraphDetails>,
    target: Option<DetailsTarget>,
    log: Option<WeakEntity<super::logs::LogDetails>>,
    focus: FocusHandle,
    scroll: gpui_kit::ScrollHandle,
    _graph_observer: gpui_kit::Subscription,
    target_subscription: Option<gpui_kit::Subscription>,
}

impl DetailsPanel {
    pub const PANEL_NAME: &'static str = "details";

    pub fn new(services: Arc<GraphServices>, window: &Window, cx: &mut Context<Self>) -> Self {
        let graph = cx.new(|cx| GraphDetails::new(services, window, cx));
        let observer = cx.observe(&graph, |view: &mut Self, _, cx| {
            if view.graph_visible(cx) {
                cx.notify();
            }
        });
        let focus = graph.read(cx).focus_handle(cx);
        Self {
            graph,
            target: None,
            log: None,
            focus,
            scroll: gpui_kit::ScrollHandle::new(),
            _graph_observer: observer,
            target_subscription: None,
        }
    }

    pub fn graph(&self, cx: &App) -> Option<Entity<GraphCanvas>> {
        self.graph.read(cx).graph()
    }
    pub(crate) fn target_id(&self) -> Option<EntityId> {
        self.target.as_ref().map(DetailsTarget::entity_id)
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.target_subscription = None;
        self.graph.update(cx, |graph, cx| graph.clear(cx));
        self.target = None;
        self.log = None;
        cx.notify();
    }

    pub(crate) fn set_target(&mut self, target: DetailsTarget, cx: &mut Context<Self>) {
        if self.target_id() == Some(target.entity_id()) && self.target_visible(cx) {
            return;
        }
        self.clear(cx);
        macro_rules! watch {
            ($editor:expr) => {{
                let mut state = $editor.read(cx).details_state().into_owned();
                cx.observe($editor, move |view, editor, cx| {
                    let next = editor.read(cx).details_state();
                    if state != next {
                        state = next.into_owned();
                        if view.target_visible(cx) {
                            cx.notify();
                        }
                    }
                })
            }};
        }
        self.target_subscription = match target.upgrade() {
            Some(EditorPanel::File(FilePanel::Document(editor))) => Some(watch!(&editor)),
            Some(EditorPanel::File(FilePanel::Mind(editor))) => Some(watch!(&editor)),
            Some(EditorPanel::File(FilePanel::Chart(editor))) => Some(watch!(&editor)),
            // A database's details state also reads its grid, so it needs the app context.
            Some(EditorPanel::Database(editor)) => {
                let mut state = editor.read(cx).details_state(cx).into_owned();
                Some(cx.observe(&editor, move |view, editor, cx| {
                    let next = editor.read(cx).details_state(cx);
                    if state != next {
                        state = next.into_owned();
                        if view.target_visible(cx) {
                            cx.notify();
                        }
                    }
                }))
            }
            Some(EditorPanel::Graph(_)) | None => None,
        };
        self.target = Some(target);
    }
    fn target_visible(&self, cx: &App) -> bool {
        self.log.as_ref().and_then(WeakEntity::upgrade).is_none()
            && !self.graph.read(cx).showing_node_definition()
    }

    fn graph_visible(&self, cx: &App) -> bool {
        self.graph.read(cx).showing_node_definition()
            || (self.log.as_ref().and_then(WeakEntity::upgrade).is_none()
                && self.graph(cx).is_some())
    }

    pub fn set_selection(
        &mut self,
        graph: WeakEntity<GraphCanvas>,
        nodes: Vec<NodeId>,
        projection: Arc<EditorProjectionModel>,
        version: GraphEditVersion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.target_subscription = None;
        self.target = None;
        self.graph.update(cx, |details, cx| {
            details.set_selection(graph, nodes, projection, version, window, cx)
        });
    }

    pub(super) fn show_log(
        &mut self,
        log: &Entity<super::logs::LogDetails>,
        cx: &mut Context<Self>,
    ) {
        if self
            .log
            .as_ref()
            .is_some_and(|current| current.entity_id() == log.entity_id())
            && !self.graph.read(cx).showing_node_definition()
        {
            return;
        }
        let weak = log.downgrade();
        self.log = Some(weak.clone());
        self.graph.update(cx, |graph, cx| {
            graph.hide_for_log(move |_| weak.upgrade().is_some(), cx)
        });
        cx.notify();
    }
    pub(super) fn clear_log(&mut self, cx: &mut Context<Self>) {
        if self.log.take().is_some() {
            self.graph.update(cx, |graph, cx| graph.clear_log(cx));
            cx.notify();
        }
    }
    pub(super) fn clear_log_if(&mut self, id: gpui_kit::EntityId, cx: &mut Context<Self>) {
        if self.log.as_ref().is_some_and(|log| log.entity_id() == id) {
            self.clear_log(cx);
        }
    }

    pub(super) fn reveal_parameter(
        &mut self,
        node_id: NodeId,
        key: &ParameterKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.graph.update(cx, |graph, cx| {
            graph.reveal_parameter(node_id, key, window, cx)
        });
    }
    pub(super) fn show_node_definition(
        &mut self,
        project: ProjectInstanceId,
        node_type: NodeTypeId,
        cx: &mut Context<Self>,
    ) {
        self.log = None;
        self.graph.update(cx, |graph, cx| {
            graph.show_node_definition(project, node_type, cx)
        });
        cx.notify();
    }
    pub(super) fn show_node_properties(&mut self, cx: &mut Context<Self>) {
        if !self.graph.read(cx).showing_node_definition() {
            return;
        }
        self.graph
            .update(cx, |graph, cx| graph.show_node_properties(cx));
        cx.notify();
    }
}

impl Render for DetailsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.graph_visible(cx) {
            return self.graph.clone().into_any_element();
        }
        let panel = div()
            .id("details")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(cx.theme().background);
        if let Some(log) = self.log.as_ref().and_then(WeakEntity::upgrade) {
            return panel.child(log).into_any_element();
        }
        let content = match self.target.as_ref().and_then(DetailsTarget::upgrade) {
            Some(EditorPanel::File(FilePanel::Document(editor))) => {
                editor.read(cx).render_details(cx).into_any_element()
            }
            Some(EditorPanel::File(FilePanel::Mind(editor))) => {
                editor.update(cx, |editor, cx| editor.render_details(window, cx))
            }
            Some(EditorPanel::File(FilePanel::Chart(editor))) => {
                editor.update(cx, |editor, cx| editor.render_details(cx))
            }
            Some(EditorPanel::Database(editor)) => {
                editor.update(cx, |editor, cx| editor.render_details(window, cx))
            }
            Some(EditorPanel::Graph(_)) | None => yss_ui::appearance::empty_state(
                IconName::FileText,
                yss_ui::text::translate("detail.noSelection"),
                yss_ui::text::translate("detail.noSelectionHint"),
                cx,
            )
            .into_any_element(),
        };
        panel.child(content).into_any_element()
    }
}
impl EventEmitter<PanelEvent> for DetailsPanel {}
impl Focusable for DetailsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for DetailsPanel {
    fn panel_name(&self) -> &'static str {
        Self::PANEL_NAME
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for DetailsPanel {
    fn tab_name(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(yss_ui::text::t("native.workbench.properties").into())
    }
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::Inspector).size_3())
            .child(yss_ui::text::t("native.workbench.properties"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
