//! Item-scale summaries and expert relevance assessments; labels remain adapter-owned.
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct ReliabilitySummary {
    pub method: &'static str,
    pub observations: usize,
    pub items: usize,
    pub raw_alpha: Option<f64>,
    pub standardized_alpha: Option<f64>,
    pub total_mean: f64,
    pub total_standard_deviation: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReliabilityItem {
    pub item: usize,
    pub mean: f64,
    pub standard_deviation: f64,
    pub corrected_item_total_correlation: Option<f64>,
    pub alpha_if_deleted: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReliabilityResult {
    pub summary: ReliabilitySummary,
    pub rows: Vec<ReliabilityItem>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DiscriminationItem {
    #[serde(flatten)]
    pub reliability: ReliabilityItem,
    pub low_mean: Option<f64>,
    pub high_mean: Option<f64>,
    pub t_statistic: Option<f64>,
    pub degrees_of_freedom: Option<f64>,
    pub p_value: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ItemScore {
    pub observation: usize,
    pub total: f64,
    /// Low = -1, middle = 0, high = 1. Tied cutoffs never put a row in both tails.
    pub group: i8,
}
#[derive(Debug, Clone, Serialize)]
pub struct DiscriminationSummary {
    #[serde(flatten)]
    pub reliability: ReliabilitySummary,
    pub tail_fraction: f64,
    pub low_cutoff: f64,
    pub high_cutoff: f64,
    pub low_count: usize,
    pub high_count: usize,
    pub tail_test: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub struct DiscriminationResult {
    pub summary: DiscriminationSummary,
    pub rows: Vec<DiscriminationItem>,
    pub scores: Vec<ItemScore>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ContentValiditySummary {
    pub method: &'static str,
    pub experts: usize,
    pub items: usize,
    pub scale_cvi_average: f64,
    pub scale_cvi_universal_agreement: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct ContentValidityItem {
    pub item: usize,
    pub relevant_experts: usize,
    pub item_cvi: f64,
    pub chance_agreement: f64,
    pub modified_kappa: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct ContentValidityResult {
    pub summary: ContentValiditySummary,
    pub rows: Vec<ContentValidityItem>,
}
