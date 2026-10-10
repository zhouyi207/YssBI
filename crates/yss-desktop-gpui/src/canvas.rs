mod authoring;
mod catalog;
mod clipboard;
mod commands;
mod connections;
mod constant_drag;
mod execution;
mod geometry;
mod menu;
mod navigation;
mod nodes;
mod palette;
mod ports;
mod presentation;
mod render;
mod selection;
mod toolbar;

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent, Window, point, px,
};
use gpui_component::dock::{BasePanel, Panel, PanelEvent};
use yss_application::activity_panel::ActivityPanelDocument;
use yss_graph_document::{ConnectionId, NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;
use yss_graph_editor::{EditorGraphMutation, NodePositionMutation};
use yss_project::GraphEditVersion;

use crate::{project::OpenedGraph, services::NativeServices};
pub use authoring::ConstantValueInput;
pub use commands::*;
pub(crate) use constant_drag::ConstantDrag;
pub(crate) use ports::scalar_input_type;

pub enum CanvasEvent {
    Selection {
        nodes: Vec<NodeId>,
        projection: Arc<EditorProjectionModel>,
    },
    Edited,
    Projection {
        nodes: Vec<NodeId>,
        projection: Arc<EditorProjectionModel>,
    },
    Execution,
    ShowResults,
    InspectResult(Arc<yss_application::graph::results::ResultLease>),
    ShowOutput,
    OpenGraph(String),
}

enum Gesture {
    Pan {
        press: Point<Pixels>,
        previous: Point<Pixels>,
        moved: bool,
        node: Option<NodeId>,
    },
    Nodes {
        press: Point<Pixels>,
        positions: BTreeMap<NodeId, NodePosition>,
        version: GraphEditVersion,
    },
    Selection {
        press: Point<Pixels>,
        current: Point<Pixels>,
        previous: BTreeSet<NodeId>,
        previous_connections: BTreeSet<ConnectionId>,
        additive: bool,
    },
    Connection(connections::ConnectionDrag),
}

struct Palette {
    point: Point<Pixels>,
    view: gpui::Entity<palette::NodePalette>,
    _subscription: gpui::Subscription,
}

pub struct GraphCanvas {
    pub(super) graph: OpenedGraph,
    services: Arc<NativeServices>,
    catalog: Arc<ActivityPanelDocument>,
    catalog_language: String,
    focus: FocusHandle,
    port_focus: FocusHandle,
    located_port: Option<PortAddress>,
    selected_connections: BTreeSet<ConnectionId>,
    hovered_connection: Option<ConnectionId>,
    context_menu: Option<menu::CanvasMenu>,
    connection_click: Option<connections::ConnectionClick>,
    read_task: Option<gpui::Task<()>>,
    offset: Point<Pixels>,
    zoom: f32,
    gesture: Option<Gesture>,
    preview: BTreeMap<NodeId, NodePosition>,
    connection_layer: Rc<RefCell<connections::ConnectionLayer>>,
    presentation: Rc<presentation::Presentation>,
    node_contents: nodes::summary::Contents,
    port_inputs: ports::input::Inputs,
    selected: BTreeSet<NodeId>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    palette: Option<Palette>,
    busy: bool,
    refreshing: bool,
    refresh_pending: bool,
    error: Option<String>,
    execution: execution::ExecutionView,
}

impl GraphCanvas {
    pub fn new(
        services: Arc<NativeServices>,
        graph: OpenedGraph,
        catalog: Arc<ActivityPanelDocument>,
        catalog_language: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let connection_layer = Rc::new(RefCell::new(connections::ConnectionLayer::new(
            &graph.projection,
        )));
        let mut view = Self {
            graph,
            services,
            catalog,
            catalog_language,
            focus: cx.focus_handle(),
            port_focus: cx.focus_handle(),
            located_port: None,
            selected_connections: BTreeSet::new(),
            hovered_connection: None,
            context_menu: None,
            connection_click: None,
            read_task: None,
            offset: point(px(40.), px(40.)),
            zoom: 1.,
            gesture: None,
            preview: BTreeMap::new(),
            connection_layer,
            presentation: Rc::default(),
            node_contents: Default::default(),
            port_inputs: Default::default(),
            selected: BTreeSet::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            palette: None,
            busy: false,
            refreshing: false,
            refresh_pending: false,
            error: None,
            execution: Default::default(),
        };
        view.reset_view();
        view.refresh_presentation();
        cx.defer_in(window, |view, _, cx| view.resync_execution(cx));
        view
    }

    pub fn path(&self) -> &str {
        self.graph.projection.graph_path.as_str()
    }

    pub(crate) fn command_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn dirty(&self) -> bool {
        self.graph.editing.dirty || self.has_dirty_port_inputs()
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub(crate) fn zoom(&self) -> f32 {
        self.zoom
    }

    pub(crate) fn viewport_offset(&self) -> Point<Pixels> {
        self.offset
    }

    pub fn set_catalog(
        &mut self,
        catalog: Arc<ActivityPanelDocument>,
        language: &str,
        cx: &mut Context<Self>,
    ) {
        self.catalog = catalog;
        self.catalog_language = language.to_owned();
        if let Some(palette) = &self.palette {
            palette.view.update(cx, |palette, cx| {
                palette.catalog_changed(self.catalog.clone(), cx)
            });
        }
        cx.notify();
    }

    pub fn focus_node(
        &mut self,
        id: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(id) = id {
            let Ok(uuid) = uuid::Uuid::parse_str(id) else {
                return false;
            };
            let id = NodeId::from_uuid(uuid);
            let Some(node) = self
                .graph
                .projection
                .nodes
                .iter()
                .find(|node| node.node_id == id)
            else {
                return false;
            };
            self.located_port = None;
            self.selected_connections.clear();
            self.connection_click = None;
            self.selected = BTreeSet::from([id]);
            self.offset = point(
                px(40. - node.position.x as f32 * self.zoom),
                px(40. - node.position.y as f32 * self.zoom),
            );
        }
        window.focus(&self.focus, cx);
        self.emit_selection(cx);
        cx.notify();
        true
    }

    pub fn title_text(&self) -> String {
        format!(
            "{}{}",
            self.path()
                .rsplit('/')
                .next()
                .unwrap_or(self.path())
                .trim_end_matches(".yssbi-event")
                .trim_end_matches(".yssbi-function"),
            if self.dirty() { " •" } else { "" }
        )
    }

    fn reset_view(&mut self) {
        self.cancel_gesture();
        self.palette = None;
        self.zoom = 1.;
        let min = self
            .graph
            .projection
            .nodes
            .iter()
            .map(|node| node.position)
            .reduce(|a, b| NodePosition {
                x: a.x.min(b.x),
                y: a.y.min(b.y),
            });
        self.offset = min.map_or(point(px(40.), px(40.)), |p| {
            point(px(40. - p.x as f32), px(40. - p.y as f32))
        });
    }

    fn world(&self, screen: Point<Pixels>) -> NodePosition {
        let point = (screen - self.bounds.get().origin - self.offset) / self.zoom;
        NodePosition {
            x: f32::from(point.x) as f64,
            y: f32::from(point.y) as f64,
        }
    }

    fn emit_selection(&self, cx: &mut Context<Self>) {
        cx.emit(CanvasEvent::Selection {
            nodes: self.selected.iter().copied().collect(),
            projection: self.graph.projection.clone(),
        });
    }

    fn cancel_gesture(&mut self) {
        if let Some(Gesture::Selection {
            previous,
            previous_connections,
            ..
        }) = self.gesture.take()
        {
            self.selected = previous;
            self.selected_connections = previous_connections;
        }
        self.gesture = None;
        if !self.busy {
            self.preview.clear();
        }
        self.hovered_connection = None;
        self.context_menu = None;
        self.read_task = None;
    }

    fn begin_node(
        &mut self,
        id: NodeId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        cx.stop_propagation();
        self.located_port = None;
        self.selected_connections.clear();
        self.connection_click = None;
        window.focus(&self.focus, cx);
        self.palette = None;
        self.hovered_connection = None;
        self.context_menu = None;
        self.read_task = None;
        let additive = event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
        if additive {
            if !self.selected.insert(id) {
                self.selected.remove(&id);
            }
        } else if !self.selected.contains(&id) {
            self.selected = BTreeSet::from([id]);
        }
        let positions = self
            .graph
            .projection
            .nodes
            .iter()
            .filter(|node| self.selected.contains(&node.node_id))
            .map(|node| (node.node_id, node.position))
            .collect();
        self.gesture = Some(Gesture::Nodes {
            press: event.position,
            positions,
            version: self.graph.editing.version,
        });
        self.emit_selection(cx);
        cx.notify();
    }

    fn begin_pane(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.begin_connection(event, window, cx) {
            return;
        }
        self.located_port = None;
        self.connection_click = None;
        self.context_menu = None;
        self.read_task = None;
        self.hovered_connection = None;
        window.focus(&self.focus, cx);
        self.palette = None;
        if matches!(event.button, MouseButton::Right | MouseButton::Middle) {
            self.gesture = Some(Gesture::Pan {
                press: event.position,
                previous: event.position,
                moved: false,
                node: None,
            });
        } else {
            let previous = self.selected.clone();
            let previous_connections = self.selected_connections.clone();
            let additive =
                event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
            if !additive {
                self.selected.clear();
                self.selected_connections.clear();
            }
            self.gesture = Some(Gesture::Selection {
                press: event.position,
                current: event.position,
                previous,
                previous_connections,
                additive,
            });
            self.emit_selection(cx);
        }
        cx.notify();
    }

    fn pointer_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.gesture.is_none() {
            self.hover_connection(event.position, cx);
            return;
        }
        if event.pressed_button.is_none() {
            self.cancel_gesture();
            cx.notify();
            return;
        }
        let Some(mut gesture) = self.gesture.take() else {
            return;
        };
        match &mut gesture {
            Gesture::Pan {
                press,
                previous,
                moved,
                ..
            } => {
                *moved |= (event.position - *press).magnitude() > 3.;
                self.offset += event.position - *previous;
                *previous = event.position;
            }
            Gesture::Nodes {
                press, positions, ..
            } => {
                let delta = (event.position - *press) / self.zoom;
                self.preview = positions
                    .iter()
                    .map(|(id, p)| {
                        (
                            *id,
                            NodePosition {
                                x: p.x + f32::from(delta.x) as f64,
                                y: p.y + f32::from(delta.y) as f64,
                            },
                        )
                    })
                    .collect();
            }
            Gesture::Selection {
                press,
                current,
                previous,
                additive,
                ..
            } => {
                *current = event.position;
                let a = self.world(*press);
                let b = self.world(*current);
                let mut selected = self
                    .graph
                    .projection
                    .nodes
                    .iter()
                    .filter(|node| {
                        let rect = geometry::node_bounds(node, node.position);
                        rect.right() >= px(a.x.min(b.x) as f32)
                            && rect.left() <= px(a.x.max(b.x) as f32)
                            && rect.bottom() >= px(a.y.min(b.y) as f32)
                            && rect.top() <= px(a.y.max(b.y) as f32)
                    })
                    .map(|node| node.node_id)
                    .collect::<BTreeSet<_>>();
                if *additive {
                    selected.extend(previous.iter().copied());
                }
                if self.selected != selected {
                    self.selected = selected;
                    self.emit_selection(cx);
                }
            }
            Gesture::Connection(drag) => drag.current = event.position,
        }
        self.gesture = Some(gesture);
        cx.notify();
    }

    fn pointer_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        match self.gesture.take() {
            Some(Gesture::Nodes { version, .. }) => {
                let positions = self
                    .preview
                    .iter()
                    .map(|(node_id, position)| NodePositionMutation {
                        node_id: *node_id,
                        position: *position,
                    })
                    .collect::<Vec<_>>();
                if !positions.is_empty() {
                    self.submit(
                        GraphCommand::Edit(EditorGraphMutation::MoveNodes { positions }),
                        Some(version),
                        cx,
                    );
                }
            }
            Some(Gesture::Pan {
                moved: false, node, ..
            }) if event.button == MouseButton::Right => {
                if let Some(id) = node {
                    self.show_node_menu(id, event.position, window, cx);
                } else {
                    self.show_palette(event.position, None, window, cx);
                }
            }
            Some(Gesture::Connection(drag)) if !drag.moving => {
                self.show_palette(event.position, Some(drag.source), window, cx);
            }
            _ => {}
        }
        self.commit_blurred_port_inputs(window, cx);
        cx.notify();
    }

    fn zoom_at_pointer(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.gesture.is_some() {
            return;
        }
        let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
        let zoom = (self.zoom * (delta * 0.002).exp()).clamp(0.1, 3.);
        let pointer = event.position - self.bounds.get().origin;
        self.offset = pointer - (pointer - self.offset) * (zoom / self.zoom);
        self.zoom = zoom;
        cx.notify();
    }
}

impl EventEmitter<PanelEvent> for GraphCanvas {}
impl EventEmitter<CanvasEvent> for GraphCanvas {}
impl Focusable for GraphCanvas {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for GraphCanvas {
    fn panel_name(&self) -> &'static str {
        "graph-editor"
    }
    fn closable(&self, _: &App) -> bool {
        !self.dirty() && !self.busy && !self.execution.running()
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            self.emit_selection(cx);
        } else {
            self.cancel_gesture();
            self.connection_click = None;
            cx.notify();
        }
    }
    fn dump(&self, cx: &App) -> gpui_component::dock::PanelState {
        let mut state = gpui_component::dock::PanelState::new(self.panel_name());
        state.info =
            gpui_component::dock::PanelInfo::Panel(serde_json::json!({"graphPath":self.path()}));
        let _ = cx;
        state
    }
}

impl Panel for GraphCanvas {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        self.title_text()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
