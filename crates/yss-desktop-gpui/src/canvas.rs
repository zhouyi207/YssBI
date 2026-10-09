mod authoring;
mod catalog;
mod commands;
mod connections;
mod constant_drag;
mod execution;
mod geometry;
mod palette;
mod render;
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
use yss_graph_document::{NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;
use yss_graph_editor::{EditorGraphMutation, NodePositionMutation};
use yss_node_protocol::PortDirection;
use yss_project::GraphEditVersion;

use crate::{project::OpenedGraph, services::NativeServices};
pub use authoring::ConstantValueInput;
pub use commands::*;
pub(crate) use constant_drag::ConstantDrag;

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
    OpenGraph(String),
}

enum Gesture {
    Pan {
        press: Point<Pixels>,
        previous: Point<Pixels>,
        moved: bool,
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
        additive: bool,
    },
    Connection {
        source: PortAddress,
        moving: bool,
        current: Point<Pixels>,
    },
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
    offset: Point<Pixels>,
    zoom: f32,
    gesture: Option<Gesture>,
    preview: BTreeMap<NodeId, NodePosition>,
    connection_layer: Rc<RefCell<connections::ConnectionLayer>>,
    selected: BTreeSet<NodeId>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    palette: Option<Palette>,
    busy: bool,
    refreshing: bool,
    refresh_pending: bool,
    connection_candidates:
        Option<BTreeMap<PortAddress, yss_graph_editor::projection::ConnectionDecision>>,
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
            offset: point(px(40.), px(40.)),
            zoom: 1.,
            gesture: None,
            preview: BTreeMap::new(),
            connection_layer,
            selected: BTreeSet::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            palette: None,
            busy: false,
            refreshing: false,
            refresh_pending: false,
            connection_candidates: None,
            error: None,
            execution: Default::default(),
        };
        view.reset_view();
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
        self.graph.editing.dirty
    }

    pub fn busy(&self) -> bool {
        self.busy
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
        if let Some(Gesture::Selection { previous, .. }) = self.gesture.take() {
            self.selected = previous;
        }
        self.gesture = None;
        if !self.busy {
            self.preview.clear();
        }
        self.connection_candidates = None;
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
        window.focus(&self.focus, cx);
        self.palette = None;
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

    fn begin_port(
        &mut self,
        address: PortAddress,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if self.busy && event.button == MouseButton::Left {
            return;
        }
        window.focus(&self.focus, cx);
        if event.modifiers.alt {
            self.submit(
                GraphCommand::Edit(EditorGraphMutation::DisconnectPort { address }),
                None,
                cx,
            );
        } else {
            self.connection_candidates = None;
            self.gesture = Some(Gesture::Connection {
                source: address.clone(),
                moving: event.modifiers.control || event.modifiers.platform,
                current: event.position,
            });
            self.query_connections(
                address,
                event.modifiers.control || event.modifiers.platform,
                cx,
            );
            cx.notify();
        }
    }

    fn end_port(&mut self, address: PortAddress, cx: &mut Context<Self>) {
        if !matches!(self.gesture, Some(Gesture::Connection { .. })) {
            return;
        }
        let Some(Gesture::Connection { source, moving, .. }) = self.gesture.take() else {
            return;
        };
        cx.stop_propagation();
        if source == address {
            cx.notify();
            return;
        }
        let mutation = if moving {
            EditorGraphMutation::MoveConnections {
                source,
                target: address,
            }
        } else {
            let output = self
                .graph
                .projection
                .nodes
                .iter()
                .flat_map(|node| node.ports.iter())
                .find(|port| port.address == source)
                .is_some_and(|port| port.direction == PortDirection::Output);
            let (output, input) = if output {
                (source, address)
            } else {
                (address, source)
            };
            EditorGraphMutation::Connect {
                output,
                input,
                order: None,
            }
        };
        self.submit(GraphCommand::Edit(mutation), None, cx);
    }

    fn begin_pane(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        window.focus(&self.focus, cx);
        self.palette = None;
        if matches!(event.button, MouseButton::Right | MouseButton::Middle) {
            self.gesture = Some(Gesture::Pan {
                press: event.position,
                previous: event.position,
                moved: false,
            });
        } else {
            let previous = self.selected.clone();
            let additive =
                event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
            if !additive {
                self.selected.clear();
            }
            self.gesture = Some(Gesture::Selection {
                press: event.position,
                current: event.position,
                previous,
                additive,
            });
            self.emit_selection(cx);
        }
        cx.notify();
    }

    fn pointer_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.gesture.is_none() {
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
            Gesture::Connection { current, .. } => *current = event.position,
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
            Some(Gesture::Pan { moved: false, .. }) if event.button == MouseButton::Right => {
                self.show_palette(event.position, None, window, cx);
            }
            Some(Gesture::Connection {
                source,
                moving: false,
                ..
            }) => {
                self.show_palette(event.position, Some(source), window, cx);
            }
            _ => {}
        }
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
