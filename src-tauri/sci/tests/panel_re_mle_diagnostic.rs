//! Diagnostic test for Panel RE MLE vs Stata.
//! Run: cargo test -p yss-sci panel_re_mle_lin -- --nocapture
//!
//! Stata: xtreg ltvfo ltlan ltwlab ltpow ltfer hrs mipric1 giprice mci ngca, re mle
//! Expected: Log likelihood = 334.64947, sigma_e = 0.1056, sigma_u = 0.2166

use ndarray::{Array1, Array2};
use std::io::Write;
use yss_sci::regression::panel::fit_panel_re_mle;

fn load_lin_csv() -> Result<(Array1<f64>, Array2<f64>, Vec<usize>), Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path("tests/data/lin.csv")?;
    let headers = rdr.headers()?.clone();
    let mut records: Vec<csv::StringRecord> = rdr.records().filter_map(|r| r.ok()).collect();

    // Find column indices (Stata: xtset province year)
    let prov_idx = headers.iter().position(|h| h == "province").or_else(|| headers.iter().position(|h| h == "prov")).ok_or("prov/province not found")?;
    let year_idx = headers.iter().position(|h| h == "year").or_else(|| headers.iter().position(|h| h == "t")).ok_or("year/t not found")?;
    let ltvfo_idx = headers.iter().position(|h| h == "ltvfo").ok_or("ltvfo not found")?;
    let ltlan_idx = headers.iter().position(|h| h == "ltlan").ok_or("ltlan not found")?;
    let ltwlab_idx = headers.iter().position(|h| h == "ltwlab").ok_or("ltwlab not found")?;
    let ltpow_idx = headers.iter().position(|h| h == "ltpow").ok_or("ltpow not found")?;
    let ltfer_idx = headers.iter().position(|h| h == "ltfer").ok_or("ltfer not found")?;
    let hrs_idx = headers.iter().position(|h| h == "hrs").ok_or("hrs not found")?;
    let mipric1_idx = headers.iter().position(|h| h == "mipric1").ok_or("mipric1 not found")?;
    let giprice_idx = headers.iter().position(|h| h == "giprice").ok_or("giprice not found")?;
    let mci_idx = headers.iter().position(|h| h == "mci").ok_or("mci not found")?;
    let ngca_idx = headers.iter().position(|h| h == "ngca").ok_or("ngca not found")?;

    let mut entity_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut entity_id: Vec<usize> = Vec::new();
    let mut endog: Vec<f64> = Vec::new();
    let mut exog_rows: Vec<Vec<f64>> = Vec::new();

    // Sort by province, year (like Stata xtset)
    records.sort_by(|a, b| {
        let pa = a.get(prov_idx).unwrap_or("");
        let pb = b.get(prov_idx).unwrap_or("");
        let cmp_prov = pa.cmp(pb);
        if cmp_prov != std::cmp::Ordering::Equal {
            cmp_prov
        } else {
            let ya: i32 = a.get(year_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
            let yb: i32 = b.get(year_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
            ya.cmp(&yb)
        }
    });

    for rec in &records {
        let prov = rec.get(prov_idx).unwrap_or("").to_string();
        let next_id = entity_map.len();
        let eid = *entity_map.entry(prov).or_insert(next_id);
        entity_id.push(eid);

        let parse_f64 = |idx: usize| -> f64 {
            rec.get(idx)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(f64::NAN)
        };
        endog.push(parse_f64(ltvfo_idx));
        exog_rows.push(vec![
            1.0,
            parse_f64(ltlan_idx),
            parse_f64(ltwlab_idx),
            parse_f64(ltpow_idx),
            parse_f64(ltfer_idx),
            parse_f64(hrs_idx),
            parse_f64(mipric1_idx),
            parse_f64(giprice_idx),
            parse_f64(mci_idx),
            parse_f64(ngca_idx),
        ]);
    }

    // Drop rows with NaN
    let n = endog.len();
    let mut endog_clean = Vec::new();
    let mut exog_clean = Vec::new();
    let mut entity_clean = Vec::new();
    for i in 0..n {
        if !endog[i].is_nan() && exog_rows[i].iter().all(|&v| !v.is_nan()) {
            endog_clean.push(endog[i]);
            exog_clean.push(exog_rows[i].clone());
            entity_clean.push(entity_id[i]);
        }
    }

    let n_clean = endog_clean.len();
    let k = 10;
    let mut exog_flat = Vec::with_capacity(n_clean * k);
    for row in &exog_clean {
        exog_flat.extend(row);
    }
    let exog = Array2::from_shape_vec((n_clean, k), exog_flat)?;
    let endog = Array1::from_vec(endog_clean);

    Ok((endog, exog, entity_clean))
}

#[test]
fn panel_re_mle_lin() {
    let (endog, exog, entity_id) = load_lin_csv().expect("load lin.csv");
    let n = endog.len();
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();

    let result = fit_panel_re_mle(&endog, &exog, &entity_id, true).expect("fit_panel_re_mle");

    let mut out = std::io::stdout().lock();
    writeln!(out, "=== Panel RE MLE Diagnostic (lin.csv) ===").ok();
    writeln!(out, "N={}, n_entities={}", n, n_entities).ok();
    writeln!(out, "").ok();
    writeln!(out, "Stata reference:").ok();
    writeln!(out, "  Log likelihood = 334.64947").ok();
    writeln!(out, "  sigma_u = 0.2166, sigma_e = 0.1056").ok();
    writeln!(out, "  LR chi2(9) = 964.50").ok();
    writeln!(out, "").ok();
    writeln!(out, "Our results:").ok();
    writeln!(out, "  Log likelihood = {:?}", result.log_likelihood).ok();
    if let Some(ref fe) = result.fe_stats {
        writeln!(out, "  sigma_u = {:.4}, sigma_e = {:.4}", fe.sigma_u, fe.sigma_e).ok();
        writeln!(out, "  rho = {:.4}", fe.rho).ok();
    }
    writeln!(out, "  LR chi2 = {:?}", result.lr_chi2).ok();
    writeln!(out, "").ok();
    writeln!(out, "Constant-only iterations:").ok();
    if let Some(ref v) = result.mle_iter_log_lik_const {
        for (i, ll) in v.iter().enumerate() {
            writeln!(out, "  Iteration {}: Log likelihood = {:.5}", i, ll).ok();
        }
    }
    writeln!(out, "").ok();
    writeln!(out, "Full model iterations:").ok();
    if let Some(ref v) = result.mle_iter_log_lik {
        for (i, ll) in v.iter().enumerate() {
            writeln!(out, "  Iteration {}: Log likelihood = {:.5}", i, ll).ok();
        }
    }
    writeln!(out, "").ok();
    writeln!(out, "Coefficients:").ok();
    let names = ["const", "ltlan", "ltwlab", "ltpow", "ltfer", "hrs", "mipric1", "giprice", "mci", "ngca"];
    for (i, &b) in result.betas.iter().enumerate() {
        let name = names.get(i).unwrap_or(&"");
        let se = result.stds.get(i).copied().unwrap_or(0.0);
        writeln!(out, "  {}: {:.6} (se={:.6})", name, b, se).ok();
    }
}
