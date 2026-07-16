//! ACF / PACF 计算命令
//!
//! 用于 OLS/WLS/GLS summary 的残差自相关诊断，参考 Stata ac / pac。

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use yss_sci::ts::acf_pacf::{acf, pacf};

#[derive(Debug, Deserialize)]
pub struct AcfPacfRequest {
    /// 残差序列
    pub residuals: Vec<f64>,
    /// 最大滞后阶数
    pub max_lag: usize,
}

#[derive(Debug, Serialize)]
pub struct AcfPacfResponse {
    /// ACF: lag 0..=max_lag，lag 0 恒为 1.0
    pub acf: Vec<f64>,
    /// PACF: lag 1..=max_lag
    pub pacf: Vec<f64>,
    /// 样本量（用于前端计算置信区间 ±1.96/√n）
    pub n: usize,
}

#[tauri::command]
pub fn compute_acf_pacf(req: AcfPacfRequest) -> Result<AcfPacfResponse, AppError> {
    let n = req.residuals.len();
    if n < 4 {
        return Err(AppError::new(
            "insufficient_observations",
            "ACF/PACF: 至少需要 4 个观测值",
        ));
    }
    let max_lag = req.max_lag.min(n / 2 - 1).min(40).max(1);

    let acf_vals = acf(&req.residuals, max_lag);
    let pacf_vals = pacf(&req.residuals, max_lag);

    Ok(AcfPacfResponse {
        acf: acf_vals,
        pacf: pacf_vals,
        n,
    })
}
