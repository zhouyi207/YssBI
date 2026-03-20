//! Parallel-trends (event-study) Wald test and placebo timing test for Panel DID.
//! Aligns with common Stata practice: `reghdfe Y i.rel#c.treat ..., absorb(id t) cluster(id)` then `test` on pre coeffs.

use faer::{linalg::solvers::Solve, Mat, Side};
use ndarray::{Array1, Array2};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use statrs::distribution::{ChiSquared, ContinuousCDF};
use std::collections::HashSet;
use yss_sci::regression::collinearity::drop_collinear_columns;
use yss_sci::regression::panel::fit_panel_fe_twoway;
use yss_sci::tools::{IntoFaer, IntoNdarray};

/// One event-time coefficient (treat × I(rel = k)) for plotting; reference period is omitted in regression → synthetic zero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidEventStudyPoint {
    pub rel_time: i32,
    pub coef: f64,
    pub std_err: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    #[serde(default)]
    pub is_reference: bool,
}

/// Wald χ² test H0: β = 0 for selected coefficient indices (cluster-robust cov as in Stata `test`).
pub(crate) fn wald_chi2_linear(
    beta: &Array1<f64>,
    cov: &Array2<f64>,
    idx: &[usize],
) -> Option<(f64, usize)> {
    let q = idx.len();
    if q == 0 {
        return None;
    }
    let mut b = vec![0f64; q];
    let mut v = Array2::<f64>::zeros((q, q));
    for (ii, &i) in idx.iter().enumerate() {
        if i >= beta.len() {
            return None;
        }
        b[ii] = beta[i];
        for (jj, &j) in idx.iter().enumerate() {
            if j >= beta.len() {
                return None;
            }
            v[[ii, jj]] = cov[[i, j]];
        }
    }
    let v_faer = v.view().into_faer().to_owned();
    let b_col = Mat::from_fn(q, 1, |r, _| b[r]);
    let solved = v_faer
        .as_ref()
        .llt(Side::Lower)
        .ok()?
        .solve(b_col.as_ref());
    let x_nd = solved.as_ref().into_ndarray();
    let chi2: f64 = b.iter().enumerate().map(|(i, bi)| bi * x_nd[[i, 0]]).sum();
    if !chi2.is_finite() || chi2 < 0.0 {
        return None;
    }
    Some((chi2, q))
}

fn event_study_label(k: i32) -> String {
    format!("rel_time[{}]#c.treat", k)
}

fn x_labels_or_fallback(x_labels: &[(String, Option<String>)], x_ncols: usize) -> Vec<(String, Option<String>)> {
    if x_labels.len() == x_ncols {
        x_labels.to_vec()
    } else {
        (0..x_ncols)
            .map(|j| (format!("x{}", j + 1), None))
            .collect()
    }
}

/// First calendar period with post==1 (ordinal time index from sorted unique times).
pub(crate) fn adoption_time_ord(time_ord: &[usize], post: &[f64]) -> Option<usize> {
    let mut m = None;
    for (&t, &p) in time_ord.iter().zip(post.iter()) {
        if p > 0.5 {
            m = Some(m.map_or(t, |a: usize| a.min(t)));
        }
    }
    m
}

