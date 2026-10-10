mod authoring;
mod catalog;
mod clipboard;
mod commands;
mod connections;
mod constant_drag;
mod execution;
mod geometry;
mod gestures;
mod menu;
mod navigation;
mod nodes;
mod palette;
mod ports;
mod presentation;
mod render;
mod selection;
mod toolbar;
mod viewport;

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use gestures::Gesture;
use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, Pixels, Point, Window, point, px,
};
use gpui_component::dock::{BasePanel, Panel, PanelEvent};
use yss_application::activity_panel::ActivityPanelDocument;
use yss_graph_document::{ConnectionId, NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;

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
    RevealNodeDetails,
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
    viewport_root: Option<String>,
    gesture: Option<Gesture>,
    _activation: gpui::Subscription,
    preview: BTreeMap<NodeId, NodePosition>,
    connection_layer: Rc<RefCell<connections::ConnectionLayer>>,
    presentation: Rc<presentation::Presentation>,
    node_contents: nodes::summary::Contents,
    port_inputs: ports::input::Inputs,
    port_details: Option<Rc<ports::details::Details>>,
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
            offset: Point::default(),
            zoom: 1.,
            viewport_root: None,
            gesture: None,
            _activation: cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() && view.gesture.is_some() {
                    view.cancel_gesture();
                    view.emit_selection(cx);
                    cx.notify();
                }
            }),
            preview: BTreeMap::new(),
            connection_layer,
            presentation: Rc::default(),
            node_contents: Default::default(),
            port_inputs: Default::default(),
            port_details: None,
            selected: BTreeSet::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            palette: None,
            busy: false,
            refreshing: false,
            refresh_pending: false,
            error: None,
            execution: Default::default(),
        };
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
            let position = node.position;
            self.cancel_gesture();
            self.palette = None;
            self.located_port = None;
            self.selected_connections.clear();
            self.connection_click = None;
            self.selected = BTreeSet::from([id]);
            self.offset = point(
                px(40. - position.x as f32 * self.zoom),
                px(40. - position.y as f32 * self.zoom),
            );
            self.checkpoint_viewport(cx);
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
