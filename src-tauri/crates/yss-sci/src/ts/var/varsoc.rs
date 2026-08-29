/// Stata `varsoc varlist, maxlag(P)`：Lag 行 **0…P**（0 = 仅截距的 VAR(0)）；各阶共用 `T−P` 个观测。
/// `LR(j)=2(LL(j)−LL(j−1))`（`j≥1`），`df=K²`；Lag 0 无 LR（与 Stata 表一致）。
pub fn var_varsoc(
    y: Array2<f64>,
    maxlag: usize,
    var_names: Option<Vec<String>>,
) -> Result<VARSocResult, String> {
    if maxlag < 1 {
        return Err("varsoc: maxlag must be >= 1".to_string());
    }
    let (t, k) = (y.nrows(), y.ncols());
    if k < 1 {
        return Err("varsoc: need at least one endogenous variable".to_string());
    }
    if t <= maxlag {
        return Err(format!(
            "varsoc: need T > maxlag ({}), got T={}",
            maxlag, t
        ));
    }

    let n_obs = t - maxlag;
    let mut rows = Vec::with_capacity(maxlag + 1);
    let mut prev_ll: Option<f64> = None;

    for p in 0..=maxlag {
        let lags: Vec<usize> = if p == 0 {
            Vec::new()
        } else {
            (1..=p).collect()
        };
        let var = VAR {
            y: y.clone(),
            exog: None,
            config: VARConfig {
                constant: true,
                lags,
                step: 1,
                dfk: false,
                mlag: 1,
                sample_start_offset: Some(maxlag),
                skip_extras: true,
            },
            var_names: var_names.clone(),
            exog_names: None,
            regression_times: None,
        };
        let r = var.fit()?;

        let (lr, lr_df, lr_p) = if p == 0 {
            (None, None, None)
        } else {
            let ll_prev = prev_ll.ok_or("varsoc: internal prev_ll")?;
            let lr_stat = 2.0 * (r.log_likelihood - ll_prev);
            let df = k * k;
            let pval =
                chi_squared_sf(df as f64, lr_stat);
            (Some(lr_stat), Some(df), Some(pval))
        };
        prev_ll = Some(r.log_likelihood);

        rows.push(VARSocRow {
            lag: p,
            log_likelihood: r.log_likelihood,
            lr,
            lr_df: lr_df,
            lr_p: lr_p,
            fpe: r.fpe,
            aic: r.aic,
            hqic: r.hqic,
            sbic: r.sbic,
        });
    }

    let names = var_names.unwrap_or_else(|| (0..k).map(|i| format!("y{}", i)).collect());
    Ok(VARSocResult {
        title: "VAR lag-order selection (varsoc)".to_string(),
        var_names: names,
        maxlag,
        num_observation: n_obs,
        rows,
    })
}
