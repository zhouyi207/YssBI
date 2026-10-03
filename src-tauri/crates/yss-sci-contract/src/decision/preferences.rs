//! Descriptive customer preference and value summaries.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct NpsResult {
    pub observations: usize,
    pub detractors: usize,
    pub passives: usize,
    pub promoters: usize,
    pub detractor_percent: f64,
    pub passive_percent: f64,
    pub promoter_percent: f64,
    pub net_promoter_score: f64,
    /// Counts for ratings 0 through 10.
    pub rating_counts: [usize; 11],
}
#[derive(Debug, Clone, Serialize)]
pub struct KanoCategory {
    pub category: &'static str,
    pub count: usize,
    pub percent: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct KanoResult {
    pub observations: usize,
    pub categories: Vec<KanoCategory>,
    pub dominant_categories: Vec<&'static str>,
    pub coefficient_observations: usize,
    pub better: Option<f64>,
    pub worse: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RfmSummary {
    pub observations: usize,
    pub scoring: &'static str,
    pub tiers: usize,
    pub recency_counts: [usize; 5],
    pub frequency_counts: [usize; 5],
    pub monetary_counts: [usize; 5],
}
#[derive(Debug, Clone)]
pub struct RfmRow {
    pub observation: usize,
    pub recency_score: usize,
    pub frequency_score: usize,
    pub monetary_score: usize,
    pub total: usize,
}
#[derive(Debug, Clone)]
pub struct RfmResult {
    pub summary: RfmSummary,
    pub rows: Vec<RfmRow>,
}
