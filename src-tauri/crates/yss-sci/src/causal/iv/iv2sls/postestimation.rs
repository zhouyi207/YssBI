use super::first_stage::{compute_first_stage_summary, is_robust_cov_type};
use super::{design::PreparedIvDesign, types::*};
use statrs::{
    distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve};

impl IV2SLS {
    pub fn first_stage(
        &self,
        for_liml: bool,
    ) -> Result<(Vec<FirstStageResult>, FirstStageSummary), String> {
        let n = self.endog.nrows();
        let k_endog = self.endog_reg.ncols();
        let PreparedIvDesign {
            z,
            ztz_inverse: ztz_inv,
            mut endog_hat,
            ..
        } = self.design()?;
        let k_z = z.ncols();
        let df_z = n.saturating_sub(k_z);
        let z_matrix = z.as_ref().to_owned();
        let ztz_inv_nd = ztz_inv.as_ref().to_owned();
        let covariance_type = &self.config.cov_type;
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
            let cov_gamma = yss_sci_linalg::Scale(sigma2) * &ztz_inv_nd;
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

        // estat firststage: First-stage regression summary statistics
        let first_stage_summary = compute_first_stage_summary(
            &z,
            &endog_hat,
            &self.endog_reg,
            &self.exog,
            &self.instruments,
            crate::causal::iv::iv2sls::FirstStageOptions {
                has_constant: self.config.constant,
                cov_type: covariance_type,
                cov_params: self.config.cov_params.as_ref(),
                small: self.config.small,
                for_liml,
            },
        )?;

        Ok((first_stage, first_stage_summary))
    }

