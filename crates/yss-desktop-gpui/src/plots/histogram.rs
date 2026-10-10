use gpui_kit::component::{ActiveTheme, chart::BarChart};
use gpui_kit::{App, IntoElement, SharedString};

#[derive(Clone)]
pub(crate) struct HistogramDatum {
    pub index: usize,
    pub label: String,
    pub count: usize,
}
#[derive(Clone, PartialEq, Eq, Hash)]
struct Band {
    index: usize,
    label: SharedString,
}
impl From<Band> for SharedString {
    fn from(value: Band) -> Self {
        value.label
    }
}

pub(crate) fn render(id: String, bins: &[HistogramDatum], cx: &App) -> impl IntoElement + use<> {
    let color = cx.theme().primary;
    BarChart::new(bins.iter().cloned())
        .id(id)
        .band(|datum: &HistogramDatum| Band {
            index: datum.index,
            label: datum.label.clone().into(),
        })
        .value(|datum: &HistogramDatum| datum.count as f64)
        .tooltip_value(|datum, _| datum.count.to_string().into())
        .fill(move |_, _, _, _| color)
        .value_axis(true)
        .band_tick_count(8)
        .appear(false)
}
