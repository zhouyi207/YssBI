use super::first_stage::{compute_first_stage_summary, is_robust_cov_type};
use super::types::{
    EndogenousTest, FirstStageResult, HausmanTest, IV2SLS, IV2SLSModel, IV2SLSResult, OveridTest,
};
use crate::regression::covariance::compute_cov_beta;

use faer::{Col, Mat};
use statrs::{
    distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, Normal, StudentsT},
    statistics::Statistics,
};
use yss_linalg::matrix_rank;
use yss_linalg::{MatrixExt, Solve};

impl IV2SLS {
    pub fn fit(&self) -> Result<IV2SLSResult, String> {
        let n = self.endog.nrows();
        let k_exog = self.exog.ncols();
        let k_endog = self.endog_reg.ncols();
        let k_iv = self.instruments.ncols();

        if k_iv < k_endog {
            return Err(format!(
                "IV2SLS: underidentified — {} instruments < {} endogenous. Need at least {} instruments.",
                k_iv, k_endog, k_endog
            ));
        }

        // Z = [exog, instruments] for stage 1 (with constant if config.constant)
        let k_z = if self.config.constant {
            k_exog + k_iv + 1
        } else {
            k_exog + k_iv
        };
        let mut z_raw = Vec::with_capacity(n * k_z);
        for i in 0..n {
            if self.config.constant {
                z_raw.push(1.0);
            }
            for j in 0..k_exog {
                z_raw.push(self.exog[(i, j)]);
            }
            for j in 0..k_iv {
                z_raw.push(self.instruments[(i, j)]);
            }
        }
        let z = faer::MatRef::from_row_major_slice(&(z_raw), n, k_z).to_owned();

        // Stage 1: endog_hat = Z * (Z'Z)^{-1} Z' * endog for each endogenous
        let z_matrix = z.as_ref().to_owned();
        let ztz = z_matrix.transpose() * z_matrix.as_ref();
        let ztz_inv = ztz
            .checked_cholesky()
            .map_err(|_| {
                "IV2SLS: Z'Z is not positive definite (stage 1). Check instruments and exog for collinearity.".to_string()
            })?
            .solve(&Mat::identity(ztz.nrows(), ztz.nrows()));

        let ztz_inv_nd = ztz_inv.as_ref().to_owned();
        let df_z = n.saturating_sub(k_z);

        let mut endog_hat = Mat::zeros(n, k_endog);
        let mut first_stage: Vec<FirstStageResult> = Vec::with_capacity(k_endog);
        for j in 0..k_endog {
            let endog_col = self.endog_reg.col(j).to_owned();
            let endog_vector = endog_col.as_ref().to_owned();
            let zty = z_matrix.transpose() * endog_vector.as_ref();
            let gamma = ztz_inv.as_ref() * zty.as_ref();
            let hat = z_matrix.as_ref() * gamma.as_ref();
            let hat_arr = hat.as_ref().to_owned();
            for i in 0..n {
                endog_hat[(i, j)] = hat_arr[i];
            }

            // First-stage stats: resid, r2, cov_gamma, stds, t, p
            let resid = &endog_col - &hat_arr;
            let ss_resid = resid.iter().map(|v| v.powi(2)).sum::<f64>();
            let y_mean = endog_col.iter().mean();
            let ss_tot = endog_col.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>();
            let r2 = if ss_tot > 1e-300 {
                1.0 - ss_resid / ss_tot
            } else {
                0.0
            };
            let ms_resid = if df_z > 0 {
                ss_resid / df_z as f64
            } else {
                0.0
            };
            let ms_tot = if n > 1 { ss_tot / (n - 1) as f64 } else { 0.0 };
            let r2_adj = if ms_tot > 1e-300 {
                1.0 - ms_resid / ms_tot
            } else {
                0.0
            };

            let sigma2 = if df_z > 0 {
                (ss_resid / df_z as f64).max(1e-300)
            } else {
                1e-300
            };
            let cov_gamma = faer::Scale(sigma2) * &ztz_inv_nd;
            let stds: Vec<f64> = (0..k_z).map(|i| cov_gamma[(i, i)].sqrt()).collect();
            let gamma_nd = gamma.as_ref().to_owned();
            let t_dist = StudentsT::new(0.0, 1.0, df_z as f64)
                .unwrap_or(StudentsT::new(0.0, 1.0, 1.0).unwrap());
            let t_values: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] / stds[i]).collect();
            let p_values: Vec<f64> = t_values
                .iter()
                .map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
                .collect();
            let t_crit = t_dist.inverse_cdf(0.975);
            let ci_left: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] - t_crit * stds[i]).collect();
            let ci_right: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] + t_crit * stds[i]).collect();

            let name = self
                .endog_names
                .as_ref()
                .and_then(|n| n.get(j))
                .cloned()
                .unwrap_or_else(|| format!("endog_{}", j + 1));
            let var_names: Vec<String> = (0..k_z)
                .map(|i| {
                    self.z_var_names
                        .as_ref()
                        .and_then(|v| v.get(i).cloned())
                        .unwrap_or_else(|| format!("z{}", i + 1))
                })
                .collect();
            first_stage.push(FirstStageResult {
                endog_name: name,
                var_names,
                betas: gamma_nd.iter().copied().collect(),
                stds,
                tvalues: t_values,
                pvalues: p_values,
                conf_int_left: ci_left,
                conf_int_right: ci_right,
                r2,
                r2_adjusted: r2_adj,
            });
        }

        // Stage 2: X = [exog, endog_hat] (with constant)
        let k_x = if self.config.constant {
            k_exog + k_endog + 1
        } else {
            k_exog + k_endog
        };
        let mut x_raw = Vec::with_capacity(n * k_x);
        for i in 0..n {
            if self.config.constant {
                x_raw.push(1.0);
            }
            for j in 0..k_exog {
                x_raw.push(self.exog[(i, j)]);
            }
            for j in 0..k_endog {
                x_raw.push(endog_hat[(i, j)]);
            }
        }
        let x = faer::MatRef::from_row_major_slice(&(x_raw), n, k_x).to_owned();

        let (rank, cond_no) = matrix_rank(x.as_ref()).unwrap_or((0, f64::INFINITY));
        let df_residual = n - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_residual + df_model;

        let covariance_type = if self.config.cov_type.is_empty() {
            "nonrobust".to_string()
        } else {
            self.config.cov_type.clone()
        };

        // OLS on second stage: β = (X'X)^{-1} X'y
        let x_matrix = x.as_ref().to_owned();
        let y_vector = self.endog.as_ref().to_owned();
        let xtx = x_matrix.transpose() * x_matrix.as_ref();
        let xty = x_matrix.transpose() * y_vector.as_ref();
        let xtx_inv = xtx
            .checked_cholesky()
            .map_err(|_| {
                "IV2SLS: X'X is not positive definite (stage 2). Check for collinearity."
                    .to_string()
            })?
            .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));
        let betas_vector = xtx_inv.as_ref() * xty.as_ref();
        let betas_nd = betas_vector.as_ref().to_owned();

        // ESS and VCE must use structural residuals: u = y - X_struct * β
        // where X_struct = [exog, endog] (actual endogenous, not endog_hat).
        // Stata: ESS = y'y - 2β'X'y + β'X'Xβ, σ² = ESS/(n-k), VCE = σ² (X'P_Z X)^{-1}.
        let mut x_struct_raw = Vec::with_capacity(n * k_x);
        for i in 0..n {
            if self.config.constant {
                x_struct_raw.push(1.0);
            }
            for j in 0..k_exog {
                x_struct_raw.push(self.exog[(i, j)]);
            }
            for j in 0..k_endog {
                x_struct_raw.push(self.endog_reg[(i, j)]);
            }
        }
        let x_struct = faer::MatRef::from_row_major_slice(&(x_struct_raw), n, k_x).to_owned();
        let u_structural: Col<f64> = &self.endog - &(x_struct.as_ref() * betas_nd.as_ref());

        let y_mean = y_vector.iter().mean();
        let ss_total = if self.config.constant {
            y_vector.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            y_vector.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let ss_residual = u_structural.transpose() * u_structural.as_ref();
        let ss_model = ss_total - ss_residual;
        let r2 = if ss_total > 1e-300 {
            1.0 - ss_residual / ss_total
        } else {
            0.0
        };

        let ms_model = ss_model / df_model as f64;
        let ms_residual = ss_residual / df_residual as f64;
        let ms_total = ss_total / df_total as f64;
        let r2_adjusted = if ms_total > 1e-300 {
            1.0 - ms_residual / ms_total
        } else {
            0.0
        };

        let x_nd = x_matrix.as_ref().to_owned();
        let xtx_inv_nd = xtx_inv.as_ref().to_owned();

        // Stata: s² = ESS/(n-k) if small, else ESS/n. Affects VCE and robust scale.
        let sigma2_df = if self.config.small { df_residual } else { n };

        let cov_beta = compute_cov_beta(
            &x_nd,
            &xtx_inv_nd,
            &u_structural,
            sigma2_df,
            &covariance_type,
            self.config.cov_params.as_ref(),
        )?;

        let std_err: Col<f64> = cov_beta.diagonal().column_vector().map(|v| v.sqrt());
        // 2SLS uses asymptotic inference: z = coef/se ~ N(0,1), not t
        let z_values: Vec<f64> = betas_nd
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| if *se > 1e-300 { b / se } else { 0.0 })
            .collect();

        let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("IV2SLS: {}", e))?;
        let p_values: Vec<f64> = z_values
            .iter()
            .map(|&z| 2.0 * (1.0 - std_normal.cdf(z.abs())))
            .collect();

        let z_crit = std_normal.inverse_cdf(0.975);
        let ci_lower = &betas_nd - faer::Scale(z_crit) * &std_err;
        let ci_upper = &betas_nd + faer::Scale(z_crit) * &std_err;

        // Wald chi2 for joint significance (2SLS uses chi2, not F). Stata Methods: "If c=1 and small is not
        // specified, a Wald statistic W of the joint significance of the k−1 parameters of β except the
        // constant term is calculated; W ∼ χ²(k−1)." W = β_s' V_s^{-1} β_s. Use solve(V_s, β_s) for stability.
        let k = betas_nd.nrows();
        let (wald_chi2, wald_p) = {
            let (beta_s, v_s, df_wald) = if self.config.constant && k > 1 {
                // Exclude constant (index 0). Our X = [const, exog, endog_hat], so const is always first.
                let beta_s = betas_nd.subrows(1, betas_nd.nrows() - 1).to_owned();
                let v_s = cov_beta
                    .submatrix(1, 1, cov_beta.nrows() - 1, cov_beta.ncols() - 1)
                    .to_owned();
                (beta_s, v_s, k - 1)
            } else {
                let beta_s = betas_nd.clone();
                let v_s = cov_beta.clone();
                (beta_s, v_s, k)
            };
            let v_s_matrix = v_s.as_ref().to_owned();
            let beta_s_vector = beta_s.as_ref().to_owned();
            // Solve V_s * x = beta_s => x = V_s^{-1} * beta_s; then wald = beta_s' * x (more stable than explicit inverse)
            let x = v_s_matrix
                .as_ref()
                .checked_cholesky()
                .map_err(|_| "IV2SLS: V_s not pd for Wald".to_string())?
                .solve(&beta_s_vector.as_ref());
            let x_nd = x.as_ref();
            let wald = beta_s.transpose() * x_nd.as_ref();
            let chi2_dist =
                ChiSquared::new(df_wald as f64).map_err(|e| format!("IV2SLS Wald: {}", e))?;
            let wald_p = 1.0 - chi2_dist.cdf(wald);
            (wald, wald_p)
        };

        // estat firststage: First-stage regression summary statistics
        let first_stage_summary = compute_first_stage_summary(
            &z,
            &endog_hat,
            &self.endog_reg,
            &self.exog,
            &self.instruments,
            n,
            k_z,
            k_exog,
            k_iv,
            k_endog,
            self.config.constant,
            &covariance_type,
            self.config.cov_params.as_ref(),
            self.config.small,
            false, // for_liml
        )?;

        // Overidentification test (estat overid): Sargan/Basmann (homoskedastic) or Wooldridge (1995) robust score (robust VCE).
        // Stata: "If you used the 2SLS estimator and requested a robust VCE, Wooldridge's robust score test of
        // overidentifying restrictions is performed instead; without a robust VCE, Wooldridge's test statistic is identical to Sargan's."
        let overid = if k_iv > k_endog {
            let df_overid = k_iv - k_endog;
            let chi2_dist = ChiSquared::new(df_overid as f64)
                .map_err(|e| format!("IV2SLS overid ChiSquared: {}", e))?;

            let is_robust = is_robust_cov_type(&covariance_type);

            if is_robust {
                // Wooldridge (1995) robust score test. Stata Methods: Let Ŷ = endog_hat, Q = excluded instruments (m cols).
                // q̂_j = residuals from regressing jth column of Q on [X1, Ŷ]. k̂_ij = q̂_ij * û_i.
                // Regress 1 on [k̂_1,...,k̂_m]: W = N - RSS ~ χ²(m).
                let m = df_overid;
                let w_mat = &x; // W = [X1, Ŷ] = [const?, exog, endog_hat]
                let wtw = w_mat.transpose() * w_mat.as_ref();
                let wtw_inv = wtw
                    .as_ref()
                    .to_owned()
                    .checked_cholesky()
                    .map_err(|_| "IV2SLS Wooldridge overid: W'W not positive definite".to_string())?
                    .solve(&Mat::identity(wtw.nrows(), wtw.nrows()));
                let wtw_inv_nd = wtw_inv.as_ref().to_owned();

                // Build K: n × m, columns k̂_j = (Q_j - W*γ_j) .* u, where γ_j = (W'W)^{-1} W' Q_j
                let mut k_mat = Mat::zeros(n, m);
                for j in 0..m {
                    let q_j = self.instruments.col(j).to_owned();
                    let wtq = w_mat.transpose() * q_j.as_ref();
                    let gamma_j = wtw_inv_nd.as_ref() * wtq.as_ref();
                    let q_hat = w_mat.as_ref() * gamma_j.as_ref(); // fitted = W * γ
                    let q_resid = &q_j - &q_hat; // q̂_j = residuals
                    for i in 0..n {
                        k_mat[(i, j)] = q_resid[i] * u_structural[i];
                    }
                }

                // Regress 1 on K: 1 = K*θ + ε. RSS = (1 - K*θ)^2. W = N - RSS.
                let ones = Col::full(n, 1.0);
                let ktk = k_mat.transpose() * k_mat.as_ref();
                let kt1 = k_mat.transpose() * ones.as_ref();
                let ktk_inv = ktk
                    .as_ref()
                    .to_owned()
                    .checked_cholesky()
                    .map_err(|_| "IV2SLS Wooldridge overid: K'K not positive definite".to_string())?
                    .solve(&Mat::identity(ktk.nrows(), ktk.nrows()));
                let theta = ktk_inv.as_ref() * kt1.as_ref();
                let fitted = k_mat.as_ref() * theta.as_ref();
                let rss: f64 = ones
                    .iter()
                    .zip(fitted.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum();
                let wooldridge_stat = n as f64 - rss;
                let wooldridge_p = 1.0 - chi2_dist.cdf(wooldridge_stat);
                Some(OveridTest {
                    test_type: "wooldridge".to_string(),
                    sargan_stat: None,
                    sargan_p_value: None,
                    basmann_stat: None,
                    basmann_p_value: None,
                    wooldridge_stat: Some(wooldridge_stat),
                    wooldridge_p_value: Some(wooldridge_p),
                    df: df_overid,
                })
            } else {
                // Sargan & Basmann (homoskedastic)
                let uu = u_structural.transpose() * u_structural.as_ref();
                if uu > 1e-300 {
                    let ztu = z.transpose() * u_structural.as_ref();
                    let ztz_inv_ztu = ztz_inv_nd.as_ref() * ztu.as_ref();
                    let u_pz_u = ztu.transpose() * ztz_inv_ztu.as_ref();
                    let sargan_stat = n as f64 * u_pz_u / uu;
                    let basmann_stat = if (n as f64 - sargan_stat).abs() > 1e-10 {
                        sargan_stat * (n as f64 - k_z as f64) / (n as f64 - sargan_stat)
                    } else {
                        sargan_stat
                    };
                    let sargan_p = 1.0 - chi2_dist.cdf(sargan_stat);
                    let basmann_p = 1.0 - chi2_dist.cdf(basmann_stat);
                    Some(OveridTest {
                        test_type: "sargan_basmann".to_string(),
                        sargan_stat: Some(sargan_stat),
                        sargan_p_value: Some(sargan_p),
                        basmann_stat: Some(basmann_stat),
                        basmann_p_value: Some(basmann_p),
                        wooldridge_stat: None,
                        wooldridge_p_value: None,
                        df: df_overid,
                    })
                } else {
                    None
                }
            }
        } else {
            None
        };

        // Hausman tests (traditional + Durbin-Wu-Hausman): only for nonrobust VCE
        let (hausman, endogenous) = if !is_robust_cov_type(&covariance_type) {
            // OLS on y ~ X_struct (treating endog as exogenous): β_ols, u_ols
            let x_struct_tx = x_struct.transpose() * x_struct.as_ref();
            let x_struct_tx_inv: Option<faer::Mat<f64>> = x_struct_tx
                .as_ref()
                .to_owned()
                .checked_cholesky()
                .ok()
                .map(|llt| llt.solve(&Mat::identity(x_struct_tx.nrows(), x_struct_tx.nrows())));
            let (beta_ols, u_ols, sigma2_ols, xtx_struct_inv_nd) =
                if let Some(ref inv) = x_struct_tx_inv {
                    let inv_nd = inv.as_ref().to_owned();
                    let xty_struct = x_struct.transpose() * self.endog.as_ref();
                    let beta_ols_nd = inv_nd.as_ref() * xty_struct.as_ref();
                    let u_ols: Col<f64> = &self.endog - &(x_struct.as_ref() * beta_ols_nd.as_ref());
                    let sigma2_ols = (u_ols.transpose() * u_ols.as_ref()) / df_residual as f64;
                    (beta_ols_nd, u_ols, sigma2_ols, inv_nd)
                } else {
                    (
                        Col::<f64>::zeros(k_x),
                        Col::<f64>::zeros(n),
                        0.0,
                        Mat::zeros(k_x, k_x),
                    )
                };

            // Traditional Hausman (sigmamore): H = (β_iv - β_ols)'(V_iv - V_ols)^{-1}(β_iv - β_ols)
            // V_iv = σ²_ols * (X̂'X̂)^{-1}, V_ols = σ²_ols * (X_struct'X_struct)^{-1}
            let hausman = if sigma2_ols > 1e-300 {
                let v_iv = faer::Scale(sigma2_ols) * &xtx_inv_nd; // X̂'X̂ from stage 2
                let v_ols = sigma2_ols * &xtx_struct_inv_nd;
                let v_diff: Mat<f64> = &v_iv - &v_ols;
                let diff_beta = &betas_nd - &beta_ols;
                let v_diff_matrix = v_diff.as_ref().to_owned();
                let svd = yss_linalg::Svd::factor(v_diff_matrix.as_ref()).ok();
                let (h_stat, h_df) = if let Some(svd) = svd {
                    let s = svd.values();
                    let u = svd.left_vectors();
                    let v = svd.right_vectors();
                    let max_s = s.iter().cloned().fold(0.0f64, f64::max);
                    let tol = max_s * (k_x as f64) * f64::EPSILON;
                    let rank = s.iter().filter(|&&si| si > tol).count();
                    if rank == 0 {
                        (0.0, 0)
                    } else {
                        // H = diff' * V_diff^{-} * diff via SVD: V_diff = U S V', inv = V S^{-1} U' (Moore-Penrose)
                        let diff_col = diff_beta.as_ref().to_owned();
                        let ut_diff =
                            u.submatrix(0, 0, u.nrows(), k_x).transpose() * diff_col.as_ref();
                        let ut_diff_nd = ut_diff.as_ref().to_owned();
                        let mut st_inv_ut_diff = Mat::zeros(k_x, 1);
                        for i in 0..k_x {
                            let si = s[i];
                            let val = if si > tol { ut_diff_nd[i] / si } else { 0.0 };
                            st_inv_ut_diff.as_mut()[(i, 0)] = val;
                        }
                        let vinv_diff = v.subcols(0, k_x) * st_inv_ut_diff.as_ref();
                        let h: f64 = diff_beta.transpose() * vinv_diff.as_ref().col(0).as_ref();
                        (h.max(0.0), rank)
                    }
                } else {
                    (0.0, 0)
                };
                let chi2_h = ChiSquared::new(h_df as f64).ok();
                let p_val = chi2_h.map(|c| 1.0 - c.cdf(h_stat)).unwrap_or(f64::NAN);
                Some(HausmanTest {
                    stat: h_stat,
                    p_value: p_val,
                    df: h_df,
                })
            } else {
                None
            };

            // Durbin-Wu-Hausman (estat endogenous): D = num/(û'ₑ ûₑ/N), WH = (num/p1)/(denom/(N-k1-p-p1))
            // ûₗ = u_structural, ûₑ = u_ols; P_Z = Z(Z'Z)^{-1}Z'; P_{ZY1} = [Z Y1]([Z Y1]'[Z Y1])^{-1}[Z Y1]'
            // Testing all endog: Y1 = Y, [Z Y1] = [Z endog_reg]
            let endogenous = if sigma2_ols > 1e-300 && (u_ols.transpose() * u_ols.as_ref()) > 1e-300
            {
                let p1 = k_endog;
                let k1 = if self.config.constant {
                    k_exog + 1
                } else {
                    k_exog
                };
                let wudf_denom = n
                    .saturating_sub(k1)
                    .saturating_sub(k_endog)
                    .saturating_sub(p1);

                // Build [Z Y1] = [Z, endog_reg] = [exog, instruments, endog_reg] with constant
                let mut zy1_raw = Vec::with_capacity(n * (k_z + k_endog));
                for i in 0..n {
                    for j in 0..k_z {
                        zy1_raw.push(z[(i, j)]);
                    }
                    for j in 0..k_endog {
                        zy1_raw.push(self.endog_reg[(i, j)]);
                    }
                }
                let zy1 =
                    faer::MatRef::from_row_major_slice(&(zy1_raw), n, k_z + k_endog).to_owned();
                let zy1_matrix = zy1.as_ref().to_owned();
                let zy1t_zy1 = zy1_matrix.transpose() * zy1_matrix.as_ref();
                let zy1t_zy1_inv: Option<faer::Mat<f64>> = zy1t_zy1
                    .checked_cholesky()
                    .ok()
                    .map(|llt| llt.solve(&Mat::identity(zy1t_zy1.nrows(), zy1t_zy1.nrows())));

                let (num, u_ols_sq) = if let Some(zy1_inv) = zy1t_zy1_inv {
                    let zy1_inv_nd = zy1_inv.as_ref().to_owned();
                    let p_zy1_u_ols = zy1.as_ref()
                        * (zy1_inv_nd.as_ref() * (zy1.transpose() * u_ols.as_ref()).as_ref())
                            .as_ref();
                    let p_z_u_iv = z.as_ref()
                        * (ztz_inv_nd.as_ref() * (z.transpose() * u_structural.as_ref()).as_ref())
                            .as_ref();
                    let num = (u_ols.transpose() * p_zy1_u_ols.as_ref())
                        - (u_structural.transpose() * p_z_u_iv.as_ref());
                    let u_ols_sq = u_ols.transpose() * u_ols.as_ref();
                    (num, u_ols_sq)
                } else {
                    (0.0, (u_ols.transpose() * u_ols.as_ref()))
                };

                let denom = u_ols_sq - num;
                let durbin_stat: f64 = if u_ols_sq > 1e-300 {
                    n as f64 * num / u_ols_sq
                } else {
                    0.0
                };
                let durbin_stat = durbin_stat.max(0.0);
                let chi2_d = ChiSquared::new(p1 as f64).ok();
                let durbin_p = chi2_d.map(|c| 1.0 - c.cdf(durbin_stat)).unwrap_or(f64::NAN);

                let wu_stat: f64 = if wudf_denom > 0 && denom > 1e-300 {
                    ((num / p1 as f64) / (denom / wudf_denom as f64)).max(0.0)
                } else {
                    0.0
                };
                let f_dist = FisherSnedecor::new(p1 as f64, wudf_denom as f64).ok();
                let wu_p = f_dist.map(|f| 1.0 - f.cdf(wu_stat)).unwrap_or(f64::NAN);

                Some(EndogenousTest {
                    durbin_stat,
                    durbin_p_value: durbin_p,
                    wu_stat,
                    wu_p_value: wu_p,
                    df: p1,
                    wu_df_denom: wudf_denom,
                })
            } else {
                None
            };

            (hausman, endogenous)
        } else {
            (None, None)
        };

        Ok(IV2SLSResult {
            num_observation: n,
            ss_model,
            ss_residual,
            ss_total,
            df_model,
            df_residual,
            df_total,
            ms_model,
            ms_residual,
            ms_total,
            covariance_type,
            r2,
            r2_adjusted,
            wald_chi2,
            wald_chi2_p_value: wald_p,
            model: IV2SLSModel {
                params: betas_nd.clone(),
            },
            betas: betas_nd,
            stds: std_err,
            zvalues: (z_values).into_iter().collect::<Col<f64>>(),
            pvalues: (p_values).into_iter().collect::<Col<f64>>(),
            conf_int_left: ci_lower,
            conf_int_right: ci_upper,
            cov_beta,
            cond_no,
            first_stage,
            first_stage_summary,
            overid,
            overid_k_iv: k_iv,
            overid_k_endog: k_endog,
            hausman,
            endogenous,
        })
    }
}
