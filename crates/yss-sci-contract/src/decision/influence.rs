use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InfluenceNormalization {
    MaxSum,
    None,
}
#[derive(Debug, Clone, Serialize)]
pub struct DematelSummary {
    pub criteria: usize,
    pub normalization: InfluenceNormalization,
    pub attenuation: f64,
    pub spectral_radius: f64,
}
#[derive(Debug, Clone)]
pub struct InfluenceRow {
    pub criterion: usize,
    pub outgoing: f64,
    pub incoming: f64,
    pub prominence: f64,
    pub net_cause: f64,
    pub weight: Option<f64>,
}
#[derive(Debug, Clone)]
pub struct InfluenceCell {
    pub source: usize,
    pub target: usize,
    pub direct: f64,
    pub total: f64,
}
#[derive(Debug, Clone)]
pub struct DematelResult {
    pub summary: DematelSummary,
    pub rows: Vec<InfluenceRow>,
    pub matrix: Vec<InfluenceCell>,
}
#[derive(Debug, Clone, Serialize)]
pub struct IsmSummary {
    pub criteria: usize,
    /// Level 1 contains the final outcomes (sink components).
    pub levels: Vec<Vec<usize>>,
    pub strongly_connected_components: Vec<Vec<usize>>,
}
#[derive(Debug, Clone)]
pub struct IsmRow {
    pub criterion: usize,
    pub level: usize,
    pub driving_power: usize,
    pub dependence: usize,
}
#[derive(Debug, Clone)]
pub struct ReachabilityCell {
    pub source: usize,
    pub target: usize,
    pub reachable: bool,
}
#[derive(Debug, Clone)]
pub struct IsmResult {
    pub summary: IsmSummary,
    pub rows: Vec<IsmRow>,
    pub matrix: Vec<ReachabilityCell>,
}
