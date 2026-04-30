//! Wald 检验：线性假设 H0: Rβ = r
//!
//! 统一处理单/多约束，F = W/q ~ F(q, df_residual)。

use ndarray::{Array1, Array2};
use serde::Serialize;
use statrs::distribution::{ContinuousCDF, FisherSnedecor};

use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};

use super::common::Alternative;

/// Wald 假设检验结果
#[derive(Debug, Clone, Serialize)]
pub struct WaldTestResult {
    pub constraint_desc: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

/// Wald 检验：H0: Rβ = r
///
/// - R: (q × k) 约束矩阵
/// - r_vec: (q) 约束向量
/// - betas: (k) 参数估计
/// - cov_beta: (k × k) 参数协方差矩阵
pub fn wald_test(
    betas: &Array1<f64>,
    cov_beta: &Array2<f64>,
    r: &Array2<f64>,
    r_vec: &Array1<f64>,
    df_residual: usize,
    alternative: Alternative,
    constraint_desc: impl Into<String>,
) -> Result<WaldTestResult, String> {
    use faer::{Side, linalg::solvers::Solve};

    let q = r.nrows();
    let k = r.ncols();

    if q == 0 {
        return Err("约束个数不能为 0".to_string());
    }
    if betas.len() != k {
        return Err(format!("betas 长度 {} 与 R 列数 {} 不一致", betas.len(), k));
    }
    if r_vec.len() != q {
        return Err(format!("r_vec 长度 {} 与 R 行数 {} 不一致", r_vec.len(), q));
    }

    let r_faer = r.view().into_faer();
    let cov_faer = cov_beta.view().into_faer();

    // contrast = R @ betas - r_vec（用 ndarray 计算避免 faer 减法 API）
    let contrast = r.dot(betas) - r_vec;
    let contrast_faer = contrast.view().into_faer_col().to_owned();

    // r_cov_r = R @ cov_beta @ R'
    let r_cov = r_faer * cov_faer;
    let r_cov_r = r_cov.as_ref() * r_faer.transpose();

    // 解 r_cov_r @ x = contrast，则 W = contrast' @ x
    let x = r_cov_r
        .llt(Side::Lower)
        .map_err(|_| "R Σ R' 矩阵奇异，约束可能冗余".to_string())?
        .solve(contrast_faer.as_ref());

    let x_nd = x.as_ref().into_ndarray().to_owned();
    let w: f64 = contrast.iter().zip(x_nd.iter()).map(|(c, xi)| c * xi).sum();

    let f_stat = w / q as f64;
    let dist = FisherSnedecor::new(q as f64, df_residual as f64)
        .map_err(|e| format!("F 分布参数错误: {}", e))?;
    let mut p_value = 1.0 - dist.cdf(f_stat);

    // 单侧：q=1 时按方向调整
    if q == 1 {
        let c = contrast[0];
        match alternative {
            Alternative::Greater => {
                if c > 0.0 {
                    p_value = p_value / 2.0;
                } else {
                    p_value = 1.0 - p_value / 2.0;
                }
            }
            Alternative::Less => {
                if c < 0.0 {
                    p_value = p_value / 2.0;
                } else {
                    p_value = 1.0 - p_value / 2.0;
                }
            }
            Alternative::TwoSided => {}
        }
    }

    let r_beta_minus_r = if q == 1 { contrast[0] } else { 0.0 };

    let alt_str = match alternative {
        Alternative::TwoSided => "two_sided",
        Alternative::Greater => "greater",
        Alternative::Less => "less",
    };

    Ok(WaldTestResult {
        constraint_desc: constraint_desc.into(),
        alternative: alt_str.to_string(),
        r_beta_minus_r,
        stat: f_stat,
        df1: q,
        df2: df_residual,
        p_value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{arr1, arr2};

    #[test]
    fn test_wald_single_constraint() {
        // 简单 OLS: y = 1 + 2*x, 假设 x 系数 = 0
        let betas = arr1(&[1.0, 2.0]);
        let cov_beta = arr2(&[[0.1, 0.0], [0.0, 0.05]]);
        let r = arr2(&[[0.0, 1.0]]); // 约束: beta_1 = 0
        let r_vec = arr1(&[0.0]);
        let df_residual = 10;

        let result = wald_test(
            &betas,
            &cov_beta,
            &r,
            &r_vec,
            df_residual,
            Alternative::TwoSided,
            "x = 0",
        )
        .unwrap();

        assert_eq!(result.df1, 1);
        assert_eq!(result.df2, 10);
        assert!((result.r_beta_minus_r - 2.0).abs() < 1e-10);
        assert!(result.stat > 0.0);
        assert!(result.p_value > 0.0 && result.p_value <= 1.0);
    }
}
