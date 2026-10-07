//! Designed-experiment summaries; factor labels and graph relations are adapter-owned.
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct StationaryPoint {
    pub coded_factors: Vec<f64>,
    pub factors: Vec<f64>,
    pub predicted_response: f64,
    pub inside_factor_ranges: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct SurfaceGeometry {
    pub centers: Vec<f64>,
    pub half_ranges: Vec<f64>,
    pub hessian_eigenvalues: Vec<f64>,
    pub classification: &'static str,
    pub stationary_point: Option<StationaryPoint>,
}
pub struct ResponseSurfaceResult {
    pub model: crate::regression::models::RegressionModelResult,
    pub geometry: SurfaceGeometry,
}
#[derive(Debug, Clone, Serialize)]
pub struct PositiveCurveParameter {
    pub estimate: f64,
    pub standard_error: Option<f64>,
    pub confidence_interval: Option<[f64; 2]>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DoseResponseParameters {
    pub hill: PositiveCurveParameter,
    pub ed50: PositiveCurveParameter,
    pub increasing: bool,
    pub ed50_inside_dose_range: bool,
}
pub struct DoseResponseResult {
    pub model: crate::regression::models::RegressionModelResult,
    pub parameters: DoseResponseParameters,
}
#[derive(Debug, Clone, Copy)]
pub enum DesignSpecification {
    FullFactorial {
        factors: usize,
        levels: usize,
    },
    Orthogonal {
        factors: usize,
        levels: usize,
    },
    Uniform {
        factors: usize,
        runs: usize,
        candidates: usize,
        seed: u64,
    },
}
#[derive(Debug, Clone, Serialize)]
pub struct DesignSummary {
    pub method: &'static str,
    pub runs: usize,
    pub factors: usize,
    pub levels: usize,
    pub candidates: Option<usize>,
    pub seed: Option<u64>,
    /// Squared centered L2 discrepancy, using points (level - 0.5) / levels.
    pub centered_l2_discrepancy: Option<f64>,
}
pub struct DesignResult {
    pub summary: DesignSummary,
    /// Run number followed by coded factor levels, all one-based.
    pub rows: Vec<Vec<f64>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct FactorRange {
    pub factor: usize,
    pub levels: usize,
    pub range: f64,
    pub balanced: bool,
    pub optimal_levels: Vec<usize>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RangeLevel {
    pub factor: usize,
    pub level: usize,
    pub observations: usize,
    pub total: f64,
    pub mean: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct RangeAnalysisSummary {
    pub observations: usize,
    pub maximize: bool,
    pub pairwise_orthogonal: Option<bool>,
    pub equal_level_counts: bool,
    pub factors: Vec<FactorRange>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RangeAnalysisResult {
    pub summary: RangeAnalysisSummary,
    pub rows: Vec<RangeLevel>,
}
