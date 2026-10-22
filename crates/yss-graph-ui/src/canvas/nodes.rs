mod menu;
pub(super) mod summary;

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::{
    App, BorderStyle, Bounds, BoxShadow, Corners, Hsla, Pixels, Point, TransformationMatrix,
    Window, fill, point, px, quad, size, transparent_black,
};
use yss_data_contract::SemanticType;
use yss_graph_document::NodeId;
use yss_graph_editor::projection::{
    EditorDiagnosticSeverity, EditorNodeModel, EditorProjectionModel,
};
use yss_node_protocol::PortDirection;

use super::{
    Gesture, GraphCanvas,
    geometry::{self, Detail, NodeGeometry},
    ports::{self, PortSlot},
    presentation::State,
    style::{CanvasStyle, DataKind},
    text::{self, Label, Typeface},
};
use crate::feedback;
use yss_ui::text as copy;

const CHIP_SIZE: f32 = 18.;
pub(super) const ICON_SIZE: f32 = 12.;
const TITLE_START: f32 = geometry::CONTENT_INSET + CHIP_SIZE + 8.;
/// The trailing header lane for the diagnostic, result count and state icons.
const STATUS_LANE: f32 = 64.;
const USER_LABEL_LIMIT: f32 = 96.;
const GAP: f32 = 6.;
/// The well's rows indent from the card by the well inset plus its own padding.
const WELL_TEXT_INSET: f32 = 16.;
pub(super) const FIELD_HEIGHT: f32 = 18.;
const FIELD_DEFAULT_WIDTH: f32 = 40.;
const TOGGLE_WIDTH: f32 = 28.;

/// Labels shaped per node, kept while their text and the space they fit are unchanged.
#[derive(Default)]
pub(super) struct NodeTexts {
    typeface: Option<Typeface>,
    projection: Option<Arc<EditorProjectionModel>>,
    nodes: BTreeMap<NodeId, NodeText>,
}

impl NodeTexts {
    /// The world width of a port's shaped label, while it has one.
    pub fn port_label_width(&self, id: NodeId, index: usize) -> Option<f32> {
        self.nodes
            .get(&id)?
            .ports
            .get(index)?
            .as_ref()
            .map(Label::width)
    }
}

#[derive(Default)]
struct NodeText {
    title: Option<Label>,
    user_label: Option<Label>,
    count: Option<((usize, usize), Label)>,
    parameters: Vec<(Label, Label)>,
    ports: Vec<Option<Label>>,
}

fn refresh(slot: &mut Option<Label>, text: &str, limit: Option<f32>, make: impl FnOnce() -> Label) {
    if slot
        .as_ref()
        .is_none_or(|label| !label.matches(text, limit))
    {
        *slot = Some(make());
    }
}

/// The header icon names the node family; its tint follows the data the node produces.
fn category(node: &EditorNodeModel) -> (IconName, DataKind) {
    let icon = match node.display.icon_id.as_deref().unwrap_or_default() {
        "builtin.numeric" => IconName::Sigma,
        "builtin.constants" => IconName::Pi,
        "builtin.dataframe" => IconName::Table,
        "builtin.statistics" => IconName::ChartScatter,
        "builtin.distribution" => IconName::ChartSpline,
        "builtin.plot" => IconName::ChartLine,
        "builtin.project" => IconName::SquareFunction,
        "builtin.logic" => IconName::Binary,
        "builtin.conversion" => IconName::ArrowLeftRight,
        "builtin.dataflow" => IconName::Workflow,
        "builtin.debug" => IconName::Bug,
        _ => IconName::Box,
    };
    let kind = node
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Output)
        .map(ports::data_kind)
        .unwrap_or_default();
    (icon, kind)
}

