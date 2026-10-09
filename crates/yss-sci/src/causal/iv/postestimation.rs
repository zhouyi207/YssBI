use super::design::{prepare_instruments, project_endogenous, regressor_design};
use super::first_stage::is_robust_covariance;
use crate::regression::covariance::score_covariance;
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor};
use yss_sci_contract::causal::iv::{
    EndogenousTest, HausmanTest, InstrumentalVariableFit, LimlOveridTest, OveridTest,
};
use yss_sci_linalg::{Col, ColRef, Mat, MatrixExt, Solve, Svd};

pub(super) fn endogeneity(
    fit: &InstrumentalVariableFit,
) -> Result<(Option<HausmanTest>, Option<EndogenousTest>), String> {
    if is_robust_covariance(&fit.options.covariance) {
        return Ok((None, None));
    }
    let n = fit.residuals.len();
    let included = fit.design.exogenous.len() + usize::from(fit.options.constant);
    let p = fit.design.endogenous.len();
    let k = included + p;
    let scale = fit
        .residuals
        .iter()
        .fold(0.0_f64, |largest, u| largest.max(u.abs()));
    if scale == 0.0 {
        return Ok((None, None));
    }
    let instrument_columns = fit
        .design
        .exogenous
        .iter()
        .chain(&fit.design.instruments)
        .map(|column| ColRef::from_slice(column))
        .collect::<Vec<_>>();
    let endogenous_columns = fit
        .design
        .endogenous
        .iter()
        .map(|column| ColRef::from_slice(column))
        .collect::<Vec<_>>();
    let (z, inverse) = prepare_instruments(n, &instrument_columns, fit.options.constant)?;
    let (_, mut v) = project_endogenous(z.as_ref(), &inverse, &endogenous_columns);
    let mut x = regressor_design(z.subcols(0, included), &endogenous_columns);
    let column_scales = (0..k)
        .map(|j| {
            x.col(j)
                .iter()
                .fold(0.0_f64, |largest, value| largest.max(value.abs()))
        })
        .collect::<Vec<_>>();
    if column_scales.contains(&0.0) {
        return Err("IV endogeneity: undefined observed design".into());
    }
    for j in 0..k {
        for i in 0..n {
            x[(i, j)] /= column_scales[j];
        }
    }
    let parent_norm = (included..k)
        .map(|j| x.col(j).iter().map(|value| value * value).sum::<f64>())
        .sum::<f64>()
        .sqrt();
    for j in 0..p {
        for i in 0..n {
            v[(i, j)] = (endogenous_columns[j][i] - v[(i, j)]) / column_scales[included + j];
        }
    }
    drop(z);
    drop(inverse);
    let ols = (x.transpose() * x.as_ref())
        .checked_cholesky()
        .map_err(|_| "IV endogeneity: singular observed design")?;
    let mut residuals = Col::from_iter(fit.residuals.iter().map(|u| u / scale));
    // beta_OLS - beta_IV = (X'X)^-1 X'u_IV. Reuse fitted residuals instead
    // of rebuilding response or subtracting fitted coefficients in their units.
    let adjustment = x.as_ref() * ols.solve(&(x.transpose() * residuals.as_ref())).as_ref();
    for i in 0..n {
        residuals[i] -= adjustment[i];
    }
    drop(adjustment);
    let ols_ss = residuals.iter().map(|value| value * value).sum::<f64>();
    if !ols_ss.is_finite() {
        return Err("IV endogeneity: nonfinite OLS residual variation".into());
    }
    if ols_ss == 0.0 {
        return Ok((None, None));
    }
    let projected_controls = x.as_ref() * ols.solve(&(x.transpose() * v.as_ref())).as_ref();
    for j in 0..p {
        for i in 0..n {
            v[(i, j)] -= projected_controls[(i, j)];
        }
    }
    drop(projected_controls);
    drop(x);
    drop(ols);
    let basis = Svd::factor_thin(v.as_ref())
        .map_err(|_| "IV endogeneity: residual direction decomposition failed")?;
    drop(v);
    if basis.values().iter().any(|value| !value.is_finite()) {
        return Err("IV endogeneity: nonfinite residual directions".into());
    }
    // Measure projection rank against the normalized parent columns, so exact
    // first-stage fits do not turn roundoff-sized residuals into test directions.
    let tolerance = n.max(p) as f64 * f64::EPSILON * parent_norm;
    let rank = basis
        .values()
        .iter()
        .filter(|&&value| value > tolerance)
        .count();
    if rank == 0 {
        return Ok((None, None));
    }
    let directions = basis.left_vectors().subcols(0, rank);
    let scores = directions.transpose() * residuals.as_ref();
    let explained = scores.iter().map(|value| value * value).sum::<f64>();
    // The residual-augmentation quadratic equals the sigmamore covariance
    // contrast; its common OLS variance gives H=(n-k)*explained/OLS_RSS.
    let statistic = (n - k) as f64 * explained / ols_ss;
    if !statistic.is_finite() {
        return Err("IV endogeneity: nonfinite Hausman statistic".into());
    }
    let chi = ChiSquared::new(rank as f64).map_err(|_| "IV endogeneity: invalid test rank")?;
    let hausman = Some(HausmanTest {
        stat: statistic,
        p_value: chi.sf(statistic),
        df: rank,
    });
    let endogenous = if rank == p {
        for i in 0..n {
            residuals[i] -= (0..rank)
                .map(|j| directions[(i, j)] * scores[j])
                .sum::<f64>();
        }
        let augmented_ss = residuals.iter().map(|value| value * value).sum::<f64>();
        if !augmented_ss.is_finite() {
            return Err("IV endogeneity: nonfinite augmented residual variation".into());
        }
        let denominator = n.saturating_sub(k + p);
        // A direct residual sum avoids cancellation when the augmentation nearly
        // explains all OLS residuals. Only its projection-error bound is excluded.
        let relative_error = n.max(k + p) as f64 * f64::EPSILON;
        let (wu_stat, wu_p_value) =
            if denominator > 0 && augmented_ss > relative_error.powi(2) * ols_ss {
                let statistic = (explained / p as f64) / (augmented_ss / denominator as f64);
                if !statistic.is_finite() {
                    return Err("IV endogeneity: nonfinite Wu-Hausman statistic".into());
                }
                let distribution = FisherSnedecor::new(p as f64, denominator as f64)
                    .map_err(|_| "IV endogeneity: invalid Wu-Hausman degrees")?;
                (
                    Some(statistic),
                    Some(crate::distribution::fisher_snedecor_sf(
                        &distribution,
                        statistic,
                    )),
                )
            } else {
                (None, None)
            };
        let statistic = n as f64 * explained / ols_ss;
        if !statistic.is_finite() {
            return Err("IV endogeneity: nonfinite Durbin statistic".into());
        }
        Some(EndogenousTest {
            durbin_stat: statistic,
            durbin_p_value: chi.sf(statistic),
            wu_stat,
            wu_p_value,
            df: p,
            wu_df_denom: denominator,
        })
    } else {
        None
    };
    Ok((hausman, endogenous))
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
    let (_, projected) = project_endogenous(z.as_ref(), inverse, &columns);
    let included = fit.design.exogenous.len() + usize::from(fit.options.constant);
    let projected_columns = projected.col_iter().collect::<Vec<_>>();
    let w = regressor_design(z.subcols(0, included), &projected_columns);
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
