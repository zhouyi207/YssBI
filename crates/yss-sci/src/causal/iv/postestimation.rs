use super::estimate::{coefficient_inference, goodness_of_fit};
use super::first_stage::{compute_first_stage_summary, is_robust_covariance};
use super::{
    design::{PreparedIvDesign, prepare_instruments, project_endogenous, regressor_design},
    model::IvModel,
};
use crate::regression::covariance::{compute_cov_beta, score_covariance};
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor};
use yss_sci_contract::causal::iv::{
    EndogenousTest, FirstStageResult, FirstStageSummary, HausmanTest, InstrumentalVariableFit,
    LimlOveridTest, OveridTest,
};
use yss_sci_linalg::{Col, ColRef, Mat, MatrixExt, Solve, Svd};

impl IvModel {
    pub fn first_stage(
        &self,
        for_liml: bool,
    ) -> Result<(Vec<FirstStageResult>, FirstStageSummary), String> {
        let n = self.endog.nrows();
        let k_endog = self.endog_reg.ncols();
        let k_z = self.exog.ncols() + self.instruments.ncols() + usize::from(self.options.constant);
        let df_z = n
            .checked_sub(k_z)
            .filter(|&degrees| degrees > 0)
            .ok_or("IV firststage: insufficient residual degrees of freedom")?;
        let design = self.design()?;
        let mut first_stage: Vec<FirstStageResult> = Vec::with_capacity(k_endog);
        for j in 0..k_endog {
            let endog_col = self.endog_reg.col(j);
            let gamma = design.first_stage_coefficients.col(j);

            let resid = endog_col - design.endog_hat.col(j);
            let (r2, r2_adj) =
                goodness_of_fit(endog_col, resid.as_ref(), self.options.constant, df_z)?;
            let cov_gamma = compute_cov_beta(
                &design.z,
                &design.ztz_inverse,
                &resid,
                df_z,
                self.options.constant.then_some(0),
                &self.options.covariance,
            )?;
            // First-stage equations are OLS regressions regardless of structural `small`.
            let inference = coefficient_inference(gamma, &cov_gamma, df_z, true)?;

            let name = format!("endog_{}", j + 1);
            let var_names = (0..k_z).map(|i| format!("z{}", i + 1)).collect();
            first_stage.push(FirstStageResult {
                endog_name: name,
                var_names,
                betas: gamma.iter().copied().collect(),
                inference,
                df_residual: df_z,
                r2,
                r2_adjusted: r2_adj,
            });
        }

        // estat firststage: First-stage regression summary statistics
        let first_stage_summary =
            compute_first_stage_summary(self, &design, &first_stage, for_liml)?;

        Ok((first_stage, first_stage_summary))
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
        let covariance = &self.options.covariance;
        let u_structural: Col<f64> = &self.endog - &(x_struct.as_ref() * betas_nd.as_ref());
        let xtx = x.transpose() * x.as_ref();
        let xtx_inv_nd = xtx
            .checked_cholesky()
            .map_err(|_| "IV endogeneity: singular projected design")?
            .solve(&Mat::identity(k_x, k_x));
        // Hausman tests (traditional + Durbin-Wu-Hausman): only for nonrobust VCE
        let (hausman, endogenous) = if !is_robust_covariance(covariance) {
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
                ChiSquared::new(h_df as f64)
                    .ok()
                    .map(|distribution| HausmanTest {
                        stat: h_stat,
                        p_value: distribution.sf(h_stat),
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
                let k1 = if self.options.constant {
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
                let durbin_p = chi2_d.map(|c| c.sf(durbin_stat)).unwrap_or(f64::NAN);

                let wu_stat: f64 = if wudf_denom > 0 && denom > 1e-300 {
                    ((num / p1 as f64) / (denom / wudf_denom as f64)).max(0.0)
                } else {
                    0.0
                };
                FisherSnedecor::new(p1 as f64, wudf_denom as f64)
                    .ok()
                    .map(|distribution| EndogenousTest {
                        durbin_stat,
                        durbin_p_value: durbin_p,
                        wu_stat,
                        wu_p_value: crate::distribution::fisher_snedecor_sf(&distribution, wu_stat),
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
}

pub(super) fn overidentification(
    fit: &InstrumentalVariableFit,
) -> Result<Option<OveridTest>, String> {
    let data = &fit.design;
    let df = data.instruments.len() - data.endogenous.len();
    if df == 0 {
        return Ok(None);
    }
    // Both auxiliary regressions are invariant to a common structural-residual scale.
    let scale = fit.residuals.iter().map(|u| u.abs()).fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok(None);
    }
    let n = fit.residuals.len();
    let residuals = Col::from_fn(n, |row| fit.residuals[row] / scale);
    let columns = data
        .exogenous
        .iter()
        .chain(&data.instruments)
        .map(|values| ColRef::from_slice(values))
        .collect::<Vec<_>>();
    let (z, inverse) = prepare_instruments(n, &columns, fit.options.constant)?;
    let distribution =
        ChiSquared::new(df as f64).map_err(|error| format!("IV2SLS overid ChiSquared: {error}"))?;
    if is_robust_covariance(&fit.options.covariance) {
        let statistic = wooldridge_score(fit, &z, &inverse, residuals.as_ref())?;
        return Ok(Some(OveridTest {
            test_type: "wooldridge".into(),
            sargan_stat: None,
            sargan_p_value: None,
            basmann_stat: None,
            basmann_p_value: None,
            wooldridge_stat: Some(statistic),
            wooldridge_p_value: Some(distribution.sf(statistic)),
            df,
        }));
    }
    let ztu = z.transpose() * residuals.as_ref();
    let delta = inverse.as_ref() * ztu.as_ref();
    let fitted = z.as_ref() * delta.as_ref();
    let auxiliary_residuals = &residuals - &fitted;
    let total = residuals.transpose() * residuals.as_ref();
    let explained = fitted.transpose() * fitted.as_ref();
    let remaining = auxiliary_residuals.transpose() * auxiliary_residuals.as_ref();
    let sargan = n as f64 * (explained / total);
    if !sargan.is_finite() || !remaining.is_finite() {
        return Err("IV2SLS overid: auxiliary regression is undefined".into());
    }
    // This equivalent Basmann ratio avoids cancellation in N-S near perfect projection.
    let basmann = n
        .checked_sub(z.ncols())
        .filter(|&degrees| degrees > 0)
        .filter(|_| remaining > 0.0)
        .map(|degrees| degrees as f64 * (explained / remaining));
    if basmann.is_some_and(|statistic| !statistic.is_finite()) {
        return Err("IV2SLS overid: Basmann statistic is undefined".into());
    }
    Ok(Some(OveridTest {
        test_type: "sargan_basmann".into(),
        sargan_stat: Some(sargan),
        sargan_p_value: Some(distribution.sf(sargan)),
        basmann_stat: basmann,
        basmann_p_value: basmann.map(|statistic| distribution.sf(statistic)),
        wooldridge_stat: None,
        wooldridge_p_value: None,
        df,
    }))
}

fn wooldridge_score(
    fit: &InstrumentalVariableFit,
    z: &Mat<f64>,
    inverse: &Mat<f64>,
    residuals: ColRef<'_, f64>,
) -> Result<f64, String> {
    let columns = fit
        .design
        .endogenous
        .iter()
        .map(|values| ColRef::from_slice(values))
        .collect::<Vec<_>>();
    let (_, projected) = project_endogenous(z, inverse, &columns);
    let included = fit.design.exogenous.len() + usize::from(fit.options.constant);
    let w = regressor_design(z.subcols(0, included), projected.as_ref());
    let restrictions = fit.design.instruments.len() - fit.design.endogenous.len();
    // A restriction direction Z*c must be orthogonal to the nuisance design W.
    // Decompose W'Z in parameter coordinates; its right nullspace spans every
    // overidentifying restriction without observation-square factors or a tall SVD.
    let orthogonality = w.transpose() * z.as_ref();
    let basis = Svd::factor(orthogonality.as_ref())
        .map_err(|_| "IV2SLS Wooldridge overid: restriction decomposition failed")?;
    if basis.values().nrows() < w.ncols()
        || !basis.values()[w.ncols() - 1].is_finite()
        || basis.values()[w.ncols() - 1] <= 0.0
    {
        return Err("IV2SLS Wooldridge overid: nuisance design is undefined".into());
    }
    let directions = basis.right_vectors().subcols(w.ncols(), restrictions);
    let mut scores = z.as_ref() * directions;
    for row in 0..scores.nrows() {
        for j in 0..restrictions {
            scores[(row, j)] *= residuals[row];
        }
    }
    let cross = score_covariance(&scores, &fit.options.covariance)?;
    let sums = Col::from_fn(restrictions, |j| scores.col(j).iter().sum());
    let theta = cross
        .checked_cholesky()
        .map_err(|_| "IV2SLS Wooldridge overid: score covariance not positive definite")?
        .solve(&sums);
    // The score quadratic equals N-RSS for independent errors and uses the
    // retained group/serial covariance for dependent observations.
    let statistic = sums.transpose() * theta.as_ref();
    if !statistic.is_finite() || statistic < 0.0 {
        return Err("IV2SLS Wooldridge overid: statistic is undefined".into());
    }
    Ok(statistic)
}

pub(super) fn liml_overidentification(
    fit: &InstrumentalVariableFit,
) -> Result<Option<LimlOveridTest>, String> {
    let k_iv = fit.design.instruments.len();
    let k_endog = fit.design.endogenous.len();
    if k_iv <= k_endog || is_robust_covariance(&fit.options.covariance) {
        return Ok(None);
    }
    let n = fit.residuals.len();
    let k_z = fit.design.exogenous.len() + k_iv + usize::from(fit.options.constant);
    let Some(df_denom) = n.checked_sub(k_z).filter(|&degrees| degrees > 0) else {
        return Ok(None);
    };
    let excess_kappa = fit.statistics.kappa - 1.0;
    if !excess_kappa.is_finite() || excess_kappa < 0.0 {
        return Err("IVLIML overid: fitted kappa is invalid".into());
    }
    let df = k_iv - k_endog;
    let anderson_rubin_stat = n as f64 * excess_kappa;
    let basmann_stat = excess_kappa * (df_denom as f64 / df as f64);
    if !anderson_rubin_stat.is_finite() || !basmann_stat.is_finite() {
        return Err("IVLIML overid: statistic is undefined".into());
    }
    let chi2 =
        ChiSquared::new(df as f64).map_err(|error| format!("IVLIML overid ChiSquared: {error}"))?;
    let f = FisherSnedecor::new(df as f64, df_denom as f64)
        .map_err(|error| format!("IVLIML overid FisherSnedecor: {error}"))?;
    Ok(Some(LimlOveridTest {
        anderson_rubin_stat,
        anderson_rubin_p_value: chi2.sf(anderson_rubin_stat),
        basmann_stat,
        basmann_p_value: crate::distribution::fisher_snedecor_sf(&f, basmann_stat),
        df,
        df_denom,
    }))
}
