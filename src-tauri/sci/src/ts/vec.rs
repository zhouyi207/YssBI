//! VEC (Vector Error-Correction) 协整模型
//!
//! 按 Stata vec 命令的 Johansen (1995) 方法实现。
//! 支持 trend(none), trend(constant), trend(trend)。

use super::vec_vecrank_cv::{max_eigen_critical_row, trace_critical_row};
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{linalg::solvers::Solve, Mat, Side};
use faer::linalg::solvers::Eigen;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use statrs::distribution::ContinuousCDF;

/// 趋势设定：与 Stata trend() 对应
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VecTrendSpec {
    /// trend(none): 无常数无趋势
    None,
    /// trend(constant): 无约束常数（默认）
    Constant,
    /// trend(trend): 协整方程含线性趋势，水平数据含二次趋势
    Trend,
}

/// VEC 配置
#[derive(Debug, Clone)]
pub struct VECConfig {
    pub trend_spec: VecTrendSpec,
    pub lags: usize,
    pub rank: usize,
    /// veclmar 最大滞后阶数，默认 2
    pub mlag: usize,
}

/// VEC 估计结果（Stata vec 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECResult {
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub rank: usize,
    pub lags: usize,
    pub trend_spec: String,
    /// 协整向量 β：变量行 + 可选 const 行，(K+1)×r 当 has_const 否则 K×r
    pub beta: Vec<Vec<f64>>,
    /// 短 run 系数：每方程 [Γ1, Γ2, ..., const/trend]
    pub coefficients: Vec<Vec<f64>>,
    pub std_errs: Vec<Vec<f64>>,
    pub z_values: Vec<Vec<f64>>,
    pub p_values: Vec<Vec<f64>>,
    pub ci_lower: Vec<Vec<f64>>,
    pub ci_upper: Vec<Vec<f64>>,
    pub coef_labels: Vec<Vec<String>>,
    pub equations: Vec<VECEquationStats>,
    /// 协整方程统计（Stata Cointegrating equations 表）
    pub cointegrating_equations: Vec<VECCointegratingEquationStats>,
    /// beta 表 Stata 风格：每元素 [std_err, z, p_value, ci_lower, ci_upper]，归一化/常数用 None
    pub beta_std_err: Vec<Vec<Option<f64>>>,
    pub beta_z_value: Vec<Vec<Option<f64>>>,
    pub beta_p_value: Vec<Vec<Option<f64>>>,
    pub beta_ci_lower: Vec<Vec<Option<f64>>>,
    pub beta_ci_upper: Vec<Vec<Option<f64>>>,
    /// veclmar: LM 残差自相关检验（Stata veclmar 命令）
    pub veclmar: Vec<VecLmarRow>,
    /// vecstable: 特征值平稳性检验（Stata vecstable 命令）
    pub vecstable: Vec<VecStableRow>,
}

/// Stata `vecrank` 风格输出（Johansen trace / max eigenvalue）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecRankRow {
    pub rank: usize,
    pub log_likelihood: f64,
    pub eigenvalue: Option<f64>,
    pub trace_statistic: Option<f64>,
    /// 右尾检验在 10% / 5% / 1% 显著性下的临界值（与 Stata/R 10pct·5pct·1pct 列一致）
    pub trace_crit_10pct: Option<f64>,
    pub trace_crit_5pct: Option<f64>,
    pub trace_crit_1pct: Option<f64>,
    pub max_eigenvalue_statistic: Option<f64>,
    pub max_eigen_crit_10pct: Option<f64>,
    pub max_eigen_crit_5pct: Option<f64>,
    pub max_eigen_crit_1pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecRankResult {
    pub kind: String,
    pub title: String,
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub n_lags: usize,
    pub trend_spec: String,
    pub show_max_eigen: bool,
    pub selected_rank_trace_95: usize,
    pub selected_rank_trace_99: usize,
    pub selected_rank_max_95: usize,
    pub selected_rank_max_99: usize,
    pub rows: Vec<VecRankRow>,
    pub note: String,
}

