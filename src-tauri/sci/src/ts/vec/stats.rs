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
        let p_chi2 = chi_squared_sf(n_free as f64, chi2);
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
    for j in 0..r {
        let a_inv_jj = a_inv.as_ref()[(j, j)].max(1e-300);
        for (ii, i) in (r..k).enumerate() {
            let coef = beta[[i, j]];
            let var_ii = scale * a_inv_jj * b_inv_nd[[ii, ii]].max(0.0);
            let se = var_ii.sqrt().max(1e-300);
            let z = coef / se;
            let p = 2.0 * (1.0 - normal_cdf(z.abs()));
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
