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
