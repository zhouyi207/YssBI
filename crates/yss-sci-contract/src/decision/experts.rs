use crate::association::ConcordanceResult;
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct DelphiSummary {
    pub experts: usize,
    pub items: usize,
    pub full_score: f64,
    pub concordance: Option<ConcordanceResult>,
    pub concordance_undefined_reason: Option<&'static str>,
}
#[derive(Debug, Clone)]
pub struct DelphiItem {
    pub item: usize,
    pub mean: f64,
    pub standard_deviation: Option<f64>,
    pub coefficient_of_variation: Option<f64>,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub full_score_percent: f64,
}
#[derive(Debug, Clone)]
pub struct DelphiResult {
    pub summary: DelphiSummary,
    pub rows: Vec<DelphiItem>,
}
