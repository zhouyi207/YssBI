/// Stata `var …, exog()`：在时刻 `t` 参与估计当且仅当 `y[t]`、每个 `y[t−lag]` 以及（若有）`exog[t]` 均有限。
/// 第一期外生缺失不删行：仍可作为 `t≥p` 时的滞后，只要当期 `exog[t]` 完整。
pub fn var_regression_times_stata(
    y: &Array2<f64>,
    lags: &[usize],
    exog: Option<&Array2<f64>>,
) -> Result<Vec<usize>, String> {
    if lags.is_empty() {
        return Err("VAR: lags cannot be empty".to_string());
    }
    let t = y.nrows();
    let k = y.ncols();
    let p_model = *lags.iter().max().expect("lags non-empty");
    if let Some(ex) = exog {
        if ex.nrows() != t {
            return Err(format!(
                "VAR: exog has {} rows, expected {} (must match Y)",
                ex.nrows(),
                t
            ));
        }
    }
    let mut out = Vec::new();
    for row_t in p_model..t {
        let mut ok = true;
        for j in 0..k {
            if !y[[row_t, j]].is_finite() {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }
        for &lag in lags {
            let lr = row_t - lag;
            for j in 0..k {
                if !y[[lr, j]].is_finite() {
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
        if !ok {
            continue;
        }
        if let Some(ex) = exog {
            for j in 0..ex.ncols() {
                if !ex[[row_t, j]].is_finite() {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            out.push(row_t);
        }
    }
    if out.is_empty() {
        return Err("VAR: no valid regression periods (check y / exog for missing)".to_string());
    }
    Ok(out)
}

