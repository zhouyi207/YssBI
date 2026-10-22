//! Canvas text is shaped once at the reference zoom; zooming only scales glyph geometry.
use gpui_kit::{
    Font, FontWeight, Hsla, Pixels, ShapedLine, SharedString, TextRun, TruncateFrom, Window, font,
    point, px,
};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct TextStyle {
    /// World units, like the node geometry the text sits in.
    pub size: f32,
    pub weight: FontWeight,
}

pub(super) const TITLE: TextStyle = TextStyle {
    size: 13.,
    weight: FontWeight::MEDIUM,
};
pub(super) const LABEL: TextStyle = TextStyle {
    size: 12.,
    weight: FontWeight::NORMAL,
};
pub(super) const CAPTION: TextStyle = TextStyle {
    size: 11.,
    weight: FontWeight::NORMAL,
};

/// The font family and the zoom at which labels are shaped: the canvas at 100%.
#[derive(Clone, PartialEq)]
pub(super) struct Typeface {
    pub family: SharedString,
    pub scale: f32,
}

impl Typeface {
    fn font(&self, weight: FontWeight) -> Font {
        Font {
            weight,
            ..font(self.family.clone())
        }
    }
}

pub(super) struct Label {
    source: SharedString,
    limit: Option<f32>,
    line: ShapedLine,
    scale: f32,
}

impl Label {
    /// Shapes `text`, truncated with an ellipsis to `limit` world units.
    pub fn new(
        text: SharedString,
        style: TextStyle,
        limit: Option<f32>,
        typeface: &Typeface,
        window: &Window,
    ) -> Self {
        let size = px(style.size * typeface.scale);
        let run = TextRun {
            len: text.len(),
            font: typeface.font(style.weight),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let text_system = window.text_system();
        let (shaped, runs) = match limit {
            Some(limit) => {
                let mut wrapper = text_system.line_wrapper(run.font.clone(), size);
                let (shaped, runs) = wrapper.truncate_line(
                    text.clone(),
                    px(limit.max(0.) * typeface.scale),
                    "…",
                    std::slice::from_ref(&run),
                    TruncateFrom::End,
                );
                (shaped, runs.into_owned())
            }
            None => (text.clone(), vec![run]),
        };
        Self {
            line: text_system.shape_line(shaped, size, &runs, None),
            source: text,
            limit,
            scale: typeface.scale,
        }
    }

    pub fn matches(&self, text: &str, limit: Option<f32>) -> bool {
        self.limit == limit && self.source.as_ref() == text
    }

    /// The shaped width in world units.
    pub fn width(&self) -> f32 {
        f32::from(self.line.width) / self.scale
    }

    /// Paints with the left edge at `x` and the line box centered on `center_y`, both in
    /// window coordinates, at `zoom` screen pixels per world unit.
    pub fn paint(&self, x: Pixels, center_y: Pixels, zoom: f32, color: Hsla, window: &mut Window) {
        if color.a <= 0. {
            return;
        }
        let factor = zoom / self.scale;
        let font_size = self.line.font_size * factor;
        let baseline = center_y + (self.line.ascent - self.line.descent) * factor / 2.;
        for run in &self.line.runs {
            for glyph in &run.glyphs {
                let origin = point(
                    x + glyph.position.x * factor,
                    baseline + glyph.position.y * factor,
                );
                let painted = if glyph.is_emoji {
                    window.paint_emoji(origin, run.font_id, glyph.id, font_size)
                } else {
                    window.paint_glyph(origin, run.font_id, glyph.id, font_size, color)
                };
                if let Err(error) = painted {
                    tracing::warn!(%error, "graph canvas glyph could not be painted");
                    return;
                }
            }
        }
    }

    /// Paints with the right edge at `right`.
    pub fn paint_end(
        &self,
        right: Pixels,
        center_y: Pixels,
        zoom: f32,
        color: Hsla,
        window: &mut Window,
    ) {
        let x = right - px(self.width() * zoom);
        self.paint(x, center_y, zoom, color, window);
    }
}
