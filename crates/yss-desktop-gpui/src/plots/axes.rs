use chrono::DateTime;
use gpui::{Bounds, Pixels, Size, point, px, size};
use yss_application::chart::PlotAxisFormat;

#[derive(Clone, Copy)]
pub(super) struct AxisDomain {
    magnitude: f64,
    start: f64,
    span: f64,
}
impl AxisDomain {
    pub fn fixed([start, end]: [f64; 2]) -> Self {
        let magnitude = start.abs().max(end.abs()).max(f64::MIN_POSITIVE);
        Self {
            magnitude,
            start: start / magnitude,
            span: end / magnitude - start / magnitude,
        }
    }
    pub fn from_values(values: impl Iterator<Item = f64>) -> Self {
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for value in values {
            min = min.min(value);
            max = max.max(value);
        }
        if !min.is_finite() || !max.is_finite() {
            return Self {
                magnitude: 1.,
                start: 0.,
                span: 1.,
            };
        }
        // Normalize before subtracting: even two finite endpoints can have an infinite span.
        let magnitude = min.abs().max(max.abs());
        let magnitude = if magnitude == 0. { 1. } else { magnitude };
        let (start, end) = (min / magnitude, max / magnitude);
        let pad = if start == end {
            0.06
        } else {
            (end - start) * 0.06
        };
        Self {
            magnitude,
            start: start - pad,
            span: end - start + 2. * pad,
        }
    }
    pub fn position(self, value: f64) -> f32 {
        ((value / self.magnitude - self.start) / self.span) as f32
    }
    pub fn at(self, fraction: f64) -> f64 {
        (self.start + self.span * fraction) * self.magnitude
    }
}
pub(super) fn chart_box(bounds: Size<Pixels>, y_format: PlotAxisFormat) -> Option<Bounds<Pixels>> {
    let left = match y_format {
        PlotAxisFormat::Number => 72.,
        PlotAxisFormat::Date => 104.,
        PlotAxisFormat::Datetime => 144.,
    };
    let width = bounds.width - px(left + 28.);
    let height = bounds.height - px(66.);
    (width > px(24.) && height > px(24.))
        .then_some(Bounds::new(point(px(left), px(18.)), size(width, height)))
}
pub(crate) fn axis_value(value: f64, format: PlotAxisFormat) -> String {
    let date = match format {
        PlotAxisFormat::Date => DateTime::from_timestamp((value * 86400.).round() as i64, 0),
        PlotAxisFormat::Datetime => DateTime::from_timestamp_micros(value.round() as i64),
        PlotAxisFormat::Number => None,
    };
    if let Some(date) = date {
        return date
            .naive_utc()
            .format(if format == PlotAxisFormat::Date {
                "%Y-%m-%d"
            } else {
                "%Y-%m-%d %H:%M"
            })
            .to_string();
    }
    if !value.is_finite() {
        return "—".into();
    }
    if value != 0. && (value.abs() >= 1e7 || value.abs() < 1e-4) {
        format!("{value:.3e}")
    } else if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into()
    }
}
