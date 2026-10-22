//! Port presentation shares its type palette with connections; decisions stay backend-owned.
pub(super) mod details;
mod glyph;
pub(super) mod input;
mod inspection;
mod menu;

use super::{
    GraphCanvas, geometry,
    style::{CanvasStyle, DataKind},
    text::Label,
};
use gpui_kit::{AnyElement, App, Bounds, Hsla, Pixels, Point, Window, px};
use yss_data_contract::{SemanticType, ValueType};
use yss_graph_editor::projection::{
    ConnectionDecision, EditorDiagnosticSeverity, EditorPortModel, EditorPortTypeState,
};
use yss_node_protocol::PortDirection;

pub(crate) fn scalar_input_type(port: &EditorPortModel) -> Option<SemanticType> {
    if port.direction != PortDirection::Input || port.orphan {
        return None;
    }
    match &port.type_state {
        EditorPortTypeState::Exact {
            data_type:
                Some(ValueType::Scalar(
                    kind @ (SemanticType::Numeric | SemanticType::Binary | SemanticType::Text),
                )),
            ..
        } => Some(*kind),
        _ => None,
    }
}

/// The element kind a port carries; a constrained port has one when all its candidates agree.
pub(super) fn data_kind(port: &EditorPortModel) -> DataKind {
    match &port.type_state {
        EditorPortTypeState::Exact {
            data_type: Some(value),
            ..
        } => value_kind(value),
        EditorPortTypeState::Constrained { domain, .. } => {
            let mut kinds = domain.iter().map(value_kind);
            let first = kinds.next().unwrap_or_default();
            if kinds.all(|kind| kind == first) {
                first
            } else {
                DataKind::Other
            }
        }
        _ => DataKind::Other,
    }
}

fn value_kind(value: &ValueType) -> DataKind {
    let mut value = value;
    while let ValueType::Array(inner) | ValueType::DataSeries(inner) = value {
        value = inner;
    }
    match value {
        ValueType::Scalar(SemanticType::Numeric) => DataKind::Numeric,
        ValueType::Scalar(SemanticType::Binary) => DataKind::Binary,
        ValueType::Scalar(SemanticType::Text) => DataKind::Text,
        ValueType::Scalar(SemanticType::Datetime) => DataKind::Datetime,
        ValueType::DataFrame => DataKind::Frame,
        _ => DataKind::Other,
    }
}

fn port_ink(port: &EditorPortModel, style: &CanvasStyle) -> Hsla {
    if port.orphan {
        style.danger
    } else {
        style.data(data_kind(port))
    }
}

/// The port as drawn on the canvas, so other surfaces show the same type shape and color.
pub(crate) fn port_glyph(port: &EditorPortModel, diameter: Pixels, cx: &App) -> AnyElement {
    use gpui_kit::component::ActiveTheme;
    let theme = cx.theme();
    glyph::element(
        glyph::Glyph::new(&port.type_state),
        diameter,
        if port.orphan {
            theme.danger
        } else {
            data_kind(port).color(theme)
        },
        theme.background,
        port.connections.current > 0,
    )
}

/// Where a port row sits on screen, derived from its node's transform.
pub(super) struct PortSlot<'a> {
    pub port: &'a EditorPortModel,
    pub label: Option<&'a Label>,
    pub anchor: Point<Pixels>,
    pub row: Bounds<Pixels>,
    pub zoom: f32,
    pub label_alpha: f32,
}

impl GraphCanvas {
    pub(super) fn paint_port(&self, slot: PortSlot<'_>, style: &CanvasStyle, window: &mut Window) {
        let PortSlot {
            port,
            label,
            anchor,
            row,
            zoom,
            label_alpha,
        } = slot;
        let output = port.direction == PortDirection::Output;
        let drag = self.connection_drag();
        let decision = drag.and_then(|drag| drag.decision(&port.address));
        let active = drag.is_some_and(|drag| drag.source == port.address);
        let target = drag.is_some_and(|drag| drag.target.as_ref() == Some(&port.address));
        let dimmed =
            !active && !target && matches!(decision, Some(ConnectionDecision::Invalid { .. }));
        let marked = self
            .context_menu
            .as_ref()
            .is_some_and(|menu| menu.port.as_ref() == Some(&port.address))
            || self.located_port.as_ref() == Some(&port.address);
        let hovered = drag.is_none() && self.hovered_port() == Some(&port.address);
        let diagnostic = self
            .port_details
            .as_ref()
            .and_then(|details| details.diagnostic(&port.address));
        let alpha = if port.orphan {
            0.35
        } else if dimmed {
            0.3
        } else {
            1.
        };
        let ink = port_ink(port, style).opacity(alpha);
        let diameter = px(geometry::GLYPH_SIZE * zoom).max(px(3.));
        // Pins show emphasis by size and fill alone, never by an outline or fill around them:
        // the pending wire's color explains a drop decision and the label carries the port's
        // own problem.
        let grown = if hovered || target || active || marked {
            diameter * 1.3
        } else {
            diameter
        };
        let accepting = target && !matches!(decision, Some(ConnectionDecision::Invalid { .. }));
        glyph::Glyph::new(&port.type_state).paint(
            anchor,
            grown,
            ink,
            style.card,
            port.connections.current > 0 || active || accepting,
            window,
        );

        let Some(label) = label.filter(|_| label_alpha > 0.) else {
            return;
        };
        let color = if marked {
            style.selection
        } else if let Some(diagnostic) = diagnostic {
            match diagnostic.severity {
                EditorDiagnosticSeverity::Error => style.danger,
                EditorDiagnosticSeverity::Warning => style.warning,
                EditorDiagnosticSeverity::Information => style.info,
            }
        } else if port.connections.current > 0 || active {
            style.text
        } else {
            style.muted_text
        };
        let color = color.opacity(alpha * label_alpha);
        let inset = px(geometry::CONTENT_INSET * zoom) + diameter / 2.;
        let center = row.center().y;
        if output {
            label.paint_end(anchor.x - inset, center, zoom, color, window);
        } else {
            label.paint(anchor.x + inset, center, zoom, color, window);
        }
    }
}
