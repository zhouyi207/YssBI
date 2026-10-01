//! Options and structured results for regression estimators and model-building workflows.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IterationOptions {
    pub max_iterations: usize,
    pub tolerance: f64,
}
impl Default for IterationOptions {
    fn default() -> Self {
        Self {
            max_iterations: 500,
            tolerance: 1e-7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlmFamily {
    Gaussian,
    Binomial,
    Poisson,
    Gamma,
    InverseGaussian,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlmLink {
    Identity,
    Log,
    Logit,
    Probit,
    Cloglog,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlmOptions {
    pub constant: bool,
    pub family: GlmFamily,
    pub link: GlmLink,
    pub fractional: bool,
    pub iteration: IterationOptions,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LikelihoodMethod {
    NegativeBinomial,
    ZeroInflatedPoisson,
    ZeroInflatedNegativeBinomial,
    Tobit,
    Beta,
    MultinomialLogit,
    OrdinalLogit,
    ConditionalLogit,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LikelihoodOptions {
    pub method: LikelihoodMethod,
    pub constant: bool,
    /// Tobit censoring at a lower boundary, or at both boundaries.
    pub lower: f64,
    pub upper: Option<f64>,
    pub iteration: IterationOptions,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustLoss {
    Huber,
    Tukey,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobustOptions {
    pub constant: bool,
    pub loss: RobustLoss,
    pub tuning: f64,
    pub iteration: IterationOptions,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Penalty {
    Ridge,
    Lasso,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenalizedOptions {
    pub constant: bool,
    pub standardize: bool,
    pub penalty: Penalty,
    pub lambda: f64,
    pub iteration: IterationOptions,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveFamily {
    Polynomial,
    Logarithmic,
    Inverse,
    Exponential,
    Power,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonlinearFamily {
    Exponential,
    Logistic,
    MichaelisMenten,
    Gompertz,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionDirection {
    Forward,
    Backward,
    Both,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionCriterion {
    Aic,
    Bic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionCoefficient {
    pub term: String,
    pub estimate: f64,
    pub standard_error: Option<f64>,
    pub statistic: Option<f64>,
    pub p_value: Option<f64>,
    pub confidence_interval: Option<[f64; 2]>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelStatistics {
    pub rss: Option<f64>,
    pub rmse: Option<f64>,
    pub r_squared: Option<f64>,
    pub adjusted_r_squared: Option<f64>,
    pub df_residual: Option<usize>,
    pub log_likelihood: Option<f64>,
    pub aic: Option<f64>,
    pub bic: Option<f64>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegressionDetails {
    Linear,
    Robust {
        loss: RobustLoss,
        tuning: f64,
        scale: f64,
        covariance_method: String,
    },
    Penalized {
        penalty: Penalty,
        lambda: f64,
        standardized: bool,
        objective: f64,
        effective_df: f64,
    },
    Pls {
        components: usize,
        standardized: bool,
        x_weights: Vec<Vec<f64>>,
        x_loadings: Vec<Vec<f64>>,
        y_loadings: Vec<f64>,
    },
    Glm {
        family: GlmFamily,
        link: GlmLink,
        dispersion: f64,
        deviance: f64,
        covariance_method: String,
    },
    Likelihood {
        method: LikelihoodMethod,
        covariance_method: String,
    },
    Firth {
        penalized_log_likelihood: f64,
        covariance_method: String,
    },
    Conditional {
        total_groups: usize,
        informative_groups: usize,
        dropped_observations: usize,
        covariance_method: String,
    },
    Curve {
        family: CurveFamily,
        degree: Option<usize>,
        response_scale: String,
    },
    Nonlinear {
        formula: String,
        initial_values: Vec<f64>,
        covariance_method: String,
    },
    Deming {
        variance_ratio: f64,
        covariance_method: String,
    },
    Quantile {
        quantile: f64,
        check_loss: f64,
        covariance_method: String,
    },
    Threshold {
        threshold: f64,
        regime_counts: [usize; 2],
        trimming: f64,
        evaluated_candidates: usize,
    },
    RestrictedCubicSpline {
        knots: Vec<f64>,
        basis_names: Vec<String>,
        basis: Vec<Vec<f64>>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegressionModelResult<L = usize> {
    pub method: String,
    pub observations: usize,
    pub constant: bool,
    pub coefficients: Vec<RegressionCoefficient>,
    pub covariance: Option<Vec<Vec<f64>>>,
    /// Fitted responses on the observation scale; for Tobit these are latent means.
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    /// Only categorical models fill these fields. Category 0 is the multinomial reference.
    pub categories: Vec<L>,
    pub fitted_categories: Vec<L>,
    pub probabilities: Vec<Vec<f64>>,
    pub statistics: ModelStatistics,
    pub iterations: usize,
    pub converged: bool,
    pub details: RegressionDetails,
}
impl<L> RegressionModelResult<L> {
    pub fn map_categories<M>(self, mut map: impl FnMut(L) -> M) -> RegressionModelResult<M> {
        RegressionModelResult {
            method: self.method,
            observations: self.observations,
            constant: self.constant,
            coefficients: self.coefficients,
            covariance: self.covariance,
            fitted: self.fitted,
            residuals: self.residuals,
            categories: self.categories.into_iter().map(&mut map).collect(),
            fitted_categories: self.fitted_categories.into_iter().map(map).collect(),
            probabilities: self.probabilities,
            statistics: self.statistics,
            iterations: self.iterations,
            converged: self.converged,
            details: self.details,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegressionStage<L = usize> {
    pub label: L,
    /// One-based predictor positions in the original input order.
    pub predictors: Vec<usize>,
    pub observation_indices: Vec<usize>,
    pub delta_r_squared: Option<f64>,
    pub change_f: Option<f64>,
    pub change_df: Option<usize>,
    pub change_p_value: Option<f64>,
    pub model: RegressionModelResult,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SelectionStep {
    pub action: String,
    pub predictor: Option<usize>,
    pub predictors: Vec<usize>,
    pub criterion_value: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegressionWorkflowResult<L = usize> {
    pub method: String,
    pub stages: Vec<RegressionStage<L>>,
    pub selection_history: Vec<SelectionStep>,
    pub selected_predictors: Vec<usize>,
}
impl<L> RegressionWorkflowResult<L> {
    pub fn map_labels<M>(self, mut map: impl FnMut(L) -> M) -> RegressionWorkflowResult<M> {
        RegressionWorkflowResult {
            method: self.method,
            stages: self
                .stages
                .into_iter()
                .map(|s| RegressionStage {
                    label: map(s.label),
                    predictors: s.predictors,
                    observation_indices: s.observation_indices,
                    delta_r_squared: s.delta_r_squared,
                    change_f: s.change_f,
                    change_df: s.change_df,
                    change_p_value: s.change_p_value,
                    model: s.model,
                })
                .collect(),
            selection_history: self.selection_history,
            selected_predictors: self.selected_predictors,
        }
    }
}
