//! Immutable geometry is prepared once on the result worker; controls only affect presentation.
use crate::plots::{
    cartesian::{CartesianData, CartesianKind, CartesianOptions, CartesianPlot},
    histogram::{self, HistogramDatum},
};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Selectable, Sizable,
    button::{Button, ButtonVariants},
    switch::Switch,
};
use gpui_kit_assets::IconName;
use std::sync::Arc;
use yss_application::graph::results::plot::{PlotAnnotation, PlotMetadata, ResultPlotProjection};
use yss_graph_execution::plan::PlotDataKind;

enum Geometry {
    Cartesian(Arc<CartesianData>),
    Histogram {
        bins: Vec<HistogramDatum>,
        x_label: Option<Box<str>>,
        y_label: Option<Box<str>>,
        observations: Option<usize>,
    },
}
pub(super) struct PlotData {
    geometry: Geometry,
    kind: PlotDataKind,
    metadata: Option<PlotMetadata>,
    annotation: PlotAnnotation,
}
impl PlotData {
    pub fn new(projection: ResultPlotProjection) -> Self {
        match projection {
            ResultPlotProjection::Cartesian(plot) => {
                let kind = match plot.kind {
                    PlotDataKind::Line | PlotDataKind::Roc => CartesianKind::Line,
                    PlotDataKind::Ecdf => CartesianKind::Ecdf,
                    PlotDataKind::Kde => CartesianKind::Density,
                    _ => CartesianKind::Scatter,
                };
                let unit_axes = matches!(
                    plot.annotation,
                    PlotAnnotation::Roc { .. } | PlotAnnotation::Probability { pp: true, .. }
                );
                let options = CartesianOptions {
                    kind,
                    reference_lines: plot.reference_lines,
                    point_sizes: plot.point_sizes,
                    x_domain: unit_axes.then_some([0., 1.]),
                    y_domain: if unit_axes {
                        Some([0., 1.])
                    } else {
                        plot.y_domain
                    },
                };
                Self {
                    geometry: Geometry::Cartesian(Arc::new(CartesianData::new(
                        plot.series,
                        options,
                    ))),
                    kind: plot.kind,
                    metadata: plot.metadata,
                    annotation: plot.annotation,
                }
            }
            ResultPlotProjection::Histogram(plot) => Self {
                geometry: Geometry::Histogram {
                    bins: plot
                        .bins
                        .into_iter()
                        .enumerate()
                        .map(|(index, bin)| HistogramDatum {
                            index,
                            label: bin.label,
                            count: bin.count,
                        })
                        .collect(),
                    x_label: plot.x_label,
                    y_label: plot.y_label,
                    observations: plot.observations,
                },
                kind: PlotDataKind::Histogram,
                metadata: None,
                annotation: PlotAnnotation::None,
            },
        }
    }
    fn information(&self) -> Vec<String> {
        let mut information = Vec::new();
        if let Some(meta) = &self.metadata {
            information.push(crate::text::format(
                if meta.sampled {
                    "plot.sampled"
                } else {
                    "plot.observations"
                },
                &[
                    ("observations", meta.observations.to_string()),
                    ("displayed", meta.displayed.to_string()),
                ],
            ));
        }
        match &self.annotation {
            PlotAnnotation::Roc {
                auc,
                positives,
                negatives,
            } => {
                information.push(format!("AUC = {auc:.4}"));
                information.push(crate::text::format(
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
            } => information.push(crate::text::format(
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
            PlotAnnotation::Quadrant { counts } => information.push(crate::text::format(
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
        if let Geometry::Histogram {
            observations: Some(count),
            ..
        } = &self.geometry
        {
            information.push(crate::text::format(
                "plot.observations",
                &[("observations", count.to_string())],
            ));
        }
        information
    }
}

pub(super) struct PlotView {
    data: PlotData,
    toolbar_open: bool,
    points_visible: bool,
}
impl PlotView {
    pub fn new(data: PlotData) -> Self {
        Self {
            data,
            toolbar_open: false,
            points_visible: true,
        }
    }
}
impl Render for PlotView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (x_label, y_label) = match &self.data.geometry {
            Geometry::Cartesian(data) => (
                data.result.x_label.as_deref(),
                data.result.y_label.as_deref(),
            ),
            Geometry::Histogram {
                x_label, y_label, ..
            } => (x_label.as_deref(), y_label.as_deref()),
        };
        div()
            .id("result-plot")
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .children(self.data.information())
                    .when(self.data.kind == PlotDataKind::Line, |bar| {
                        bar.child(
                            Button::new("line-toolbar")
                                .small()
                                .ghost()
                                .icon(IconName::Settings2)
                                .tooltip(crate::text::translate("native.plots.toolbar"))
                                .selected(self.toolbar_open)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.toolbar_open = !view.toolbar_open;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(
                self.data.kind == PlotDataKind::Line && self.toolbar_open,
                |view| {
                    view.child(
                        Switch::new("line-points")
                            .small()
                            .label(crate::text::translate("native.plots.points"))
                            .checked(self.points_visible)
                            .on_click(cx.listener(|view, checked: &bool, _, cx| {
                                view.points_visible = *checked;
                                cx.notify();
                            })),
                    )
                },
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::format(
                        "native.plots.axes",
                        &[
                            ("x", x_label.unwrap_or("X").into()),
                            ("y", y_label.unwrap_or("Y").into()),
                        ],
                    )),
            )
            .child(match &self.data.geometry {
                Geometry::Cartesian(data) => div()
                    .flex_1()
                    .min_h_0()
                    .child(CartesianPlot {
                        data: data.clone(),
                        id: format!("result-plot-{}", cx.entity_id()).into(),
                        generation: 0,
                        show_points: self.data.kind == PlotDataKind::Line && self.points_visible,
                    })
                    .into_any_element(),
                Geometry::Histogram { bins, .. } => div()
                    .flex_1()
                    .min_h_0()
                    .child(histogram::render(
                        format!("result-histogram-{}", cx.entity_id()),
                        bins,
                        cx,
                    ))
                    .into_any_element(),
            })
    }
}
