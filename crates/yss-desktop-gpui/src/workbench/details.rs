use super::controls;
mod connections;
mod description;
mod documentation;
mod domain;
mod parameters;
mod ports;
mod relational;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, WeakEntity,
    Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    input::{Input, InputState},
};
use gpui_component::{Disableable, Icon, IconName, Sizable};
use std::sync::Arc;
use yss_graph_document::NodeId;
use yss_graph_editor::{
    EditorGraphMutation,
    projection::{EditorNodeModel, EditorProjectionModel},
};
use yss_node_protocol::NodeTypeId;
use yss_project::GraphEditVersion;

use crate::canvas::{GraphCanvas, GraphCommand};
use crate::{appearance, assets::NativeIcon, services::NativeServices};
use parameters::ParameterField;
use ports::PortField;

pub struct DetailsPanel {
    services: Arc<NativeServices>,
    connection_picker: Option<(
        yss_graph_document::PortAddress,
        Entity<gpui_component::list::ListState<connections::ConnectionPicker>>,
    )>,
    ports_open: [bool; 2],
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
    fields: Vec<ParameterField>,
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
        Self {
            services,
            connection_picker: None,
            ports_open: [false; 2],
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
            fields: vec![],
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
        self.graph = None;
        self.document = None;
        self.mind = None;
        self.database = None;
        self.chart = None;
        self.log = None;
        self.projection = None;
        self.selected.clear();
        self.version = None;
        self.fields.clear();
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
        }
        let old_label = self.node().map(|node| node.display.user_label.clone());
        self.graph = Some(graph.clone());
        self.selected = nodes;
        self.projection = Some(projection);
        self.version = Some(version);
        self.error = None;
        self.epoch = self.epoch.wrapping_add(1);
        let node = self.node().cloned();
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
        let mut fields = if same_node {
            std::mem::take(&mut self.fields)
                .into_iter()
                .map(|field| (field.model.key.clone(), field))
                .collect::<std::collections::BTreeMap<_, _>>()
        } else {
            std::collections::BTreeMap::new()
        };
        self.fields = node
            .iter()
            .flat_map(|node| node.parameter_groups.iter())
            .flat_map(|group| group.parameters.iter())
            .map(|parameter| {
                if let Some(mut field) = fields
                    .remove(&parameter.key)
                    .filter(|field| field.accepts_projection(parameter))
                {
                    field.model = parameter.clone();
                    field
                } else {
                    ParameterField::new(parameter.clone(), window, cx)
                }
            })
            .collect();
        let mut ports = if same_node {
            std::mem::take(&mut self.ports)
        } else {
            vec![]
        };
        self.ports = node
            .iter()
            .flat_map(|node| node.ports.iter())
            .map(|port| {
                if let Some(index) = ports.iter().position(|field| field.model == *port) {
                    ports.remove(index)
                } else {
                    PortField::new(port.clone(), window, cx)
                }
            })
            .collect();
        self.install_connections();
        let needed = node.is_none()
            || self
                .fields
                .iter()
                .any(|field| matches!(field.draft, parameters::ParameterDraft::Constant));
        self.properties.update(cx, |properties, cx| {
            properties.set_graph(graph, needed, window, cx)
        });
        self.refresh_node_documentation(cx);
        self.refresh_node_description(window, cx);
        cx.notify();
    }

    fn node(&self) -> Option<&EditorNodeModel> {
        if self.selected.len() != 1 {
            return None;
        }
        self.projection
            .as_ref()?
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

    fn apply_parameter(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.fields[index].value(cx) {
            Ok(value) => self.commit_parameter(index, value, cx),
            Err(error) => {
                self.fields[index].error = Some(error);
                cx.notify();
            }
        }
    }

    fn commit_parameter(&mut self, index: usize, value: serde_json::Value, cx: &mut Context<Self>) {
        let Some(node) = self.node() else {
            return;
        };
        let node_id = node.node_id;
        let field = &mut self.fields[index];
        field.error = None;
        if field.model.value.as_ref() == Some(&value) && !value.is_null() {
            cx.notify();
            return;
        }
        let key = field.model.key.clone();
        self.submit(
            GraphCommand::SetParameter {
                node_id,
                key,
                value,
            },
            cx,
        );
        cx.notify();
    }
}

impl DetailsPanel {
    fn render_node_details(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let busy = self.graph().is_some_and(|graph| graph.read(cx).busy());
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
                                Icon::new(NativeIcon::Graph)
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
                            .child("显示名称"),
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
                                    .disabled(busy),
                            )
                            .child(
                                Button::new("node-label")
                                    .small()
                                    .ghost()
                                    .icon(IconName::Check)
                                    .tooltip("应用名称")
                                    .disabled(busy)
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
            .child(self.render_parameters(busy, cx))
            .child(self.description.clone())
            .child(self.render_ports(busy, cx))
            .child(self.documentation.clone())
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
