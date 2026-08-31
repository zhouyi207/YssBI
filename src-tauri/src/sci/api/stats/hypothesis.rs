//! Linear hypothesis-test application API and Rust backend orchestration.

use ndarray::{Array1, Array2};

use crate::sci::backends::rust;
use crate::sci::engine::SciContext;
use yss_sci_contract::SciError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alternative {
    TwoSided,
    Greater,
    Less,
}

pub struct LinearHypothesisTestInput<'a> {
    pub betas: &'a Array1<f64>,
    pub cov_beta: &'a Array2<f64>,
    pub r: &'a Array2<f64>,
    pub r_vec: &'a Array1<f64>,
    pub df_residual: usize,
    pub alternative: Alternative,
    pub constraint_desc: String,
}

#[derive(Debug, Clone)]
pub struct TTestOutput {
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df: usize,
    pub p_value: f64,
}

#[derive(Debug, Clone)]
pub struct WaldTestOutput {
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

pub fn t_test(
    _context: &SciContext,
    input: LinearHypothesisTestInput<'_>,
) -> Result<TTestOutput, SciError> {
    rust::stats::hypothesis::t_test(input)
}

pub fn wald_test(
    _context: &SciContext,
    input: LinearHypothesisTestInput<'_>,
) -> Result<WaldTestOutput, SciError> {
    rust::stats::hypothesis::wald_test(input)
}
