mod connections;
mod description;
mod diagnostics;
mod documentation;
mod node;
mod ports;

use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, WeakEntity, Window, div,
    prelude::*,
};
use std::{collections::BTreeMap, sync::Arc};
use yss_graph_document::NodeId;
use yss_graph_editor::projection::{EditorNodeModel, EditorProjectionModel};
use yss_node_protocol::NodeTypeId;
use yss_project::GraphEditVersion;

use super::parameters::{ParameterChange, ParameterForm};
use crate::GraphServices;
use crate::canvas::{GraphCanvas, GraphCommand};
use ports::PortField;
use yss_ui::appearance;

type HiddenCheck = Box<dyn Fn(&App) -> bool>;

pub struct GraphDetails {
    services: Arc<GraphServices>,
    connection_picker: Option<(
        yss_graph_document::PortAddress,
        Entity<gpui_kit::component::list::ListState<connections::ConnectionPicker>>,
    )>,
    ports_open: [bool; 2],
    diagnostics_open: bool,
    diagnostics_page: usize,
    diagnostics_scroll: gpui_kit::ScrollHandle,
    scroll: gpui_kit::ScrollHandle,
    properties: Entity<super::graph_properties::GraphProperties>,
    documentation: Entity<documentation::NodeDocumentation>,
    description: Entity<description::NodeDescription>,
    node_definition: Option<NodeTypeId>,
    _properties_observer: gpui_kit::Subscription,
    focus: FocusHandle,
    graph: Option<WeakEntity<GraphCanvas>>,
    hidden: Option<HiddenCheck>,
    projection: Option<Arc<EditorProjectionModel>>,
    selected: Vec<NodeId>,
    version: Option<GraphEditVersion>,
    parameters: Entity<ParameterForm>,
    _parameter_subscription: gpui_kit::Subscription,
    ports: Vec<PortField>,
    epoch: u64,
    notifications: yss_ui::notifications::Notifications,
}

impl GraphDetails {
    pub fn new(services: Arc<GraphServices>, window: &Window, cx: &mut Context<Self>) -> Self {
        let properties =
            cx.new(|_| super::graph_properties::GraphProperties::new(services.clone(), window));
        let documentation =
            cx.new(|_| documentation::NodeDocumentation::new(services.clone(), window));
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
                window,
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
            diagnostics_scroll: gpui_kit::ScrollHandle::new(),
            scroll: gpui_kit::ScrollHandle::new(),
            properties,
            documentation,
            description,
            node_definition: None,
            _properties_observer: properties_observer,
            focus: cx.focus_handle(),
            graph: None,
            hidden: None,
            projection: None,
            selected: vec![],
            version: None,
            parameters,
            _parameter_subscription: parameter_subscription,
            ports: vec![],
            epoch: 0,
            notifications: yss_ui::notifications::Notifications::new(window),
        }
    }

    pub fn reveal_parameter(
        &mut self,
        node_id: NodeId,
        key: &yss_node_protocol::ParameterKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.node().is_some_and(|node| node.node_id == node_id)
            && self.accepts_input(self.epoch, cx)
        {
            self.parameters.update(cx, |form, cx| {
                form.reveal(key, self.scroll.clone(), window, cx)
            });
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
        self.diagnostics_scroll
            .set_offset(gpui_kit::Point::default());
        self.graph = None;
        self.hidden = None;
        self.projection = None;
        self.selected.clear();
        self.version = None;
        self.parameters
            .update(cx, |parameters, cx| parameters.clear(cx));
        self.ports.clear();
        self.properties
            .update(cx, |properties, cx| properties.clear(cx));
        self.epoch = self.epoch.wrapping_add(1);
        cx.notify();
    }

    /// Retain graph bindings and drafts while invalidating hidden-form menus.
    pub fn hide_for_log(
        &mut self,
        visible: impl Fn(&App) -> bool + 'static,
        cx: &mut Context<Self>,
    ) {
        self.node_definition = None;
        self.refresh_node_documentation(cx);
        self.connection_picker = None;
        self.hidden = Some(Box::new(visible));
        self.epoch = self.epoch.wrapping_add(1);
        cx.notify();
    }

    pub fn clear_log(&mut self, cx: &mut Context<Self>) {
        if self.hidden.take().is_some() {
            cx.notify();
        }
    }

    pub fn showing_node_definition(&self) -> bool {
        self.node_definition.is_some()
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
            self.diagnostics_scroll
                .set_offset(gpui_kit::Point::default());
        }
        self.graph = Some(graph.clone());
        self.selected = nodes;
        self.projection = Some(projection.clone());
        self.version = Some(version);
        self.epoch = self.epoch.wrapping_add(1);
        let node = self.node_in(&projection);
        let diagnostics_page = self.diagnostics_page.min(node.map_or(0, |node| {
            node.diagnostics.len().saturating_sub(1) / diagnostics::PAGE_DIAGNOSTICS
        }));
        if diagnostics_page != self.diagnostics_page {
            self.diagnostics_scroll
                .set_offset(gpui_kit::Point::default());
        }
        self.diagnostics_page = diagnostics_page;
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
                    .filter(|field| field.accepts_projection(port))
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
            && !self.hidden.as_ref().is_some_and(|visible| visible(cx))
            && epoch == self.epoch
            && self.graph().is_some_and(|graph| {
                let graph = graph.read(cx);
                graph.can_edit()
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

impl Focusable for GraphDetails {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GraphDetails {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = div()
            .id("details")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(cx.theme().background);
        if let Some(node_type) = &self.node_definition {
            return panel
                .child(self.render_node_definition(node_type, cx))
                .into_any_element();
        }
        let content = if self.graph().is_some() {
            if let Some(node) = self.node() {
                self.render_node_details(node, cx).into_any_element()
            } else {
                self.properties.clone().into_any_element()
            }
        } else {
            appearance::empty_state(
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
