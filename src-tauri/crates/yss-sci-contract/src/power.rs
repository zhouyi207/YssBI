//! Prospective scalar power designs; sample size is expressed in each design's own unit.
use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerAlternative {
    TwoSided,
    Greater,
    Less,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeanDesign {
    OneSample,
    Independent,
    Paired,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum PowerModel {
    NormalMean {
        standardized_effect: f64,
    },
    TMean {
        standardized_effect: f64,
        design: MeanDesign,
    },
    Variance {
        variance_ratio: f64,
    },
    Proportion {
        null_proportion: f64,
        proportion: f64,
    },
    ProportionDifference {
        proportion1: f64,
        proportion2: f64,
    },
    Correlation {
        null_correlation: f64,
        correlation: f64,
    },
    Anova {
        groups: usize,
        effect_f: f64,
    },
    LinearRegression {
        predictors: usize,
        effect_f_squared: f64,
    },
    PoissonRate {
        baseline_rate: f64,
        rate_ratio: f64,
        exposure: f64,
    },
    Logistic {
        baseline_probability: f64,
        odds_ratio: f64,
    },
    Survival {
        hazard_ratio: f64,
        event_fraction: f64,
        predictor_variance: f64,
    },
    ClusterRandomized {
        standardized_effect: f64,
        cluster_size: usize,
        intraclass_correlation: f64,
    },
    Noninferiority {
        standardized_difference: f64,
        margin: f64,
        higher_is_better: bool,
    },
    Equivalence {
        standardized_difference: f64,
        margin: f64,
    },
}
#[derive(Debug, Clone, Copy)]
pub enum PowerRequest {
    Power { sample_size: usize },
    SampleSize { target_power: f64 },
}
#[derive(Debug, Clone, Copy)]
pub struct PowerOptions {
    pub alpha: f64,
    pub alternative: PowerAlternative,
    pub request: PowerRequest,
}
#[derive(Debug, Clone, Serialize)]
pub struct PowerResult {
    pub model: PowerModel,
    pub method: String,
    pub alpha: f64,
    pub alternative: PowerAlternative,
    pub sample_size: usize,
    pub sample_unit: String,
    pub total_observations: usize,
    pub power: f64,
    pub type_ii_error: f64,
    pub target_power: Option<f64>,
    pub expected_events: Option<f64>,
}
