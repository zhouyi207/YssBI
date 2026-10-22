//! Type glyphs are built from rounded quads only, so pins never add a path pass to a frame.
use gpui_kit::{
    AnyElement, BorderStyle, Bounds, Hsla, IntoElement, Pixels, Point, Window, canvas, div, fill,
    point, prelude::*, px, quad, size,
};
use yss_data_contract::ValueType;
use yss_graph_editor::projection::EditorPortTypeState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Shape {
    /// One value, or a type the graph cannot name yet.
    Circle,
    Array,
    /// A column: a capsule standing upright.
    Series,
    /// A table: a square divided into a header and a column.
    Frame,
    Struct,
}

#[derive(Clone, Copy)]
pub(in crate::canvas) struct Glyph {
    shape: Shape,
    dashed: bool,
}

impl Glyph {
    pub fn new(state: &EditorPortTypeState) -> Self {
        let data_type = match state {
            EditorPortTypeState::Exact { data_type, .. } => data_type.as_ref(),
            _ => None,
        };
        Self {
            shape: match data_type {
                Some(ValueType::Array(_)) => Shape::Array,
                Some(ValueType::DataSeries(_)) => Shape::Series,
                Some(ValueType::DataFrame) => Shape::Frame,
                Some(ValueType::Struct(_)) => Shape::Struct,
                _ => Shape::Circle,
            },
            dashed: matches!(data_type, Some(ValueType::OneOf(_))),
        }
    }

    /// Paints the glyph centered on `center` within a `diameter` square. A connected glyph
    /// is filled with `ink`; an open one keeps `surface` inside its outline, so a card edge
    /// running through the pin stays hidden.
    pub fn paint(
        self,
        center: Point<Pixels>,
        diameter: Pixels,
        ink: Hsla,
        surface: Hsla,
        connected: bool,
        window: &mut Window,
    ) {
        let extent = match self.shape {
            Shape::Series => size(diameter * 0.7, diameter * 1.1),
            _ => size(diameter, diameter),
        };
        let bounds = Bounds::new(
            center - point(extent.width / 2., extent.height / 2.),
            extent,
        );
        let radius = match self.shape {
            Shape::Circle | Shape::Series => extent.width.min(extent.height) / 2.,
            Shape::Array => diameter * 0.22,
            Shape::Frame => diameter * 0.18,
            Shape::Struct => diameter * 0.32,
        };
        let stroke = (diameter * 0.15).max(px(1.));
        let marks = if connected {
            window.paint_quad(fill(bounds, ink).corner_radii(radius));
            surface
        } else {
            let style = if self.dashed {
                BorderStyle::Dashed
            } else {
                BorderStyle::Solid
            };
            window.paint_quad(quad(bounds, radius, surface, stroke, ink, style));
            ink
        };
        let line = (diameter * 0.12).max(px(1.));
        match self.shape {
            Shape::Frame => {
                let inner = bounds.dilate(-stroke);
                let header = inner.origin.y + inner.size.height * 0.36;
                window.paint_quad(fill(
                    Bounds::new(
                        point(inner.origin.x, header - line / 2.),
                        size(inner.size.width, line),
                    ),
                    marks,
                ));
                window.paint_quad(fill(
                    Bounds::new(
                        point(center.x - line / 2., header),
                        size(line, inner.bottom() - header),
                    ),
                    marks,
                ));
            }
            Shape::Struct => {
                let dot = diameter * 0.3;
                window.paint_quad(
                    fill(
                        Bounds::new(center - point(dot / 2., dot / 2.), size(dot, dot)),
                        marks,
                    )
                    .corner_radii(dot / 2.),
                );
            }
            Shape::Circle | Shape::Array | Shape::Series => {}
        }
    }
}

/// A standalone glyph for surfaces outside the canvas, drawn like a canvas pin.
pub(in crate::canvas) fn element(
    glyph: Glyph,
    diameter: Pixels,
    ink: Hsla,
    surface: Hsla,
    connected: bool,
) -> AnyElement {
    div()
        .size(diameter)
        .flex_shrink_0()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    glyph.paint(
                        bounds.center(),
                        diameter * 0.84,
                        ink,
                        surface,
                        connected,
                        window,
                    );
                },
            )
            .size_full(),
        )
        .into_any_element()
}
