use super::*;

impl PlotData {
    pub(super) fn information(&self) -> Vec<String> {
        let mut information = Vec::new();
        if let Some(meta) = &self.metadata {
            information.push(if meta.sampled {
                yss_ui::text::format(
                    "plot.sampled",
                    &[
                        (
                            "points",
                            yss_ui::text::plural("counts.points", meta.displayed),
                        ),
                        (
                            "observations",
                            yss_ui::text::plural("counts.observations", meta.observations),
                        ),
                    ],
                )
            } else {
                yss_ui::text::format(
                    "plot.observations",
                    &[("observations", meta.observations.to_string())],
                )
            });
        }
        match &self.annotation {
            PlotAnnotation::Roc {
                auc,
                positives,
                negatives,
            } => {
                information.push(format!("AUC = {auc:.4}"));
                information.push(yss_ui::text::format(
                    "plot.rocCounts",
                    &[
                        ("positives", positives.to_string()),
                        ("negatives", negatives.to_string()),
                    ],
                ));
            }
            PlotAnnotation::Probability {
                mean,
                standard_deviation,
                ..
            } => information.push(yss_ui::text::format(
                "plot.normalReference",
                &[
                    (
                        "mean",
                        crate::plots::axis_value(
                            *mean,
                            yss_application::chart::PlotAxisFormat::Number,
                        ),
                    ),
                    (
                        "sd",
                        crate::plots::axis_value(
                            *standard_deviation,
                            yss_application::chart::PlotAxisFormat::Number,
                        ),
                    ),
                ],
            )),
            PlotAnnotation::Quadrant { counts } => information.push(yss_ui::text::format(
                "plot.quadrantCounts",
                &[(
                    "counts",
                    counts
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(" / "),
                )],
            )),
            PlotAnnotation::None => {}
        }
        match &self.geometry {
            Geometry::Histogram {
                observations: Some(count),
                ..
            }
            | Geometry::Matrix {
                observations: Some(count),
                ..
            }
            | Geometry::Correlogram {
                observations: count,
                ..
            }
            | Geometry::Composite {
                observations: Some(count),
                ..
            } => information.push(yss_ui::text::format(
                "plot.observations",
                &[("observations", count.to_string())],
            )),
            Geometry::Interval {
                confidence: Some(level),
                ..
            } => information.push(yss_ui::text::format(
                "plot.confidence",
                &[("level", format!("{:.1}", level * 100.))],
            )),
            Geometry::Nomogram { horizon, .. } => information.push(yss_ui::text::format(
                "native.plots.horizon",
                &[(
                    "value",
                    crate::plots::axis_value(
                        *horizon,
                        yss_application::chart::PlotAxisFormat::Number,
                    ),
                )],
            )),
            _ => {}
        }
        information
    }
}
