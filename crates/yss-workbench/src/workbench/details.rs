//! Dock and cross-editor selection routing; graph drafts stay in the retained graph owner.
#[cfg(test)]
pub(super) mod tests;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon,
    dock::{BasePanel, Panel, PanelEvent},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    WeakEntity, Window, div, prelude::*,
};
use std::sync::Arc;
use yss_data_ui::databases::DatabaseEditor;
use yss_graph_document::NodeId;
use yss_graph_editor::projection::EditorProjectionModel;
use yss_graph_ui::{GraphServices, canvas::GraphCanvas, details::GraphDetails};
use yss_node_protocol::{NodeTypeId, ParameterKey};
use yss_project::GraphEditVersion;
use yss_project_identity::ProjectInstanceId;
use yss_resource_ui::{documents::DocumentEditor, minds::MindCanvas};
use yss_results_ui::charts::ChartEditor;

pub struct DetailsPanel {
    graph: Entity<GraphDetails>,
    document: Option<WeakEntity<DocumentEditor>>,
    mind: Option<WeakEntity<MindCanvas>>,
    database: Option<WeakEntity<DatabaseEditor>>,
    chart: Option<WeakEntity<ChartEditor>>,
    log: Option<WeakEntity<super::logs::LogDetails>>,
    focus: FocusHandle,
    scroll: gpui_kit::ScrollHandle,
    _graph_observer: gpui_kit::Subscription,
    target_subscription: Option<gpui_kit::Subscription>,
}

impl DetailsPanel {
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
            document: None,
            mind: None,
            database: None,
            chart: None,
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
    pub fn document(&self) -> Option<Entity<DocumentEditor>> {
        self.document.as_ref().and_then(WeakEntity::upgrade)
    }
    pub fn mind(&self) -> Option<Entity<MindCanvas>> {
        self.mind.as_ref().and_then(WeakEntity::upgrade)
    }
    pub fn database(&self) -> Option<Entity<DatabaseEditor>> {
        self.database.as_ref().and_then(WeakEntity::upgrade)
    }
    pub fn chart(&self) -> Option<Entity<ChartEditor>> {
        self.chart.as_ref().and_then(WeakEntity::upgrade)
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.target_subscription = None;
        self.graph.update(cx, |graph, cx| graph.clear(cx));
        self.document = None;
        self.mind = None;
        self.database = None;
        self.chart = None;
        self.log = None;
        cx.notify();
    }

    pub fn set_document(&mut self, document: WeakEntity<DocumentEditor>, cx: &mut Context<Self>) {
        if self.document.as_ref() == Some(&document) && self.target_visible(cx) {
            return;
        }
        self.clear(cx);
        if let Some(editor) = document.upgrade() {
            let mut state = editor.read(cx).details_state().into_owned();
            self.target_subscription = Some(cx.observe(&editor, move |view, editor, cx| {
                let next = editor.read(cx).details_state();
                if state != next {
                    state = next.into_owned();
                    if view.target_visible(cx) {
                        cx.notify();
                    }
                }
            }));
        }
        self.document = Some(document);
    }
    pub fn set_mind(&mut self, mind: WeakEntity<MindCanvas>, cx: &mut Context<Self>) {
        if self.mind.as_ref() == Some(&mind) && self.target_visible(cx) {
            return;
        }
        self.clear(cx);
        if let Some(editor) = mind.upgrade() {
            let mut state = editor.read(cx).details_state().into_owned();
            self.target_subscription = Some(cx.observe(&editor, move |view, editor, cx| {
                let next = editor.read(cx).details_state();
                if state != next {
                    state = next.into_owned();
                    if view.target_visible(cx) {
                        cx.notify();
                    }
                }
            }));
        }
        self.mind = Some(mind);
    }
    pub fn set_database(&mut self, database: WeakEntity<DatabaseEditor>, cx: &mut Context<Self>) {
        if self.database.as_ref() == Some(&database) && self.target_visible(cx) {
            return;
        }
        self.clear(cx);
        if let Some(editor) = database.upgrade() {
            let mut state = editor.read(cx).details_state(cx).into_owned();
            self.target_subscription = Some(cx.observe(&editor, move |view, editor, cx| {
                let next = editor.read(cx).details_state(cx);
                if state != next {
                    state = next.into_owned();
                    if view.target_visible(cx) {
                        cx.notify();
                    }
                }
            }));
        }
        self.database = Some(database);
    }
    pub fn set_chart(&mut self, chart: WeakEntity<ChartEditor>, cx: &mut Context<Self>) {
        if self.chart.as_ref() == Some(&chart) && self.target_visible(cx) {
            return;
        }
        self.clear(cx);
        if let Some(editor) = chart.upgrade() {
            let mut state = editor.read(cx).details_state().into_owned();
            self.target_subscription = Some(cx.observe(&editor, move |view, editor, cx| {
                let next = editor.read(cx).details_state();
                if state != next {
                    state = next.into_owned();
                    if view.target_visible(cx) {
                        cx.notify();
                    }
                }
            }));
        }
        self.chart = Some(chart);
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
        self.document = None;
        self.mind = None;
        self.database = None;
        self.chart = None;
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
        let content = if let Some(document) = self.document() {
            document.read(cx).render_details(cx).into_any_element()
        } else if let Some(mind) = self.mind() {
            mind.update(cx, |mind, cx| mind.render_details(window, cx))
        } else if let Some(database) = self.database() {
            database.update(cx, |editor, cx| editor.render_details(window, cx))
        } else if let Some(chart) = self.chart() {
            chart.update(cx, |editor, cx| editor.render_details(cx))
        } else {
            yss_ui::appearance::empty_state(
                IconName::FileText,
                yss_ui::text::translate("detail.noSelection"),
                yss_ui::text::translate("detail.noSelectionHint"),
                cx,
            )
            .into_any_element()
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
        "details"
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
