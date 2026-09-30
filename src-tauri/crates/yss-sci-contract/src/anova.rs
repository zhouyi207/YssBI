//! Neutral designs and reports for univariate, multivariate and repeated ANOVA.
use serde::{Deserialize, Serialize};

pub const MAX_ANOVA_FACTORS: usize = 8;
pub const MAX_ANOVA_LEVELS: usize = 32;
pub const MAX_ANOVA_COLUMNS: usize = 256;
pub const MAX_ANOVA_COVARIATES: usize = 32;
pub const MAX_MANOVA_RESPONSES: usize = 16;
pub const MAX_REPEATED_FACTORS: usize = 4;
pub const MAX_REPEATED_CELLS: usize = 256;

/// Observed levels are contiguous codes; tabular labels remain adapter-owned.
#[derive(Debug, Clone)]
pub struct Factor {
    pub values: Vec<usize>,
    pub levels: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactorialModel {
    MainEffects,
    FullFactorial,
}

/// Checked dense-design width, including the intercept and parallel covariates.
pub fn design_columns(levels: &[usize], covariates: usize, model: FactorialModel) -> Option<usize> {
    if levels.is_empty()
        || levels.len() > MAX_ANOVA_FACTORS
        || covariates > MAX_ANOVA_COVARIATES
        || levels.iter().any(|&n| !(2..=MAX_ANOVA_LEVELS).contains(&n))
    {
        return None;
    }
    let columns = match model {
        FactorialModel::MainEffects => levels
            .iter()
            .try_fold(1usize, |n, &level| n.checked_add(level - 1))?,
        FactorialModel::FullFactorial => levels
            .iter()
            .try_fold(1usize, |n, &level| n.checked_mul(level))?,
    }
    .checked_add(covariates)?;
    (columns <= MAX_ANOVA_COLUMNS).then_some(columns)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SumsOfSquares {
    #[serde(rename = "type_i")]
    TypeI,
    #[serde(rename = "type_ii")]
    TypeII,
    #[serde(rename = "type_iii")]
    TypeIII,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AnovaOptions {
    pub model: FactorialModel,
    pub sums_of_squares: SumsOfSquares,
}

impl Default for AnovaOptions {
    fn default() -> Self {
        Self {
            model: FactorialModel::FullFactorial,
            sums_of_squares: SumsOfSquares::TypeIII,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorLevels<L = usize> {
    pub name: String,
    pub levels: Vec<L>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnovaTerm {
    pub term: String,
    pub sum_squares: f64,
    pub df: usize,
    pub mean_square: f64,
    pub f_statistic: f64,
    pub p_value: f64,
    pub partial_eta_squared: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnovaError {
    pub sum_squares: f64,
    pub df: usize,
    pub mean_square: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnovaResult<L = usize> {
    pub method: String,
    pub observations: usize,
    pub factors: Vec<FactorLevels<L>>,
    pub covariate_means: Vec<f64>,
    pub options: AnovaOptions,
    pub table: Vec<AnovaTerm>,
    pub error: AnovaError,
    pub total_sum_squares: f64,
    pub total_df: usize,
    pub r_squared: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultivariateTest {
    pub test: String,
    pub statistic: f64,
    /// A statistic can exist without a valid small-sample F approximation.
    pub f_statistic: Option<f64>,
    pub df_numerator: Option<f64>,
    pub df_denominator: Option<f64>,
    pub p_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManovaTerm {
    pub term: String,
    pub hypothesis_df: usize,
    /// Row-major response-by-response hypothesis SSCP matrix.
    pub hypothesis_sscp: Vec<Vec<f64>>,
    pub tests: Vec<MultivariateTest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManovaResult<L = usize> {
    pub method: String,
    pub observations: usize,
    pub responses: usize,
    pub factors: Vec<FactorLevels<L>>,
    pub options: AnovaOptions,
    pub error_df: usize,
    /// Row-major response-by-response residual SSCP matrix.
    pub error_sscp: Vec<Vec<f64>>,
    pub table: Vec<ManovaTerm>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SphericityCorrection {
    None,
    GreenhouseGeisser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeatedTerm {
    pub term: String,
    pub sum_squares: f64,
    pub error_sum_squares: f64,
    pub df: usize,
    pub error_df: usize,
    pub mean_square: f64,
    pub error_mean_square: f64,
    pub f_statistic: f64,
    pub p_value_uncorrected: f64,
    pub epsilon_greenhouse_geisser: f64,
    pub df_numerator: f64,
    pub df_denominator: f64,
    pub p_value: f64,
    pub partial_eta_squared: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeatedMeasuresResult<L = usize> {
    pub method: String,
    pub observations: usize,
    pub subjects: usize,
    pub cells_per_subject: usize,
    pub factors: Vec<FactorLevels<L>>,
    pub correction: SphericityCorrection,
    pub table: Vec<RepeatedTerm>,
}

fn map_levels<L, T>(
    factors: Vec<FactorLevels<L>>,
    mut map: impl FnMut(usize, L) -> T,
) -> Vec<FactorLevels<T>> {
    factors
        .into_iter()
        .enumerate()
        .map(|(i, factor)| FactorLevels {
            name: factor.name,
            levels: factor
                .levels
                .into_iter()
                .map(|level| map(i, level))
                .collect(),
        })
        .collect()
}

impl<L> AnovaResult<L> {
    pub fn map_levels<T>(self, map: impl FnMut(usize, L) -> T) -> AnovaResult<T> {
        AnovaResult {
            method: self.method,
            observations: self.observations,
            factors: map_levels(self.factors, map),
            covariate_means: self.covariate_means,
            options: self.options,
            table: self.table,
            error: self.error,
            total_sum_squares: self.total_sum_squares,
            total_df: self.total_df,
            r_squared: self.r_squared,
        }
    }
}
impl<L> ManovaResult<L> {
    pub fn map_levels<T>(self, map: impl FnMut(usize, L) -> T) -> ManovaResult<T> {
        ManovaResult {
            method: self.method,
            observations: self.observations,
            responses: self.responses,
            factors: map_levels(self.factors, map),
            options: self.options,
            error_df: self.error_df,
            error_sscp: self.error_sscp,
            table: self.table,
        }
    }
}
impl<L> RepeatedMeasuresResult<L> {
    pub fn map_levels<T>(self, map: impl FnMut(usize, L) -> T) -> RepeatedMeasuresResult<T> {
        RepeatedMeasuresResult {
            method: self.method,
            observations: self.observations,
            subjects: self.subjects,
            cells_per_subject: self.cells_per_subject,
            factors: map_levels(self.factors, map),
            correction: self.correction,
            table: self.table,
        }
    }
}