fn state_icon(state: State) -> IconName {
    match state {
        State::Unexecuted => IconName::CircleDashed,
        State::Running => IconName::LoaderCircle,
        State::Error => IconName::CircleAlert,
        State::Valid => IconName::CircleCheck,
        State::Stale => IconName::Clock,
        State::Partial => IconName::CircleDot,
    }
}

fn paint_icon(
    icon: IconName,
    center: Point<Pixels>,
    extent: Pixels,
    color: Hsla,
    window: &mut Window,
    cx: &App,
) {
    if extent < px(4.) || color.a <= 0. {
        return;
    }
    let bounds = Bounds::new(
        center - point(extent / 2., extent / 2.),
        size(extent, extent),
    );
    if let Err(error) = window.paint_svg(
        bounds,
        icon.path(),
        None,
        TransformationMatrix::unit(),
        color,
        cx,
    ) {
        tracing::warn!(%error, "graph canvas icon could not be painted");
    }
}

/// A rounded bar standing in for text too small to read.
fn paint_skeleton(
    x: Pixels,
    center_y: Pixels,
    width: Pixels,
    height: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    if color.a <= 0. || width <= px(0.) {
        return;
    }
    window.paint_quad(
        fill(
            Bounds::new(point(x, center_y - height / 2.), size(width, height)),
            color,
        )
        .corner_radii(height / 2.),
    );
}

fn estimated_width(text: &str, style: text::TextStyle, limit: f32) -> f32 {
    let width: f32 = text
        .chars()
        .map(|character| if character.is_ascii() { 0.55 } else { 1. })
        .sum();
    (width * style.size).min(limit)
}

fn scaled_shadows(shadows: &[BoxShadow], factor: f32, alpha: f32) -> Vec<BoxShadow> {
    shadows
        .iter()
        .map(|shadow| BoxShadow {
            color: shadow.color.opacity(alpha),
            offset: shadow.offset * factor,
            blur_radius: shadow.blur_radius * factor,
            spread_radius: shadow.spread_radius * factor,
            inset: shadow.inset,
        })
        .collect()
}

impl GraphCanvas {
    /// Nodes back to front: the current selection is raised above the rest of the graph.
    pub(super) fn painted_nodes(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&EditorNodeModel, &NodeGeometry)> {
        let pairs = || {
            self.graph
                .projection
                .nodes
                .iter()
                .zip(self.connection_layer.nodes())
        };
        pairs()
            .filter(|(node, _)| !self.selected.contains(&node.node_id))
            .chain(pairs().filter(|(node, _)| self.selected.contains(&node.node_id)))
    }

    pub(super) fn node_screen_rect(
        &self,
        node: &EditorNodeModel,
        geometry: &NodeGeometry,
        canvas: Point<Pixels>,
    ) -> Bounds<Pixels> {
        let rect = geometry
            .layout
            .bounds(geometry::position(node, &self.preview));
        Bounds::new(
            canvas + self.offset + rect.origin * self.zoom,
            rect.size.map(|length| length * self.zoom),
        )
    }

    pub(super) fn detail(&self) -> Detail {
        Detail::at(self.zoom / self.font_scale)
    }

    pub(super) fn typeface(&self) -> Typeface {
        Typeface {
            family: self.font_family.clone(),
            scale: self.font_scale,
        }
    }

    /// The width an inline value occupies, in world units, so labels can leave room for it.
    pub(super) fn field_width(
        &self,
        port: &yss_graph_editor::projection::EditorPortModel,
    ) -> Option<f32> {
        let kind = ports::input::eligible(port)?;
        Some(if kind == SemanticType::Binary {
            TOGGLE_WIDTH
        } else {
            self.port_inputs
                .width(&port.address)
                .unwrap_or(FIELD_DEFAULT_WIDTH)
        })
    }

