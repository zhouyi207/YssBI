// Panel Random Effects (GLS with variance components)
//
// Quasi-demeaning: y*_it = y_it - θ_i·ȳ_i, where θ_i = 1 - sqrt(σ²_e/(T_i·σ²_u + σ²_e)).
// Stata xtreg, re default: consistent variance components (harmonic mean T̄ for σ²_u).

use crate::regression::collinearity::drop_collinear_columns;
use crate::regression::linear_model::OLS;
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::linalg::solvers::Solve;
use faer::{Mat, Side};
use ndarray::{Array1, Array2};
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};
use std::collections::HashMap;

/// Within transformation (same as FE)
fn within_transform(v: &[f64], entity_id: &[usize]) -> Array1<f64> {
    let n = v.len();
    let mut sums: HashMap<usize, (f64, usize)> = HashMap::new();
    for (i, &eid) in entity_id.iter().enumerate() {
        let val = v[i];
        if !val.is_nan() {
            let entry = sums.entry(eid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
    }
    let out: Vec<f64> = (0..n)
        .map(|i| {
            let eid = entity_id[i];
            let (s, cnt) = sums.get(&eid).copied().unwrap_or((0.0, 0));
            let mean = if cnt > 0 { s / cnt as f64 } else { 0.0 };
            v[i] - mean
        })
        .collect();
    Array1::from_vec(out)
}

/// Between transformation: replace each obs with entity mean (for quasi-demeaning)
fn between_transform(v: &[f64], entity_id: &[usize]) -> Array1<f64> {
    let n = v.len();
    let mut sums: HashMap<usize, (f64, usize)> = HashMap::new();
    for (i, &eid) in entity_id.iter().enumerate() {
        let val = v[i];
        if !val.is_nan() {
            let entry = sums.entry(eid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
    }
    let out: Vec<f64> = (0..n)
        .map(|i| {
            let eid = entity_id[i];
            let (s, cnt) = sums.get(&eid).copied().unwrap_or((0.0, 0));
            if cnt > 0 {
                s / cnt as f64
            } else {
                v[i]
            }
        })
        .collect();
    Array1::from_vec(out)
}

/// Obs per entity T_i and harmonic mean T̄ = n / Σ(1/T_i) (Stata xtreg, re)
fn obs_per_entity_and_harmonic_mean(entity_id: &[usize]) -> (HashMap<usize, usize>, f64) {
    obs_per_group_and_harmonic_mean(entity_id)
}

/// Obs per group T_i and harmonic mean T̄ = n / Σ(1/T_i). Generic over group_id.
fn obs_per_group_and_harmonic_mean(group_id: &[usize]) -> (HashMap<usize, usize>, f64) {
    let mut cnt: HashMap<usize, usize> = HashMap::new();
    for &gid in group_id {
        *cnt.entry(gid).or_insert(0) += 1;
    }
    let n = cnt.len();
    let inv_sum: f64 = cnt.values().map(|&t| 1.0 / (t as f64).max(1e-10)).sum();
    let t_bar_harmonic = if inv_sum > 1e-300 { n as f64 / inv_sum } else { 0.0 };
    (cnt, t_bar_harmonic)
}

/// Compute entity-level means. Returns (entity_ids, y_means, x_means) for between regression.
fn entity_means(
    endog: &[f64],
    exog: &Array2<f64>,
    entity_id: &[usize],
) -> (Vec<usize>, Vec<f64>, Vec<Vec<f64>>) {
    group_means(endog, exog, entity_id)
}

/// Compute group-level means (generic for entity or time). Returns (group_ids, y_means, x_means).
fn group_means(
    endog: &[f64],
    exog: &Array2<f64>,
    group_id: &[usize],
) -> (Vec<usize>, Vec<f64>, Vec<Vec<f64>>) {
    let mut sums_y: HashMap<usize, (f64, usize)> = HashMap::new();
    let k = exog.ncols();
    let mut sums_x: HashMap<usize, (Vec<f64>, usize)> = HashMap::new();

    for (i, &gid) in group_id.iter().enumerate() {
        let val = endog[i];
        if !val.is_nan() {
            let entry = sums_y.entry(gid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
        let entry = sums_x.entry(gid).or_insert_with(|| (vec![0.0; k], 0));
        for c in 0..k {
            entry.0[c] += exog[[i, c]];
        }
        entry.1 += 1;
    }

    let mut gids: Vec<usize> = sums_y.keys().copied().collect();
    gids.sort_unstable();
    let mut y_means = Vec::new();
    let mut x_means = Vec::new();
    for &gid in &gids {
        let (sy, cy) = sums_y.get(&gid).copied().unwrap_or((0.0, 0));
        let (sx, cx) = sums_x.get(&gid).cloned().unwrap_or_else(|| (vec![0.0; k], 0));
        y_means.push(if cy > 0 { sy / cy as f64 } else { 0.0 });
        x_means.push(if cx > 0 {
            sx.iter().map(|v| v / cx as f64).collect()
        } else {
            vec![0.0; k]
        });
    }
    (gids, y_means, x_means)
}

