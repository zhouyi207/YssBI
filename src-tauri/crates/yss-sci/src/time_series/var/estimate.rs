impl VAR {
    pub fn fit(&self) -> Result<VarFit, String> {
        let y = &self.y;
        let (t, k) = (y.nrows(), y.ncols());
        let lags = &self.config.lags;
        let constant = self.config.constant;

        let p_model = lags.iter().copied().max().unwrap_or(0);
        let sample_start = self.config.sample_start_offset.unwrap_or(p_model);
        if p_model > sample_start {
            return Err(format!(
                "VAR: sample_start_offset ({}) must be >= max(lag) ({})",
                sample_start, p_model
            ));
        }
        if t <= sample_start {
            return Err(format!(
                "VAR: need T > sample_start ({}), got T={}",
                sample_start, t
            ));
        }

        // 构建 Z: 每行 [y_{t-1}', ..., y_{t-p}', exog_t', 1] 展平
        let n_lag_coefs = k * lags.len();
        let n_exog = self.exog.as_ref().map(|x| x.ncols()).unwrap_or(0);
        let n_z = n_lag_coefs + n_exog + if constant { 1 } else { 0 };

        if let Some(ref exog) = self.exog
            && exog.nrows() != t
        {
            return Err(format!(
                "VAR: exog has {} rows, expected {} (must match Y length)",
                exog.nrows(),
                t
            ));
        }

        let row_indices: Vec<usize> = if let Some(ref rt) = self.regression_times {
            if rt.is_empty() {
                return Err("VAR: regression_times is empty".to_string());
            }
            for &row_t in rt {
                if row_t < sample_start {
                    return Err(format!(
                        "VAR: regression_times contains {} < sample_start ({})",
                        row_t, sample_start
                    ));
                }
                if row_t >= t {
                    return Err(format!(
                        "VAR: regression_times contains {} >= T ({})",
                        row_t, t
                    ));
                }
            }
            rt.clone()
        } else {
            (sample_start..t).collect()
        };
        let n_obs = row_indices.len();

        let mut z = Mat::zeros(n_obs, n_z);
        let mut y_dep = Mat::zeros(n_obs, k);

        for (i, &row_t) in row_indices.iter().enumerate() {
            let mut col_z = 0;
            // 先遍历每个 y，再遍历其 L1, L2, ... → L1.y1, L2.y1; L1.y2, L2.y2; ...
            for j in 0..k {
                for &lag in lags.iter() {
                    let lag_row = row_t - lag;
                    z[(i, col_z)] = y[(lag_row, j)];
                    col_z += 1;
                }
            }
            if let Some(ref exog) = self.exog {
                for j in 0..exog.ncols() {
                    z[(i, col_z)] = exog[(row_t, j)];
                    col_z += 1;
                }
            }
            if constant {
                z[(i, col_z)] = 1.0;
            }
            for j in 0..k {
                y_dep[(i, j)] = y[(row_t, j)];
            }
        }

        // 每方程 OLS
        let z_matrix = z.as_ref().to_owned();
        let zt = z_matrix.transpose();
        let ztz = zt.as_ref() * z_matrix.as_ref();
        let ztz_inv = ztz
            .checked_cholesky()
            .map_err(|_| "VAR: Z'Z not positive definite (check collinearity)".to_string())?
            .solve(&Mat::<f64>::identity(ztz.nrows(), ztz.nrows()));

        let mut coefficients = Vec::with_capacity(k);
        let mut std_errs = Vec::with_capacity(k);
        let mut residuals = Vec::with_capacity(k);
        let mut ss_residual = Vec::with_capacity(k);
        let mut ss_total = Vec::with_capacity(k);
        let mut coef_labels = Vec::with_capacity(k);
        let mut cov_beta = Vec::with_capacity(k);

        for eq in 0..k {
            let y_col = y_dep.col(eq).to_owned();
            let y_vector = y_col.as_ref().to_owned();
            let zty = zt.as_ref() * y_vector.as_ref();
            let beta = ztz_inv.as_ref() * zty.as_ref();
            let y_hat = z_matrix.as_ref() * beta.as_ref();
            let u = y_vector.as_ref() - y_hat.as_ref();

            let beta_nd = beta.as_ref().to_owned();
            let u_nd = u.as_ref().to_owned();

            let ss_r: f64 = u_nd.iter().map(|x| x * x).sum();
            let y_mean = y_col.iter().sum::<f64>() / (y_col.nrows() as f64).max(1.0);
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
            let xtx_inv_nd = ztz_inv.as_ref().to_owned();
            let cov_eq = yss_sci_linalg::Scale(sigma2_eq) * &xtx_inv_nd;
            cov_beta.push(cov_eq.clone());
            let se: Col<f64> = Col::from_fn(cov_eq.nrows().min(cov_eq.ncols()), |i| {
                cov_eq[(i, i)].sqrt()
            });

            let mut labels = Vec::with_capacity(n_z);
            let names = self.var_names.as_deref().unwrap_or(&[]);
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

            coefficients.push(beta_nd.iter().copied().collect::<Vec<_>>());
            std_errs.push(se.iter().copied().collect::<Vec<_>>());
            residuals.push(u_nd.iter().copied().collect::<Vec<_>>());
            ss_residual.push(ss_r);
            ss_total.push(ss_t);
            coef_labels.push(labels);
        }

        // 残差矩阵 U (n_obs × K)
        let u_mat = Mat::from_fn(n_obs, k, |i, j| residuals[j][i]);
        let te = if self.config.dfk {
            let m = n_z as f64;
            (n_obs as f64 - m).max(1.0)
        } else {
            n_obs as f64
        };
        let sigma = (u_mat.transpose() * u_mat.as_ref()) / yss_sci_linalg::Scale(te);
        if n_obs <= n_z {
            return Err("VAR: insufficient residual degrees of freedom".into());
        }
        let df_r = n_obs - n_z;

        // Cholesky: Σ = GG', G lower triangular (in-place on sigma copy)
        let mut g_nd = sigma.clone();
        cholesky_lower_in_place(&mut g_nd)
            .map_err(|_| "VAR: Sigma not positive definite for Cholesky".to_string())?;

        // det(Σ) = det(G)^2, G lower triangular => det(G) = prod(diag(G))
        let det_g: f64 = (0..k).map(|i| g_nd[(i, i)]).product();
        let det_sigma = (det_g * det_g).abs();

        // Log likelihood, AIC, etc.
        let det_sigma_ml = if det_sigma > 1e-300 {
            det_sigma
        } else {
            1e-300
        };
        let ll = -0.5
            * (n_obs as f64)
            * (k as f64 * (2.0 * std::f64::consts::PI).ln() + det_sigma_ml.ln() + k as f64);
        let n_parms = (k * n_z) as f64;
        let aic = -2.0 * ll / (n_obs as f64) + 2.0 * n_parms / (n_obs as f64);
        let fpe =
            det_sigma_ml * ((n_obs as f64 + n_parms) / (n_obs as f64 - n_parms)).powi(k as i32);
        let hqic =
            -2.0 * ll / (n_obs as f64) + 2.0 * n_parms * (n_obs as f64).ln().ln() / (n_obs as f64);
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
            let p_chi2 = chi_squared_sf(n_z as f64, chi2);

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
                let p_val = normal_two_sided_p(z_val);
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

            let eq_name = var_names
                .get(eq)
                .cloned()
                .unwrap_or_else(|| format!("eq{}", eq));
            equations.push(EquationStatistics {
                eq_name,
                parms: n_z,
                rmse,
                r_sq,
                chi2,
                p_chi2,
            });
        }

        let inference = (0..k)
            .map(|eq| RegressionCoefficientStatistics {
                covariance: covariance_rows(&cov_beta[eq]),
                standard_errors: std::mem::take(&mut std_errs[eq]),
                statistic_values: std::mem::take(&mut z_values[eq]),
                p_values: std::mem::take(&mut p_values[eq]),
                confidence_interval_lower: std::mem::take(&mut ci_lower[eq]),
                confidence_interval_upper: std::mem::take(&mut ci_upper[eq]),
            })
            .collect();
        Ok(VarFit {
            var_names,
            lags: lags.clone(),
            constant,
            dfk: self.config.dfk,
            exogenous_names: self.exog_names.clone().unwrap_or_else(||(0..n_exog).map(|j|format!("exog{j}")).collect()),
            sample_rows: row_indices,
            coefficients,
            residuals,
            design: covariance_rows(&z),
            sigma: covariance_rows(&sigma),
            statistics: MultivariateStatistics {
                observations: n_obs,
                log_likelihood: ll,
                aic,
                hqic,
                sbic,
                det_sigma_ml,
                equations,
                coefficients: inference,
                coefficient_labels: coef_labels,
            },
            fpe,
        })
    }
}
