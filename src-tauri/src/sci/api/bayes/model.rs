use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BayesModelSpec {
    pub dataset: DatasetRef,
    pub response: ResponseSpec,
    pub predictor: Expression,
    pub data_variables: BTreeMap<String, String>,
    pub likelihood: LikelihoodSpec,
    pub parameters: Vec<ParameterSpec>,
    pub sampler: InferenceConfig,
    pub display_formula: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetRef {
    pub source_type: DatasetSourceType,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSourceType {
    Table,
    Query,
    ResultSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSpec {
    pub expression: Expression,
    pub data_variables: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Expression {
    Number {
        value: f64,
    },
    DataVariable {
        name: String,
    },
    Column {
        name: String,
    },
    Parameter {
        name: String,
    },
    Unary {
        op: UnaryOp,
        arg: Box<Expression>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Call {
        function: MathFunction,
        args: Vec<Expression>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOp {
    Neg,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MathFunction {
    Exp,
    Ln,
    Sqrt,
    Abs,
    Sin,
    Cos,
    Min,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LikelihoodSpec {
    Normal {
        mean: PredictorSource,
        sigma: ParameterRef,
    },
    BernoulliLogit {
        logit: PredictorSource,
    },
    PoissonLog {
        #[serde(rename = "logRate")]
        log_rate: PredictorSource,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PredictorSource {
    pub source: PredictorSourceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PredictorSourceKind {
    Predictor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParameterRef {
    pub parameter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParameterSpec {
    pub name: String,
    pub constraint: ParameterConstraint,
    pub prior: PriorSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParameterConstraint {
    Real,
    Positive,
    Unit,
    Bounded {
        lower: f64,
        upper: f64,
        #[serde(rename = "includeLower")]
        include_lower: bool,
        #[serde(rename = "includeUpper")]
        include_upper: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "distribution", content = "args", rename_all = "snake_case")]
pub enum PriorSpec {
    Normal([f64; 2]),
    LogNormal([f64; 2]),
    Uniform([f64; 2]),
    Beta([f64; 2]),
    Gamma([f64; 2]),
    Exponential([f64; 1]),
    StudentT([f64; 3]),
    Cauchy([f64; 2]),
    HalfNormal([f64; 1]),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InferenceConfig {
    pub algorithm: SamplerAlgorithm,
    pub chains: usize,
    pub samples: usize,
    pub warmup: usize,
    pub seed: Option<u64>,
    pub target_accept: Option<f64>,
    pub max_tree_depth: Option<usize>,
    pub save_samples: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SamplerAlgorithm {
    Nuts,
}

impl BayesModelSpec {
    pub fn parameter_names(&self) -> BTreeSet<&str> {
        self.parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect()
    }
}
