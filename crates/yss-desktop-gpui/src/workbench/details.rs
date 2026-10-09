use super::controls;
mod connections;
mod description;
mod diagnostics;
mod documentation;
mod node;
mod ports;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, WeakEntity,
    Window, div, prelude::*,
};
use gpui_component::Icon;
use gpui_component::IconName;
use gpui_component::{
    ActiveTheme,
    dock::{BasePanel, Panel, PanelEvent},
    input::InputState,
};
use std::{collections::BTreeMap, sync::Arc};
use yss_graph_document::NodeId;
use yss_graph_editor::projection::{EditorNodeModel, EditorProjectionModel};
use yss_node_protocol::NodeTypeId;
use yss_project::GraphEditVersion;

use super::parameters::{ParameterChange, ParameterForm};
use crate::canvas::{GraphCanvas, GraphCommand};
use crate::{appearance, services::NativeServices};
use ports::PortField;

pub struct DetailsPanel {
    services: Arc<NativeServices>,
    connection_picker: Option<(
        yss_graph_document::PortAddress,
        Entity<gpui_component::list::ListState<connections::ConnectionPicker>>,
    )>,
    ports_open: [bool; 2],
    diagnostics_open: bool,
    diagnostics_page: usize,
    diagnostics_scroll: gpui::ScrollHandle,
    properties: Entity<super::graph_properties::GraphProperties>,
    documentation: Entity<documentation::NodeDocumentation>,
    description: Entity<description::NodeDescription>,
    node_definition: Option<NodeTypeId>,
    _properties_observer: gpui::Subscription,
    focus: FocusHandle,
    graph: Option<WeakEntity<GraphCanvas>>,
    document: Option<WeakEntity<crate::documents::DocumentEditor>>,
    mind: Option<WeakEntity<crate::minds::MindCanvas>>,
    database: Option<WeakEntity<crate::databases::DatabaseEditor>>,
    chart: Option<WeakEntity<crate::charts::ChartEditor>>,
    log: Option<WeakEntity<super::logs::LogDetails>>,
    projection: Option<Arc<EditorProjectionModel>>,
    selected: Vec<NodeId>,
    version: Option<GraphEditVersion>,
    label: Entity<InputState>,
    parameters: Entity<ParameterForm>,
    _parameter_subscription: gpui::Subscription,
    ports: Vec<PortField>,
    epoch: u64,
    error: Option<String>,
}

impl DetailsPanel {
    pub fn new(services: Arc<NativeServices>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let properties =
            cx.new(|_| super::graph_properties::GraphProperties::new(services.clone()));
        let documentation = cx.new(|_| documentation::NodeDocumentation::new(services.clone()));
        let description = cx.new(|_| description::NodeDescription::new(services.clone()));
        let properties_observer = cx.observe(&properties, |_, _, cx| cx.notify());
        let host = cx.entity();
        let owner = host.downgrade();
        let parameters = cx.new(|cx| {
            ParameterForm::new(
                properties.clone(),
                &host,
                move |cx| {
                    owner.upgrade().is_some_and(|owner| {
                        let view = owner.read(cx);
                        view.accepts_input(view.epoch, cx)
                    })
                },
                cx,
            )
        });
        let parameter_subscription =
            cx.subscribe(&parameters, |view, _, event: &ParameterChange, cx| {
                if !view.accepts_input(view.epoch, cx) {
                    return;
                }
                if let Some(node) = view.node() {
                    view.submit(
                        GraphCommand::SetParameter {
                            node_id: node.node_id,
                            key: event.key.clone(),
                            value: event.value.clone(),
                        },
                        cx,
                    );
                }
            });
        Self {
            services,
            connection_picker: None,
            ports_open: [false; 2],
            diagnostics_open: true,
            diagnostics_page: 0,
            diagnostics_scroll: gpui::ScrollHandle::new(),
            properties,
            documentation,
            description,
            node_definition: None,
            _properties_observer: properties_observer,
            focus: cx.focus_handle(),
            graph: None,
            document: None,
            mind: None,
            database: None,
            chart: None,
            log: None,
            projection: None,
            selected: vec![],
            version: None,
            label: cx.new(|cx| InputState::new(window, cx).placeholder("节点显示名称")),
            parameters,
            _parameter_subscription: parameter_subscription,
            ports: vec![],
            epoch: 0,
            error: None,
        }
    }

