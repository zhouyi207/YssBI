//! Numerical plot data; pixel layout and rendering remain caller-owned.
use serde::Serialize;

pub const MAX_PLOT_POINTS: usize = 2048;
pub const MAX_PLOT_GROUPS: usize = 64;
pub const MAX_PLOT_BINS: usize = 128;
pub const MAX_PLOT_WORDS: usize = 256;
pub const MAX_HEATMAP_ROWS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PlotPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotMetadata {
    pub observations: usize,
    pub displayed: usize,
    pub sampled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReferenceLine {
    pub start: PlotPoint,
    pub end: PlotPoint,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XyPlot {
    pub data: Vec<PlotPoint>,
    pub x_label: String,
    pub y_label: String,
    pub reference_lines: Vec<ReferenceLine>,
    pub metadata: PlotMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistogramBin {
    pub label: String,
    pub lower: f64,
    pub upper: f64,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistogramPlot {
    pub data: Vec<HistogramBin>,
    pub x_label: String,
    pub y_label: String,
    pub observations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrelationPlot {
    pub labels: Vec<String>,
    pub matrix: Vec<Vec<Option<f64>>>,
    pub p_matrix: Vec<Vec<Option<f64>>>,
    pub observations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrelogramPoint {
    pub lag: usize,
    pub value: f64,
    pub q_stat: Option<f64>,
    pub p_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrelogramPlot {
    pub acf: Vec<CorrelogramPoint>,
    pub pacf: Vec<CorrelogramPoint>,
    pub ci_half_width: f64,
    pub n: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DistributionGroup {
    pub label: String,
    pub observations: usize,
    pub lower_whisker: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub upper_whisker: f64,
    pub outliers: Vec<f64>,
    pub outlier_count: usize,
    pub density: Vec<PlotPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DistributionPlot {
    pub groups: Vec<DistributionGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordCount {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordCloudPlot {
    pub words: Vec<WordCount>,
    pub observations: usize,
    pub unique_words: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct IntervalPoint {
    pub x: f64,
    pub y: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntervalPlot {
    pub data: Vec<IntervalPoint>,
    pub metadata: PlotMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbabilityPlotMode {
    Pp,
    Qq,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbabilityPlot {
    #[serde(flatten)]
    pub plot: XyPlot,
    pub mode: ProbabilityPlotMode,
    pub reference_mean: f64,
    pub reference_standard_deviation: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RocPlot {
    #[serde(flatten)]
    pub plot: XyPlot,
    pub auc: f64,
    pub positives: usize,
    pub negatives: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantPlot {
    #[serde(flatten)]
    pub plot: XyPlot,
    pub x_cut: f64,
    pub y_cut: f64,
    /// Upper-right, upper-left, lower-left, lower-right; boundaries go right/up.
    pub counts: [usize; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParetoCategory {
    pub label: String,
    pub count: usize,
    pub cumulative: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParetoPlot {
    pub data: Vec<ParetoCategory>,
    pub observations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinationPlot {
    pub labels: Vec<String>,
    pub bars: Vec<f64>,
    pub line: Vec<f64>,
    pub dual_axis: bool,
    pub metadata: PlotMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BubblePoint {
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BubblePlot {
    pub data: Vec<BubblePoint>,
    pub metadata: PlotMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapPlot {
    pub x_labels: Vec<String>,
    pub y_labels: Vec<String>,
    pub matrix: Vec<Vec<f64>>,
    pub metadata: PlotMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoefficientPoint {
    pub label: String,
    pub value: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoefficientPlot {
    pub data: Vec<CoefficientPoint>,
    pub confidence_level: f64,
}