/// veclmar 单行：lag 阶的 LM 检验
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecLmarRow {
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// vecstable 单行：特征值及其模
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecStableRow {
    pub re: f64,
    pub im: f64,
    pub modulus: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECCointegratingEquationStats {
    pub eq_name: String,
    pub parms: usize,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECEquationStats {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

/// 对 y (T×K) 做一阶差分，首行 NaN
fn diff_y(y: &Array2<f64>) -> Array2<f64> {
    let (n, k) = (y.nrows(), y.ncols());
    let mut dy = Array2::zeros((n, k));
    for j in 0..k {
        dy[[0, j]] = f64::NAN;
        for i in 1..n {
            dy[[i, j]] = y[[i, j]] - y[[i - 1, j]];
        }
    }
    dy
}

/// Johansen MLE 第一阶段：Z、S 矩阵与降序特征值（与 Stata [TS] vec / vecrank 公式一致）
pub(crate) struct JohansenStage1 {
    pub n: usize,
    pub m1: usize,
    pub m2: usize,
    pub has_const: bool,
    pub has_trend: bool,
    pub z0: Array2<f64>,
    pub z1: Array2<f64>,
    pub z2: Array2<f64>,
    pub s00: Array2<f64>,
    pub s01: Array2<f64>,
    pub s10: Array2<f64>,
    pub s11: Array2<f64>,
    /// (列索引, λ)，λ 降序
    pub eval_pairs: Vec<(usize, f64)>,
    /// 特征向量矩阵实部（m1×m1），与 faer 复特征向量 U 的 .re 一致
    pub u_eigen_real: Array2<f64>,
}

pub(crate) fn johansen_stage1(
    y: &Array2<f64>,
    p: usize,
    trend_spec: VecTrendSpec,
    sindicators: Option<&Array2<f64>>,
) -> Result<JohansenStage1, String> {
    let (n_full, k) = (y.nrows(), y.ncols());
    let dy = diff_y(y);
    let n = n_full - p;
    if n <= 0 {
        return Err("VEC: not enough observations after lag adjustment".to_string());
    }

    let m_si = sindicators.map(|s| s.ncols()).unwrap_or(0);
    if let Some(si) = sindicators {
        if si.nrows() != n_full {
            return Err("VEC: sindicators rows must match y".to_string());
        }
    }

    let (m1, has_const, has_trend): (usize, bool, bool) = match trend_spec {
        VecTrendSpec::None => (k, false, false),
        VecTrendSpec::Constant => (k, true, false),
        VecTrendSpec::Trend => (k, true, true),
    };

    let n_lag_dy = k * (p - 1);
    let m2 = n_lag_dy + if has_const { 1 } else { 0 } + if has_trend { 1 } else { 0 } + m_si;

    if n <= m2 {
        return Err(format!(
            "VEC: need n > m2 ({}), got n={}",
            m2, n
        ));
    }

    let mut z0 = Array2::zeros((n, k));
    let mut z1 = Array2::zeros((n, m1));
    let mut z2 = Array2::zeros((n, m2));

    for i in 0..n {
        let t = p + i;
        for j in 0..k {
            z0[[i, j]] = dy[[t, j]];
            z1[[i, j]] = y[[t - 1, j]];
        }
        let mut col_z2 = 0;
        for lag in 1..p {
            for j in 0..k {
                z2[[i, col_z2]] = dy[[t - lag, j]];
                col_z2 += 1;
            }
        }
        if has_const {
            z2[[i, col_z2]] = 1.0;
            col_z2 += 1;
        }
        if has_trend {
            z2[[i, col_z2]] = t as f64;
            col_z2 += 1;
        }
        if let Some(si) = sindicators {
            for j in 0..m_si {
                z2[[i, col_z2]] = si[[t, j]];
                col_z2 += 1;
            }
        }
    }

    let t_inv = 1.0 / (n as f64);
    let m02 = (z0.t().dot(&z2)) * t_inv;
    let m12 = (z1.t().dot(&z2)) * t_inv;
    let m22 = (z2.t().dot(&z2)) * t_inv;

    let m22_faer = m22.view().into_faer().to_owned();
    let m22_inv = m22_faer
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "VEC: M22 not positive definite (collinearity in Z2)".to_string())?
        .solve(Mat::identity(m22.nrows(), m22.ncols()));

    let m02_m22i = m02.view().into_faer().to_owned() * m22_inv.as_ref();
    let m12_m22i = m12.view().into_faer().to_owned() * m22_inv.as_ref();

    let mut r0 = z0.clone();
    let mut r1 = z1.clone();
    for i in 0..n {
        for j in 0..k {
            let mut s = 0.0;
            for c in 0..m2 {
                s += m02_m22i.as_ref()[(j, c)] * z2[[i, c]];
            }
            r0[[i, j]] -= s;
        }
        for j in 0..m1 {
            let mut s = 0.0;
            for c in 0..m2 {
                s += m12_m22i.as_ref()[(j, c)] * z2[[i, c]];
            }
            r1[[i, j]] -= s;
        }
    }

    let s00 = (r0.t().dot(&r0)) * t_inv;
    let s01 = (r0.t().dot(&r1)) * t_inv;
    let s10 = (r1.t().dot(&r0)) * t_inv;
    let s11 = (r1.t().dot(&r1)) * t_inv;

    let s00_faer = s00.view().into_faer().to_owned();
    let s00_inv = s00_faer
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "VEC: S00 not positive definite".to_string())?
        .solve(Mat::identity(s00.nrows(), s00.ncols()));

    let s11_faer = s11.view().into_faer().to_owned();
    let s11_inv = s11_faer
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "VEC: S11 not positive definite".to_string())?
        .solve(Mat::identity(s11.nrows(), s11.ncols()));

    let s10_s00i_s01 = s10.view().into_faer().to_owned() * s00_inv.as_ref() * s01.view().into_faer();
    let e_mat = s11_inv.as_ref() * s10_s00i_s01.as_ref();

    let evd = faer::linalg::solvers::Eigen::new_from_real(e_mat.as_ref())
        .map_err(|_| "VEC: eigenvalue decomposition failed".to_string())?;

    let s_diag = evd.S().column_vector();
    let u_c = evd.U();
    let u_nr = u_c.nrows();
    let u_nc = u_c.ncols();
    let mut u_eigen_real = Array2::zeros((u_nr, u_nc));
    for i in 0..u_nr {
        for j in 0..u_nc {
            u_eigen_real[[i, j]] = u_c[(i, j)].re;
        }
    }

    let mut eval_pairs: Vec<(usize, f64)> = (0..m1)
        .map(|i| {
            let ev = s_diag.get(i);
            (i, ev.re)
        })
        .collect();
    eval_pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    Ok(JohansenStage1 {
        n,
        m1,
        m2,
        has_const,
        has_trend,
        z0,
        z1,
        z2,
        s00,
        s01,
        s10,
        s11,
        eval_pairs,
        u_eigen_real,
    })
}

/// VEC 估计：Johansen 方法
pub fn vec_estimate(
    y: &Array2<f64>,
    config: &VECConfig,
    var_names: Option<Vec<String>>,
    sindicators: Option<&Array2<f64>>,
) -> Result<VECResult, String> {
    let (_, k) = (y.nrows(), y.ncols());
    let p = config.lags;
    let r = config.rank;

    if p < 1 {
        return Err("VEC: lags must be >= 1".to_string());
    }
    if r >= k {
        return Err(format!(
            "VEC: rank({}) must be < number of variables ({})",
            r, k
        ));
    }

    let var_names = var_names.unwrap_or_else(|| (0..k).map(|i| format!("y{}", i)).collect());

    let s1 = johansen_stage1(y, p, config.trend_spec, sindicators)?;
    let n = s1.n;
    let m1 = s1.m1;
    let m2 = s1.m2;
    let has_const = s1.has_const;
    let has_trend = s1.has_trend;
    let z0 = s1.z0;
    let z1 = s1.z1;
    let z2 = s1.z2;
    let s00 = s1.s00;
    let s01 = s1.s01;
    let s10 = s1.s10;
    let s11 = s1.s11;
    let evals = s1.eval_pairs;
    let u_eigen = s1.u_eigen_real;
    let m_si = sindicators.map(|s| s.ncols()).unwrap_or(0);

    let n_lag_dy = k * (p - 1);
    let s11_faer = s11.view().into_faer().to_owned();

    let mut beta_tilde = Mat::zeros(m1, r);
    for (col, &(idx, _)) in evals.iter().take(r).enumerate() {
        for row in 0..m1 {
            beta_tilde.as_mut()[(row, col)] = u_eigen[[row, idx]];
        }
    }

    // Johansen 归一化: 前 r×r 块为 I_r
    let beta_1 = beta_tilde.as_ref().submatrix(0, 0, r, r);
    let beta_1_inv = beta_1.partial_piv_lu().solve(Mat::identity(r, r));

    let beta_norm = beta_tilde.as_ref() * beta_1_inv.as_ref();

    let beta_s11_beta = (beta_norm.as_ref().transpose() * s11_faer.as_ref() * beta_norm.as_ref()).to_owned();
    let beta_s11_beta_inv = beta_s11_beta
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "VEC: beta' S11 beta not positive definite".to_string())?
        .solve(Mat::identity(r, r));

    let s01_beta = s01.view().into_faer().to_owned() * beta_norm.as_ref();
    let alpha_mat = s01_beta.as_ref() * beta_s11_beta_inv.as_ref();

    let alpha_beta_s10 = alpha_mat.as_ref() * beta_norm.as_ref().transpose() * s10.view().into_faer();
    let omega = (s00.view().into_faer().to_owned() - alpha_beta_s10.as_ref()).to_owned();

    let omega_nd = omega.as_ref().into_ndarray().to_owned();
    let mut omega_chol = omega_nd.clone();
    cholesky_lower_in_place(&mut omega_chol)
        .map_err(|_| "VEC: Omega not positive definite".to_string())?;
    let det_omega: f64 = (0..k).map(|i| omega_chol[[i, i]]).product();
    let det_sigma_ml = (det_omega * det_omega).abs().max(1e-300);

    let ln_det_omega = 2.0 * (0..k).map(|i| omega_chol[[i, i]].ln()).sum::<f64>();
    let ll = -0.5
        * (n as f64)
        * (k as f64 * (2.0 * std::f64::consts::PI).ln() + k as f64 + ln_det_omega);

    let n_parms = (k * r + m1 * r + k * m2) as f64 - (r * r) as f64;
    let d = (n_parms / k as f64).floor() as usize;
    let aic = -2.0 * ll / (n as f64) + 2.0 * n_parms / (n as f64);
    let hqic = -2.0 * ll / (n as f64) + 2.0 * n_parms * (n as f64).ln().ln() / (n as f64);
    let sbic = -2.0 * ll / (n as f64) + n_parms * (n as f64).ln() / (n as f64);

    let mut beta_y_data = Vec::with_capacity(k * r);
    for i in 0..k {
        for j in 0..r {
            beta_y_data.push(beta_norm[(i, j)]);
        }
    }
    let beta_y = Array2::from_shape_vec((k, r), beta_y_data)
        .map_err(|_| "VEC: beta_y shape".to_string())?;

    let alpha_nd = alpha_mat.as_ref().into_ndarray().to_owned();

    let mut mu_rho: Vec<f64> = Vec::new();
    if has_const || has_trend {
        // Use Z1 (y_{t-1}) not r1 for backing out μ,ρ per Stata eq.(11)
        let ce_nd = z1.dot(&beta_y);
        let mut x_ce = Array2::zeros((n, r + m2));
        for i in 0..n {
            for j in 0..r {
                x_ce[[i, j]] = ce_nd[[i, j]];
            }
            for j in 0..m2 {
                x_ce[[i, r + j]] = z2[[i, j]];
            }
        }
        let x_faer = x_ce.view().into_faer().to_owned();
        let xt = x_faer.as_ref().transpose();
        let xtx = xt.as_ref() * x_faer.as_ref();
        let xtx_inv = xtx
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "VEC: X'X not positive definite in short-run regression".to_string())?
            .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
        let z0_faer = z0.view().into_faer().to_owned();
        let xty = xt.as_ref() * z0_faer.as_ref();
        let gamma_full = xtx_inv.as_ref() * xty.as_ref();
        let gamma_nd = gamma_full.as_ref().into_ndarray().to_owned();

        let const_row = r + n_lag_dy;
        let trend_row = r + n_lag_dy + 1;

        let v_hat = if has_const {
            Array1::from_iter((0..k).map(|i| gamma_nd[[const_row, i]]))
        } else {
            Array1::zeros(k)
        };
        let delta_hat = if has_trend {
            Array1::from_iter((0..k).map(|i| gamma_nd[[trend_row, i]]))
        } else {
            Array1::zeros(k)
        };

        let alpha_aa = alpha_nd.t().dot(&alpha_nd);
        let alpha_aa_faer = alpha_aa.view().into_faer().to_owned();
        let alpha_aa_inv = alpha_aa_faer
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "VEC: alpha'alpha singular".to_string())?
            .solve(Mat::identity(r, r));
        if has_const && config.trend_spec == VecTrendSpec::Constant {
            let alpha_t_v = alpha_nd.t().dot(&v_hat);
            let v_col = Mat::from_fn(r, 1, |i, _| alpha_t_v[i]);
            let mu_col = alpha_aa_inv.as_ref() * v_col.as_ref();
            mu_rho.extend((0..r).map(|i| mu_col[(i, 0)]));
        }
        if has_trend {
            let alpha_t_d = alpha_nd.t().dot(&delta_hat);
            let d_col = Mat::from_fn(r, 1, |i, _| alpha_t_d[i]);
            let rho_col = alpha_aa_inv.as_ref() * d_col.as_ref();
            mu_rho.extend((0..r).map(|i| rho_col[(i, 0)]));
        }
    }

    // Stata uses demeaned CE: Ê_{t-1} = β'y_{t-1} + μ + ρ(t-1) (not r1 = residual of Z1 on Z2)
    let n_ce = r;
    let mut ce_vals = Array2::zeros((n, n_ce));
    for i in 0..n {
        let t_lag = (p + i - 1) as f64; // t-1 for Ê_{t-1}
        for j in 0..r {
            ce_vals[[i, j]] = (0..k).map(|kk| z1[[i, kk]] * beta_y[[kk, j]]).sum::<f64>()
                + mu_rho.get(j).copied().unwrap_or(0.0)
                + if has_trend {
                    mu_rho.get(r + j).copied().unwrap_or(0.0) * t_lag
                } else {
                    0.0
                };
        }
    }

    let n_z_sr = r + m2;
    let mut x_sr = Array2::zeros((n, n_z_sr));
    for i in 0..n {
        for j in 0..r {
            x_sr[[i, j]] = ce_vals[[i, j]];
        }
        for j in 0..m2 {
            x_sr[[i, r + j]] = z2[[i, j]];
        }
    }

    let x_faer = x_sr.view().into_faer().to_owned();
    let xt = x_faer.as_ref().transpose();
    let xtx = xt.as_ref() * x_faer.as_ref();
    let xtx_inv = xtx
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "VEC: short-run X'X not positive definite".to_string())?
        .solve(Mat::identity(xtx.nrows(), xtx.ncols()));

    let mut coefficients = Vec::with_capacity(k);
    let mut std_errs = Vec::with_capacity(k);
    let mut residuals = Vec::with_capacity(k);
    let mut ss_res = Vec::with_capacity(k);
    let mut ss_tot = Vec::with_capacity(k);
    let mut cov_beta = Vec::with_capacity(k);
    let mut coef_labels = Vec::with_capacity(k);

    let sigma2_divisor = (n as f64 - d as f64).max(1.0);

    for eq in 0..k {
        let y_col = z0.column(eq).into_owned();
        let y_faer = y_col.view().into_faer_col().to_owned();
        let xty = xt.as_ref() * y_faer.as_ref();
        let beta_sr = xtx_inv.as_ref() * xty.as_ref();
        let y_hat = x_faer.as_ref() * beta_sr.as_ref();
        let u = y_faer.as_ref() - y_hat.as_ref();
        let u_nd = u.as_ref().into_ndarray().to_owned();

        let ss_r: f64 = u_nd.iter().map(|x| x * x).sum();
        let y_mean = y_col.mean().unwrap_or(0.0);
        let ss_t: f64 = y_col.iter().map(|x| (x - y_mean).powi(2)).sum();
        // R² = 1 - RSS/TSS per Stata reg3; TSS = Σ(Δy-Δȳ)² (standard formula)
        let ss_t_final: f64 = ss_t;

        let sigma2_eq = ss_r / sigma2_divisor;
        let xtx_inv_nd = xtx_inv.as_ref().into_ndarray().to_owned();
        let cov_eq = sigma2_eq * &xtx_inv_nd;
        cov_beta.push(cov_eq.clone());
        let se: Array1<f64> = cov_eq.diag().mapv(f64::sqrt);

        let mut labels = Vec::with_capacity(n_z_sr);
        for j in 0..r {
            labels.push(format!("_ce{}_L1.", j + 1));
        }
        for lag in 1..p {
            for j in 0..k {
                let name = var_names.get(j).cloned().unwrap_or_else(|| format!("y{}", j));
                // Stata notation: LD = lag of difference, L2D = lag 2 of difference
                let lag_prefix = if lag == 1 {
                    "LD.".to_string()
                } else {
                    format!("L{}D.", lag)
                };
                labels.push(format!("{}{}", lag_prefix, name));
            }
        }
        if has_const {
            labels.push("const".to_string());
        }
        if has_trend {
            labels.push("trend".to_string());
        }
        for j in 0..m_si {
            labels.push(format!("sind{}", j));
        }

        let beta_nd = beta_sr.as_ref().into_ndarray().to_owned();
        coefficients.push(beta_nd.to_vec());
        std_errs.push(se.to_vec());
        residuals.push(u_nd.to_vec());
        ss_res.push(ss_r);
        ss_tot.push(ss_t_final);
        coef_labels.push(labels);
    }

    let df_r = (n - d).max(1); // for RMSE/sigma: Stata VCE uses (T-d)
    let _df_r_eq = (n - n_z_sr).max(1); // Stata e(df r#) = n - params per equation

    let mut equations = Vec::with_capacity(k);
    let mut z_values = Vec::with_capacity(k);
    let mut p_values = Vec::with_capacity(k);
    let mut ci_lower = Vec::with_capacity(k);
    let mut ci_upper = Vec::with_capacity(k);

    for eq in 0..k {
        let rmse = (ss_res[eq] / df_r as f64).sqrt();
        // R² = 1 - SS_res/SS_tot (standard formula, Stata vec uses different definition)
        let r_sq = if ss_tot[eq] > 1e-300 {
            (1.0 - ss_res[eq] / ss_tot[eq]).max(0.0)
        } else {
            0.0
        };
        // chi2: Wald statistic W = β̂' V^{-1} β̂ (independent of R²)
        let chi2 = {
            let beta = Array1::from_vec(coefficients[eq].clone());
            let v = &cov_beta[eq];
            let v_faer = v.view().into_faer().to_owned();
            let beta_faer = beta.view().into_faer_col().to_owned();
            match v_faer.as_ref().llt(Side::Lower) {
                Ok(llt) => {
                    let x = llt.solve(beta_faer.as_ref());
                    let x_nd = x.as_ref().into_ndarray().to_owned();
                    beta.dot(&x_nd)
                }
                Err(_) => n as f64 * r_sq / (1.0 - r_sq.max(1e-10)),
            }
        };
        let p_chi2 = 1.0 - statrs::distribution::ChiSquared::new(n_z_sr as f64).unwrap().cdf(chi2);

        let mut zv = Vec::with_capacity(n_z_sr);
        let mut pv = Vec::with_capacity(n_z_sr);
        let mut cl = Vec::with_capacity(n_z_sr);
        let mut cu = Vec::with_capacity(n_z_sr);
        for j in 0..n_z_sr {
            let z_val = if std_errs[eq][j].abs() > 1e-300 {
                coefficients[eq][j] / std_errs[eq][j]
            } else {
                0.0
            };
            let p_val = 2.0 * (1.0 - statrs::distribution::Normal::new(0.0, 1.0).unwrap().cdf(z_val.abs()));
            let ci_half = 1.96 * std_errs[eq][j];
            zv.push(z_val);
            pv.push(p_val);
            cl.push(coefficients[eq][j] - ci_half);
            cu.push(coefficients[eq][j] + ci_half);
        }
        z_values.push(zv);
        p_values.push(pv);
        ci_lower.push(cl);
        ci_upper.push(cu);

        let eq_name = format!(
            "D_{}",
            var_names.get(eq).cloned().unwrap_or_else(|| format!("y{}", eq))
        );
        equations.push(VECEquationStats {
            eq_name,
            parms: n_z_sr,
            rmse,
            r_sq,
            chi2,
            p_chi2,
        });
    }

    let mut beta_out: Vec<Vec<f64>> = (0..k).map(|i| (0..r).map(|j| beta_y[[i, j]]).collect()).collect();
    if has_const && mu_rho.len() >= r {
        beta_out.push((0..r).map(|j| mu_rho[j]).collect());
    }

    // Cointegrating equations chi2 (Stata formula: Wald on free params in beta)
    let cointegrating_equations = compute_cointegrating_equations_chi2(
        &beta_y,
        &alpha_nd,
        &omega_nd,
        &s11,
        n,
        d,
        r,
        k,
    );

    // beta 表 Stata 风格：Std. err., z, P>|z|, [95% conf. interval]（Stata 公式 15）
    let (mut beta_std_err, mut beta_z_value, mut beta_p_value, mut beta_ci_lower, mut beta_ci_upper) =
        compute_beta_ce_stats(&beta_y, &alpha_nd, &omega_nd, &s11, n, d, r, k);
    if has_const {
        beta_std_err.push(vec![None; r]);
        beta_z_value.push(vec![None; r]);
        beta_p_value.push(vec![None; r]);
        beta_ci_lower.push(vec![None; r]);
        beta_ci_upper.push(vec![None; r]);
    }

    let trend_spec_str = match config.trend_spec {
        VecTrendSpec::None => "none",
        VecTrendSpec::Constant => "constant",
        VecTrendSpec::Trend => "trend",
    };

    // veclmar: LM 残差自相关检验（Stata veclmar，与 varlmar 相同思路）
    // LM_s = (T - d - 0.5) * ln(|Σ̂| / |Σ̃_s|)，df = K²，使用 ML 估计 Σ
    let u_mat = Array2::from_shape_fn((n, k), |(i, j)| residuals[j][i]);
    let sigma_ml = (u_mat.t().dot(&u_mat) / n as f64).to_owned();
    let mut det_sigma_ml_copy = sigma_ml.clone();
    let det_sigma_hat = match cholesky_lower_in_place(&mut det_sigma_ml_copy) {
        Ok(()) => {
            let det_g: f64 = (0..k).map(|i| det_sigma_ml_copy[[i, i]]).product();
            (det_g * det_g).abs().max(1e-300)
        }
        Err(()) => 1e-300,
    };

    let mlag = config.mlag;
    let mut veclmar = Vec::new();
    let n_z_aug_base = n_z_sr + k;
    for s in 1..=mlag {
        if s >= n {
            break;
        }
        let mut x_aug = Array2::zeros((n, n_z_aug_base));
        x_aug.slice_mut(ndarray::s![.., ..n_z_sr]).assign(&x_sr.view());
        for j in 0..k {
            for i in 0..n {
                x_aug[[i, n_z_sr + j]] = if i >= s { residuals[j][i - s] } else { 0.0 };
            }
        }

        let x_aug_faer = x_aug.view().into_faer().to_owned();
        let xt_aug = x_aug_faer.as_ref().transpose();
        let xtx_aug = xt_aug.as_ref() * x_aug_faer.as_ref();
        let xtx_aug_inv = match xtx_aug.as_ref().llt(Side::Lower) {
            Ok(llt) => llt.solve(Mat::identity(xtx_aug.nrows(), xtx_aug.ncols())),
            Err(_) => continue,
        };

        let mut u_aug = Array2::zeros((n, k));
        for eq in 0..k {
            let y_col = z0.column(eq).into_owned();
            let y_faer = y_col.view().into_faer_col().to_owned();
            let xty = xt_aug.as_ref() * y_faer.as_ref();
            let beta_aug = xtx_aug_inv.as_ref() * xty.as_ref();
            let y_hat = x_aug_faer.as_ref() * beta_aug.as_ref();
            let u = y_faer.as_ref() - y_hat.as_ref();
            let u_nd = u.as_ref().into_ndarray().to_owned();
            for i in 0..n {
                u_aug[[i, eq]] = u_nd[i];
            }
        }

        let sigma_tilde = (u_aug.t().dot(&u_aug) / n as f64).to_owned();
        let mut det_tilde = sigma_tilde.clone();
        let det_sigma_tilde = match cholesky_lower_in_place(&mut det_tilde) {
            Ok(()) => {
                let det_g: f64 = (0..k).map(|i| det_tilde[[i, i]]).product();
                (det_g * det_g).abs().max(1e-300)
            }
            Err(()) => continue,
        };

        let lm_stat = (n as f64 - n_z_aug_base as f64 - 0.5) * (det_sigma_hat / det_sigma_tilde).ln();
        let lm_stat = lm_stat.max(0.0);
        let df_lm = k * k;
        let p_lm = 1.0 - statrs::distribution::ChiSquared::new(df_lm as f64).unwrap().cdf(lm_stat);

        veclmar.push(VecLmarRow {
            lag: s,
            chi2: lm_stat,
            df: df_lm,
            p_value: p_lm,
        });
    }

    // vecstable: 特征值平稳性检验（Stata vecstable）
    // VEC 隐含 VAR 水平形式: y_t = A_1 y_{t-1} + ... + A_p y_{t-p}
    // A_1 = I + Π + Γ_1, A_i = Γ_i - Γ_{i-1} (i=2..p-1), A_p = -Γ_{p-1}, Π = αβ'
    let pi = alpha_nd.dot(&beta_y.t());
    let mut gamma_mats: Vec<Array2<f64>> = Vec::with_capacity(p);
    gamma_mats.push(Array2::zeros((k, k))); // Γ_0 = 0
    for i in 1..p {
        let mut g = Array2::zeros((k, k));
        for eq in 0..k {
            for j in 0..k {
                let idx = r + (i - 1) * k + j;
                if idx < coefficients[0].len() {
                    g[[eq, j]] = coefficients[eq][idx];
                }
            }
        }
        gamma_mats.push(g);
    }

    let mut a_mats: Vec<Array2<f64>> = Vec::with_capacity(p + 1);
    a_mats.push(Array2::zeros((k, k)));
    let eye = Array2::eye(k);
    a_mats.push((&eye + &pi + &gamma_mats[1]).to_owned());
    for i in 2..p {
        a_mats.push((&gamma_mats[i] - &gamma_mats[i - 1]).to_owned());
    }
    a_mats.push((-&gamma_mats[p - 1]).to_owned());

    let kp = k * p;
    let mut companion = Mat::zeros(kp, kp);
    for (lag_idx, a) in a_mats.iter().skip(1).enumerate() {
        for i in 0..k {
            for j in 0..k {
                companion.as_mut()[(i, lag_idx * k + j)] = a[[i, j]];
            }
        }
    }
    for block in 0..(p - 1) {
        for i in 0..k {
            companion.as_mut()[(k + block * k + i, block * k + i)] = 1.0;
        }
    }

    let vecstable = match Eigen::new_from_real(companion.as_ref()) {
        Ok(evd) => {
            let s_diag = evd.S().column_vector();
            (0..kp)
                .map(|i| {
                    let ev = s_diag.get(i);
                    let re = ev.re;
                    let im = ev.im;
                    let modulus = (re * re + im * im).sqrt();
                    VecStableRow { re, im, modulus }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    };

    Ok(VECResult {
        var_names,
        num_observation: n,
        log_likelihood: ll,
        aic,
        hqic,
        sbic,
        det_sigma_ml,
        rank: r,
        lags: p,
        trend_spec: trend_spec_str.to_string(),
        beta: beta_out,
        coefficients,
        std_errs,
        z_values,
        p_values,
        ci_lower,
        ci_upper,
        coef_labels,
        equations,
        cointegrating_equations,
        beta_std_err,
        beta_z_value,
        beta_p_value,
        beta_ci_lower,
        beta_ci_upper,
        veclmar,
        vecstable,
    })
}

/// Johansen 协整秩检验（LR_trace、LR_max、LL(r) 与 Stata [TS] vecrank 公式一致；临界值为 Osterwald–Lenum，与 Stata 打印一致）
pub fn vec_vecrank_stats(
    y: &Array2<f64>,
    lags: usize,
    trend_spec: VecTrendSpec,
    sindicators: Option<&Array2<f64>>,
    show_max_eigen: bool,
    var_names: Option<Vec<String>>,
) -> Result<VecRankResult, String> {
    let k = y.ncols();
    if k < 2 {
        return Err("vecrank: need at least 2 variables".to_string());
    }
    if k > 12 {
        return Err("vecrank: Johansen tables only defined for K <= 12".to_string());
    }
    if lags < 1 {
        return Err("vecrank: lags must be >= 1".to_string());
    }

    let s1 = johansen_stage1(y, lags, trend_spec, sindicators)?;
    let n = s1.n;
    let t = n as f64;
    let evals: Vec<f64> = s1.eval_pairs.iter().map(|(_, v)| *v).collect();
    if evals.len() != k {
        return Err("vecrank: internal eigenvalue count mismatch".to_string());
    }

    let det_order = match trend_spec {
        VecTrendSpec::None => -1,
        VecTrendSpec::Constant => 0,
        VecTrendSpec::Trend => 1,
    };

    let mut s00_chol = s1.s00.clone();
    cholesky_lower_in_place(&mut s00_chol).map_err(|_| "vecrank: S00 not positive definite".to_string())?;
    let ln_det_s00: f64 = 2.0 * (0..k).map(|i| s00_chol[[i, i]].ln()).sum::<f64>();
    let k_bracket = k as f64 * ((2.0 * std::f64::consts::PI).ln() + 1.0);

    let log_1m = |lam: f64| -> f64 {
        let lam = lam.clamp(0.0, 1.0 - 1e-15);
        (1.0 - lam).max(1e-300).ln()
    };

    let mut trace = vec![0.0_f64; k];
    for r in 0..k {
        let s: f64 = (r..k).map(|j| log_1m(evals[j])).sum();
        trace[r] = -t * s;
    }

    let mut maxe = vec![0.0_f64; k];
    for r in 0..k {
        maxe[r] = -t * log_1m(evals[r]);
    }

    let mut sel_tr_95 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = trace_critical_row(dim, det_order) {
            if trace[r] < cv[1] {
                sel_tr_95 = r;
                break;
            }
        }
    }
    let mut sel_tr_99 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = trace_critical_row(dim, det_order) {
            if trace[r] < cv[2] {
                sel_tr_99 = r;
                break;
            }
        }
    }

    let mut sel_mx_95 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = max_eigen_critical_row(dim, det_order) {
            if maxe[r] < cv[1] {
                sel_mx_95 = r;
                break;
            }
        }
    }
    let mut sel_mx_99 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = max_eigen_critical_row(dim, det_order) {
            if maxe[r] < cv[2] {
                sel_mx_99 = r;
                break;
            }
        }
    }

    let trend_str = match trend_spec {
        VecTrendSpec::None => "none",
        VecTrendSpec::Constant => "constant",
        VecTrendSpec::Trend => "trend",
    }
    .to_string();

    let names = var_names.unwrap_or_else(|| (0..k).map(|i| format!("y{}", i)).collect());

    let mut rows = Vec::with_capacity(k + 1);
    for rank in 0..=k {
        let sum_r: f64 = (0..rank).map(|j| log_1m(evals[j])).sum();
        let ll_r = -0.5 * t * (k_bracket + ln_det_s00 + sum_r);

        let eigenvalue = if rank >= 1 && rank <= k {
            Some(evals[rank - 1])
        } else {
            None
        };

        let trace_stat = if rank < k {
            Some(trace[rank])
        } else {
            None
        };

        let max_stat = if rank < k {
            Some(maxe[rank])
        } else {
            None
        };

        let (t10, t5, t1) = if rank < k {
            let dim = k - rank;
            trace_critical_row(dim, det_order)
                .map(|cv| (Some(cv[0]), Some(cv[1]), Some(cv[2])))
                .unwrap_or((None, None, None))
        } else {
            (None, None, None)
        };

        let (m10, m5, m1) = if rank < k {
            let dim = k - rank;
            max_eigen_critical_row(dim, det_order)
                .map(|cv| (Some(cv[0]), Some(cv[1]), Some(cv[2])))
                .unwrap_or((None, None, None))
        } else {
            (None, None, None)
        };

        rows.push(VecRankRow {
            rank,
            log_likelihood: ll_r,
            eigenvalue,
            trace_statistic: trace_stat,
            trace_crit_10pct: t10,
            trace_crit_5pct: t5,
            trace_crit_1pct: t1,
            max_eigenvalue_statistic: max_stat,
            max_eigen_crit_10pct: m10,
            max_eigen_crit_5pct: m5,
            max_eigen_crit_1pct: m1,
        });
    }

    Ok(VecRankResult {
        kind: "vecrank".to_string(),
        title: "Johansen tests for cointegration".to_string(),
        var_names: names,
        num_observation: n,
        n_lags: lags,
        trend_spec: trend_str,
        show_max_eigen,
        selected_rank_trace_95: sel_tr_95,
        selected_rank_trace_99: sel_tr_99,
        selected_rank_max_95: sel_mx_95,
        selected_rank_max_99: sel_mx_99,
        rows,
        note: "Trace and max-eigenvalue statistics follow Johansen (1995) and Stata [TS] vecrank. Critical columns are 10% / 5% / 1% significance (right tail). Critical values: Osterwald–Lenum (1992), same digits as Stata vecrank (see johans.ado Case tables); dim=12 uses MacKinnon–Haug–Michelis tail row. If trace/LL differ from Stata but critical values match, check the same sample length (T) and lag order — LR statistics scale with T.".to_string(),
    })
}

