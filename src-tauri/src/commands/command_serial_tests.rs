//! 序列相关检验命令：BG、Q、DW
//!
//! 参考 Stata: estat bgodfrey, wntestq, estat dwatson

use serde::{Deserialize, Serialize};
use crate::error::AppError;
use yss_sci::ts::serial_correlation::{breusch_godfrey, durbin_watson, ljung_box_q};

#[derive(Debug, Deserialize)]
pub struct SerialTestsRequest {
    /// 残差序列
    pub residuals: Vec<f64>,
    /// 滞后阶数（BG、Q 检验用）
    pub lags: usize,
    /// 回归设计矩阵 X（行优先），BG 检验需要；无则跳过 BG
    #[serde(default)]
    pub exog: Option<Vec<Vec<f64>>>,
    /// BG 检验: true = nomiss0（缺失用0填充，n个观测）；false = 去掉前p个观测
    #[serde(default = "default_bg_nomiss0")]
    pub bg_nomiss0: bool,
}

fn default_bg_nomiss0() -> bool {
    true
}

/// BG / Q 检验结果（需 lag）
#[derive(Debug, Serialize)]
pub struct SerialTestWithLag {
    pub stat: f64,
    pub p_value: f64,
    pub lags: usize,
}

/// DW 检验结果（无需 lag）
#[derive(Debug, Serialize)]
pub struct DurbinWatsonResult {
    pub d: f64,
}

#[derive(Debug, Serialize)]
pub struct SerialTestsResponse {
    /// Breusch-Godfrey LM 检验（estat bgodfrey, lags(p) nomiss0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<SerialTestWithLag>,
    /// Ljung-Box Q 检验（wntestq, lags(p)）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<SerialTestWithLag>,
    /// Durbin-Watson 统计量（estat dwatson）
    pub dw: DurbinWatsonResult,
}

#[tauri::command]
pub fn compute_serial_tests(req: SerialTestsRequest) -> Result<SerialTestsResponse, AppError> {
    let n = req.residuals.len();
    if n < 4 {
        return Err(AppError::new("insufficient_observations", "序列相关检验: 至少需要 4 个观测值"));
    }
    let lags = req.lags.min(n / 2 - 1).min(40).max(1);

    let dw = durbin_watson(&req.residuals);

    let bg = req
        .exog
        .as_ref()
        .and_then(|exog| breusch_godfrey(&req.residuals, exog, lags, req.bg_nomiss0))
        .map(|(stat, p_value)| SerialTestWithLag {
            stat,
            p_value,
            lags,
        });

    let q = ljung_box_q(&req.residuals, lags).map(|(stat, p_value)| SerialTestWithLag {
        stat,
        p_value,
        lags,
    });

    Ok(SerialTestsResponse {
        bg,
        q,
        dw: DurbinWatsonResult { d: dw },
    })
}
