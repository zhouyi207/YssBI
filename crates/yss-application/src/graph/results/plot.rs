//! Native plot reads project a complete retained result; they never compute statistics.
mod read;
#[cfg(test)]
mod tests;

use super::{MAX_RESULT_PAGE_BYTES, ResultQueryApplicationError, charge_value};
use crate::{
    chart::{ChartPlotResult, PlotPoint},
    session::ApplicationState,
};
use yss_graph_execution::{
    plan::{PlotDataKind, ResultCategory},
    result::ResultReference,
};
pub use yss_sci_contract::visualization::PlotMetadata;
pub use yss_sci_contract::{
    survival::{NomogramAxis, NomogramPlot, NomogramTick},
    visualization::{
        CoefficientPlot, CoefficientPoint, CombinationPlot, CorrelationPlot, CorrelogramPlot,
        CorrelogramPoint, DistributionGroup, DistributionPlot, HeatmapPlot, IntervalPlot,
        IntervalPoint, ParetoCategory, ParetoPlot, PlotPoint as DensityPoint, WordCloudPlot,
        WordCount,
    },
};

#[derive(Debug, PartialEq)]
pub enum ResultPlotProjection {
    Cartesian(CartesianResultPlot),
    Histogram(HistogramResultPlot),
    Correlation(CorrelationPlot),
    Correlogram(CorrelogramPlot),
    Distribution {
        plot: DistributionPlot,
        violin: bool,
    },
    Heatmap(HeatmapPlot),
    Interval(IntervalPlot),
    Coefficient(CoefficientPlot),
    Nomogram(NomogramPlot),
    Pareto(ParetoPlot),
    Combination(CombinationPlot),
    WordCloud(WordCloudPlot),
}

#[derive(Debug, PartialEq)]
pub struct CartesianResultPlot {
    pub kind: PlotDataKind,
    pub series: ChartPlotResult,
    pub reference_lines: Vec<[PlotPoint; 2]>,
    pub point_sizes: Option<Vec<f64>>,
    pub y_domain: Option<[f64; 2]>,
    pub metadata: Option<PlotMetadata>,
    pub annotation: PlotAnnotation,
}

#[derive(Debug, PartialEq)]
pub enum PlotAnnotation {
    None,
    Probability {
        pp: bool,
        mean: f64,
        standard_deviation: f64,
    },
    Roc {
        auc: f64,
        positives: usize,
        negatives: usize,
    },
    Quadrant {
        counts: [usize; 4],
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistogramResultBin {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, PartialEq)]
pub struct HistogramResultPlot {
    pub bins: Vec<HistogramResultBin>,
    pub x_label: Option<Box<str>>,
    pub y_label: Option<Box<str>>,
    pub observations: Option<usize>,
}

impl ApplicationState {
    /// Returns None for unavailable results or non-plot categories.
    /// Malformed plot data fails instead of falling back to a partial plot.
    pub fn query_result_plot(
        &self,
        reference: ResultReference,
    ) -> Result<Option<ResultPlotProjection>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        let Some(snapshot) = captured.execution().query_result(reference.result_id) else {
            return Ok(None);
        };
        let projection = match snapshot.value().category() {
            ResultCategory::PlotData(kind) => {
                // Bound traversal and allocation before projecting any rows, including full
                // control-chart outputs that deliberately retain every observation.
                let mut budget = MAX_RESULT_PAGE_BYTES;
                charge_value(snapshot.value().value(), &mut budget, 0)?;
                read::project(kind, snapshot.value().value()).map(Some)
            }
            _ => Ok(None),
        };
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        if captured
            .execution()
            .query_result(reference.result_id)
            .is_none()
        {
            return Ok(None);
        }
        projection
    }
}