    pub fn graph(&self) -> Option<Entity<GraphCanvas>> {
        self.graph.as_ref().and_then(WeakEntity::upgrade)
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.clear_documentation(cx);
        self.description
            .update(cx, |description, cx| description.clear(cx));
        self.connection_picker = None;
        self.ports_open = [false; 2];
        self.diagnostics_open = true;
        self.diagnostics_page = 0;
        self.diagnostics_scroll.set_offset(gpui::Point::default());
        self.graph = None;
        self.document = None;
        self.mind = None;
        self.database = None;
        self.chart = None;
        self.log = None;
        self.projection = None;
        self.selected.clear();
        self.version = None;
        self.parameters
            .update(cx, |parameters, cx| parameters.clear(cx));
        self.ports.clear();
        self.properties
            .update(cx, |properties, cx| properties.clear(cx));
        self.epoch = self.epoch.wrapping_add(1);
        self.error = None;
        cx.notify();
    }

    pub fn document(&self) -> Option<Entity<crate::documents::DocumentEditor>> {
        self.document.as_ref().and_then(WeakEntity::upgrade)
    }

    pub(super) fn show_log(
        &mut self,
        log: &Entity<super::logs::LogDetails>,
        cx: &mut Context<Self>,
    ) {
        self.node_definition = None;
        self.refresh_node_documentation(cx);
        self.connection_picker = None;
        self.log = Some(log.downgrade());
        // Keep the editor binding and drafts, but invalidate menus from the hidden form.
        self.epoch = self.epoch.wrapping_add(1);
        cx.notify();
    }

