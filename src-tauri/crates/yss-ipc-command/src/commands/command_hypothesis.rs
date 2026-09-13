//! 假设检验 Tauri 命令（薄包装）

use crate::error::CommandError;
use crate::schema::statistics::HypothesisTestResponseDto as HypothesisTestResponse;
use serde::Deserialize;
use yss_application::hypothesis::{HypothesisTestInput, run_hypothesis_test};

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

/// Tauri 命令：假设检验
#[tauri::command]
pub fn hypothesis_test(req: HypothesisTestRequest) -> Result<HypothesisTestResponse, CommandError> {
    run_hypothesis_test(req.into())
        .map(Into::into)
        .map_err(CommandError::internal)
}
