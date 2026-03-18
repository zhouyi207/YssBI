//! VAR (Vector Autoregression) 模型与 IRF (Impulse Response Function)
//!
//! 实现与 Stata varbasic 一致：VAR(p) 估计（每方程 OLS）、正交化 IRF、FEVD。
//! 参考 Lutkepohl (2005) New Introduction to Multiple Time Series Analysis.

use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};

/// Cholesky 分解 L 使得 A = L L'，L 为下三角，原地覆盖 A 的下三角部分
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

use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use statrs::distribution::ContinuousCDF;

/// VAR 配置
#[derive(Debug, Clone)]
pub struct VARConfig {
    /// 是否包含常数项
    pub constant: bool,
    /// 滞后阶数列表，如 [1, 2] 表示 L1 和 L2
    pub lags: Vec<usize>,
    /// IRF 步数（预测 horizon），默认 8
    pub step: usize,
    /// 小样本自由度调整：dfk 时用 1/(T-m) 估计 Σ
    pub dfk: bool,
    /// varlmar 最大滞后阶数，默认 2
    pub mlag: usize,
}

impl Default for VARConfig {
    fn default() -> Self {
        Self {
            constant: true,
            lags: vec![1, 2],
            step: 8,
            dfk: false,
            mlag: 2,
        }
    }
}

/// VAR 模型
pub struct VAR {
    /// 内生变量 Y (T × K)，每列一个变量
    pub y: Array2<f64>,
    /// 外生变量 X (T × M)，可选
    pub exog: Option<Array2<f64>>,
    pub config: VARConfig,
    /// 变量名，用于系数标签
    pub var_names: Option<Vec<String>>,
    /// 外生变量名，用于系数标签
    pub exog_names: Option<Vec<String>>,
}

/// 单方程估计结果（Stata 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VAREquationStats {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

/// VAR 估计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARResult {
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub fpe: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub equations: Vec<VAREquationStats>,
    /// 系数：按方程分组，每方程 [L1.var1, L1.var2, ..., Lp.varK, const]
    pub coefficients: Vec<Vec<f64>>,
    pub std_errs: Vec<Vec<f64>>,
    pub z_values: Vec<Vec<f64>>,
    pub p_values: Vec<Vec<f64>>,
    pub ci_lower: Vec<Vec<f64>>,
    pub ci_upper: Vec<Vec<f64>>,
    /// 系数标签：如 ["dln_inv:L1.dln_inv", "dln_inv:L1.dln_inc", ...]
    pub coef_labels: Vec<Vec<String>>,
    /// 残差协方差矩阵 Σ (K×K)
    pub sigma: Vec<Vec<f64>>,
    /// 正交化 IRF: step+1 个 K×K 矩阵，[step][response][impulse]
    pub oirf: Vec<Vec<Vec<f64>>>,
    /// FEVD: step+1 个 K×K 矩阵，[step][response][impulse]
    pub fevd: Vec<Vec<Vec<f64>>>,
    /// varwle: Wald lag-exclusion 检验（Stata varwle 命令）
    pub varwle: Vec<VARWleRow>,
    /// varlmar: LM 残差自相关检验（Stata varlmar 命令）
    pub varlmar: Vec<VARLmarRow>,
    /// varstable: 特征值平稳性检验（Stata varstable 命令）
    pub varstable: Vec<VARStableRow>,
    /// vargranger: 格兰杰因果 Wald 检验（Stata vargranger 命令）
    pub vargranger: Vec<VARGrangerRow>,
}

/// varstable 单行：特征值及其模
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARStableRow {
    /// 特征值实部
    pub re: f64,
    /// 特征值虚部
    pub im: f64,
    /// 模 |λ|
    pub modulus: f64,
}

