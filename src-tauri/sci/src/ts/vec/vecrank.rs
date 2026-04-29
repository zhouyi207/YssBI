/// Johansen 协整秩检验（LR_trace、LR_max、LL(r) 与 Stata [TS] vecrank 公式一致；临界值为 Osterwald–Lenum，与 Stata 打印一致）
pub fn vec_vecrank_stats(
    y: &Array2<f64>,
    lags: usize,
    trend_spec: VecTrendSpec,
    sindicators: Option<&Array2<f64>>,
    show_max_eigen: bool,
    var_names: Option<Vec<String>>,
) -> Result<VecRankResult, String> {
    let k = y.ncols();
    if k < 2 {
        return Err("vecrank: need at least 2 variables".to_string());
    }
    if k > 12 {
        return Err("vecrank: Johansen tables only defined for K <= 12".to_string());
    }
    if lags < 1 {
        return Err("vecrank: lags must be >= 1".to_string());
    }

    let s1 = johansen_stage1(y, lags, trend_spec, sindicators)?;
    let n = s1.n;
    let t = n as f64;
    let evals: Vec<f64> = s1.eval_pairs.iter().map(|(_, v)| *v).collect();
    if evals.len() != k {
        return Err("vecrank: internal eigenvalue count mismatch".to_string());
    }

    let det_order = match trend_spec {
        VecTrendSpec::None => -1,
        VecTrendSpec::Constant => 0,
        VecTrendSpec::Trend => 1,
    };

    let mut s00_chol = s1.s00.clone();
    cholesky_lower_in_place(&mut s00_chol).map_err(|_| "vecrank: S00 not positive definite".to_string())?;
    let ln_det_s00: f64 = 2.0 * (0..k).map(|i| s00_chol[[i, i]].ln()).sum::<f64>();
    let k_bracket = k as f64 * ((2.0 * std::f64::consts::PI).ln() + 1.0);

    let log_1m = |lam: f64| -> f64 {
        let lam = lam.clamp(0.0, 1.0 - 1e-15);
        (1.0 - lam).max(1e-300).ln()
    };

    let mut trace = vec![0.0_f64; k];
    for r in 0..k {
        let s: f64 = (r..k).map(|j| log_1m(evals[j])).sum();
        trace[r] = -t * s;
    }

    let mut maxe = vec![0.0_f64; k];
    for r in 0..k {
        maxe[r] = -t * log_1m(evals[r]);
    }

    let mut sel_tr_95 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = trace_critical_row(dim, det_order) {
            if trace[r] < cv[1] {
                sel_tr_95 = r;
                break;
            }
        }
    }
    let mut sel_tr_99 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = trace_critical_row(dim, det_order) {
            if trace[r] < cv[2] {
                sel_tr_99 = r;
                break;
            }
        }
    }

    let mut sel_mx_95 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = max_eigen_critical_row(dim, det_order) {
            if maxe[r] < cv[1] {
                sel_mx_95 = r;
                break;
            }
        }
    }
    let mut sel_mx_99 = k;
    for r in 0..k {
        let dim = k - r;
        if let Some(cv) = max_eigen_critical_row(dim, det_order) {
            if maxe[r] < cv[2] {
                sel_mx_99 = r;
                break;
            }
        }
    }

    let trend_str = match trend_spec {
        VecTrendSpec::None => "none",
        VecTrendSpec::Constant => "constant",
        VecTrendSpec::Trend => "trend",
    }
    .to_string();

    let names = var_names.unwrap_or_else(|| (0..k).map(|i| format!("y{}", i)).collect());

    let mut rows = Vec::with_capacity(k + 1);
    for rank in 0..=k {
        let sum_r: f64 = (0..rank).map(|j| log_1m(evals[j])).sum();
        let ll_r = -0.5 * t * (k_bracket + ln_det_s00 + sum_r);

        let eigenvalue = if rank >= 1 && rank <= k {
            Some(evals[rank - 1])
        } else {
            None
        };

        let trace_stat = if rank < k {
            Some(trace[rank])
        } else {
            None
        };

        let max_stat = if rank < k {
            Some(maxe[rank])
        } else {
            None
        };

        let (t10, t5, t1) = if rank < k {
            let dim = k - rank;
            trace_critical_row(dim, det_order)
                .map(|cv| (Some(cv[0]), Some(cv[1]), Some(cv[2])))
                .unwrap_or((None, None, None))
        } else {
            (None, None, None)
        };

        let (m10, m5, m1) = if rank < k {
            let dim = k - rank;
            max_eigen_critical_row(dim, det_order)
                .map(|cv| (Some(cv[0]), Some(cv[1]), Some(cv[2])))
                .unwrap_or((None, None, None))
        } else {
            (None, None, None)
        };

        rows.push(VecRankRow {
            rank,
            log_likelihood: ll_r,
            eigenvalue,
            trace_statistic: trace_stat,
            trace_crit_10pct: t10,
            trace_crit_5pct: t5,
            trace_crit_1pct: t1,
            max_eigenvalue_statistic: max_stat,
            max_eigen_crit_10pct: m10,
            max_eigen_crit_5pct: m5,
            max_eigen_crit_1pct: m1,
        });
    }

    Ok(VecRankResult {
        kind: "vecrank".to_string(),
        title: "Johansen tests for cointegration".to_string(),
        var_names: names,
        num_observation: n,
        n_lags: lags,
        trend_spec: trend_str,
        show_max_eigen,
        selected_rank_trace_95: sel_tr_95,
        selected_rank_trace_99: sel_tr_99,
        selected_rank_max_95: sel_mx_95,
        selected_rank_max_99: sel_mx_99,
        rows,
        note: "Trace and max-eigenvalue statistics follow Johansen (1995) and Stata [TS] vecrank. Critical columns are 10% / 5% / 1% significance (right tail). Critical values: Osterwald–Lenum (1992), same digits as Stata vecrank (see johans.ado Case tables); dim=12 uses MacKinnon–Haug–Michelis tail row. If trace/LL differ from Stata but critical values match, check the same sample length (T) and lag order — LR statistics scale with T.".to_string(),
    })
}
