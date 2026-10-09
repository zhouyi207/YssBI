//! t 检验：单约束 H0: Rβ = r
//!
//! 仅支持 q=1，t = (Rβ - r) / se(Rβ - r) ~ t(df_residual)，支持单侧。

use statrs::distribution::StudentsT;
use yss_sci_linalg::{Col, Mat};

use yss_sci_contract::hypothesis::{Alternative, TTestResult};

/// t 检验：H0: Rβ = r（仅 q=1）
///
/// t = (Rβ - r) / se(Rβ - r)，se = sqrt(R Σ R')
pub(super) fn t_test(
    betas: &Col<f64>,
    cov_beta: &Mat<f64>,
    r: &Mat<f64>,
    r_vec: &Col<f64>,
    df_residual: usize,
    alternative: Alternative,
    constraint_desc: impl Into<String>,
) -> Result<TTestResult, String> {
    let q = r.nrows();
    let k = r.ncols();

    if q != 1 {
        return Err("t 检验仅支持单约束 (q=1)".to_string());
    }
    if betas.nrows() != k {
        return Err(format!(
            "betas 长度 {} 与 R 列数 {} 不一致",
            betas.nrows(),
            k
        ));
    }

    let contrast = r * betas - r_vec;
    let c = contrast[0];

    let r_cov_r = (r * cov_beta) * r.transpose();
    let variance = r_cov_r[(0, 0)];
    if variance.is_nan() || variance <= 0.0 {
        return Err("R Σ R' 非正，无法计算标准误".to_string());
    }
    let se = variance.sqrt();

    let t_stat = c / se;
    let dist = StudentsT::new(0.0, 1.0, df_residual as f64)
        .map_err(|e| format!("t 分布参数错误: {}", e))?;

    let p_value = crate::distribution::student_t_probability(&dist, t_stat, alternative);

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
    use yss_sci_linalg::{col, mat};

    #[test]
    fn test_t_single_constraint() {
        let betas = col![1.0, 2.0];
        let cov_beta = mat![[0.1, 0.0], [0.0, 0.05]];
        let r = mat![[0.0, 1.0]];
        let r_vec = col![0.0];
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

        for (statistic, df, expected) in [
            (1e308, 1, 2.0 * (1e-308 / std::f64::consts::PI)),
            // The exact df=2 two-sided tail is 1 - t/sqrt(t^2 + 2).
            // Rationalizing gives 2 / (hypot(t, sqrt(2)) * (hypot(t, sqrt(2)) + t)).
            (1e155, 2, 1e-310),
            // Rounding a one-sided tail first would incorrectly erase this value.
            (9e107, 3, f64::from_bits(1)),
            (1e108, 3, 0.0),
            (1e100, 5, 0.0),
            (1e308, 1_000_000, 0.0),
        ] {
            let result = t_test(
                &col![statistic],
                &mat![[1.0]],
                &mat![[1.0]],
                &col![0.0],
                df,
                Alternative::TwoSided,
                "x = 0",
            )
            .unwrap();
            if expected == 0.0 {
                assert_eq!(result.p_value, 0.0);
                continue;
            }
            assert!(
                (result.p_value / expected - 1.0).abs() < 2e-12,
                "df={df}, t={statistic}: {}, expected {expected}",
                result.p_value
            );
        }
    }
}
