//! t 检验：单约束 H0: Rβ = r
//!
//! 仅支持 q=1，t = (Rβ - r) / se(Rβ - r) ~ t(df_residual)，支持单侧。

use ndarray::{Array1, Array2};
use serde::Serialize;
use statrs::distribution::{ContinuousCDF, StudentsT};

use super::common::Alternative;

/// t 检验结果
#[derive(Debug, Clone, Serialize)]
pub struct TTestResult {
    pub constraint_desc: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df: usize,
    pub p_value: f64,
}

/// t 检验：H0: Rβ = r（仅 q=1）
///
/// t = (Rβ - r) / se(Rβ - r)，se = sqrt(R Σ R')
pub fn t_test(
    betas: &Array1<f64>,
    cov_beta: &Array2<f64>,
    r: &Array2<f64>,
    r_vec: &Array1<f64>,
    df_residual: usize,
    alternative: Alternative,
    constraint_desc: impl Into<String>,
) -> Result<TTestResult, String> {
    let q = r.nrows();
    let k = r.ncols();

    if q != 1 {
        return Err("t 检验仅支持单约束 (q=1)".to_string());
    }
    if betas.len() != k {
        return Err(format!("betas 长度 {} 与 R 列数 {} 不一致", betas.len(), k));
    }

    let contrast = r.dot(betas) - r_vec;
    let c = contrast[0];

    let r_cov_r = (r.dot(cov_beta)).dot(&r.t());
    let se = r_cov_r[[0, 0]].sqrt();
    if se <= 0.0 {
        return Err("R Σ R' 非正，无法计算标准误".to_string());
    }

    let t_stat = c / se;
    let dist = StudentsT::new(0.0, 1.0, df_residual as f64)
        .map_err(|e| format!("t 分布参数错误: {}", e))?;

    let p_value = match alternative {
        Alternative::TwoSided => 2.0 * (1.0 - dist.cdf(t_stat.abs())),
        Alternative::Greater => 1.0 - dist.cdf(t_stat),
        Alternative::Less => dist.cdf(t_stat),
    };

    let alt_str = match alternative {
        Alternative::TwoSided => "two_sided",
        Alternative::Greater => "greater",
        Alternative::Less => "less",
    };

    Ok(TTestResult {
        constraint_desc: constraint_desc.into(),
        alternative: alt_str.to_string(),
        r_beta_minus_r: c,
        stat: t_stat,
        df: df_residual,
        p_value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{arr1, arr2};

    #[test]
    fn test_t_single_constraint() {
        let betas = arr1(&[1.0, 2.0]);
        let cov_beta = arr2(&[[0.1, 0.0], [0.0, 0.05]]);
        let r = arr2(&[[0.0, 1.0]]);
        let r_vec = arr1(&[0.0]);
        let df_residual = 10;

        let result = t_test(
            &betas,
            &cov_beta,
            &r,
            &r_vec,
            df_residual,
            Alternative::TwoSided,
            "x = 0",
        )
        .unwrap();

        assert_eq!(result.df, 10);
        assert!((result.r_beta_minus_r - 2.0).abs() < 1e-10);
        assert!((result.stat - 2.0 / 0.05_f64.sqrt()).abs() < 1e-6);
        assert!(result.p_value > 0.0 && result.p_value <= 1.0);
    }
}
