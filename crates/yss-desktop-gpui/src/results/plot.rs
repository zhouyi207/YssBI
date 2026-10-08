//! Immutable geometry is prepared once on the result worker; controls only affect presentation.
mod information;
mod render;
use crate::plots::{
    cartesian::{CartesianData, CartesianKind, CartesianOptions, CartesianPlot},
    correlogram::{Correlogram, CorrelogramData},
    distribution::{Distribution, DistributionData},
    histogram::{self, HistogramDatum},
    interval::{Interval, IntervalData},
    matrix::{MatrixData, MatrixPlot},
    nomogram::{Nomogram, NomogramData},
};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable,
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
    Matrix {
        data: Arc<MatrixData>,
        observations: Option<usize>,
    },
    Correlogram {
        acf: Arc<CorrelogramData>,
        pacf: Arc<CorrelogramData>,
        observations: usize,
    },
    Distribution(Arc<DistributionData>),
    Interval {
        data: Arc<IntervalData>,
        confidence: Option<f64>,
    },
    Nomogram {
        data: Arc<NomogramData>,
        horizon: f64,
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
            ResultPlotProjection::Correlation(plot) => {
                let observations = Some(plot.observations);
                Self::statistical(
                    Geometry::Matrix {
                        data: Arc::new(MatrixData::correlation(plot)),
                        observations,
                    },
                    PlotDataKind::Correlation,
                    None,
                )
            }
            ResultPlotProjection::Heatmap(plot) => {
                let metadata = Some(plot.metadata.clone());
                Self::statistical(
                    Geometry::Matrix {
                        data: Arc::new(MatrixData::heatmap(plot)),
                        observations: None,
                    },
                    PlotDataKind::Heatmap,
                    metadata,
                )
            }
            ResultPlotProjection::Correlogram(plot) => Self::statistical(
                Geometry::Correlogram {
                    acf: Arc::new(CorrelogramData::new(plot.acf, plot.ci_half_width)),
                    pacf: Arc::new(CorrelogramData::new(plot.pacf, plot.ci_half_width)),
                    observations: plot.n,
                },
                PlotDataKind::Correlogram,
                None,
            ),
            ResultPlotProjection::Distribution { plot, violin } => Self::statistical(
                Geometry::Distribution(Arc::new(DistributionData::new(plot.groups, violin))),
                if violin {
                    PlotDataKind::Violin
                } else {
                    PlotDataKind::Boxplot
                },
                None,
            ),
            ResultPlotProjection::Interval(plot) => Self::statistical(
                Geometry::Interval {
                    data: Arc::new(IntervalData::errors(plot.data)),
                    confidence: None,
                },
                PlotDataKind::Errorbar,
                Some(plot.metadata),
            ),
            ResultPlotProjection::Coefficient(plot) => Self::statistical(
                Geometry::Interval {
                    data: Arc::new(IntervalData::coefficients(plot.data)),
                    confidence: Some(plot.confidence_level),
                },
                PlotDataKind::Coefficient,
                None,
            ),
            ResultPlotProjection::Nomogram(plot) => Self::statistical(
                Geometry::Nomogram {
                    data: Arc::new(NomogramData::new(plot.axes)),
                    horizon: plot.horizon,
                },
                PlotDataKind::Nomogram,
                None,
            ),
        }
    }
    fn statistical(geometry: Geometry, kind: PlotDataKind, metadata: Option<PlotMetadata>) -> Self {
        Self {
            geometry,
            kind,
            metadata,
            annotation: PlotAnnotation::None,
        }
    }
    fn page_count(&self) -> usize {
        if let Geometry::Interval { data, .. } = &self.geometry {
            data.page_count()
        } else {
            1
        }
    }
}

pub(super) struct PlotView {
    data: PlotData,
    toolbar_open: bool,
    points_visible: bool,
    page: usize,
}
impl PlotView {
    pub fn new(data: PlotData) -> Self {
        Self {
            data,
            toolbar_open: false,
            points_visible: true,
            page: 0,
        }
    }
}
