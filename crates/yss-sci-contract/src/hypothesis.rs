//! Neutral hypothesis requests, alternatives, computed results and errors.
use crate::{SciError, execution::ScientificComputationError};
use serde::Serialize;
use std::collections::BTreeMap;
/// 备择假设类型（t 检验、Wald 检验共用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alternative {
    TwoSided,
    Greater,
    Less,
}

/// Inputs shared by the independent sample mean tests.
pub enum ClassicalHypothesisTest {
    OneSample {
        values: Vec<f64>,
        null_mean: f64,
        alternative: Alternative,
    },
    Independent {
        first: Vec<f64>,
        second: Vec<f64>,
        equal_variance: bool,
        alternative: Alternative,
    },
    Paired {
        before: Vec<f64>,
        after: Vec<f64>,
        alternative: Alternative,
    },
    Summary {
        design: SummaryTDesign,
        first_count: usize,
        first_mean: f64,
        first_sd: f64,
        second_count: Option<usize>,
        second_mean: Option<f64>,
        second_sd: Option<f64>,
        null_difference: f64,
        equal_variance: bool,
        alternative: Alternative,
    },
    OneSampleZ {
        values: Vec<f64>,
        null_mean: f64,
        population_sd: f64,
        alternative: Alternative,
    },
    OneProportionZ {
        successes: usize,
        trials: usize,
        null_probability: f64,
        alternative: Alternative,
    },
    ExactBinomial {
        successes: usize,
        trials: usize,
        null_probability: f64,
        alternative: Alternative,
    },
    TwoProportions {
        first_successes: usize,
        first_trials: usize,
        second_successes: usize,
        second_trials: usize,
        null_difference: f64,
        alternative: Alternative,
    },
    PoissonRate {
        counts: Vec<f64>,
        null_rate_per_observation: f64,
        alternative: Alternative,
    },
    Equivalence {
        values: Vec<f64>,
        lower_bound: f64,
        upper_bound: f64,
    },
}

#[derive(Clone, Copy)]
pub enum SummaryTDesign {
    OneSample,
    Independent,
    Paired,
}

/// Common result fields for catalogued hypothesis tests.
#[derive(Debug, Clone, Serialize)]
pub struct ClassicalTestResult {
    pub method: String,
    pub null_hypothesis: String,
    pub alternative: String,
    pub statistic_name: String,
    pub statistic: f64,
    pub degrees_of_freedom: Vec<f64>,
    pub p_value: f64,
    pub estimate: Option<f64>,
    pub standard_error: Option<f64>,
    pub sample_sizes: Vec<usize>,
    pub details: BTreeMap<String, f64>,
}

/// Categorical observations and tabulations for exact and asymptotic count tests.
pub enum CategoricalHypothesisTest {
    Independence {
        row: Vec<Box<str>>,
        column: Vec<Box<str>>,
    },
    GoodnessOfFit {
        observed: Vec<f64>,
        expected: Vec<f64>,
    },
    PearsonTable {
        observed: Vec<f64>,
        rows: usize,
        columns: usize,
    },
    FisherExact {
        row: Vec<Box<str>>,
        column: Vec<Box<str>>,
    },
    McNemar {
        before: Vec<f64>,
        after: Vec<f64>,
    },
    Cmh {
        exposed: Vec<f64>,
        outcome: Vec<f64>,
        strata: Vec<Box<str>>,
    },
    MultipleProportions {
        successes_and_trials: Vec<f64>,
    },
}

pub enum RankHypothesisTest {
    WilcoxonOneSample {
        values: Vec<f64>,
        null_median: f64,
        alternative: Alternative,
    },
    WilcoxonPaired {
        before: Vec<f64>,
        after: Vec<f64>,
        alternative: Alternative,
    },
    Friedman {
        conditions: Vec<Vec<f64>>,
    },
    Runs {
        values: Vec<f64>,
    },
    CochranQ {
        conditions: Vec<Vec<f64>>,
    },
    MoodMedian {
        groups: Vec<Vec<f64>>,
    },
    MannKendall {
        values: Vec<f64>,
        alternative: Alternative,
    },
    MannWhitney {
        first: Vec<f64>,
        second: Vec<f64>,
        alternative: Alternative,
    },
    KruskalWallis {
        groups: Vec<Vec<f64>>,
    },
}

pub enum VarianceHomogeneityTest {
    Levene { groups: Vec<Vec<f64>> },
    BrownForsythe { groups: Vec<Vec<f64>> },
    Bartlett { groups: Vec<Vec<f64>> },
}

#[derive(Debug, Clone, Serialize)]
pub struct TTestResult {
    pub constraint_desc: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df: usize,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaldTestResult {
    pub constraint_desc: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

pub struct HypothesisTestInput {
    pub betas: Vec<f64>,
    pub cov_beta: Vec<Vec<f64>>,
    pub df_residual: usize,
    pub param_names: Vec<String>,
    pub hypothesis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HypothesisTestOutput {
    pub test_type: String,
    pub h0_form: String,
    pub h1_form: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum HypothesisError {
    #[error("hypothesis input is invalid: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Scientific(#[from] SciError),
    #[error(transparent)]
    Execution(#[from] ScientificComputationError),
}

impl From<String> for HypothesisError {
    fn from(detail: String) -> Self {
        Self::InvalidInput(detail)
    }
}

impl From<&str> for HypothesisError {
    fn from(detail: &str) -> Self {
        Self::InvalidInput(detail.to_owned())
    }
}
