//! Process variation, capability and measurement-system results.
use crate::visualization::XyPlot;
use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlChartKind {
    Individuals,
    MovingRange,
}
#[derive(Debug, Clone, Serialize)]
pub struct ControlChartSummary {
    pub method: ControlChartKind,
    pub observations: usize,
    pub plotted: usize,
    pub center: f64,
    pub lower: f64,
    pub upper: f64,
    pub within_standard_deviation: f64,
    pub mean_moving_range: f64,
    pub outside_count: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct ControlChartRow {
    pub observation: usize,
    pub value: f64,
    pub outside: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct ControlChartResult {
    pub summary: ControlChartSummary,
    pub plot: XyPlot,
    pub rows: Vec<ControlChartRow>,
}
#[derive(Debug, Clone, Copy)]
pub struct CapabilityLimits {
    pub lower: f64,
    pub upper: f64,
    pub target: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct CapabilityResult {
    pub observations: usize,
    pub within_method: &'static str,
    pub subgroups: Option<usize>,
    pub lower_specification: f64,
    pub upper_specification: f64,
    pub target: f64,
    pub mean: f64,
    pub overall_standard_deviation: f64,
    pub within_standard_deviation: f64,
    pub cp: Option<f64>,
    pub cpk: Option<f64>,
    pub pp: Option<f64>,
    pub ppk: Option<f64>,
    pub cpm: Option<f64>,
    pub below_specification: usize,
    pub above_specification: usize,
    pub observed_outside_percent: f64,
    pub within_normal_ppm: Option<f64>,
    pub overall_normal_ppm: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GageAnovaTerm {
    pub source: &'static str,
    pub sum_squares: f64,
    pub degrees_of_freedom: usize,
    pub mean_square: f64,
    pub f_statistic: Option<f64>,
    pub denominator_degrees_of_freedom: Option<usize>,
    pub p_value: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GageComponent {
    pub source: &'static str,
    pub variance: f64,
    pub standard_deviation: f64,
    pub study_variation: f64,
    pub contribution_percent: Option<f64>,
    pub study_variation_percent: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GageResult {
    pub method: &'static str,
    pub observations: usize,
    pub parts: usize,
    pub operators: usize,
    pub repetitions: usize,
    pub include_interaction: bool,
    pub total_sum_squares: f64,
    pub anova: Vec<GageAnovaTerm>,
    pub components: Vec<GageComponent>,
    pub negative_components_truncated: Vec<&'static str>,
}