    pub(super) fn clear_log(&mut self, cx: &mut Context<Self>) {
        if self.log.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn clear_log_if(&mut self, id: gpui::EntityId, cx: &mut Context<Self>) {
        if self.log.as_ref().is_some_and(|log| log.entity_id() == id) {
            self.clear_log(cx);
        }
    }

    pub fn set_document(
        &mut self,
        document: WeakEntity<crate::documents::DocumentEditor>,
        cx: &mut Context<Self>,
    ) {
        self.clear(cx);
        self.document = Some(document);
        cx.notify();
    }

    pub fn mind(&self) -> Option<Entity<crate::minds::MindCanvas>> {
        self.mind.as_ref().and_then(WeakEntity::upgrade)
    }
    pub fn database(&self) -> Option<Entity<crate::databases::DatabaseEditor>> {
        self.database.as_ref().and_then(WeakEntity::upgrade)
    }
    pub fn chart(&self) -> Option<Entity<crate::charts::ChartEditor>> {
        self.chart.as_ref().and_then(WeakEntity::upgrade)
    }
    pub fn set_chart(
        &mut self,
        chart: WeakEntity<crate::charts::ChartEditor>,
        cx: &mut Context<Self>,
    ) {
        self.clear(cx);
        self.chart = Some(chart);
        cx.notify();
    }
    pub fn set_database(
        &mut self,
        database: WeakEntity<crate::databases::DatabaseEditor>,
        cx: &mut Context<Self>,
    ) {
        self.clear(cx);
        self.database = Some(database);
        cx.notify();
    }
    pub fn set_mind(&mut self, mind: WeakEntity<crate::minds::MindCanvas>, cx: &mut Context<Self>) {
        self.clear(cx);
        self.mind = Some(mind);
        cx.notify();
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
        self.document = None;
        self.mind = None;
        self.database = None;
        self.chart = None;
        if self.selected == nodes
            && self.version == Some(version)
            && self
                .projection
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &projection))
            && self
                .graph
                .as_ref()
                .is_some_and(|current| current.entity_id() == graph.entity_id())
        {
            return;
        }
        let same_node = self.selected == nodes
            && self
                .graph
                .as_ref()
                .is_some_and(|current| current.entity_id() == graph.entity_id());
        self.connection_picker = None;
        if !same_node {
            self.ports_open = [false; 2];
            self.diagnostics_open = true;
            self.diagnostics_page = 0;
            self.diagnostics_scroll.set_offset(gpui::Point::default());
        }
        let old_label = self.node().map(|node| node.display.user_label.clone());
        self.graph = Some(graph.clone());
        self.selected = nodes;
        self.projection = Some(projection.clone());
        self.version = Some(version);
        self.error = None;
        self.epoch = self.epoch.wrapping_add(1);
        let node = self.node_in(&projection);
        let diagnostics_page = self.diagnostics_page.min(node.map_or(0, |node| {
            node.diagnostics.len().saturating_sub(1) / diagnostics::PAGE_DIAGNOSTICS
        }));
        if diagnostics_page != self.diagnostics_page {
            self.diagnostics_scroll.set_offset(gpui::Point::default());
        }
        self.diagnostics_page = diagnostics_page;
        if !same_node || old_label != node.as_ref().map(|node| node.display.user_label.clone()) {
            self.label.update(cx, |input, cx| {
                input.set_value(
                    node.as_ref()
                        .and_then(|node| node.display.user_label.as_deref())
                        .unwrap_or_default()
                        .to_owned(),
                    window,
                    cx,
                )
            });
        }
        self.parameters.update(cx, |parameters, cx| {
            parameters.install(
                node.as_ref()
                    .map_or(&[], |node| node.parameter_groups.as_ref()),
                node.as_ref().map_or_else(Vec::new, |node| node.diagnostics.iter()
                    .filter(|diagnostic| matches!(&diagnostic.location,
                        yss_graph_analysis_contract::DiagnosticLocation::Parameter {node_id, ..} if *node_id == node.node_id))
                    .cloned().collect()),
                same_node,
                window,
                cx,
            );
        });
        let mut ports: BTreeMap<_, _> = if same_node {
            std::mem::take(&mut self.ports)
                .into_iter()
                .map(|field| (field.model.address.clone(), field))
                .collect()
        } else {
            BTreeMap::new()
        };
        self.ports = node
            .iter()
            .flat_map(|node| node.ports.iter())
            .map(|port| {
                if let Some(mut field) = ports
                    .remove(&port.address)
                    .filter(|field| field.model == *port)
                {
                    field.model = port.clone();
                    field
                } else {
                    PortField::new(port.clone(), window, cx)
                }
            })
            .collect();
        self.install_connections();
        let needed = node.is_none() || self.parameters.read(cx).needs_constants();
        self.properties.update(cx, |properties, cx| {
            properties.set_graph(graph, needed, window, cx)
        });
        self.refresh_node_documentation(cx);
        self.refresh_node_description(window, cx);
        cx.notify();
    }

    fn node(&self) -> Option<&EditorNodeModel> {
        self.node_in(self.projection.as_ref()?)
    }

    fn node_in<'a>(&self, projection: &'a EditorProjectionModel) -> Option<&'a EditorNodeModel> {
        if self.selected.len() != 1 {
            return None;
        }
        projection
            .nodes
            .iter()
            .find(|node| node.node_id == self.selected[0])
    }

    fn accepts_input(&self, epoch: u64, cx: &App) -> bool {
        self.node_definition.is_none()
            && self.log.as_ref().and_then(WeakEntity::upgrade).is_none()
            && epoch == self.epoch
            && self.graph().is_some_and(|graph| {
                let graph = graph.read(cx);
                !graph.busy()
                    && self.version == Some(graph.graph.editing.version)
                    && self
                        .projection
                        .as_ref()
                        .is_some_and(|projection| Arc::ptr_eq(projection, &graph.graph.projection))
            })
    }

    fn submit(&self, command: GraphCommand, cx: &mut Context<Self>) {
        if let Some(graph) = self.graph.as_ref().and_then(WeakEntity::upgrade) {
            graph.update(cx, |view, cx| view.submit(command, self.version, cx));
        }
    }
}

impl Render for DetailsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = div()
            .id("details")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().background);
        if let Some(node_type) = &self.node_definition {
            return panel
                .child(self.render_node_definition(node_type, cx))
                .into_any_element();
        }
        if let Some(log) = self.log.as_ref().and_then(WeakEntity::upgrade) {
            return panel.child(log).into_any_element();
        }
        let content = if self.graph().is_some() {
            if let Some(node) = self.node() {
                self.render_node_details(node, cx).into_any_element()
            } else {
                self.properties.clone().into_any_element()
            }
        } else if let Some(document) = self.document() {
            document.read(cx).render_details(cx).into_any_element()
        } else if let Some(mind) = self.mind() {
            mind.update(cx, |mind, cx| mind.render_details(window, cx))
        } else if let Some(database) = self.database() {
            database.update(cx, |editor, cx| editor.render_details(window, cx))
        } else if let Some(chart) = self.chart() {
            chart.update(cx, |editor, cx| editor.render_details(cx))
        } else {
            appearance::empty_state(
                IconName::FileText,
                crate::text::translate("detail.noSelection"),
                crate::text::translate("detail.noSelectionHint"),
                cx,
            )
            .into_any_element()
        };
        panel
            .child(content)
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .m_3()
                        .p_3()
                        .rounded_md()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .into_any_element()
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
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::Inspector).size_3())
            .child("属性")
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
