//! Correlation and agreement contracts over aligned observations.
use crate::hypothesis::Alternative;
use serde::Serialize;

pub const MAX_EXACT_RANK_OBSERVATIONS: usize = 9;
pub const MAX_BLAND_ALTMAN_POINTS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankInference {
    Auto,
    PermutationExact,
    Asymptotic,
}

#[derive(Debug, Clone, Copy)]
pub struct CorrelationOptions {
    pub alternative: Alternative,
    pub confidence_level: f64,
}
impl Default for CorrelationOptions {
    fn default() -> Self {
        Self {
            alternative: Alternative::TwoSided,
            confidence_level: 0.95,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct RankCorrelationOptions {
    pub alternative: Alternative,
    pub inference: RankInference,
}
impl Default for RankCorrelationOptions {
    fn default() -> Self {
        Self {
            alternative: Alternative::TwoSided,
            inference: RankInference::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfidenceInterval {
    pub lower: f64,
    pub upper: f64,
    pub level: f64,
    pub method: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestInference {
    pub statistic_name: &'static str,
    pub statistic: Option<f64>,
    pub degrees_of_freedom: Vec<f64>,
    pub p_value: Option<f64>,
    pub method: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrelationResult {
    pub method: &'static str,
    pub coefficient: f64,
    pub observations: usize,
    pub control_variables: usize,
    pub alternative: &'static str,
    pub inference: TestInference,
    pub confidence_interval: Option<ConfidenceInterval>,
    pub concordant_pairs: Option<u64>,
    pub discordant_pairs: Option<u64>,
    pub tied_pairs_x: Option<u64>,
    pub tied_pairs_y: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KappaMethod {
    Cohen,
    Fleiss,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KappaWeighting {
    None,
    Linear,
    Quadratic,
}
#[derive(Debug, Clone, Copy)]
pub struct KappaOptions {
    pub method: KappaMethod,
    pub weighting: KappaWeighting,
    pub confidence_level: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KappaResult<Category = usize> {
    pub method: &'static str,
    pub weighting: &'static str,
    pub coefficient: f64,
    pub observations: usize,
    pub raters: usize,
    pub observed_agreement: f64,
    pub expected_agreement: f64,
    pub standard_error: Option<f64>,
    pub confidence_interval: Option<ConfidenceInterval>,
    pub inference: Option<TestInference>,
    pub categories: Vec<Category>,
    pub category_counts_by_rater: Vec<Vec<usize>>,
    pub contingency_table: Option<Vec<Vec<usize>>>,
}
impl KappaResult {
    pub fn map_categories<Category>(
        self,
        label: impl Fn(usize) -> Category,
    ) -> KappaResult<Category> {
        KappaResult {
            method: self.method,
            weighting: self.weighting,
            coefficient: self.coefficient,
            observations: self.observations,
            raters: self.raters,
            observed_agreement: self.observed_agreement,
            expected_agreement: self.expected_agreement,
            standard_error: self.standard_error,
            confidence_interval: self.confidence_interval,
            inference: self.inference,
            categories: self.categories.into_iter().map(label).collect(),
            category_counts_by_rater: self.category_counts_by_rater,
            contingency_table: self.contingency_table,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IccType {
    Icc1,
    Icc2,
    Icc3,
    Icc1k,
    Icc2k,
    Icc3k,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IccResult {
    pub method: &'static str,
    pub model: &'static str,
    pub measurement: &'static str,
    pub definition: &'static str,
    pub coefficient: f64,
    pub observations: usize,
    pub raters: usize,
    pub subject_mean_square: f64,
    pub rater_mean_square: f64,
    pub error_mean_square: f64,
    pub within_mean_square: f64,
    pub inference: TestInference,
    pub confidence_interval: Option<ConfidenceInterval>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlandAltmanPoint {
    pub observation: usize,
    pub mean: f64,
    pub difference: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlandAltmanResult {
    pub method: &'static str,
    pub observations: usize,
    pub bias: f64,
    pub standard_deviation: f64,
    pub coverage: f64,
    pub lower_limit: f64,
    pub upper_limit: f64,
    pub bias_confidence_interval: ConfidenceInterval,
    pub lower_limit_confidence_interval: ConfidenceInterval,
    pub upper_limit_confidence_interval: ConfidenceInterval,
    pub points: Vec<BlandAltmanPoint>,
    pub sampled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConcordanceResult {
    pub method: &'static str,
    pub coefficient: f64,
    pub observations: usize,
    pub raters: usize,
    pub tie_correction: f64,
    pub inference: TestInference,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RiditCategory<Category = usize> {
    pub category: Category,
    pub reference_count: usize,
    pub sample_count: usize,
    pub reference_proportion: f64,
    pub sample_proportion: f64,
    pub ridit: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RiditResult<Category = usize> {
    pub method: &'static str,
    pub reference_observations: usize,
    pub sample_observations: usize,
    pub mean_ridit: f64,
    pub alternative: &'static str,
    pub continuity_correction: bool,
    pub inference: TestInference,
    pub categories: Vec<RiditCategory<Category>>,
}
impl RiditResult {
    pub fn map_categories<Category>(
        self,
        label: impl Fn(usize) -> Category,
    ) -> RiditResult<Category> {
        RiditResult {
            method: self.method,
            reference_observations: self.reference_observations,
            sample_observations: self.sample_observations,
            mean_ridit: self.mean_ridit,
            alternative: self.alternative,
            continuity_correction: self.continuity_correction,
            inference: self.inference,
            categories: self
                .categories
                .into_iter()
                .map(|row| RiditCategory {
                    category: label(row.category),
                    reference_count: row.reference_count,
                    sample_count: row.sample_count,
                    reference_proportion: row.reference_proportion,
                    sample_proportion: row.sample_proportion,
                    ridit: row.ridit,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AgreementNull {
    Uniform { scale_points: usize },
    SpecifiedVariance { variance: f64 },
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RwgItem {
    pub item: usize,
    pub observed_variance: f64,
    pub raw_rwg: f64,
    pub rwg: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RwgResult {
    pub method: &'static str,
    pub observations: usize,
    pub item_count: usize,
    pub null_distribution: &'static str,
    pub expected_variance: f64,
    pub mean_observed_variance: f64,
    pub rwg_j: f64,
    pub variance_truncated: bool,
    pub items: Vec<RwgItem>,
}
