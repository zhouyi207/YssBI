//! OLS / WLS 回归结果 golden 测试
//! 覆盖 ols_summary / wls_summary 窗口展示的所有内容
//! 使用当前已验证正确的计算结果作为参考，重构后若计算不一致则测试失败

use ndarray::{Array1, Array2};
use std::f64::consts::PI;
use yss_sci::regression::diagnostics;
use yss_sci::regression::linear_model::{OLS, OLSConfig, WLS, WLSConfig};

const TOL: f64 = 1e-10;
const TOL_REL: f64 = 1e-8;

/// 与 info_nodes::compute_aic_bic 一致
fn compute_aic_bic(n: usize, k: usize, ss_residual: f64) -> (f64, f64) {
    let n_f = n as f64;
    let k_f = k as f64;
    let sigma2 = if n > 0 && ss_residual >= 0.0 {
        (ss_residual / n_f).max(1e-300)
    } else {
        1e-300
    };
    let ln_2pi = (2.0 * PI).ln();
    let llf = -n_f / 2.0 * (ln_2pi + sigma2.ln() + 1.0);
    let aic = -2.0 * llf + 2.0 * k_f;
    let bic = -2.0 * llf + k_f * n_f.ln();
    (aic, bic)
}

fn load_iris() -> (Array2<f64>, Array1<f64>, Array1<f64>) {
    let mut rdr = csv::Reader::from_path("tests/data/iris.csv").unwrap();
    let mut sepal_length = Vec::new();
    let mut sepal_width = Vec::new();
    let mut petal_length = Vec::new();
    let mut petal_width = Vec::new();
    for result in rdr.records() {
        let record = result.unwrap();
        sepal_length.push(record[0].parse::<f64>().unwrap());
        sepal_width.push(record[1].parse::<f64>().unwrap());
        petal_length.push(record[2].parse::<f64>().unwrap());
        petal_width.push(record[3].parse::<f64>().unwrap());
    }
    let n = sepal_length.len();
    let mut exog_data = Vec::with_capacity(n * 4);
    for i in 0..n {
        exog_data.push(1.0);
        exog_data.push(sepal_width[i]);
        exog_data.push(petal_length[i]);
        exog_data.push(petal_width[i]);
    }
    let exog = Array2::from_shape_vec((n, 4), exog_data).unwrap();
    let endog = Array1::from_vec(sepal_length);
    let weights = Array1::from_vec(sepal_width);
    (exog, endog, weights)
}

fn approx_eq(a: f64, b: f64, tol_abs: f64, tol_rel: f64) -> bool {
    if a == b {
        return true;
    }
    if a.abs() < 1e-300 && b.abs() < 1e-300 {
        return true;
    }
    (a - b).abs() <= tol_abs || (a - b).abs() <= a.abs().max(b.abs()) * tol_rel
}