    pub fn overidentification(&self, betas: &Col<f64>) -> Result<Option<OveridTest>, String> {
        let n = self.endog.nrows();
        let k_iv = self.instruments.ncols();
        let k_endog = self.endog_reg.ncols();
        let PreparedIvDesign {
            z,
            ztz_inverse: ztz_inv_nd,
            x,
            x_struct,
            ..
        } = self.design()?;
        let k_z = z.ncols();
        let covariance_type = &self.config.cov_type;
        let u_structural: Col<f64> = &self.endog - &(x_struct.as_ref() * betas.as_ref());
        // Overidentification test (estat overid): Sargan/Basmann (homoskedastic) or Wooldridge (1995) robust score (robust VCE).
        // Stata: "If you used the 2SLS estimator and requested a robust VCE, Wooldridge's robust score test of
        // overidentifying restrictions is performed instead; without a robust VCE, Wooldridge's test statistic is identical to Sargan's."
        let overid = if k_iv > k_endog {
            let df_overid = k_iv - k_endog;
            let chi2_dist = ChiSquared::new(df_overid as f64)
                .map_err(|e| format!("IV2SLS overid ChiSquared: {}", e))?;

            let is_robust = is_robust_cov_type(covariance_type);

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

        Ok(overid)
    }

    pub fn endogeneity(
        &self,
        betas_nd: &Col<f64>,
    ) -> Result<(Option<HausmanTest>, Option<EndogenousTest>), String> {
        let n = self.endog.nrows();
        let k_exog = self.exog.ncols();
        let k_endog = self.endog_reg.ncols();
        let PreparedIvDesign {
            z,
            ztz_inverse: ztz_inv_nd,
            x,
            x_struct,
            ..
        } = self.design()?;
        let k_z = z.ncols();
        let k_x = x.ncols();
        let df_residual = n.saturating_sub(k_x);
        let covariance_type = &self.config.cov_type;
        let u_structural: Col<f64> = &self.endog - &(x_struct.as_ref() * betas_nd.as_ref());
        let xtx = x.transpose() * x.as_ref();
        let xtx_inv_nd = xtx
            .checked_cholesky()
            .map_err(|_| "IV endogeneity: singular projected design")?
            .solve(&Mat::identity(k_x, k_x));
        // Hausman tests (traditional + Durbin-Wu-Hausman): only for nonrobust VCE
        let (hausman, endogenous) = if !is_robust_cov_type(covariance_type) {
            // OLS on y ~ X_struct (treating endog as exogenous): β_ols, u_ols
            let x_struct_tx = x_struct.transpose() * x_struct.as_ref();
            let x_struct_tx_inv: Option<yss_sci_linalg::Mat<f64>> = x_struct_tx
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
                let v_iv = yss_sci_linalg::Scale(sigma2_ols) * &xtx_inv_nd; // X̂'X̂ from stage 2
                let v_ols = sigma2_ols * &xtx_struct_inv_nd;
                let v_diff: Mat<f64> = &v_iv - &v_ols;
                let diff_beta = betas_nd - &beta_ols;
                let v_diff_matrix = v_diff.as_ref().to_owned();
                let svd = yss_sci_linalg::Svd::factor(v_diff_matrix.as_ref()).ok();
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
                    yss_sci_linalg::MatRef::from_row_major_slice(&(zy1_raw), n, k_z + k_endog)
                        .to_owned();
                let zy1_matrix = zy1.as_ref().to_owned();
                let zy1t_zy1 = zy1_matrix.transpose() * zy1_matrix.as_ref();
                let zy1t_zy1_inv: Option<yss_sci_linalg::Mat<f64>> = zy1t_zy1
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

        Ok((hausman, endogenous))
    }
    pub fn liml_overidentification(
        &self,
        betas: &Col<f64>,
    ) -> Result<Option<yss_sci_contract::causal::iv::LimlOveridTest>, String> {
        use yss_sci_contract::causal::iv::LimlOveridTest;
        let n = self.endog.nrows();
        let k_iv = self.instruments.ncols();
        let k_endog = self.endog_reg.ncols();
        let PreparedIvDesign {
            z,
            ztz_inverse: ztz_inv_nd,
            x_struct,
            ..
        } = self.design()?;
        let k_z = z.ncols();
        let covariance_type = &self.config.cov_type;
        let u_structural: Col<f64> = &self.endog - &(x_struct.as_ref() * betas.as_ref());
        // Overidentification test (estat overid): Anderson-Rubin chi2, Basmann F.
        // Only when nonrobust VCE. With robust (vce(robust)), Stata does not compute overid.
        let overid = if k_iv > k_endog && !is_robust_cov_type(covariance_type) {
            let df_overid = k_iv - k_endog;
            let df_denom = n.saturating_sub(k_z);
            let uu = u_structural.transpose() * u_structural.as_ref();
            if df_denom > 0 && uu > 1e-300 {
                let ztu = z.transpose() * u_structural.as_ref();
                let ztz_inv_ztu = ztz_inv_nd.as_ref() * ztu.as_ref();
                let u_pz_u = ztu.transpose() * ztz_inv_ztu.as_ref();
                let sargan_stat = n as f64 * u_pz_u / uu;
                let basmann_chi2 = if (n as f64 - sargan_stat).abs() > 1e-10 {
                    sargan_stat * (n as f64 - k_z as f64) / (n as f64 - sargan_stat)
                } else {
                    sargan_stat
                };
                let chi2_dist = ChiSquared::new(df_overid as f64)
                    .map_err(|e| format!("IVLIML overid ChiSquared: {}", e))?;
                let ar_p = 1.0 - chi2_dist.cdf(sargan_stat);
                let basmann_f_stat = basmann_chi2 / (df_overid as f64);
                let f_dist = FisherSnedecor::new(df_overid as f64, df_denom as f64)
                    .map_err(|e| format!("IVLIML overid FisherSnedecor: {}", e))?;
                let basmann_p = 1.0 - f_dist.cdf(basmann_f_stat);

                Some(LimlOveridTest {
                    anderson_rubin_stat: sargan_stat,
                    anderson_rubin_p_value: ar_p,
                    basmann_stat: basmann_f_stat,
                    basmann_p_value: basmann_p,
                    df: df_overid,
                    df_denom,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(overid)
    }
}
