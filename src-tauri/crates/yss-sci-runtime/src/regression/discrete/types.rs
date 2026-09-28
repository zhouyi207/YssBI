use crate::regression::types::VariableSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogitConfigure {
    pub constant: bool,
}

impl Default for LogitConfigure {
    fn default() -> Self {
        Self { constant: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogitModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbitConfigure {
    pub constant: bool,
}

impl Default for ProbitConfigure {
    fn default() -> Self {
        Self { constant: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbitModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
}

/// Classification table for binary choice models (Stata estat classification)
/// Rows: Classified + (pred≥cutoff), Classified - (pred<cutoff)
/// Cols: True D (y=1), True ~D (y=0), Total
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationTable {
    /// True positives: classified +, actual D
    pub tp: usize,
    /// False positives: classified +, actual ~D
    pub fp: usize,
    /// False negatives: classified -, actual D
    pub fn_: usize,
    /// True negatives: classified -, actual ~D
    pub tn: usize,
    /// Cutoff used (default 0.5)
    pub cutoff: f64,
    /// Sensitivity Pr(+|D) = TP/(TP+FN)
    pub sensitivity: f64,
    /// Specificity Pr(-|~D) = TN/(TN+FP)
    pub specificity: f64,
    /// Positive predictive value Pr(D|+)
    pub ppv: f64,
    /// Negative predictive value Pr(~D|-)
    pub npv: f64,
    /// False + rate for true ~D Pr(+|~D)
    pub false_pos_rate: f64,
    /// False - rate for true D Pr(-|D)
    pub false_neg_rate: f64,
    /// Percent correctly classified
    pub pct_correct: f64,
}
