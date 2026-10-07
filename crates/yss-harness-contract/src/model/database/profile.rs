//! Statistics exposed to models, without storage metadata or runtime identities.
use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseProfileMetricSelection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completeness: Option<DatabaseCompleteness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<Vec<DatabaseColumnStatistics>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distributions: Option<Vec<DatabaseColumnDistribution>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCompleteness {
    pub row_count: usize,
    pub column_count: usize,
    pub total_nulls: usize,
    pub null_ratio: f64,
    pub columns_with_nulls: usize,
    pub rows_with_nulls: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DatabaseColumnStatistics {
    Numeric {
        column: String,
        count: usize,
        null_count: usize,
        min: Option<f64>,
        max: Option<f64>,
        mean: Option<f64>,
        median: Option<f64>,
        std: Option<f64>,
        variance: Option<f64>,
    },
    Categorical {
        column: String,
        count: usize,
        null_count: usize,
        empty_count: usize,
        valid_ratio: f64,
        unique: usize,
        mode: Option<String>,
        mode_count: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DatabaseColumnDistribution {
    Numeric {
        column: String,
        bins: Vec<DatabaseDistributionCount>,
    },
    Categorical {
        column: String,
        categories: Vec<DatabaseDistributionCount>,
        other_count: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseDistributionCount {
    pub label: String,
    pub count: usize,
}