/// 协整方程 chi2 (Stata Cointegrating equations 表): Wald 检验自由参数
fn compute_cointegrating_equations_chi2(
    beta: &Array2<f64>,
    alpha: &Array2<f64>,
    omega: &Array2<f64>,
    s11: &Array2<f64>,
    n: usize,
    d: usize,
    r: usize,
    k: usize,
) -> Vec<VECCointegratingEquationStats> {
    let n_free = k.saturating_sub(r);
    if n_free == 0 {
        return (0..r)
            .map(|j| VECCointegratingEquationStats {
                eq_name: format!("_ce{}", j + 1),
                parms: 0,
                chi2: 0.0,
                p_chi2: 1.0,
            })
            .collect();
    }

    let omega_faer = omega.view().into_faer().to_owned();
    let omega_inv = match omega_faer.as_ref().llt(Side::Lower) {
        Ok(llt) => llt.solve(Mat::identity(k, k)),
        Err(_) => return (0..r).map(|j| VECCointegratingEquationStats {
            eq_name: format!("_ce{}", j + 1),
            parms: n_free,
            chi2: 0.0,
            p_chi2: 1.0,
        }).collect(),
    };

    // A = α' Ω^{-1} α (r×r)
    let alpha_t = alpha.t();
    let omega_inv_nd = omega_inv.as_ref().into_ndarray().to_owned();
    let alpha_oa_nd = alpha_t.dot(&omega_inv_nd).dot(alpha);
    let alpha_oa = alpha_oa_nd.view().into_faer().to_owned();
    let a_inv = match alpha_oa.as_ref().llt(Side::Lower) {
        Ok(llt) => llt.solve(Mat::identity(r, r)),
        Err(_) => return (0..r).map(|j| VECCointegratingEquationStats {
            eq_name: format!("_ce{}", j + 1),
            parms: n_free,
            chi2: 0.0,
            p_chi2: 1.0,
        }).collect(),
    };

    let s11_bottom = s11.slice(ndarray::s![r..k, r..k]).to_owned();
    let b_mat = s11_bottom;

    let mut result = Vec::with_capacity(r);
    for j in 0..r {
        let beta_free: Array1<f64> = Array1::from_iter((r..k).map(|i| beta[[i, j]]));
        let a_inv_jj = a_inv.as_ref()[(j, j)].max(1e-300);
        let b_beta = b_mat.dot(&beta_free);
        let chi2 = (n - d) as f64 * (1.0 / a_inv_jj) * beta_free.dot(&b_beta);
        let chi2 = chi2.max(0.0);
        let p_chi2 = 1.0 - statrs::distribution::ChiSquared::new(n_free as f64).unwrap().cdf(chi2);
        result.push(VECCointegratingEquationStats {
            eq_name: format!("_ce{}", j + 1),
            parms: n_free,
            chi2,
            p_chi2,
        });
    }
    result
}