    /// Shapes the labels of `nodes` that legible zoom levels will paint.
    pub(super) fn prepare_node_texts(&mut self, nodes: &[&EditorNodeModel], window: &Window) {
        let detail = self.detail();
        if detail.title <= 0. {
            return;
        }
        let typeface = self.typeface();
        if self.node_texts.typeface.as_ref() != Some(&typeface) {
            self.node_texts.nodes.clear();
            self.node_texts.typeface = Some(typeface.clone());
        }
        if self
            .node_texts
            .projection
            .as_ref()
            .is_none_or(|previous| !Arc::ptr_eq(previous, &self.graph.projection))
        {
            let projection = &self.graph.projection;
            self.node_texts
                .nodes
                .retain(|id, _| projection.nodes.iter().any(|node| node.node_id == *id));
            self.node_texts.projection = Some(projection.clone());
        }
        for node in nodes {
            if self
                .connection_layer
                .node(node.node_id)
                .is_none_or(|geometry| geometry.layout.compact)
            {
                continue;
            }
            let fields = node
                .ports
                .iter()
                .map(|port| self.field_width(port))
                .collect::<Vec<_>>();
            let rows = self.node_contents.rows(node.node_id);
            let count = self
                .presentation
                .nodes
                .get(&node.node_id)
                .map(|display| (display.cache.valid, display.cache.total))
                .filter(|(_, total)| *total > 0);
            let entry = self.node_texts.nodes.entry(node.node_id).or_default();

            let user_limit = Some(USER_LABEL_LIMIT);
            match node.display.user_label.as_deref() {
                Some(label) => refresh(&mut entry.user_label, label, user_limit, || {
                    Label::new(
                        label.to_owned().into(),
                        text::CAPTION,
                        user_limit,
                        &typeface,
                        window,
                    )
                }),
                None => entry.user_label = None,
            }
            let reserved = entry
                .user_label
                .as_ref()
                .map_or(0., |label| label.width() + GAP);
            let title_limit = Some(
                geometry::NODE_WIDTH
                    - TITLE_START
                    - STATUS_LANE
                    - geometry::CONTENT_INSET
                    - reserved,
            );
            refresh(&mut entry.title, &node.display.title, title_limit, || {
                Label::new(
                    node.display.title.to_string().into(),
                    text::TITLE,
                    title_limit,
                    &typeface,
                    window,
                )
            });
            entry.count = match (count, entry.count.take()) {
                (Some(count), Some((previous, label))) if previous == count => Some((count, label)),
                (Some((valid, total)), _) => Some((
                    (valid, total),
                    Label::new(
                        format!("{valid}/{total}").into(),
                        text::CAPTION,
                        None,
                        &typeface,
                        window,
                    ),
                )),
                (None, _) => None,
            };

            let row_width = geometry::NODE_WIDTH - 2. * WELL_TEXT_INSET;
            entry.parameters.truncate(rows.len());
            for (index, row) in rows.iter().enumerate() {
                let title_limit = Some(row_width * 0.5);
                if entry.parameters.len() <= index {
                    let title = Label::new(
                        row.title.clone(),
                        text::LABEL,
                        title_limit,
                        &typeface,
                        window,
                    );
                    let value_limit = Some(row_width - title.width() - GAP);
                    let value = Label::new(
                        row.value.clone(),
                        text::LABEL,
                        value_limit,
                        &typeface,
                        window,
                    );
                    entry.parameters.push((title, value));
                    continue;
                }
                let (title, value) = &mut entry.parameters[index];
                if !title.matches(&row.title, title_limit) {
                    *title = Label::new(
                        row.title.clone(),
                        text::LABEL,
                        title_limit,
                        &typeface,
                        window,
                    );
                }
                let value_limit = Some(row_width - title.width() - GAP);
                if !value.matches(&row.value, value_limit) {
                    *value = Label::new(
                        row.value.clone(),
                        text::LABEL,
                        value_limit,
                        &typeface,
                        window,
                    );
                }
            }

            entry.ports.resize_with(node.ports.len(), || None);
            let half = geometry::NODE_WIDTH / 2.;
            for ((slot, port), field) in entry.ports.iter_mut().zip(node.ports.iter()).zip(fields) {
                let text = port
                    .display
                    .instance_label
                    .as_deref()
                    .unwrap_or(&port.display.label);
                let limit = Some(
                    half - geometry::CONTENT_INSET - GAP - field.map_or(0., |width| width + GAP),
                );
                refresh(slot, text, limit, || {
                    Label::new(
                        text.to_owned().into(),
                        text::LABEL,
                        limit,
                        &typeface,
                        window,
                    )
                });
            }
        }
    }