#[test]
fn test_ols_golden() {
    let (exog, endog, _weights) = load_iris();
    let ols = OLS {
        endog: endog.clone(),
        exog: exog.clone(),
        config: OLSConfig {
            constant: true,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    };
    let o = ols.fit().unwrap();

    // 模型摘要
    assert_eq!(o.num_observation, 150);
    assert!(
        approx_eq(o.ss_model, 87.78473462614721, TOL, TOL_REL),
        "ss_model: got {}",
        o.ss_model
    );
    assert!(
        approx_eq(o.ss_residual, 14.38359870718614, TOL, TOL_REL),
        "ss_residual: got {}",
        o.ss_residual
    );
    assert!(
        approx_eq(o.ss_total, 102.1683333333334, TOL, TOL_REL),
        "ss_total: got {}",
        o.ss_total
    );
    assert_eq!(o.df_model, 3);
    assert_eq!(o.df_residual, 146);
    assert_eq!(o.df_total, 149);
    assert!(
        approx_eq(o.ms_model, 29.26157820871574, TOL, TOL_REL),
        "ms_model: got {}",
        o.ms_model
    );
    assert!(
        approx_eq(o.ms_residual, 0.09851779936428864, TOL, TOL_REL),
        "ms_residual: got {}",
        o.ms_residual
    );
    assert!(
        approx_eq(o.ms_total, 0.6856935123042507, TOL, TOL_REL),
        "ms_total: got {}",
        o.ms_total
    );
    assert!(
        approx_eq(o.r2, 0.8592166649106592, TOL, TOL_REL),
        "r2: got {}",
        o.r2
    );
    assert!(
        approx_eq(o.r2_adjusted, 0.8563238566553988, TOL, TOL_REL),
        "r2_adjusted: got {}",
        o.r2_adjusted
    );
    assert!(
        approx_eq(o.fvalue, 297.0181875512199, TOL, TOL_REL),
        "fvalue: got {}",
        o.fvalue
    );
    assert!(o.f_p_value < 1e-10, "f_p_value: got {}", o.f_p_value);
    assert!(
        approx_eq(o.cond_no, 54.74692827940089, TOL, TOL_REL),
        "cond_no: got {}",
        o.cond_no
    );

    // AIC / BIC（与 info_nodes::compute_aic_bic 一致）
    let (aic, bic) = compute_aic_bic(o.num_observation, o.betas.len(), o.ss_residual);
    assert!(
        approx_eq(aic, 81.99955266474048, TOL, TOL_REL),
        "AIC: got {}",
        aic
    );
    assert!(
        approx_eq(bic, 94.04209384112551, TOL, TOL_REL),
        "BIC: got {}",
        bic
    );

    // Breusch-Pagan 四种变体
    let fitted: Array1<f64> = exog
        .rows()
        .into_iter()
        .map(|row| row.iter().zip(o.betas.iter()).map(|(x, b)| x * b).sum())
        .collect();
    let resid = &endog - &fitted;
    let bp_s = diagnostics::breusch_pagan_stata(&resid, &fitted).unwrap();
    let bp_k = diagnostics::breusch_pagan_koenker(&resid, &fitted).unwrap();
    let bp_sr = diagnostics::breusch_pagan_stata_rhs(&exog, &resid).unwrap();
    let bp_kr = diagnostics::breusch_pagan_koenker_rhs(&exog, &resid).unwrap();
    assert!(
        approx_eq(bp_s.lm_stat, 4.693183923613674, TOL, TOL_REL),
        "BP stata lm: got {}",
        bp_s.lm_stat
    );
    assert!(
        approx_eq(bp_s.p_value, 0.03028248489676322, TOL, TOL_REL),
        "BP stata p: got {}",
        bp_s.p_value
    );
    assert!(
        approx_eq(bp_k.lm_stat, 5.403019717270058, TOL, TOL_REL),
        "BP koenker lm: got {}",
        bp_k.lm_stat
    );
    assert!(
        approx_eq(bp_k.p_value, 0.02010194220464956, TOL, TOL_REL),
        "BP koenker p: got {}",
        bp_k.p_value
    );
    assert!(
        approx_eq(bp_sr.lm_stat, 6.31385587347927, TOL, TOL_REL),
        "BP stata_rhs lm: got {}",
        bp_sr.lm_stat
    );
    assert!(
        approx_eq(bp_sr.p_value, 0.09729983457576108, TOL, TOL_REL),
        "BP stata_rhs p: got {}",
        bp_sr.p_value
    );
    assert!(
        approx_eq(bp_kr.lm_stat, 7.268815442064058, TOL, TOL_REL),
        "BP koenker_rhs lm: got {}",
        bp_kr.lm_stat
    );
    assert!(
        approx_eq(bp_kr.p_value, 0.06380578710280205, TOL, TOL_REL),
        "BP koenker_rhs p: got {}",
        bp_kr.p_value
    );

    // Cameron & Trivedi IM-test
    let im = diagnostics::im_test(&exog, &resid).unwrap();
    assert!(
        approx_eq(im.heteroskedasticity.chi2, 10.68721848919021, TOL, TOL_REL),
        "IM hetero chi2: got {}",
        im.heteroskedasticity.chi2
    );
    assert!(
        approx_eq(
            im.heteroskedasticity.p_value,
            0.2977591814157219,
            TOL,
            TOL_REL
        ),
        "IM hetero p: got {}",
        im.heteroskedasticity.p_value
    );
    assert!(
        approx_eq(im.skewness.chi2, 1.096486284852272, TOL, TOL_REL),
        "IM skew chi2: got {}",
        im.skewness.chi2
    );
    assert!(
        approx_eq(im.kurtosis.chi2, 0.8870835450978678, TOL, TOL_REL),
        "IM kurt chi2: got {}",
        im.kurtosis.chi2
    );
    assert!(
        approx_eq(im.total.chi2, 12.67078831906225, TOL, TOL_REL),
        "IM total chi2: got {}",
        im.total.chi2
    );

    // fitted_values / residuals（残差图用，抽样前 3 个）
    assert!(
        approx_eq(fitted[0], 5.020060161228604, TOL, TOL_REL),
        "fitted[0]: got {}",
        fitted[0]
    );
    assert!(
        approx_eq(resid[0], 0.07993983877139588, TOL, TOL_REL),
        "residual[0]: got {}",
        resid[0]
    );
    assert!(
        approx_eq(fitted[1], 4.692628039835949, TOL, TOL_REL),
        "fitted[1]: got {}",
        fitted[1]
    );
    assert!(
        approx_eq(resid[1], 0.2073719601640516, TOL, TOL_REL),
        "residual[1]: got {}",
        resid[1]
    );
    assert!(
        approx_eq(fitted[2], 4.752494596937745, TOL, TOL_REL),
        "fitted[2]: got {}",
        fitted[2]
    );
    assert!(
        approx_eq(resid[2], -0.05249459693774483, TOL, TOL_REL),
        "residual[2]: got {}",
        resid[2]
    );

    // 系数与推断
    let betas_golden = [
        1.8450608032166922,
        0.6548642427853103,
        0.7110629145526568,
        -0.562567860551966,
    ];
    let stds_golden = [
        0.2504224582117989,
        0.06666949060300735,
        0.05661479647507111,
        0.12711108285128656,
    ];
    let tvalues_golden = [
        7.367792874456178,
        9.822547568044122,
        12.55966564969911,
        -4.4257970897009935,
    ];
    let ci_left_golden = [
        1.3501394661172146,
        0.5231022847381035,
        0.5991725075534622,
        -0.8137832967957634,
    ];
    let ci_right_golden = [
        2.33998214031617,
        0.7866262008325171,
        0.8229533215518514,
        -0.31135242430816845,
    ];

    for i in 0..4 {
        assert!(
            approx_eq(o.betas[i], betas_golden[i], TOL, TOL_REL),
            "betas[{}]: got {}",
            i,
            o.betas[i]
        );
        assert!(
            approx_eq(o.stds[i], stds_golden[i], TOL, TOL_REL),
            "stds[{}]: got {}",
            i,
            o.stds[i]
        );
        assert!(
            approx_eq(o.tvalues[i], tvalues_golden[i], TOL, TOL_REL),
            "tvalues[{}]: got {}",
            i,
            o.tvalues[i]
        );
        assert!(
            approx_eq(o.conf_int_left[i], ci_left_golden[i], TOL, TOL_REL),
            "conf_int_left[{}]: got {}",
            i,
            o.conf_int_left[i]
        );
        assert!(
            approx_eq(o.conf_int_right[i], ci_right_golden[i], TOL, TOL_REL),
            "conf_int_right[{}]: got {}",
            i,
            o.conf_int_right[i]
        );
    }
    assert!(o.pvalues[0] < 1e-10);
    assert!(o.pvalues[1] < 1e-10);
    assert!(o.pvalues[2] < 1e-10);
    assert!(
        approx_eq(o.pvalues[3], 1.868955199046951e-5, TOL, TOL_REL),
        "pvalues[3]: got {}",
        o.pvalues[3]
    );

    // 正态性检验（原始残差）
    let nt = diagnostics::normality_tests(&resid).unwrap();
    assert!(
        approx_eq(nt.skewness, 2.876525278040253e-3, TOL, TOL_REL),
        "skewness: got {}",
        nt.skewness
    );
    assert!(
        approx_eq(nt.kurtosis, 2.737244788728977, TOL, TOL_REL),
        "kurtosis: got {}",
        nt.kurtosis
    );
    assert!(
        approx_eq(nt.omnibus_stat, 0.2654355017700693, TOL, TOL_REL),
        "omnibus_stat: got {}",
        nt.omnibus_stat
    );
    assert!(
        approx_eq(nt.omnibus_p_value, 0.8757122262312549, TOL, TOL_REL),
        "omnibus_p_value: got {}",
        nt.omnibus_p_value
    );
    assert!(
        approx_eq(nt.jarque_bera_stat, 0.4317087415048803, TOL, TOL_REL),
        "jarque_bera_stat: got {}",
        nt.jarque_bera_stat
    );
    assert!(
        approx_eq(nt.jarque_bera_p_value, 0.8058526490436064, TOL, TOL_REL),
        "jarque_bera_p_value: got {}",
        nt.jarque_bera_p_value
    );
}

#[test]
fn test_wls_golden() {
    let (exog, endog, weights) = load_iris();
    let n = exog.nrows();
    let wls = WLS {
        endog: endog.clone(),
        exog: exog.clone(),
        weights: weights.clone(),
        config: WLSConfig {
            constant: true,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    };
    let w = wls.fit().unwrap();

    // 模型摘要
    assert_eq!(w.num_observation, 150);
    assert!(
        approx_eq(w.ss_model, 316.2887714961151, TOL, TOL_REL),
        "ss_model: got {}",
        w.ss_model
    );
    assert!(
        approx_eq(w.ss_residual, 42.98617674226099, TOL, TOL_REL),
        "ss_residual: got {}",
        w.ss_residual
    );
    assert!(
        approx_eq(w.ss_total, 359.2749482383761, TOL, TOL_REL),
        "ss_total: got {}",
        w.ss_total
    );
    assert_eq!(w.df_model, 3);
    assert_eq!(w.df_residual, 146);
    assert_eq!(w.df_total, 149);
    assert!(
        approx_eq(w.ms_model, 105.4295904987051, TOL, TOL_REL),
        "ms_model: got {}",
        w.ms_model
    );
    assert!(
        approx_eq(w.ms_residual, 0.294425868097678, TOL, TOL_REL),
        "ms_residual: got {}",
        w.ms_residual
    );
    assert!(
        approx_eq(w.ms_total, 2.411241263344806, TOL, TOL_REL),
        "ms_total: got {}",
        w.ms_total
    );
    assert!(
        approx_eq(w.r2, 0.8803529804874122, TOL, TOL_REL),
        "r2: got {}",
        w.r2
    );
    assert!(
        approx_eq(w.r2_adjusted, 0.8778944800864685, TOL, TOL_REL),
        "r2_adjusted: got {}",
        w.r2_adjusted
    );
    assert!(
        approx_eq(w.fvalue, 358.0853516027606, TOL, TOL_REL),
        "fvalue: got {}",
        w.fvalue
    );
    assert!(w.f_p_value < 1e-10, "f_p_value: got {}", w.f_p_value);
    assert!(
        approx_eq(w.cond_no, 54.92594150218402, TOL, TOL_REL),
        "cond_no: got {}",
        w.cond_no
    );

    // AIC / BIC（WLS 使用 ss_residual_for_ic = ss_residual * (n/sum_w)）
    let sum_w: f64 = weights.iter().sum();
    let ss_residual_for_ic = w.ss_residual * (n as f64 / sum_w);
    let (aic_w, bic_w) = compute_aic_bic(n, w.betas.len(), ss_residual_for_ic);
    assert!(
        approx_eq(aic_w, 78.75022376256308, TOL, TOL_REL),
        "AIC: got {}",
        aic_w
    );
    assert!(
        approx_eq(bic_w, 90.79276493894810, TOL, TOL_REL),
        "BIC: got {}",
        bic_w
    );

    // Breusch-Pagan 加权四种变体
    let fitted_w: Array1<f64> = exog
        .rows()
        .into_iter()
        .map(|row| row.iter().zip(w.betas.iter()).map(|(x, b)| x * b).sum())
        .collect();
    let resid_w = &endog - &fitted_w;
    let w_norm: Array1<f64> = Array1::from_shape_fn(n, |i| weights[i] * n as f64 / sum_w);
    let bp_ws = diagnostics::breusch_pagan_stata_weighted(&resid_w, &fitted_w, &w_norm).unwrap();
    let bp_wk = diagnostics::breusch_pagan_koenker_weighted(&resid_w, &fitted_w, &w_norm).unwrap();
    let bp_wsr = diagnostics::breusch_pagan_stata_rhs_weighted(&exog, &resid_w, &w_norm).unwrap();
    let bp_wkr = diagnostics::breusch_pagan_koenker_rhs_weighted(&exog, &resid_w, &w_norm).unwrap();
    assert!(
        approx_eq(bp_ws.lm_stat, 4.614451696082235, TOL, TOL_REL),
        "BP stata lm: got {}",
        bp_ws.lm_stat
    );
    assert!(
        approx_eq(bp_ws.p_value, 0.03170362926002279, TOL, TOL_REL),
        "BP stata p: got {}",
        bp_ws.p_value
    );
    assert!(
        approx_eq(bp_wk.lm_stat, 5.3063466937464, TOL, TOL_REL),
        "BP koenker lm: got {}",
        bp_wk.lm_stat
    );
    assert!(
        approx_eq(bp_wk.p_value, 0.02124786744445606, TOL, TOL_REL),
        "BP koenker p: got {}",
        bp_wk.p_value
    );
    assert!(
        approx_eq(bp_wsr.lm_stat, 6.467101016080033, TOL, TOL_REL),
        "BP stata_rhs lm: got {}",
        bp_wsr.lm_stat
    );
    assert!(
        approx_eq(bp_wsr.p_value, 0.09096902730631795, TOL, TOL_REL),
        "BP stata_rhs p: got {}",
        bp_wsr.p_value
    );
    assert!(
        approx_eq(bp_wkr.lm_stat, 7.436783903043398, TOL, TOL_REL),
        "BP koenker_rhs lm: got {}",
        bp_wkr.lm_stat
    );
    assert!(
        approx_eq(bp_wkr.p_value, 0.05920518616001602, TOL, TOL_REL),
        "BP koenker_rhs p: got {}",
        bp_wkr.p_value
    );

    // IM-test 加权
    let im_w = diagnostics::im_test_weighted(&exog, &resid_w, &w_norm).unwrap();
    assert!(
        approx_eq(
            im_w.heteroskedasticity.chi2,
            9.114915503673393,
            TOL,
            TOL_REL
        ),
        "IM hetero chi2: got {}",
        im_w.heteroskedasticity.chi2
    );
    assert!(
        approx_eq(
            im_w.heteroskedasticity.p_value,
            0.4267348728793023,
            TOL,
            TOL_REL
        ),
        "IM hetero p: got {}",
        im_w.heteroskedasticity.p_value
    );
    assert!(
        approx_eq(im_w.skewness.chi2, 1.072040214595416, TOL, TOL_REL),
        "IM skew chi2: got {}",
        im_w.skewness.chi2
    );
    assert!(
        approx_eq(im_w.kurtosis.chi2, 0.9451071764399521, TOL, TOL_REL),
        "IM kurt chi2: got {}",
        im_w.kurtosis.chi2
    );
    assert!(
        approx_eq(im_w.total.chi2, 11.13206289470876, TOL, TOL_REL),
        "IM total chi2: got {}",
        im_w.total.chi2
    );

    // fitted_values / residuals
    assert!(
        approx_eq(fitted_w[0], 5.025914842190915, TOL, TOL_REL),
        "fitted[0]: got {}",
        fitted_w[0]
    );
    assert!(
        approx_eq(resid_w[0], 0.07408515780908509, TOL, TOL_REL),
        "residual[0]: got {}",
        resid_w[0]
    );
    assert!(
        approx_eq(fitted_w[1], 4.693633244752995, TOL, TOL_REL),
        "fitted[1]: got {}",
        fitted_w[1]
    );
    assert!(
        approx_eq(resid_w[1], 0.2063667552470054, TOL, TOL_REL),
        "residual[1]: got {}",
        resid_w[1]
    );
    assert!(
        approx_eq(fitted_w[2], 4.755970933684549, TOL, TOL_REL),
        "fitted[2]: got {}",
        fitted_w[2]
    );
    assert!(
        approx_eq(resid_w[2], -0.05597093368454864, TOL, TOL_REL),
        "residual[2]: got {}",
        resid_w[2]
    );

    // 系数与推断
    let betas_golden = [
        1.8223555263973794,
        0.6645631948758386,
        0.7057495004361467,
        -0.5523058344125235,
    ];
    let stds_golden = [
        0.25117956588056634,
        0.06596389371778973,
        0.056406191198474905,
        0.12654895229167926,
    ];
    let tvalues_golden = [
        7.255190206292073,
        10.074650803953578,
        12.511915544037453,
        -4.364365128361779,
    ];
    let ci_left_golden = [
        1.3259378828466741,
        0.5341957401666262,
        0.5942713695688783,
        -0.802410306367527,
    ];
    let ci_right_golden = [
        2.3187731699480847,
        0.794930649585051,
        0.8172276313034151,
        -0.30220136245752,
    ];

    for i in 0..4 {
        assert!(
            approx_eq(w.betas[i], betas_golden[i], TOL, TOL_REL),
            "betas[{}]: got {}",
            i,
            w.betas[i]
        );
        assert!(
            approx_eq(w.stds[i], stds_golden[i], TOL, TOL_REL),
            "stds[{}]: got {}",
            i,
            w.stds[i]
        );
        assert!(
            approx_eq(w.tvalues[i], tvalues_golden[i], TOL, TOL_REL),
            "tvalues[{}]: got {}",
            i,
            w.tvalues[i]
        );
        assert!(
            approx_eq(w.conf_int_left[i], ci_left_golden[i], TOL, TOL_REL),
            "conf_int_left[{}]: got {}",
            i,
            w.conf_int_left[i]
        );
        assert!(
            approx_eq(w.conf_int_right[i], ci_right_golden[i], TOL, TOL_REL),
            "conf_int_right[{}]: got {}",
            i,
            w.conf_int_right[i]
        );
    }
    assert!(w.pvalues[0] < 1e-10);
    assert!(w.pvalues[1] < 1e-10);
    assert!(w.pvalues[2] < 1e-10);
    assert!(
        approx_eq(w.pvalues[3], 2.3989050815798052e-5, TOL, TOL_REL),
        "pvalues[3]: got {}",
        w.pvalues[3]
    );

    // 正态性检验（加权残差 wresid = sqrt(w)*resid）
    let wresid: Array1<f64> = resid_w
        .iter()
        .zip(weights.iter())
        .map(|(r, w)| r * w.sqrt())
        .collect();
    let nt = diagnostics::normality_tests(&wresid).unwrap();
    assert!(
        approx_eq(nt.skewness, 3.332968103051957e-2, TOL, TOL_REL),
        "skewness: got {}",
        nt.skewness
    );
    assert!(
        approx_eq(nt.kurtosis, 2.677524721893009, TOL, TOL_REL),
        "kurtosis: got {}",
        nt.kurtosis
    );
    assert!(
        approx_eq(nt.omnibus_stat, 0.5553130456635863, TOL, TOL_REL),
        "omnibus_stat: got {}",
        nt.omnibus_stat
    );
    assert!(
        approx_eq(nt.omnibus_p_value, 0.7575569803588351, TOL, TOL_REL),
        "omnibus_p_value: got {}",
        nt.omnibus_p_value
    );
    assert!(
        approx_eq(nt.jarque_bera_stat, 0.6777110971285353, TOL, TOL_REL),
        "jarque_bera_stat: got {}",
        nt.jarque_bera_stat
    );
    assert!(
        approx_eq(nt.jarque_bera_p_value, 0.7125853756356614, TOL, TOL_REL),
        "jarque_bera_p_value: got {}",
        nt.jarque_bera_p_value
    );
}

#[test]
fn test_diagnostics_direct_helpers() {
    let (exog, endog, _weights) = load_iris();
    let ols = OLS {
        endog: endog.clone(),
        exog: exog.clone(),
        config: OLSConfig {
            constant: true,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    };
    let o = ols.fit().unwrap();
    let fitted: Array1<f64> = exog
        .rows()
        .into_iter()
        .map(|row| row.iter().zip(o.betas.iter()).map(|(x, b)| x * b).sum())
        .collect();
    let resid = &endog - &fitted;

    let white = diagnostics::white_test(&exog, &resid).unwrap();
    assert_eq!(white.df, 9);
    assert!(white.lm_stat > 0.0);
    assert!((0.0..=1.0).contains(&white.p_value));

    let reset = diagnostics::reset_test(&endog, &exog, &fitted, None).unwrap();
    assert_eq!(reset.df1, 3);
    assert_eq!(reset.df2, 143);
    assert!(reset.f_stat.is_finite());
    assert!((0.0..=1.0).contains(&reset.p_value));

    let reset_rhs = diagnostics::reset_test_rhs(&endog, &exog, None).unwrap();
    assert_eq!(reset_rhs.df1, 9);
    assert_eq!(reset_rhs.df2, 137);
    assert!(reset_rhs.f_stat.is_finite());
    assert!((0.0..=1.0).contains(&reset_rhs.p_value));

    let vif = diagnostics::vif_centered(&exog, true).unwrap();
    assert_eq!(vif.len(), 4);
    assert!(vif[0].vif.is_nan());
    assert!(
        vif.iter()
            .skip(1)
            .all(|entry| entry.vif >= 1.0 || entry.vif.is_infinite())
    );

    let leverage = diagnostics::leverage(&exog).unwrap();
    assert_eq!(leverage.len(), exog.nrows());
    assert!(leverage.iter().all(|v| *v >= 0.0 && *v <= 1.0));
    assert!(approx_eq(
        leverage.iter().sum::<f64>(),
        exog.ncols() as f64,
        1e-8,
        1e-8
    ));
}