/// beta 表 Stata 风格：Std. err., z, P>|z|, [95% conf. interval]
/// Stata 公式 (15): VCE = (1/(T-d)) (I⊗H_J) {(α'Ω⁻¹α)⊗(H_J'S11 H_J)}⁻¹ (I⊗H_J)'
/// 对 CE j 的自由参数：V = (1/(n-d)) * a_inv_jj * B⁻¹，B = S11[r..k, r..k]
fn compute_beta_ce_stats(
    beta: &Array2<f64>,
    alpha: &Array2<f64>,
    omega: &Array2<f64>,
    s11: &Array2<f64>,
    n: usize,
    d: usize,
    r: usize,
    k: usize,
) -> (
    Vec<Vec<Option<f64>>>,
    Vec<Vec<Option<f64>>>,
    Vec<Vec<Option<f64>>>,
    Vec<Vec<Option<f64>>>,
    Vec<Vec<Option<f64>>>,
) {
    let n_free = k.saturating_sub(r);
    let mut std_err = vec![vec![None; r]; k];
    let mut z_val = vec![vec![None; r]; k];
    let mut p_val = vec![vec![None; r]; k];
    let mut ci_lo = vec![vec![None; r]; k];
    let mut ci_hi = vec![vec![None; r]; k];

    if n_free == 0 {
        return (std_err, z_val, p_val, ci_lo, ci_hi);
    }

    let omega_faer = omega.view().into_faer().to_owned();
    let omega_inv = match omega_faer.as_ref().llt(Side::Lower) {
        Ok(llt) => llt.solve(Mat::identity(k, k)),
        Err(_) => return (std_err, z_val, p_val, ci_lo, ci_hi),
    };

    let alpha_t = alpha.t();
    let omega_inv_nd = omega_inv.as_ref().into_ndarray().to_owned();
    let alpha_oa_nd = alpha_t.dot(&omega_inv_nd).dot(alpha);
    let alpha_oa = alpha_oa_nd.view().into_faer().to_owned();
    let a_inv = match alpha_oa.as_ref().llt(Side::Lower) {
        Ok(llt) => llt.solve(Mat::identity(r, r)),
        Err(_) => return (std_err, z_val, p_val, ci_lo, ci_hi),
    };

    let s11_bottom = s11.slice(ndarray::s![r..k, r..k]).to_owned();
    let s11_bottom_faer = s11_bottom.view().into_faer().to_owned();
    let b_inv = match s11_bottom_faer.as_ref().llt(Side::Lower) {
        Ok(llt) => llt.solve(Mat::identity(n_free, n_free)),
        Err(_) => return (std_err, z_val, p_val, ci_lo, ci_hi),
    };
    let b_inv_nd = b_inv.as_ref().into_ndarray().to_owned();

    let scale = 1.0 / ((n - d) as f64).max(1.0);
    let norm = statrs::distribution::Normal::new(0.0, 1.0).unwrap();

    for j in 0..r {
        let a_inv_jj = a_inv.as_ref()[(j, j)].max(1e-300);
        for (ii, i) in (r..k).enumerate() {
            let coef = beta[[i, j]];
            let var_ii = scale * a_inv_jj * b_inv_nd[[ii, ii]].max(0.0);
            let se = var_ii.sqrt().max(1e-300);
            let z = coef / se;
            let p = 2.0 * (1.0 - norm.cdf(z.abs()));
            let half_width = 1.96 * se;
            std_err[i][j] = Some(se);
            z_val[i][j] = Some(z);
            p_val[i][j] = Some(p);
            ci_lo[i][j] = Some(coef - half_width);
            ci_hi[i][j] = Some(coef + half_width);
        }
    }
    (std_err, z_val, p_val, ci_lo, ci_hi)
}

fn cholesky_lower_in_place(a: &mut Array2<f64>) -> Result<(), ()> {
    let n = a.nrows();
    if a.ncols() != n {
        return Err(());
    }
    for j in 0..n {
        let mut s = 0.0;
        for k in 0..j {
            s += a[[j, k]].powi(2);
        }
        let d = a[[j, j]] - s;
        if d <= 0.0 {
            return Err(());
        }
        let ljj = d.sqrt();
        a[[j, j]] = ljj;
        for i in (j + 1)..n {
            let mut s = 0.0;
            for k in 0..j {
                s += a[[i, k]] * a[[j, k]];
            }
            a[[i, j]] = (a[[i, j]] - s) / ljj;
        }
        for i in 0..j {
            a[[i, j]] = 0.0;
        }
    }
    Ok(())
}