/// Build event-study design: const + X + treat×I(rel=k) for each k ≠ k_ref.
/// `rel[i] = time_ord[i] as i32 - t_adopt as i32`.
pub(crate) fn build_event_study_exog(
    n: usize,
    exog_main: &Array2<f64>,
    x_ncols: usize,
    x_labels: &[(String, Option<String>)],
    treat: &[f64],
    time_ord: &[usize],
    t_adopt: usize,
) -> Result<(Array2<f64>, Vec<(String, Option<String>)>, i32, Vec<i32>), String> {
    if exog_main.nrows() != n || treat.len() != n || time_ord.len() != n {
        return Err("DID event study: length mismatch".to_string());
    }
    let k_ex = exog_main.ncols();
    if k_ex < 2 + x_ncols {
        return Err("DID event study: unexpected exog shape".to_string());
    }
    let ta = t_adopt as i32;
    let rel: Vec<i32> = time_ord
        .iter()
        .map(|&t| t as i32 - ta)
        .collect();
    let mut ks_set = std::collections::BTreeSet::new();
    for &r in &rel {
        ks_set.insert(r);
    }
    let ks: Vec<i32> = ks_set.into_iter().collect();
    let k_ref = if ks.contains(&-1) {
        -1
    } else {
        let pre: Vec<i32> = ks.iter().copied().filter(|&k| k < 0).collect();
        if pre.is_empty() {
            return Err(
                "DID parallel trends: no pre-policy periods (rel_time < 0); need periods before first post==1"
                    .to_string(),
            );
        }
        *pre.iter().max().unwrap()
    };

    let event_ks: Vec<i32> = ks.into_iter().filter(|&k| k != k_ref).collect();
    if event_ks.is_empty() {
        return Err("DID event study: no event-time dummies after choosing reference".to_string());
    }

    let n_evt = event_ks.len();
    let ncols = 1 + x_ncols + n_evt;
    let mut raw = Vec::with_capacity(n * ncols);
    for i in 0..n {
        raw.push(1.0);
        for xc in 0..x_ncols {
            raw.push(exog_main[[i, 1 + xc]]);
        }
        for &k in &event_ks {
            let hit = if rel[i] == k { 1.0 } else { 0.0 };
            raw.push(treat[i] * hit);
        }
    }
    let exog =
        Array2::from_shape_vec((n, ncols), raw).map_err(|e| format!("DID event exog: {:?}", e))?;

    let mut labels: Vec<(String, Option<String>)> = vec![("const".to_string(), None)];
    labels.extend(x_labels_or_fallback(x_labels, x_ncols));
    for &k in &event_ks {
        labels.push((event_study_label(k), None));
    }
    Ok((exog, labels, k_ref, event_ks))
}

/// Indices into `beta` / `cov` for pre-treatment event coefficients (rel < 0, rel ≠ k_ref).
pub(crate) fn pre_trend_beta_indices(
    kept_labels: &[(String, Option<String>)],
    k_ref: i32,
) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, (name, _)) in kept_labels.iter().enumerate() {
        let Some(rest) = name.strip_prefix("rel_time[") else {
            continue;
        };
        let Some(end) = rest.find(']') else {
            continue;
        };
        if let Ok(k) = rest[..end].parse::<i32>() {
            if k < 0 && k != k_ref {
                out.push(i);
            }
        }
    }
    out.sort();
    out
}

