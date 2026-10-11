mod layout;
use gpui_kit::component::{
    ActiveTheme,
    plot::{
        IntoPlot, Plot, PlotLabel, TooltipState,
        label::{Text, truncate_text_to_width},
        tooltip::Tooltip,
    },
};
use gpui_kit::{
    AnyElement, App, Bounds, ElementId, Entity, IntoElement, Pixels, Point, SharedString,
    TextAlign, Window, point, px,
};
use layout::{Layout, Word};
use std::sync::Arc;
use yss_application::graph::results::plot::WordCloudPlot;

pub(crate) struct WordCloudData {
    words: Vec<Word>,
    maximum: usize,
    observations: usize,
    unique_words: usize,
}
impl WordCloudData {
    pub fn new(plot: WordCloudPlot) -> Self {
        Self {
            maximum: plot.words.iter().map(|word| word.count).max().unwrap_or(1),
            words: plot
                .words
                .into_iter()
                .map(|source| Word {
                    // SVG text collapsed whitespace; native shaping requires a single line.
                    text: source
                        .label
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .into(),
                    source,
                })
                .collect(),
            observations: plot.observations,
            unique_words: plot.unique_words,
        }
    }
}

#[derive(IntoPlot)]
pub(crate) struct WordCloud {
    data: Arc<WordCloudData>,
    id: SharedString,
    layout: Option<Entity<Option<Arc<Layout>>>>,
}
impl WordCloud {
    pub fn new(data: Arc<WordCloudData>, id: SharedString) -> Self {
        Self {
            data,
            id,
            layout: None,
        }
    }
}
impl Plot for WordCloud {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let id: SharedString = format!("{}-layout", self.id).into();
        let state = window.use_keyed_state(id, cx, |_, _| None::<Arc<Layout>>);
        let mut font = window.text_style().font();
        font.weight = gpui_kit::FontWeight::SEMIBOLD;
        state.update(cx, |cached, _| {
            if cached.as_ref().is_none_or(|layout| {
                layout.size != bounds.size
                    || layout.font != font
                    || layout.rem_size != window.rem_size()
                    || !Arc::ptr_eq(&layout.data, &self.data)
            }) {
                *cached = Some(Arc::new(Layout::new(
                    self.data.clone(),
                    bounds.size,
                    font,
                    window,
                )));
            }
        });
        self.layout = Some(state);
        vec![]
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(layout) = self
            .layout
            .as_ref()
            .and_then(|state| state.read(cx).clone())
        else {
            return;
        };
        let palette = [
            cx.theme().chart_1,
            cx.theme().chart_2,
            cx.theme().chart_3,
            cx.theme().chart_4,
            cx.theme().chart_5,
        ];
        let unit = window.rem_size() / 14.;
        window.with_content_mask(Some(gpui_kit::ContentMask { bounds }), |window| {
            for (i, placed) in layout.placed.iter().enumerate() {
                let word = &self.data.words[placed.index];
                let line = layout::shape(
                    word,
                    placed.font_size,
                    &layout.font,
                    palette[i % palette.len()],
                    window,
                );
                let _ = line.paint(
                    bounds.origin + placed.bounds.origin + point(unit * 4., unit * 2.),
                    placed.bounds.size.height - unit * 4.,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            let information: SharedString = crate::text::format(
                "plot.wordCounts",
                &[
                    ("displayed", layout.placed.len().to_string()),
                    ("unique", self.data.unique_words.to_string()),
                    ("observations", self.data.observations.to_string()),
                ],
            )
            .into();
            let information = truncate_text_to_width(
                &information,
                unit * 11.,
                (bounds.size.width - unit * 8.).as_f32(),
                window,
            );
            PlotLabel::new(vec![
                Text::new(
                    information,
                    point(unit * 4., px(0.)),
                    cx.theme().muted_foreground,
                )
                .font_size(unit * 11.),
            ])
            .paint(&bounds, window, cx);
        });
    }
    fn tooltip_state(&self, p: Point<Pixels>, _: Bounds<Pixels>, cx: &App) -> Option<TooltipState> {
        let layout = self.layout.as_ref()?.read(cx).as_ref()?;
        let word = layout.placed.iter().find(|word| word.bounds.contains(&p))?;
        Some(TooltipState::new(word.index, p, vec![]))
    }
    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let word = &self.data.words.get(state.index)?.source;
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(word.label.clone())
                .plain_row(
                    crate::text::translate("plot.frequency"),
                    word.count.to_string(),
                )
                .into_any_element(),
        )
    }
}
