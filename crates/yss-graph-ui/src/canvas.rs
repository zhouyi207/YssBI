mod authoring;
mod catalog;
mod clipboard;
mod commands;
mod connections;
mod constant_drag;
mod drop_target;
mod execution;
mod geometry;
mod gestures;
mod loading;
mod menu;
mod navigation;
mod nodes;
mod palette;
mod ports;
mod presentation;
mod render;
mod result_catalog;
mod result_search;
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
use gpui_kit::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, Pixels, Point, Window, point, px,
};
use gpui_kit::{
    AppContext,
    component::dock::{BasePanel, Panel, PanelEvent},
};
use yss_application::activity_panel::ActivityPanelDocument;
use yss_graph_document::{ConnectionId, NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;

use crate::{GraphServices, OpenedGraph};
pub use authoring::ConstantValueInput;
pub use clipboard::GraphEditCommand;
pub use commands::*;
pub(crate) use constant_drag::ConstantDrag;
pub(crate) use ports::{port_glyph, scalar_input_type};
pub(crate) use result_catalog::ResultEntry;

pub enum CanvasEvent {
    Selection {
        nodes: Vec<NodeId>,
        projection: Arc<EditorProjectionModel>,
    },
    RevealNodeDetails,
    Projection {
        nodes: Vec<NodeId>,
        projection: Arc<EditorProjectionModel>,
    },
    ResourceLocated {
        path: yss_graph_document::GraphResourcePath,
        version: yss_project::GraphEditVersion,
    },
    Execution,
    ShowResults,
    InspectResult(Arc<yss_application::graph::results::ResultLease>),
    ShowOutput,
    OpenGraph(String),
}

struct Palette {
    point: Point<Pixels>,
    view: gpui_kit::Entity<palette::NodePalette>,
    _subscription: gpui_kit::Subscription,
}

pub struct GraphCanvas {
    pub(super) graph: OpenedGraph,
    services: Arc<GraphServices>,
    catalog: Arc<ActivityPanelDocument>,
    catalog_language: String,
    focus: FocusHandle,
    toolbar: gpui_kit::Entity<toolbar::Toolbar>,
    port_focus: FocusHandle,
    located_port: Option<PortAddress>,
    selected_connections: BTreeSet<ConnectionId>,
    hovered_connection: Option<ConnectionId>,
    context_menu: Option<menu::CanvasMenu>,
    connection_click: Option<connections::ConnectionClick>,
    read_task: Option<gpui_kit::Task<()>>,
    offset: Point<Pixels>,
    zoom: f32,
    // World geometry and hit-testing share this UI scale; persisted zoom excludes it.
    font_scale: f32,
    font_family: gpui_kit::SharedString,
    _appearance: gpui_kit::Subscription,
    viewport_root: Option<String>,
    gesture: Option<Gesture>,
    _activation: gpui_kit::Subscription,
    preview: geometry::NodeMovement,
    connection_layer: Rc<RefCell<connections::ConnectionLayer>>,
    presentation: Rc<presentation::Presentation>,
    node_contents: nodes::summary::Contents,
    port_inputs: ports::input::Inputs,
    parameter_forms: BTreeMap<
        NodeId,
        (
            gpui_kit::Entity<crate::parameters::ParameterForm>,
            gpui_kit::Subscription,
        ),
    >,
    port_details: Option<Rc<ports::details::Details>>,
    selected: BTreeSet<NodeId>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    palette: Option<Palette>,
    busy: bool,
    refresh_task: Option<gpui_kit::Task<()>>,
    refresh_failed: bool,
    refresh_pending: bool,
    resource_move: Option<loading::ResourceMove>,
    change_delivery: loading::ChangeDelivery,
    notifications: yss_ui::notifications::Notifications,
    execution: execution::ExecutionView,
}

impl GraphCanvas {
    pub fn graph(&self) -> &OpenedGraph {
        &self.graph
    }

    /// Retained graph controls for host chrome, outside the cached canvas content.
    pub fn toolbar_view(&self) -> gpui_kit::AnyView {
        self.toolbar.clone().into()
    }

    pub fn new(
        services: Arc<GraphServices>,
        graph: OpenedGraph,
        catalog: Arc<ActivityPanelDocument>,
        catalog_language: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let connection_layer = Rc::new(RefCell::new(connections::ConnectionLayer::new(
            &graph.projection,
        )));
        let font_scale = yss_ui::preferences::current(cx).appearance.ui_font_size / 14.;
        let text_system = window.text_system().clone();
        let owner = cx.entity();
        let mut view = Self {
            graph,
            services,
            catalog,
            catalog_language,
            focus: cx.focus_handle(),
            toolbar: cx.new(|cx| toolbar::Toolbar::new(&owner, window, cx)),
            port_focus: cx.focus_handle(),
            located_port: None,
            selected_connections: BTreeSet::new(),
            hovered_connection: None,
            context_menu: None,
            connection_click: None,
            read_task: None,
            offset: Point::default(),
            zoom: font_scale,
            font_scale,
            font_family: gpui_kit::component::Theme::global(cx).font_family.clone(),
            _appearance: cx.observe_global::<yss_ui::preferences::Preferences>(move |view, cx| {
                let scale = yss_ui::preferences::current(cx).appearance.ui_font_size / 14.;
                let family = gpui_kit::component::Theme::global(cx).font_family.clone();
                let changed = family != view.font_family || scale != view.font_scale;
                if scale != view.font_scale {
                    view.cancel_gesture();
                    let ratio = scale / view.font_scale;
                    let center = point(
                        view.bounds.get().size.width / 2.,
                        view.bounds.get().size.height / 2.,
                    );
                    view.offset = center - (center - view.offset) * ratio;
                    view.zoom *= ratio;
                    view.font_scale = scale;
                }
                if changed {
                    view.font_family = family;
                    view.port_inputs.remeasure(&text_system, cx);
                    cx.notify();
                }
            }),
            viewport_root: None,
            gesture: None,
            _activation: cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() && view.gesture.is_some() {
                    view.cancel_gesture();
                    view.emit_selection(cx);
                    cx.notify();
                }
            }),
            preview: Default::default(),
            connection_layer,
            presentation: Rc::default(),
            node_contents: Default::default(),
            port_inputs: Default::default(),
            parameter_forms: BTreeMap::new(),
            port_details: None,
            selected: BTreeSet::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            palette: None,
            busy: false,
            refresh_task: None,
            refresh_failed: false,
            refresh_pending: false,
            resource_move: None,
            change_delivery: Default::default(),
            notifications: yss_ui::notifications::Notifications::new(window),
            execution: Default::default(),
        };
        view.refresh_presentation();
        view.toolbar
            .update(cx, |toolbar, cx| toolbar.initialize(&view, cx));
        cx.defer_in(window, |view, _, cx| {
            view.resync_execution(cx);
            if view.graph.language != yss_ui::text::locale() {
                view.refresh(cx);
            }
        });
        view
    }

    /// The dock factory that restores graph editors, including still-opening ones.
    pub const PANEL_NAME: &'static str = "graph-editor";
    /// The persisted panel field holding the graph path.
    pub const PATH_KEY: &'static str = "graphPath";

    pub fn path(&self) -> &str {
        self.resource_path().as_str()
    }

    /// Whether a change to `resource` can alter this graph's projection.
    pub fn shows(&self, resource: &yss_project_history::ResourceKey) -> bool {
        resource.path() == Some(self.path()) || self.graph.depends_on_resource(resource)
    }

    pub fn dirty(&self, cx: &App) -> bool {
        self.graph.editing.dirty || self.has_dirty_inputs(cx)
    }

    fn has_dirty_inputs(&self, cx: &App) -> bool {
        self.has_dirty_port_inputs()
            || self
                .parameter_forms
                .values()
                .any(|(form, _)| form.read(cx).dirty())
    }

    pub(crate) fn parameter_form(
        &self,
        node: NodeId,
    ) -> Option<gpui_kit::Entity<crate::parameters::ParameterForm>> {
        self.parameter_forms
            .get(&node)
            .map(|(form, _)| form.clone())
    }

    pub(crate) fn retain_parameter_form(
        &mut self,
        node: NodeId,
        form: gpui_kit::Entity<crate::parameters::ParameterForm>,
        cx: &mut Context<Self>,
    ) {
        let mut dirty = form.read(cx).dirty();
        let observer = cx.observe(&form, move |_, form, cx| {
            let next = form.read(cx).dirty();
            if dirty == next {
                return;
            }
            dirty = next;
            cx.emit(PanelEvent::LayoutChanged);
            cx.notify();
        });
        self.parameter_forms.insert(node, (form, observer));
    }

    pub(crate) fn can_edit(&self) -> bool {
        !self.busy && !self.refresh_failed && self.resource_move.is_none()
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn zoom(&self) -> f32 {
        self.zoom / self.font_scale
    }

    pub fn viewport_offset(&self) -> Point<Pixels> {
        self.offset
    }

    pub fn set_catalog(
        &mut self,
        catalog: Arc<ActivityPanelDocument>,
        language: &str,
        cx: &mut Context<Self>,
    ) {
        if self.catalog_language == language && self.catalog.same_content(&catalog) {
            return;
        }
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

    pub fn title_text(&self, cx: &App) -> String {
        format!(
            "{}{}",
            self.path()
                .rsplit('/')
                .next()
                .unwrap_or(self.path())
                .trim_end_matches(".yssbi-event")
                .trim_end_matches(".yssbi-function"),
            if self.dirty(cx) { " *" } else { "" }
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
        Self::PANEL_NAME
    }
    fn closable(&self, cx: &App) -> bool {
        !self.dirty(cx) && !self.busy && !self.execution.running()
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
    fn dump(&self, cx: &App) -> gpui_kit::component::dock::PanelState {
        let mut state = gpui_kit::component::dock::PanelState::new(self.panel_name());
        state.info = gpui_kit::component::dock::PanelInfo::Panel(
            serde_json::json!({ (Self::PATH_KEY): self.path() }),
        );
        let _ = cx;
        state
    }
}

impl Panel for GraphCanvas {
    fn tab_name(&self, cx: &App) -> Option<gpui_kit::SharedString> {
        Some(self.title_text(cx).into())
    }
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.title_text(cx)
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
