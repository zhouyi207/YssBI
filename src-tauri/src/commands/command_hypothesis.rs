//! 假设检验 Tauri 命令（薄包装）

use crate::application::hypothesis::{
    HypothesisTestInput, HypothesisTestOutput, run_hypothesis_test,
};
use crate::error::CommandError;
use serde::{Deserialize, Serialize};

/// 假设检验请求
#[derive(Debug, Deserialize)]
pub struct HypothesisTestRequest {
    /// OLS 参数估计 (k)
    pub betas: Vec<f64>,
    /// 参数协方差矩阵 (k×k)，行优先
    pub cov_beta: Vec<Vec<f64>>,
    /// 残差自由度
    pub df_residual: usize,
    /// 参数名，与 OLS exog 列序一致
    pub param_names: Vec<String>,
    /// 自然语言约束，如 "x1 = 0" 或 "x1 > x2"
    pub hypothesis: String,
}

/// 假设检验结果（统一格式，便于前端）
#[derive(Debug, Serialize)]
pub struct HypothesisTestResponse {
    /// 检验类型："t" | "wald"
    pub test_type: String,
    /// 原假设 H0 的线性形式（恒为 Rβ = r）
    pub h0_form: String,
    /// 备择假设 H1 的线性形式（= / ≠ / < / ≤ / > / ≥）
    pub h1_form: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    /// t 统计量或 F 统计量
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

impl From<HypothesisTestRequest> for HypothesisTestInput {
    fn from(req: HypothesisTestRequest) -> Self {
        Self {
            betas: req.betas,
            cov_beta: req.cov_beta,
            df_residual: req.df_residual,
            param_names: req.param_names,
            hypothesis: req.hypothesis,
        }
    }
}

impl From<HypothesisTestOutput> for HypothesisTestResponse {
    fn from(out: HypothesisTestOutput) -> Self {
        Self {
            test_type: out.test_type,
            h0_form: out.h0_form,
            h1_form: out.h1_form,
            alternative: out.alternative,
            r_beta_minus_r: out.r_beta_minus_r,
            stat: out.stat,
            df1: out.df1,
            df2: out.df2,
            p_value: out.p_value,
        }
    }
}

/// Tauri 命令：假设检验
#[tauri::command]
pub fn hypothesis_test(req: HypothesisTestRequest) -> Result<HypothesisTestResponse, CommandError> {
    run_hypothesis_test(req.into())
        .map(Into::into)
        .map_err(CommandError::internal)
}