fn parse_event_rel_time(name: &str) -> Option<i32> {
    let rest = name.strip_prefix("rel_time[")?;
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

/// Coefficients on `rel_time[k]#c.treat` from the same event-study TWFE as the Wald test (+ reference at 0).
pub(crate) fn extract_event_study_plot_points(
    pr: &yss_sci::regression::panel::PanelOLSResult,
    kept_labels: &[(String, Option<String>)],
    k_ref: i32,
) -> Vec<DidEventStudyPoint> {
    let n = pr.betas.len().min(kept_labels.len());
    let mut out = Vec::new();
    for i in 0..n {
        let name = &kept_labels[i].0;
        if !name.starts_with("rel_time[") {
            continue;
        }
        if let Some(k) = parse_event_rel_time(name) {
            out.push(DidEventStudyPoint {
                rel_time: k,
                coef: pr.betas[i],
                std_err: pr.stds[i],
                ci_low: pr.conf_int_left[i],
                ci_high: pr.conf_int_right[i],
                is_reference: false,
            });
        }
    }
    out.sort_by_key(|p| p.rel_time);
    if !out.iter().any(|p| p.rel_time == k_ref) {
        let ins = out
            .iter()
            .position(|p| p.rel_time > k_ref)
            .unwrap_or(out.len());
        out.insert(
            ins,
            DidEventStudyPoint {
                rel_time: k_ref,
                coef: 0.0,
                std_err: 0.0,
                ci_low: 0.0,
                ci_high: 0.0,
                is_reference: true,
            },
        );
    }
    out
}

/// Run event-study TWFE, return Wald χ² on pre-treatment treat×event interactions (Stata `test` on pre leads).
pub(crate) fn run_parallel_trends_test(
    endog: &Array1<f64>,
    exog_main: &Array2<f64>,
    x_ncols: usize,
    x_labels: &[(String, Option<String>)],
    entity_id: &[usize],
    time_id: &[usize],
    time_ord: &[usize],
    t_adopt: usize,
    treat: &[f64],
    constant: bool,
    cov_type: &str,
) -> Result<(f64, usize, f64, i32, Vec<i32>, String, Vec<DidEventStudyPoint>), String> {
    let n = endog.len();
    let (exog_evt, labels_evt, k_ref, event_ks) =
        build_event_study_exog(n, exog_main, x_ncols, x_labels, treat, time_ord, t_adopt)?;

    let kcols = exog_evt.ncols();
    let col_is_dummy = vec![false; kcols];
    let intercept_col = if constant { Some(0) } else { None };
    let (exog_u, omitted) =
        drop_collinear_columns(&exog_evt, &col_is_dummy, intercept_col).map_err(|e| {
            format!(
                "DID parallel trends: collinearity drop failed: {}",
                e
            )
        })?;

    let labels_u: Vec<(String, Option<String>)> = (0..kcols)
        .filter(|i| !omitted.contains(i))
        .filter_map(|i| labels_evt.get(i).cloned())
        .collect();

    let cov_params: Option<yss_sci::regression::covariance::CovParams> = None;
    let pr = fit_panel_fe_twoway(
        endog,
        &exog_u,
        entity_id,
        time_id,
        constant,
        cov_type,
        cov_params,
    )
    .map_err(|e| format!("DID parallel trends TWFE: {}", e))?;

    let kept_labels = merge_labels_after_fe_omit(&labels_u, pr.omitted_indices.as_deref());
    let pre_idx = pre_trend_beta_indices(&kept_labels, k_ref);
    if pre_idx.is_empty() {
        return Err(
            "DID parallel trends: no estimable pre-policy interactions (need ≥2 pre periods or non-collinear leads)"
                .to_string(),
        );
    }

    let tested: Vec<i32> = event_ks
        .into_iter()
        .filter(|&k| k < 0 && k != k_ref)
        .collect();

    let (chi2, df) = wald_chi2_linear(&pr.betas, &pr.cov_beta, &pre_idx).ok_or_else(|| {
        "DID parallel trends: Wald χ² failed (singular VCE); try fewer collinear X".to_string()
    })?;

    let p = ChiSquared::new(df as f64)
        .map(|d| 1.0 - d.cdf(chi2))
        .unwrap_or(f64::NAN);

    let method = format!(
        "Wald χ² test on pre-policy coefficients (rel_time[k]×treat, k<0, k≠{}), same VCE as main DID (typically cluster by entity). Reference period rel_time={} omitted. Stata analog: reghdfe Y ibn.rel_time#c.treat X, absorb(id t) cluster(id); test <pre leads>.",
        k_ref, k_ref
    );

    let plot_points = extract_event_study_plot_points(&pr, &kept_labels, k_ref);

    Ok((chi2, df, p, k_ref, tested, method, plot_points))
}

fn merge_labels_after_fe_omit(
    labels_u: &[(String, Option<String>)],
    fe_omitted: Option<&[usize]>,
) -> Vec<(String, Option<String>)> {
    match fe_omitted {
        None | Some([]) => labels_u.to_vec(),
        Some(om) => (0..labels_u.len())
            .filter(|i| !om.contains(i))
            .filter_map(|i| labels_u.get(i).cloned())
            .collect(),
    }
}

/// Placebo: fake policy window [t_adopt−H, t_adopt−1] for treated only; TWFE + Treat×placebo (Stata-style falsification).
pub(crate) fn run_placebo_test(
    endog: &Array1<f64>,
    exog_main: &Array2<f64>,
    x_ncols: usize,
    x_labels: &[(String, Option<String>)],
    entity_id: &[usize],
    time_id: &[usize],
    time_ord: &[usize],
    t_adopt: usize,
    treat: &[f64],
    horizon: usize,
    constant: bool,
    cov_type: &str,
    treat_name: &str,
) -> Result<(f64, f64, f64, f64, String), String> {
    if horizon == 0 {
        return Err("DID placebo: horizon must be ≥1".to_string());
    }
    if t_adopt < horizon {
        return Err(format!(
            "DID placebo: adoption time index {} < horizon {}; need more pre-policy periods",
            t_adopt, horizon
        ));
    }
    let n = endog.len();
    let lo = t_adopt - horizon;
    let hi = t_adopt - 1;
    let placebo_col: Vec<f64> = (0..n)
        .map(|i| {
            let t = time_ord[i];
            if treat[i] > 0.5 && t >= lo && t <= hi {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    if placebo_col.iter().all(|&v| v < 0.5) {
        return Err("DID placebo: no treated observations in fake window".to_string());
    }

    let ncols = 2 + x_ncols;
    let mut raw = Vec::with_capacity(n * ncols);
    for i in 0..n {
        raw.push(1.0);
        for xc in 0..x_ncols {
            raw.push(exog_main[[i, 1 + xc]]);
        }
        raw.push(placebo_col[i]);
    }
    let exog_pb =
        Array2::from_shape_vec((n, ncols), raw).map_err(|e| format!("DID placebo exog: {:?}", e))?;

    let mut labels: Vec<(String, Option<String>)> = vec![("const".to_string(), None)];
    labels.extend(x_labels_or_fallback(x_labels, x_ncols));
    let plabel = format!("placebo({}×fake_pre_{}p)", treat_name, horizon);
    labels.push((plabel.clone(), None));

    let col_is_dummy = vec![false; ncols];
    let intercept_col = if constant { Some(0) } else { None };
    let (exog_u, omitted) = drop_collinear_columns(&exog_pb, &col_is_dummy, intercept_col)
        .map_err(|e| format!("DID placebo: collinearity: {}", e))?;

    let labels_u: Vec<(String, Option<String>)> = (0..ncols)
        .filter(|i| !omitted.contains(i))
        .filter_map(|i| labels.get(i).cloned())
        .collect();

    let cov_params: Option<yss_sci::regression::covariance::CovParams> = None;
    let pr = fit_panel_fe_twoway(
        endog,
        &exog_u,
        entity_id,
        time_id,
        constant,
        cov_type,
        cov_params,
    )
    .map_err(|e| format!("DID placebo TWFE: {}", e))?;

    let kept = merge_labels_after_fe_omit(&labels_u, pr.omitted_indices.as_deref());
    let idx = kept
        .iter()
        .position(|(n, _)| n == &plabel)
        .ok_or_else(|| {
            "DID placebo: interaction absorbed or collinear; cannot report placebo coef".to_string()
        })?;

    if idx >= pr.betas.len() {
        return Err("DID placebo: label index mismatch".to_string());
    }

    Ok((
        pr.betas[idx],
        pr.stds[idx],
        pr.tvalues[idx],
        pr.pvalues[idx],
        format!(
            "Fake policy window: treated units with time_ord in [{}, {}] (H={} periods ending just before adoption). Under parallel trends, coef should be ≈0. Stata analog: construct placebo post for pre-window, reghdfe Y c.treat#c.placebo X, absorb(id t) cluster(id).",
            lo, hi, horizon
        ),
    ))
}

const FAKE_GROUP_PERM_CAP: usize = 2000;
const FAKE_GROUP_PERM_MIN_VALID: usize = 10;

/// Permute **which entities** are “treated” (same number as truly ever-treated), rebuild fake×post, TWFE each time; RI two-sided p = (#{|β_perm| ≥ |β_obs|} + 1) / (B + 1).
pub fn run_placebo_fake_treatment_ri(
    endog: &Array1<f64>,
    exog_use: &Array2<f64>,
    all_labels_use: &[(String, Option<String>)],
    entity_id: &[usize],
    time_id: &[usize],
    post: &[f64],
    treat: &[f64],
    constant: bool,
    cov_type: &str,
    did_label: &str,
    observed_coef: f64,
    n_perm: usize,
    seed: u64,
) -> Result<(usize, usize, f64, f64, f64, String), String> {
    let n = endog.len();
    if exog_use.nrows() != n || entity_id.len() != n || time_id.len() != n || post.len() != n || treat.len() != n {
        return Err("DID fake-group placebo: length mismatch".to_string());
    }
    let ncols = exog_use.ncols();
    if all_labels_use.len() != ncols {
        return Err("DID fake-group placebo: label count != exog columns".to_string());
    }
    let k_last = ncols.saturating_sub(1);
    if all_labels_use
        .get(k_last)
        .map(|(name, _)| name.as_str() != did_label)
        .unwrap_or(true)
    {
        return Err(format!(
            "DID fake-group placebo: last exog column should be '{}', got {:?}",
            did_label,
            all_labels_use.get(k_last).map(|(a, _)| a.as_str())
        ));
    }

    let n_entities = entity_id.iter().copied().max().map(|m| m + 1).ok_or_else(|| {
        "DID fake-group placebo: empty entity_id".to_string()
    })?;

    let mut ever_treat = vec![false; n_entities];
    for i in 0..n {
        if treat[i] > 0.5 {
            ever_treat[entity_id[i]] = true;
        }
    }
    let n_treat_e = ever_treat.iter().filter(|&&t| t).count();
    if n_treat_e == 0 {
        return Err(
            "DID fake-group placebo: no entity is ever treated (cannot fix fake treated count)".to_string(),
        );
    }
    if n_treat_e >= n_entities {
        return Err(
            "DID fake-group placebo: every entity is treated — no pool to reassign fake treatment".to_string(),
        );
    }

    let n_rep = n_perm.max(1).min(FAKE_GROUP_PERM_CAP);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut pool: Vec<usize> = (0..n_entities).collect();
    let mut exog_perm = exog_use.clone();

    let mut perm_coefs: Vec<f64> = Vec::with_capacity(n_rep);

    for _ in 0..n_rep {
        pool.shuffle(&mut rng);
        let fake_treated: HashSet<usize> = pool.iter().take(n_treat_e).copied().collect();
        for i in 0..n {
            let tf = if fake_treated.contains(&entity_id[i]) {
                1.0
            } else {
                0.0
            };
            exog_perm[[i, k_last]] = tf * post[i];
        }

        let pr = match fit_panel_fe_twoway(
            endog,
            &exog_perm,
            entity_id,
            time_id,
            constant,
            cov_type,
            None,
        ) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let kept = merge_labels_after_fe_omit(all_labels_use, pr.omitted_indices.as_deref());
        if let Some(idx) = kept.iter().position(|(name, _)| name == did_label) {
            if idx < pr.betas.len() {
                let b = pr.betas[idx];
                if b.is_finite() {
                    perm_coefs.push(b);
                }
            }
        }
    }

    let n_valid = perm_coefs.len();
    if n_valid < FAKE_GROUP_PERM_MIN_VALID {
        return Err(format!(
            "DID fake-group placebo: only {} valid permutations (need ≥{}); try fewer collinear X or smaller panel",
            n_valid, FAKE_GROUP_PERM_MIN_VALID
        ));
    }

    let obs_abs = observed_coef.abs();
    let count_ge = perm_coefs.iter().filter(|&&b| b.abs() >= obs_abs).count();
    let p_ri = (count_ge as f64 + 1.0) / (n_valid as f64 + 1.0);

    let mean_c = perm_coefs.iter().sum::<f64>() / n_valid as f64;
    let var_c = perm_coefs
        .iter()
        .map(|&b| (b - mean_c).powi(2))
        .sum::<f64>()
        / n_valid as f64;
    let std_c = var_c.max(0.0).sqrt();

    let note = format!(
        "Entity-level random assignment: {} permutation draws each assign {} fake-treated entities uniformly among {} units (same count as observed ever-treated); regressor = I(fake treated)×post; same TWFE and VCE as main DID. RI two-sided p = (1 + count of |perm coef| ≥ |observed|) / (B+1) with B={} successful fits; observed coef is main Treat×Post.",
        n_rep, n_treat_e, n_entities, n_valid
    );

    Ok((n_rep, n_valid, p_ri, mean_c, std_c, note))
}
