pub const DEFAULT_MAX_PLOT_POINTS: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotAxisFormat {
    Number,
    Date,
    Datetime,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartPlotResult {
    pub data: Vec<PlotPoint>,
    pub x_label: Option<Box<str>>,
    pub y_label: Option<Box<str>>,
    pub x_format: PlotAxisFormat,
    pub y_format: PlotAxisFormat,
}

#[derive(Clone, Copy)]
pub(super) struct ChartPlotInput<'a> {
    pub(super) x: &'a [Option<f64>],
    pub(super) y: &'a [Option<f64>],
    pub(super) x_label: Option<&'a str>,
    pub(super) y_label: Option<&'a str>,
    pub(super) x_format: PlotAxisFormat,
    pub(super) y_format: PlotAxisFormat,
}

pub(super) fn project_chart_plot(
    pair: ChartPlotInput<'_>,
    max_points: Option<usize>,
) -> Option<ChartPlotResult> {
    let points = || {
        pair.x.iter().zip(pair.y).filter_map(|(x, y)| match (x, y) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => {
                Some(PlotPoint { x: *x, y: *y })
            }
            _ => None,
        })
    };
    let count = points().count();
    if count == 0 {
        return None;
    }
    let max_points = max_points.unwrap_or(DEFAULT_MAX_PLOT_POINTS);
    // Count before sampling so the output allocation stays bounded by the requested cap.
    let data = if max_points == 0 {
        Vec::new()
    } else {
        points()
            .step_by(count.div_ceil(max_points))
            .take(max_points)
            .collect()
    };
    Some(ChartPlotResult {
        data,
        x_label: pair.x_label.map(Into::into),
        y_label: pair.y_label.map(Into::into),
        x_format: pair.x_format,
        y_format: pair.y_format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_projection_filters_caps_and_preserves_formats() {
        let x = [Some(1.0), Some(f64::NAN), Some(3.0), Some(4.0), Some(5.0)];
        let y = [
            Some(10.0),
            Some(20.0),
            Some(f64::INFINITY),
            Some(40.0),
            Some(50.0),
        ];
        let result = project_chart_plot(
            ChartPlotInput {
                x: &x,
                y: &y,
                x_label: Some("observed_date"),
                y_label: Some("measure"),
                x_format: PlotAxisFormat::Date,
                y_format: PlotAxisFormat::Number,
            },
            Some(2),
        )
        .expect("finite points remain after filtering");

        assert_eq!(
            result.data,
            vec![PlotPoint { x: 1.0, y: 10.0 }, PlotPoint { x: 5.0, y: 50.0 },]
        );
        assert_eq!(result.x_label.as_deref(), Some("observed_date"));
        assert_eq!(result.y_label.as_deref(), Some("measure"));
        assert_eq!(result.x_format, PlotAxisFormat::Date);
        assert_eq!(result.y_format, PlotAxisFormat::Number);
        assert!(
            project_chart_plot(
                ChartPlotInput {
                    x: &x,
                    y: &y,
                    x_label: None,
                    y_label: None,
                    x_format: PlotAxisFormat::Datetime,
                    y_format: PlotAxisFormat::Number,
                },
                Some(0),
            )
            .expect("zero cap preserves the existing empty-success behavior")
            .data
            .is_empty()
        );
    }
}
