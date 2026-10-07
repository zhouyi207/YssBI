//! Empirical inequality measures and their subgroup decomposition.
use serde::Serialize;

/// Bounds the number of pairwise rows in a Dagum report.

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GiniResult {
    pub gini: f64,
    pub mean: f64,
    pub observations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DagumGroup<Group = usize> {
    pub group: Group,
    pub observations: usize,
    pub mean: f64,
    /// Undefined for an all-zero subgroup, whose contribution is still zero.
    pub gini: Option<f64>,
    pub population_share: f64,
    pub income_share: f64,
    pub within_contribution: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DagumPair<Group = usize> {
    pub group_a: Group,
    pub group_b: Group,
    pub gini: Option<f64>,
    /// Absolute mean difference divided by the cross-group mean absolute difference.
    pub economic_distance: Option<f64>,
    pub between_contribution: f64,
    pub transvariation_contribution: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DagumResult<Group = usize> {
    pub gini: f64,
    pub mean: f64,
    pub observations: usize,
    pub within: f64,
    pub between: f64,
    pub transvariation: f64,
    /// Component shares are undefined when overall inequality is zero.
    pub within_share: Option<f64>,
    pub between_share: Option<f64>,
    pub transvariation_share: Option<f64>,
    pub groups: Vec<DagumGroup<Group>>,
    pub pairs: Vec<DagumPair<Group>>,
}

impl DagumResult {
    /// Restore caller-owned category labels without changing computed statistics.
    pub fn map_groups<Group>(self, label: impl Fn(usize) -> Group) -> DagumResult<Group> {
        DagumResult {
            gini: self.gini,
            mean: self.mean,
            observations: self.observations,
            within: self.within,
            between: self.between,
            transvariation: self.transvariation,
            within_share: self.within_share,
            between_share: self.between_share,
            transvariation_share: self.transvariation_share,
            groups: self
                .groups
                .into_iter()
                .map(|group| DagumGroup {
                    group: label(group.group),
                    observations: group.observations,
                    mean: group.mean,
                    gini: group.gini,
                    population_share: group.population_share,
                    income_share: group.income_share,
                    within_contribution: group.within_contribution,
                })
                .collect(),
            pairs: self
                .pairs
                .into_iter()
                .map(|pair| DagumPair {
                    group_a: label(pair.group_a),
                    group_b: label(pair.group_b),
                    gini: pair.gini,
                    economic_distance: pair.economic_distance,
                    between_contribution: pair.between_contribution,
                    transvariation_contribution: pair.transvariation_contribution,
                })
                .collect(),
        }
    }
}
