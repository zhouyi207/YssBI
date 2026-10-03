use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TurfResult {
    pub observations: usize,
    pub criteria: usize,
    pub combination_size: usize,
    pub selected_criteria: Vec<usize>,
    pub reach_count: usize,
    pub reach_percent: f64,
    pub total_exposures: usize,
    pub exposures_per_reached: Option<f64>,
    pub combinations_evaluated: u64,
    pub equally_optimal_combinations: u64,
    pub exact: bool,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceRangeDefinition {
    Original,
    Narrower,
}
#[derive(Debug, Clone, Serialize)]
pub struct PriceIntersection {
    pub lower: f64,
    pub upper: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct PriceSummary {
    pub observations: usize,
    pub range_definition: PriceRangeDefinition,
    pub curve_convention: &'static str,
    pub marginal_cheapness: Option<PriceIntersection>,
    pub marginal_expensiveness: Option<PriceIntersection>,
    pub indifference: Option<PriceIntersection>,
    pub optimal: Option<PriceIntersection>,
}
#[derive(Debug, Clone)]
pub struct PriceRow {
    pub price: f64,
    pub too_cheap: f64,
    pub cheap: f64,
    pub expensive: f64,
    pub too_expensive: f64,
}
#[derive(Debug, Clone)]
pub struct PriceResult {
    pub summary: PriceSummary,
    pub rows: Vec<PriceRow>,
}
