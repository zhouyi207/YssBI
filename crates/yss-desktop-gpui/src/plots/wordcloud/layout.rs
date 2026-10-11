use super::WordCloudData;
use gpui_kit::{
    Bounds, Font, Hsla, Pixels, Point, ShapedLine, SharedString, Size, TextRun, Window, point, px,
    size,
};
use std::sync::Arc;
use yss_application::graph::results::plot::WordCount;

pub(super) struct Word {
    pub source: WordCount,
    pub text: SharedString,
}
pub(super) struct PlacedWord {
    pub index: usize,
    pub bounds: Bounds<Pixels>,
    pub font_size: Pixels,
}
pub(super) struct Layout {
    pub data: Arc<WordCloudData>,
    pub size: Size<Pixels>,
    pub font: Font,
    pub rem_size: Pixels,
    pub placed: Vec<PlacedWord>,
}
impl Layout {
    pub fn new(
        data: Arc<WordCloudData>,
        bounds: Size<Pixels>,
        font: Font,
        window: &mut Window,
    ) -> Self {
        let rem_size = window.rem_size();
        let scale = rem_size / px(14.);
        let header_height = 28. * scale;
        let margin = 8. * scale;
        let mut layout = Self {
            data,
            size: bounds,
            font,
            rem_size,
            placed: vec![],
        };
        let (width, height) = (
            bounds.width.as_f32(),
            bounds.height.as_f32() - header_height,
        );
        if width < 40. * scale || height < 40. * scale {
            return layout;
        }
        let largest = (70. * scale).min(width / 8.).min(height / 5.);
        // Every word searches the same deterministic spiral, measured once per viewport.
        let positions: Vec<Point<Pixels>> = (0..1600)
            .map(|attempt| {
                let angle = attempt as f32 * 0.32;
                let radius = (attempt as f32).sqrt() * 4.5 * scale;
                point(
                    px(width / 2. + angle.cos() * radius * (width / height).max(1.)),
                    px(header_height + height / 2. + angle.sin() * radius),
                )
            })
            .collect();
        let area = Bounds::new(
            point(px(margin), px(header_height + margin)),
            size(px(width - margin * 2.), px(height - margin * 2.)),
        );
        for (index, word) in layout.data.words.iter().enumerate() {
            let minimum = 10. * scale;
            let mut font_size = px(minimum
                + (largest - minimum)
                    * (word.source.count as f64 / layout.data.maximum as f64).sqrt() as f32);
            let mut line = shape(word, font_size, &layout.font, Hsla::default(), window);
            if line.width() > px(width - 30. * scale) {
                font_size *= px(width - 30. * scale) / line.width();
                line = shape(word, font_size, &layout.font, Hsla::default(), window);
            }
            let word_size = size(
                line.width() + px(8. * scale),
                (font_size * 1.2).max(line.ascent + line.descent) + px(4. * scale),
            );
            let Some(bounds) = positions
                .iter()
                .map(|center| {
                    Bounds::new(
                        *center - point(word_size.width / 2., word_size.height / 2.),
                        word_size,
                    )
                })
                .find(|candidate| {
                    area.contains(&candidate.origin)
                        && candidate.right() <= area.right()
                        && candidate.bottom() <= area.bottom()
                        && !layout
                            .placed
                            .iter()
                            .any(|other| other.bounds.intersects(candidate))
                })
            else {
                continue;
            };
            layout.placed.push(PlacedWord {
                index,
                bounds,
                font_size,
            });
        }
        layout
    }
}

pub(super) fn shape(
    word: &Word,
    font_size: Pixels,
    font: &Font,
    color: Hsla,
    window: &mut Window,
) -> ShapedLine {
    window.text_system().shape_line(
        word.text.clone(),
        font_size,
        &[TextRun {
            len: word.text.len(),
            font: font.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    )
}