/// varlmar 单行：lag 阶的 LM 检验
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARLmarRow {
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// vargranger 单行：格兰杰因果 Wald 检验（Stata vargranger 命令）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARGrangerRow {
    /// 方程（因变量）
    pub eq_name: String,
    /// 被排除的变量（或 "ALL"）
    pub excluded: String,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// varwle 单行：某方程（或 All）在 lag 上的 Wald 检验
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARWleRow {
    pub eq_name: String,
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

impl VAR {
    pub fn fit(&self) -> Result<VARResult, String> {
        let y = &self.y;
        let (t, k) = (y.nrows(), y.ncols());
        let lags = &self.config.lags;
        let step = self.config.step;
        let constant = self.config.constant;

        if lags.is_empty() {
            return Err("VAR: lags cannot be empty".to_string());
        }
        let p_max = *lags.iter().max().ok_or("VAR: empty lags")?;
        if t <= p_max {
            return Err(format!(
                "VAR: need T > p_max ({}), got T={}",
                p_max, t
            ));
        }

        // 构建 Z: 每行 [y_{t-1}', ..., y_{t-p}', exog_t', 1] 展平
        let n_lag_coefs = k * lags.len();
        let n_exog = self.exog.as_ref().map(|x| x.ncols()).unwrap_or(0);
        let n_z = n_lag_coefs + n_exog + if constant { 1 } else { 0 };
        let n_obs = t - p_max;

        if let Some(ref exog) = self.exog {
            if exog.nrows() != t {
                return Err(format!(
                    "VAR: exog has {} rows, expected {} (must match Y length)",
                    exog.nrows(), t
                ));
            }
        }

        let mut z = Array2::zeros((n_obs, n_z));
        let mut y_dep = Array2::zeros((n_obs, k));

        for i in 0..n_obs {
            let row_t = p_max + i;
            let mut col_z = 0;
            // 先遍历每个 y，再遍历其 L1, L2, ... → L1.y1, L2.y1; L1.y2, L2.y2; ...
            for j in 0..k {
                for &lag in lags.iter() {
                    let lag_row = row_t - lag;
                    z[[i, col_z]] = y[[lag_row, j]];
                    col_z += 1;
                }
            }
            if let Some(ref exog) = self.exog {
                for j in 0..exog.ncols() {
                    z[[i, col_z]] = exog[[row_t, j]];
                    col_z += 1;
                }
            }
            if constant {
                z[[i, col_z]] = 1.0;
            }
            for j in 0..k {
                y_dep[[i, j]] = y[[row_t, j]];
            }
        }

        // 每方程 OLS
        let z_faer = z.view().into_faer().to_owned();
        let zt = z_faer.transpose();
        let ztz = zt.as_ref() * z_faer.as_ref();
        let ztz_inv = ztz
            .llt(Side::Lower)
            .map_err(|_| "VAR: Z'Z not positive definite (check collinearity)".to_string())?
            .solve(Mat::identity(ztz.nrows(), ztz.ncols()));

        let mut coefficients = Vec::with_capacity(k);
        let mut std_errs = Vec::with_capacity(k);
        let mut residuals = Vec::with_capacity(k);
        let mut ss_residual = Vec::with_capacity(k);
        let mut ss_total = Vec::with_capacity(k);
        let mut coef_labels = Vec::with_capacity(k);
        let mut cov_beta = Vec::with_capacity(k);

        for eq in 0..k {
            let y_col = y_dep.column(eq).into_owned();
            let y_faer = y_col.view().into_faer_col().to_owned();
            let zty = zt.as_ref() * y_faer.as_ref();
            let beta = ztz_inv.as_ref() * zty;
            let y_hat = z_faer.as_ref() * beta.as_ref();
            let u = y_faer.as_ref() - y_hat.as_ref();

            let beta_nd = beta.as_ref().into_ndarray().to_owned();
            let u_nd = u.as_ref().into_ndarray().to_owned();

            let ss_r: f64 = u_nd.iter().map(|x| x * x).sum();
            let y_mean = y_col.iter().sum::<f64>() / (y_col.len() as f64).max(1.0);
            let ss_t: f64 = y_col.iter().map(|x| (x - y_mean).powi(2)).sum();

            // 系数 VCE 的残差方差：与 Stata 一致
            // 无 dfk: σ̂²_i = SSR_i / T (ML divisor)
            // 有 dfk: σ̂²_i = SSR_i / (T - m)，m = n_z
            let sigma2_divisor = if self.config.dfk {
                (n_obs as f64 - n_z as f64).max(1.0)
            } else {
                n_obs as f64
            };
            let sigma2_eq = ss_r / sigma2_divisor;
            let xtx_inv_nd = ztz_inv.as_ref().into_ndarray().to_owned();
            let cov_eq = sigma2_eq * &xtx_inv_nd;
            cov_beta.push(cov_eq.clone());
            let se: Array1<f64> = cov_eq.diag().mapv(f64::sqrt);

            let mut labels = Vec::with_capacity(n_z);
            let names = self.var_names.as_ref().map(|n| n.as_slice()).unwrap_or(&[]);
            // 先遍历每个 y，再遍历其 L1, L2, ... → L1.y1, L2.y1; L1.y2, L2.y2; ...
            for v in 0..k {
                for &lag in lags.iter() {
                    let label = match names.get(v) {
                        Some(s) => format!("L{}.{}", lag, s),
                        None => format!("L{}.y{}", lag, v),
                    };
                    labels.push(label);
                }
            }
            if let Some(ref exog_names) = self.exog_names {
                for name in exog_names {
                    labels.push(name.clone());
                }
            } else if n_exog > 0 {
                for v in 0..n_exog {
                    labels.push(format!("exog{}", v));
                }
            }
            if constant {
                labels.push("const".to_string());
            }

            coefficients.push(beta_nd.to_vec());
            std_errs.push(se.to_vec());
            residuals.push(u_nd.to_vec());
            ss_residual.push(ss_r);
            ss_total.push(ss_t);
            coef_labels.push(labels);
        }

        // 残差矩阵 U (n_obs × K)
        let u_mat = Array2::from_shape_fn((n_obs, k), |(i, j)| residuals[j][i]);
        let te = if self.config.dfk {
            let m = n_z as f64;
            (n_obs as f64 - m).max(1.0)
        } else {
            n_obs as f64
        };
        let sigma = (u_mat.t().dot(&u_mat) / te).to_owned();
        let df_r = n_obs - n_z;

        // 提取 A 矩阵：A_i 为 K×K，对应 lag i
        let mut a_mats: Vec<Array2<f64>> = Vec::with_capacity(p_max + 1);
        for _ in 0..=p_max {
            a_mats.push(Array2::zeros((k, k)));
        }
        let n_lags = lags.len();
        for (lag_idx, &lag) in lags.iter().enumerate() {
            for i in 0..k {
                for j in 0..k {
                    let coef_idx = j * n_lags + lag_idx;
                    a_mats[lag][[i, j]] = coefficients[i][coef_idx];
                }
            }
        }

        // IRF: Φ_0 = I, Φ_s = Σ A_i Φ_{s-i}
        let mut phi: Vec<Array2<f64>> = vec![Array2::eye(k)];
        for s in 1..=step {
            let mut phi_s = Array2::zeros((k, k));
            for i in 1..=s.min(p_max) {
                if lags.contains(&i) {
                    phi_s = phi_s + a_mats[i].dot(&phi[s - i]);
                }
            }
            phi.push(phi_s);
        }

        // Cholesky: Σ = GG', G lower triangular (in-place on sigma copy)
        let mut g_nd = sigma.clone();
        cholesky_lower_in_place(&mut g_nd)
            .map_err(|_| "VAR: Sigma not positive definite for Cholesky".to_string())?;

        // det(Σ) = det(G)^2, G lower triangular => det(G) = prod(diag(G))
        let det_g: f64 = (0..k).map(|i| g_nd[[i, i]]).product();
        let det_sigma = (det_g * det_g).abs();

        // OIRF: Θ_s = Φ_s G
        let mut theta: Vec<Array2<f64>> = Vec::with_capacity(step + 1);
        let mut oirf: Vec<Vec<Vec<f64>>> = Vec::with_capacity(step + 1);
        for s in 0..=step {
            let theta_s = phi[s].dot(&g_nd);
            oirf.push(
                (0..k)
                    .map(|i| (0..k).map(|j| theta_s[[i, j]]).collect())
                    .collect(),
            );
            theta.push(theta_s);
        }

        // FEVD: MSE(h) = Σ_{s=0}^{h-1} Θ_s Θ_s', FEVD_ij(h) = Σ_{s=0}^{h-1} Θ_s[i,j]^2 / MSE_ii(h)
        let mut fevd = Vec::with_capacity(step + 1);
        let mut mse: Array2<f64> = Array2::zeros((k, k));
        for s in 0..=step {
            let theta_s = &theta[s];
            mse = mse + theta_s.dot(&theta_s.t());
            let mut fevd_s = vec![vec![0.0; k]; k];
            for i in 0..k {
                let mse_ii = mse[[i, i]];
                if mse_ii > 1e-300 {
                    for j in 0..k {
                        let mut sum = 0.0;
                        for m in 0..=s {
                            sum += theta[m][[i, j]].powi(2);
                        }
                        fevd_s[i][j] = sum / mse_ii;
                    }
                } else {
                    for j in 0..k {
                        fevd_s[i][j] = if i == j { 1.0 } else { 0.0 };
                    }
                }
            }
            fevd.push(fevd_s);
        }

        // Log likelihood, AIC, etc.
        let det_sigma_ml = if det_sigma > 1e-300 {
            det_sigma
        } else {
            1e-300
        };
        let ll = -0.5 * (n_obs as f64) * (k as f64 * (2.0 * std::f64::consts::PI).ln()
            + det_sigma_ml.ln()
            + k as f64);
        let n_parms = (k * n_z) as f64;
        let aic = -2.0 * ll / (n_obs as f64) + 2.0 * n_parms / (n_obs as f64);
        let fpe = det_sigma_ml * ((n_obs as f64 + n_parms) / (n_obs as f64 - n_parms)).powi(k as i32);
        let hqic = -2.0 * ll / (n_obs as f64) + 2.0 * n_parms * (n_obs as f64).ln().ln() / (n_obs as f64);
        let sbic = -2.0 * ll / (n_obs as f64) + n_parms * (n_obs as f64).ln() / (n_obs as f64);

        // 方程统计
        let var_names = self
            .var_names
            .clone()
            .unwrap_or_else(|| (0..k).map(|i| format!("y{}", i)).collect());

        let mut equations = Vec::with_capacity(k);
        let mut z_values = Vec::with_capacity(k);
        let mut p_values = Vec::with_capacity(k);
        let mut ci_lower = Vec::with_capacity(k);
        let mut ci_upper = Vec::with_capacity(k);

        for eq in 0..k {
            let rmse = (ss_residual[eq] / df_r as f64).sqrt();
            let r_sq = if ss_total[eq] > 1e-300 {
                1.0 - ss_residual[eq] / ss_total[eq]
            } else {
                0.0
            };
            // Stata var (default, asymptotic): chi2 = n * R²/(1-R²), df = n_z
            let chi2 = if r_sq < 1.0 - 1e-10 {
                n_obs as f64 * r_sq / (1.0 - r_sq)
            } else {
                0.0
            };
            let p_chi2 = 1.0 - statrs::distribution::ChiSquared::new(n_z as f64).unwrap().cdf(chi2);

            let mut zv = Vec::with_capacity(n_z);
            let mut pv = Vec::with_capacity(n_z);
            let mut cl = Vec::with_capacity(n_z);
            let mut cu = Vec::with_capacity(n_z);
            for j in 0..n_z {
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

            let eq_name = var_names.get(eq).cloned().unwrap_or_else(|| format!("eq{}", eq));
            equations.push(VAREquationStats {
                eq_name,
                parms: n_z,
                rmse,
                r_sq,
                chi2,
                p_chi2,
            });
        }

        // varwle: Wald lag-exclusion 检验（Stata varwle 命令）
        // 对每个 lag，检验该 lag 的 k 个系数是否联合为零
        // 系数顺序：先 y 再 lag → L1.y1, L2.y1; L1.y2, L2.y2; ... 故 lag_idx 对应索引 j*n_lags+lag_idx
        let n_lags = lags.len();
        let mut varwle = Vec::new();
        for (lag_idx, &lag) in lags.iter().enumerate() {
            let lag_indices: Vec<usize> = (0..k).map(|j| j * n_lags + lag_idx).collect();

            // 每个方程
            let mut chi2_all = 0.0;
            for eq in 0..k {
                let beta_lag: Array1<f64> = Array1::from_iter(
                    lag_indices.iter().map(|&idx| coefficients[eq][idx]),
                );
                let v_block = Array2::from_shape_fn((k, k), |(r, c)| {
                    cov_beta[eq][[lag_indices[r], lag_indices[c]]]
                });
                let v_faer = v_block.view().into_faer();
                let beta_faer = beta_lag.view().into_faer_col().to_owned();
                let x = v_faer
                    .llt(Side::Lower)
                    .map_err(|_| "VAR varwle: lag block V not positive definite".to_string())?
                    .solve(beta_faer.as_ref());
                let x_nd = x.as_ref().into_ndarray().to_owned();
                let wald_eq: f64 = beta_lag.iter().zip(x_nd.iter()).map(|(b, xi)| b * xi).sum();
                chi2_all += wald_eq;

                let p_eq = 1.0 - statrs::distribution::ChiSquared::new(k as f64).unwrap().cdf(wald_eq);
                let eq_name = var_names.get(eq).cloned().unwrap_or_else(|| format!("eq{}", eq));
                varwle.push(VARWleRow {
                    eq_name,
                    lag,
                    chi2: wald_eq,
                    df: k,
                    p_value: p_eq,
                });
            }

            // All equations jointly
            let p_all = 1.0 - statrs::distribution::ChiSquared::new((k * k) as f64).unwrap().cdf(chi2_all);
            varwle.push(VARWleRow {
                eq_name: "All".to_string(),
                lag,
                chi2: chi2_all,
                df: k * k,
                p_value: p_all,
            });
        }

        // varlmar: LM 残差自相关检验（Stata varlmar 命令，Johansen 1995）
        // LM_s = (T - d - 0.5) * ln(|Σ̂| / |Σ̃_s|)，df = K²
        // varlmar 始终使用 ML 估计 Σ（除数 T）
        let sigma_ml = (u_mat.t().dot(&u_mat) / n_obs as f64).to_owned();
        let mut det_sigma_ml_var = sigma_ml.clone();
        cholesky_lower_in_place(&mut det_sigma_ml_var)
            .map_err(|_| "VAR varlmar: Sigma_ml not positive definite".to_string())?;
        let det_g_ml: f64 = (0..k).map(|i| det_sigma_ml_var[[i, i]]).product();
        let det_sigma_hat = (det_g_ml * det_g_ml).abs().max(1e-300);

        let mlag = self.config.mlag;
        let mut varlmar = Vec::new();
        for s in 1..=mlag {
            if s >= n_obs {
                break;
            }
            // 构建 augmented Z: [Z_orig | res_lag_s]，res_lag_s 为 K 列，第 j 列为 residuals[j] 滞后 s 期（前 s 行填 0）
            let n_z_aug = n_z + k;
            let mut z_aug = Array2::zeros((n_obs, n_z_aug));
            z_aug.slice_mut(ndarray::s![.., ..n_z]).assign(&z.view());
            for j in 0..k {
                for i in 0..n_obs {
                    z_aug[[i, n_z + j]] = if i >= s { residuals[j][i - s] } else { 0.0 };
                }
            }

            let z_aug_faer = z_aug.view().into_faer().to_owned();
            let zt_aug = z_aug_faer.transpose();
            let ztz_aug = zt_aug.as_ref() * z_aug_faer.as_ref();
            let ztz_aug_inv = ztz_aug
                .llt(Side::Lower)
                .map_err(|_| "VAR varlmar: augmented Z'Z not positive definite".to_string())?
                .solve(Mat::identity(ztz_aug.nrows(), ztz_aug.ncols()));

            let mut u_aug = Array2::zeros((n_obs, k));
            for eq in 0..k {
                let y_col = y_dep.column(eq).into_owned();
                let y_faer = y_col.view().into_faer_col().to_owned();
                let zty = zt_aug.as_ref() * y_faer.as_ref();
                let beta = ztz_aug_inv.as_ref() * zty;
                let y_hat = z_aug_faer.as_ref() * beta.as_ref();
                let u = y_faer.as_ref() - y_hat.as_ref();
                let u_nd = u.as_ref().into_ndarray().to_owned();
                for i in 0..n_obs {
                    u_aug[[i, eq]] = u_nd[i];
                }
            }

            let sigma_tilde = (u_aug.t().dot(&u_aug) / n_obs as f64).to_owned();
            let mut det_tilde = sigma_tilde.clone();
            cholesky_lower_in_place(&mut det_tilde)
                .map_err(|_| "VAR varlmar: Sigma_tilde not positive definite".to_string())?;
            let det_g_tilde: f64 = (0..k).map(|i| det_tilde[[i, i]]).product();
            let det_sigma_tilde = (det_g_tilde * det_g_tilde).abs().max(1e-300);

            let d = n_z_aug;
            let lm_stat = (n_obs as f64 - d as f64 - 0.5) * (det_sigma_hat / det_sigma_tilde).ln();
            let lm_stat = lm_stat.max(0.0);
            let df_lm = k * k;
            let p_lm = 1.0 - statrs::distribution::ChiSquared::new(df_lm as f64).unwrap().cdf(lm_stat);

            varlmar.push(VARLmarRow {
                lag: s,
                chi2: lm_stat,
                df: df_lm,
                p_value: p_lm,
            });
        }

        // varstable: 特征值平稳性检验（Stata varstable 命令）
        // 伴随机阵 A = [A1 A2 ... Ap; I 0 ... 0; ...; 0 ... I 0]，VAR 平稳当且仅当所有特征值模 < 1
        let kp = k * n_lags;
        let mut companion = Mat::zeros(kp, kp);
        for (lag_idx, &lag) in lags.iter().enumerate() {
            for i in 0..k {
                for j in 0..k {
                    companion.as_mut()[(i, lag_idx * k + j)] = a_mats[lag][[i, j]];
                }
            }
        }
        for block in 0..(n_lags - 1) {
            for i in 0..k {
                companion.as_mut()[(k + block * k + i, block * k + i)] = 1.0;
            }
        }
        let evd = faer::linalg::solvers::Eigen::new_from_real(companion.as_ref())
            .map_err(|_| "VAR varstable: eigendecomposition failed".to_string())?;
        let s_diag = evd.S().column_vector();
        let mut varstable = Vec::with_capacity(kp);
        for ev in s_diag.iter() {
            let re: f64 = ev.re;
            let im: f64 = ev.im;
            let modulus = (re * re + im * im).sqrt();
            varstable.push(VARStableRow { re, im, modulus });
        }

        // vargranger: 格兰杰因果 Wald 检验（Stata vargranger 命令）
        // 对每个方程 i，检验排除变量 j（j≠i）的滞后项是否显著；Excluded "ALL" 为排除所有其他变量
        // 系数顺序：变量优先 → L1.y0, L2.y0, ...; L1.y1, L2.y1, ...；变量 j 的索引为 j*n_lags .. j*n_lags+n_lags-1
        let mut vargranger = Vec::new();
        for eq in 0..k {
            let eq_name = var_names.get(eq).cloned().unwrap_or_else(|| format!("eq{}", eq));
            let cov = &cov_beta[eq];
            let beta = &coefficients[eq];

            // 对每个被排除的变量 j（j != eq）
            for j in 0..k {
                if j == eq {
                    continue;
                }
                let indices: Vec<usize> = (0..n_lags).map(|s| j * n_lags + s).collect();
                let beta_r: Array1<f64> = Array1::from_iter(indices.iter().map(|&idx| beta[idx]));
                let v_block = Array2::from_shape_fn((n_lags, n_lags), |(r, c)| {
                    cov[[indices[r], indices[c]]]
                });
                let v_faer = v_block.view().into_faer();
                let beta_faer = beta_r.view().into_faer_col().to_owned();
                let x = v_faer
                    .llt(Side::Lower)
                    .map_err(|_| "VAR vargranger: block V not positive definite".to_string())?
                    .solve(beta_faer.as_ref());
                let x_nd = x.as_ref().into_ndarray().to_owned();
                let wald: f64 = beta_r.iter().zip(x_nd.iter()).map(|(b, xi)| b * xi).sum();
                let p_val = 1.0 - statrs::distribution::ChiSquared::new(n_lags as f64).unwrap().cdf(wald);
                let excluded_name = var_names.get(j).cloned().unwrap_or_else(|| format!("y{}", j));
                vargranger.push(VARGrangerRow {
                    eq_name: eq_name.clone(),
                    excluded: excluded_name,
                    chi2: wald,
                    df: n_lags,
                    p_value: p_val,
                });
            }

            // Excluded ALL：排除所有 j != eq
            let mut all_indices = Vec::new();
            for j in 0..k {
                if j != eq {
                    for s in 0..n_lags {
                        all_indices.push(j * n_lags + s);
                    }
                }
            }
            let r = all_indices.len();
            if r > 0 {
                let beta_r: Array1<f64> = Array1::from_iter(all_indices.iter().map(|&idx| beta[idx]));
                let v_block = Array2::from_shape_fn((r, r), |(ri, ci)| {
                    cov[[all_indices[ri], all_indices[ci]]]
                });
                let v_faer = v_block.view().into_faer();
                let beta_faer = beta_r.view().into_faer_col().to_owned();
                let x = v_faer
                    .llt(Side::Lower)
                    .map_err(|_| "VAR vargranger: ALL block V not positive definite".to_string())?
                    .solve(beta_faer.as_ref());
                let x_nd = x.as_ref().into_ndarray().to_owned();
                let wald: f64 = beta_r.iter().zip(x_nd.iter()).map(|(b, xi)| b * xi).sum();
                let p_val = 1.0 - statrs::distribution::ChiSquared::new(r as f64).unwrap().cdf(wald);
                vargranger.push(VARGrangerRow {
                    eq_name: eq_name.clone(),
                    excluded: "ALL".to_string(),
                    chi2: wald,
                    df: r,
                    p_value: p_val,
                });
            }
        }

        Ok(VARResult {
            var_names,
            num_observation: n_obs,
            log_likelihood: ll,
            aic,
            fpe,
            hqic,
            sbic,
            det_sigma_ml,
            equations,
            coefficients,
            std_errs,
            z_values,
            p_values,
            ci_lower,
            ci_upper,
            coef_labels,
            sigma: sigma.rows().into_iter().map(|r| r.iter().cloned().collect()).collect(),
            oirf,
            fevd,
            varwle,
            varlmar,
            varstable,
            vargranger,
        })
    }
}
