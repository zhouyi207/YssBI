//! Neutral multivariate options, summaries and row-major coordinate outputs.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdinationOutput<R> {
    pub report: R,
    /// Observations/categories by retained axes, in original row order.
    pub coordinates: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PcaOptions {
    pub components: usize,
    pub standardize: bool,
}
impl Default for PcaOptions {
    fn default() -> Self {
        Self {
            components: 2,
            standardize: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcaReport {
    pub method: String,
    pub observations: usize,
    pub variables: usize,
    pub components: usize,
    pub standardized: bool,
    pub rank: usize,
    pub means: Vec<f64>,
    pub scales: Vec<f64>,
    /// Variables by retained components; scores = centered/scaled data * weights.
    pub weights: Vec<Vec<f64>>,
    pub eigenvalues: Vec<f64>,
    pub explained_variance_ratio: Vec<f64>,
    pub cumulative_variance_ratio: Vec<f64>,
    pub retained_variance_ratio: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactorRotation {
    None,
    Varimax,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FactorOptions {
    pub factors: usize,
    pub rotation: FactorRotation,
    pub max_iterations: usize,
    pub tolerance: f64,
}
impl Default for FactorOptions {
    fn default() -> Self {
        Self {
            factors: 1,
            rotation: FactorRotation::Varimax,
            max_iterations: 500,
            tolerance: 1e-6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorReport {
    pub method: String,
    pub observations: usize,
    pub variables: usize,
    pub factors: usize,
    pub rotation: FactorRotation,
    pub iterations: usize,
    pub rotation_iterations: usize,
    pub means: Vec<f64>,
    pub scales: Vec<f64>,
    pub loadings: Vec<Vec<f64>>,
    pub communalities: Vec<f64>,
    pub uniquenesses: Vec<f64>,
    pub variance_proportions: Vec<f64>,
    pub kmo: Option<f64>,
    pub bartlett_chi_square: f64,
    pub bartlett_df: usize,
    pub bartlett_p_value: f64,
    pub score_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalTest {
    /// Tests this and all remaining canonical roots, numbered from one.
    pub first_axis: usize,
    pub wilks_lambda: f64,
    pub chi_square: Option<f64>,
    pub df: usize,
    pub p_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalReport {
    pub method: String,
    pub observations: usize,
    pub x_variables: usize,
    pub y_variables: usize,
    pub components: usize,
    pub correlations: Vec<f64>,
    pub x_means: Vec<f64>,
    pub y_means: Vec<f64>,
    pub x_scales: Vec<f64>,
    pub y_scales: Vec<f64>,
    pub x_weights: Vec<Vec<f64>>,
    pub y_weights: Vec<Vec<f64>>,
    pub x_loadings: Vec<Vec<f64>>,
    pub y_loadings: Vec<Vec<f64>>,
    pub tests: Vec<CanonicalTest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalOutput {
    pub report: CanonicalReport,
    pub x_scores: Vec<Vec<f64>>,
    pub y_scores: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrespondenceReport {
    pub method: String,
    pub rows: usize,
    pub columns: usize,
    pub components: usize,
    pub total_weight: f64,
    pub total_inertia: f64,
    pub chi_square: f64,
    pub row_masses: Vec<f64>,
    pub column_masses: Vec<f64>,
    pub eigenvalues: Vec<f64>,
    pub inertia_proportions: Option<Vec<f64>>,
    pub retained_inertia_proportion: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrespondenceOutput {
    pub report: CorrespondenceReport,
    pub row_coordinates: Vec<Vec<f64>>,
    pub column_coordinates: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscriminantMethod {
    Linear,
    Quadratic,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassPriors {
    Empirical,
    Equal,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DiscriminantOptions {
    pub method: DiscriminantMethod,
    pub priors: ClassPriors,
    pub shrinkage: f64,
}
impl Default for DiscriminantOptions {
    fn default() -> Self {
        Self {
            method: DiscriminantMethod::Linear,
            priors: ClassPriors::Empirical,
            shrinkage: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscriminantReport<L = usize> {
    pub method: DiscriminantMethod,
    pub observations: usize,
    pub prediction_observations: usize,
    pub variables: usize,
    pub classes: Vec<L>,
    pub class_counts: Vec<usize>,
    pub class_priors: Vec<f64>,
    pub class_means: Vec<Vec<f64>>,
    pub shrinkage: f64,
    pub training_accuracy: f64,
    /// Actual class by predicted class; training, without cross-validation.
    pub training_confusion: Vec<Vec<usize>>,
}
impl<L> DiscriminantReport<L> {
    pub fn map_classes<T>(self, map: impl FnMut(L) -> T) -> DiscriminantReport<T> {
        DiscriminantReport {
            method: self.method,
            observations: self.observations,
            prediction_observations: self.prediction_observations,
            variables: self.variables,
            classes: self.classes.into_iter().map(map).collect(),
            class_counts: self.class_counts,
            class_priors: self.class_priors,
            class_means: self.class_means,
            shrinkage: self.shrinkage,
            training_accuracy: self.training_accuracy,
            training_confusion: self.training_confusion,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscriminantOutput {
    pub report: DiscriminantReport,
    pub predictions: Vec<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RdaOptions {
    pub components: usize,
    pub standardize: bool,
    pub permutations: usize,
    pub seed: u64,
}
impl Default for RdaOptions {
    fn default() -> Self {
        Self {
            components: 1,
            standardize: false,
            permutations: 199,
            seed: 42,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdaReport {
    pub method: String,
    pub observations: usize,
    pub response_variables: usize,
    pub constraints: usize,
    pub components: usize,
    pub standardized: bool,
    pub means: Vec<f64>,
    pub scales: Vec<f64>,
    pub response_weights: Vec<Vec<f64>>,
    pub total_inertia: f64,
    pub constrained_inertia: f64,
    pub residual_inertia: f64,
    pub constrained_eigenvalues: Vec<f64>,
    pub residual_eigenvalues: Vec<f64>,
    pub r_squared: f64,
    pub adjusted_r_squared: f64,
    pub f_statistic: Option<f64>,
    pub df_numerator: usize,
    pub df_denominator: usize,
    pub permutation_p_value: Option<f64>,
    pub permutations: usize,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MdsInput {
    Observations,
    DissimilarityMatrix,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MdsOptions {
    pub components: usize,
    pub input: MdsInput,
    pub standardize: bool,
}
impl Default for MdsOptions {
    fn default() -> Self {
        Self {
            components: 2,
            input: MdsInput::Observations,
            standardize: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdsReport {
    pub method: String,
    pub observations: usize,
    pub components: usize,
    pub input: MdsInput,
    pub standardized: bool,
    pub positive_rank: usize,
    pub negative_eigenvalues: usize,
    pub selected_eigenvalues: Vec<f64>,
    pub positive_inertia: f64,
    pub negative_inertia: f64,
    pub goodness_of_fit_positive: f64,
    pub goodness_of_fit_absolute: f64,
    pub stress: f64,
}