    pub(super) fn paint_nodes(
        &self,
        bounds: Bounds<Pixels>,
        style: &CanvasStyle,
        window: &mut Window,
        cx: &App,
    ) {
        let detail = self.detail();
        let elevation = self.zoom.clamp(0.5, 1.);
        let shadows = detail.shadows.then(|| {
            [
                scaled_shadows(&style.resting_shadow, elevation, 1.),
                scaled_shadows(&style.raised_shadow, elevation, 1.),
                scaled_shadows(&style.resting_shadow, elevation, 0.4),
            ]
        });
        let reach = px((geometry::PORT_REACH + geometry::GLYPH_SIZE) * self.zoom) + px(4.);
        let controls = self.controls_visible();
        for (node, geometry) in self.painted_nodes() {
            let rect = self.node_screen_rect(node, geometry, bounds.origin);
            let extent = rect.dilate(reach);
            if !extent.intersects(&bounds) {
                continue;
            }
            window.paint_layer(extent, |window| {
                self.paint_node(
                    node,
                    geometry,
                    rect,
                    (detail, controls),
                    shadows.as_ref(),
                    style,
                    window,
                    cx,
                )
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_node(
        &self,
        node: &EditorNodeModel,
        geometry: &NodeGeometry,
        rect: Bounds<Pixels>,
        (detail, controls): (Detail, bool),
        shadows: Option<&[Vec<BoxShadow>; 3]>,
        style: &CanvasStyle,
        window: &mut Window,
        cx: &App,
    ) {
        let zoom = self.zoom;
        let layout = geometry.layout;
        let id = node.node_id;
        let display = self
            .presentation
            .nodes
            .get(&id)
            .copied()
            .unwrap_or_default();
        let selected = self.selected.contains(&id);
        let hovered = self.hovered_node() == Some(id);
        let dimmed = self
            .connection_drag()
            .is_some_and(|drag| drag.dimmed_nodes.contains(&id));
        let alpha = if dimmed { 0.4 } else { 1. };
        let radius = if layout.compact {
            rect.size.height.min(rect.size.width) / 2.
        } else {
            px(style.radius * zoom)
        };

        if let Some(shadows) = shadows {
            let lifted =
                selected && matches!(self.gesture, Some(Gesture::Nodes { moved: true, .. }));
            let shadow = if dimmed {
                &shadows[2]
            } else if lifted {
                &shadows[1]
            } else {
                &shadows[0]
            };
            window.paint_drop_shadows(rect, Corners::all(radius), shadow);
        }
        if selected {
            let ring = rect.dilate(px(3.));
            window.paint_quad(quad(
                ring,
                radius + px(3.),
                transparent_black(),
                px(2.),
                style.selection,
                BorderStyle::Solid,
            ));
        }
        let border = match display.state {
            State::Error => style.danger,
            State::Running => style.info,
            _ if hovered => style.hover_border,
            _ => style.border,
        };
        window.paint_quad(quad(
            rect,
            radius,
            style.card.opacity(alpha),
            px(1.),
            border.opacity(alpha),
            BorderStyle::Solid,
        ));
        if !layout.compact {
            self.paint_header(
                node,
                rect,
                detail,
                display,
                (alpha, radius),
                style,
                window,
                cx,
            );
            self.paint_parameters(node, rect, detail, alpha, style, window);
        }

        let texts = self.node_texts.nodes.get(&id);
        let label_alpha = if layout.compact { 0. } else { detail.labels };
        for (output, indices) in [(false, &geometry.inputs), (true, &geometry.outputs)] {
            for (row, index) in indices.iter().enumerate() {
                let Some(port) = node.ports.get(*index) else {
                    continue;
                };
                let offset = layout.port_offset(output, row);
                let anchor = rect.origin + offset * zoom;
                let region = layout.port_region(output, row);
                let row_bounds = Bounds::new(
                    rect.origin + region.origin * zoom,
                    region.size.map(|length| length * zoom),
                );
                self.paint_port(
                    PortSlot {
                        port,
                        label: texts
                            .and_then(|texts| texts.ports.get(*index))
                            .and_then(Option::as_ref),
                        anchor,
                        row: row_bounds,
                        zoom,
                        label_alpha: label_alpha * alpha,
                    },
                    style,
                    window,
                );
                if !output && !layout.compact && !controls {
                    self.paint_value_placeholder(port, rect, row_bounds, alpha, style, window);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_header(
        &self,
        node: &EditorNodeModel,
        rect: Bounds<Pixels>,
        detail: Detail,
        display: super::presentation::NodeAppearance,
        (alpha, radius): (f32, Pixels),
        style: &CanvasStyle,
        window: &mut Window,
        cx: &App,
    ) {
        let zoom = self.zoom;
        let height = px(geometry::HEADER_HEIGHT * zoom);
        let inner = rect.dilate(px(-1.));
        let corner = (radius - px(1.)).max(px(0.));
        window.paint_quad(
            fill(
                Bounds::new(inner.origin, size(inner.size.width, height - px(1.))),
                style.header.opacity(alpha),
            )
            .corner_radii(Corners {
                top_left: corner,
                top_right: corner,
                bottom_left: px(0.),
                bottom_right: px(0.),
            }),
        );
        window.paint_quad(fill(
            Bounds::new(
                point(inner.left(), rect.top() + height - px(1.)),
                size(inner.size.width, px(1.)),
            ),
            style.border.opacity(alpha),
        ));

        let center_y = rect.top() + height / 2.;
        let (icon, kind) = category(node);
        let accent = style.data(kind);
        let chip = px(CHIP_SIZE * zoom);
        let chip_bounds = Bounds::new(
            point(
                rect.left() + px(geometry::CONTENT_INSET * zoom),
                center_y - chip / 2.,
            ),
            size(chip, chip),
        );
        window.paint_quad(
            fill(chip_bounds, accent.opacity(0.16 * alpha)).corner_radii(px(5. * zoom)),
        );
        paint_icon(
            icon,
            chip_bounds.center(),
            px(ICON_SIZE * zoom),
            accent.opacity(alpha * detail.title),
            window,
            cx,
        );

        let texts = self.node_texts.nodes.get(&node.node_id);
        let title_x = rect.left() + px(TITLE_START * zoom);
        let title = texts.and_then(|texts| texts.title.as_ref());
        if let Some(title) = title.filter(|_| detail.title > 0.) {
            title.paint(
                title_x,
                center_y,
                zoom,
                style.text.opacity(alpha * detail.title),
                window,
            );
            if let Some(label) = texts.and_then(|texts| texts.user_label.as_ref()) {
                label.paint(
                    title_x + px((title.width() + GAP) * zoom),
                    center_y,
                    zoom,
                    style.muted_text.opacity(alpha * detail.title),
                    window,
                );
            }
        }
        if detail.title < 1. {
            let limit = geometry::NODE_WIDTH - TITLE_START - STATUS_LANE - geometry::CONTENT_INSET;
            let width = title.map_or_else(
                || estimated_width(&node.display.title, text::TITLE, limit),
                Label::width,
            );
            paint_skeleton(
                title_x,
                center_y,
                px(width * zoom),
                px(8. * zoom),
                style.skeleton.opacity(alpha * (1. - detail.title)),
                window,
            );
        }

        // The status lane fills from the trailing edge: state, result count, diagnostic.
        let icon_extent = px(ICON_SIZE * zoom);
        let mut trailing = rect.right() - px(geometry::CONTENT_INSET * zoom);
        let state_color = style.state(display.state).opacity(alpha);
        let state_center = self.status_center(rect);
        if display.state == State::Running {
            // The activity overlay turns an arc over this track while the node runs.
            let track = icon_extent * 0.9;
            window.paint_quad(quad(
                Bounds::new(
                    state_center - point(track / 2., track / 2.),
                    size(track, track),
                ),
                track / 2.,
                transparent_black(),
                (track * 0.14).max(px(1.)),
                state_color.opacity(0.25),
                BorderStyle::Solid,
            ));
        } else if detail.title > 0. {
            paint_icon(
                state_icon(display.state),
                state_center,
                icon_extent,
                state_color.opacity(detail.title),
                window,
                cx,
            );
        }
        if detail.title < 1. {
            let dot = px(6. * zoom);
            window.paint_quad(
                fill(
                    Bounds::new(state_center - point(dot / 2., dot / 2.), size(dot, dot)),
                    state_color.opacity(1. - detail.title),
                )
                .corner_radii(dot / 2.),
            );
        }
        trailing -= icon_extent + px(4. * zoom);
        if let Some((_, count)) = texts.and_then(|texts| texts.count.as_ref()) {
            if detail.title > 0. {
                count.paint_end(
                    trailing,
                    center_y,
                    zoom,
                    style.muted_text.opacity(alpha * detail.title),
                    window,
                );
            }
            trailing -= px((count.width() + 4.) * zoom);
        }
        let diagnostic = node
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.blocking)
            .or_else(|| node.diagnostics.first())
            .filter(|_| display.state != State::Error);
        if let Some(diagnostic) = diagnostic {
            let (icon, color) = match diagnostic.severity {
                EditorDiagnosticSeverity::Error => (IconName::CircleAlert, style.danger),
                EditorDiagnosticSeverity::Warning => (IconName::TriangleAlert, style.warning),
                EditorDiagnosticSeverity::Information => (IconName::Info, style.info),
            };
            paint_icon(
                icon,
                point(trailing - icon_extent / 2., center_y),
                icon_extent,
                color.opacity(alpha * detail.title.max(0.6)),
                window,
                cx,
            );
        }
    }

    fn paint_parameters(
        &self,
        node: &EditorNodeModel,
        rect: Bounds<Pixels>,
        detail: Detail,
        alpha: f32,
        style: &CanvasStyle,
        window: &mut Window,
    ) {
        let zoom = self.zoom;
        let Some(layout) = self
            .connection_layer
            .node(node.node_id)
            .map(|geometry| geometry.layout)
        else {
            return;
        };
        let Some(well) = layout.parameter_well() else {
            return;
        };
        window.paint_quad(
            fill(
                Bounds::new(
                    rect.origin + well.origin * zoom,
                    well.size.map(|length| length * zoom),
                ),
                style.well.opacity(alpha),
            )
            .corner_radii(px(6. * zoom)),
        );
        let rows = self.node_contents.rows(node.node_id);
        let labels = self
            .node_texts
            .nodes
            .get(&node.node_id)
            .map_or(&[][..], |texts| &texts.parameters[..]);
        let left = rect.left() + px(WELL_TEXT_INSET * zoom);
        let right = rect.right() - px(WELL_TEXT_INSET * zoom);
        for index in 0..rows.len() {
            let center_y = rect.top()
                + px((layout.parameter_row_top(index) + geometry::PARAMETER_HEIGHT / 2.) * zoom);
            if let Some((title, value)) = labels.get(index).filter(|_| detail.labels > 0.) {
                title.paint(
                    left,
                    center_y,
                    zoom,
                    style.muted_text.opacity(alpha * detail.labels),
                    window,
                );
                value.paint_end(
                    right,
                    center_y,
                    zoom,
                    style.text.opacity(alpha * detail.labels),
                    window,
                );
            }
            if detail.labels < 1. {
                let bar = px(6. * zoom);
                let color = style.skeleton.opacity(alpha * (1. - detail.labels));
                paint_skeleton(left, center_y, (right - left) * 0.4, bar, color, window);
                paint_skeleton(
                    right - (right - left) * 0.25,
                    center_y,
                    (right - left) * 0.25,
                    bar,
                    color,
                    window,
                );
            }
        }
    }

    /// Stands in for an inline value while the zoom level hides editable controls.
    fn paint_value_placeholder(
        &self,
        port: &yss_graph_editor::projection::EditorPortModel,
        rect: Bounds<Pixels>,
        row: Bounds<Pixels>,
        alpha: f32,
        style: &CanvasStyle,
        window: &mut Window,
    ) {
        let Some(width) = self.field_width(port) else {
            return;
        };
        let lane = self.field_lane(rect, row, width);
        window
            .paint_quad(fill(lane, style.well.opacity(alpha)).corner_radii(lane.size.height / 2.));
    }

    /// Screen bounds of the inline value beside an input label, ending at the card's center line.
    pub(super) fn field_lane(
        &self,
        rect: Bounds<Pixels>,
        row: Bounds<Pixels>,
        width: f32,
    ) -> Bounds<Pixels> {
        let zoom = self.zoom;
        let height = px(FIELD_HEIGHT * zoom);
        let right = rect.center().x - px(GAP * zoom);
        Bounds::new(
            point(right - px(width * zoom), row.center().y - height / 2.),
            size(px(width * zoom), height),
        )
    }

    pub(super) fn status_tooltip(&self, id: NodeId) -> Option<String> {
        let node = self
            .graph
            .projection
            .nodes
            .iter()
            .find(|node| node.node_id == id)?;
        let display = self
            .presentation
            .nodes
            .get(&id)
            .copied()
            .unwrap_or_default();
        let mut lines = vec![display.state.label().into_owned()];
        if let Some(code) = display.failure {
            lines.push(feedback::run_failure(code));
        }
        let diagnostic = node
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.blocking)
            .or_else(|| node.diagnostics.first());
        if let Some(diagnostic) = diagnostic {
            lines.push(feedback::graph_diagnostic(diagnostic));
        }
        if display.cache.total > 0 {
            if display.cache.state() != display.state {
                lines.push(display.cache.state().label().into_owned());
            }
            lines.push(copy::plural_format(
                "canvas.resultCount",
                display.cache.total,
                &[
                    ("valid", display.cache.valid.to_string()),
                    ("count", display.cache.total.to_string()),
                ],
            ));
        }
        Some(lines.join("\n"))
    }

    /// The screen center of a node's state icon.
    pub(super) fn status_center(&self, rect: Bounds<Pixels>) -> Point<Pixels> {
        let zoom = self.zoom;
        point(
            rect.right() - px((geometry::CONTENT_INSET + ICON_SIZE / 2.) * zoom),
            rect.top() + px(geometry::HEADER_HEIGHT / 2. * zoom),
        )
    }

    /// Screen bounds of a node's status lane, for its tooltip.
    pub(super) fn status_bounds(&self, rect: Bounds<Pixels>) -> Bounds<Pixels> {
        let zoom = self.zoom;
        let lane = px((STATUS_LANE + geometry::CONTENT_INSET) * zoom);
        Bounds::new(
            point(rect.right() - lane, rect.top()),
            size(lane, px(geometry::HEADER_HEIGHT * zoom)),
        )
    }

    pub(super) fn controls_visible(&self) -> bool {
        self.detail().controls
    }
}
