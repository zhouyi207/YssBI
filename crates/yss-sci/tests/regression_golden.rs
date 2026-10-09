//! OLS / WLS 回归结果 golden 测试
//! 覆盖 linear_regression_summary / wls_summary 窗口展示的所有内容
//! 使用当前已验证正确的计算结果作为参考，重构后若计算不一致则测试失败

use std::f64::consts::PI;
use yss_sci::causal::iv::iv2sls::IV2SLS;
use yss_sci::diagnostics;
use yss_sci::regression::linear::{OLS, WLS, WLSConfig};
use yss_sci_linalg::{Col, Mat};

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

fn load_iris() -> (Mat<f64>, Col<f64>, Col<f64>) {
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
    let exog = yss_sci_linalg::MatRef::from_row_major_slice(&(exog_data), n, 4).to_owned();
    let endog = (sepal_length).into_iter().collect::<Col<f64>>();
    let weights = (sepal_width).into_iter().collect::<Col<f64>>();
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
fn iv2sls_recovers_known_coefficients_with_single_and_multiple_endogenous_regressors() {
    // Balanced binary signals are mutually orthogonal. The structural error
    // correlates with endogenous residuals but is orthogonal to all instruments.
    let signal = |row: usize, bit: usize| if row & (1 << bit) == 0 { -1.0 } else { 1.0 };
    let observations = 64;
    for endogenous_count in [1, 2] {
        let exog = Mat::from_fn(observations, 1, |row, _| signal(row, 0));
        let instruments = Mat::from_fn(observations, 3, |row, column| match column {
            0 => signal(row, 1),
            1 => signal(row, 2),
            _ => signal(row, 0) * signal(row, 1),
        });
        let endogenous = Mat::from_fn(observations, endogenous_count, |row, column| {
            if column == 0 {
                2.0 * signal(row, 1) + 0.5 * signal(row, 2) + 0.3 * signal(row, 0) + signal(row, 3)
            } else {
                0.2 * signal(row, 1) + 1.5 * signal(row, 2) + 0.4 * signal(row, 0) + signal(row, 4)
            }
        });
        let response = Col::from_fn(observations, |row| {
            1.0 + 0.7 * exog[(row, 0)] + 2.0 * endogenous[(row, 0)]
                - if endogenous_count == 2 {
                    0.5 * endogenous[(row, 1)]
                } else {
                    0.0
                }
                + 0.8 * signal(row, 3)
                + 0.5 * signal(row, 4)
                + signal(row, 5)
        });
        for covariance_type in ["nonrobust", "HC1", "HAC"] {
            let estimator = IV2SLS {
                endog: response.clone(),
                exog: exog.clone(),
                endog_reg: endogenous.clone(),
                instruments: instruments.clone(),
                options: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
                    true,
                    covariance_type,
                    (covariance_type == "HAC")
                        .then(|| yss_sci_contract::regression::CovParams::HAC {
                            kernel: "bartlett".into(),
                            bandwidth: None,
                        })
                        .as_ref(),
                )
                .unwrap(),
                small: false,
                endog_names: None,
                z_var_names: None,
            };
            let result = estimator.fit().unwrap();
            let (first_stage, first_stage_summary) = estimator.first_stage(false).unwrap();
            let expected = &[1.0, 0.7, 2.0, -0.5][..2 + endogenous_count];
            assert_eq!(result.betas.nrows(), expected.len());
            for (&actual, &expected) in result.betas.iter().zip(expected) {
                assert!(
                    approx_eq(actual, expected, 1e-10, 1e-10),
                    "{actual} != {expected}"
                );
            }
            assert_eq!(first_stage.len(), endogenous_count);
            assert!(first_stage_summary.min_eigenvalue.is_finite());
            assert!(first_stage_summary.min_eigenvalue > 0.0);
            if endogenous_count == 1 {
                let statistic = first_stage_summary.f_stat.unwrap();
                assert!(statistic.is_finite() && statistic > 0.0);
                assert!((0.0..=1.0).contains(&first_stage_summary.f_p_value.unwrap()));
                assert!(
                    [
                        first_stage_summary.r2,
                        first_stage_summary.r2_adjusted,
                        first_stage_summary.partial_r2,
                    ]
                    .into_iter()
                    .all(|value| value.is_some_and(f64::is_finite))
                );
            }
            assert!(
                result
                    .inference
                    .standard_errors
                    .iter()
                    .all(|value| value.is_finite() && *value > 0.0)
            );
        }
    }

    // Distinct Fourier frequencies are orthogonal to each other and the included
    // regressor. Removing the other endogenous columns leaves signal + noise,
    // so Shea's partial R² has the independent signal-variance ratio below.
    let observations = 128;
    let angle = |row: usize| 2.0 * PI * row as f64 / observations as f64;
    let signal_scale = [2.0_f64, 1.5, 0.8];
    let noise_scale = [1.0_f64, 0.7, 0.5];
    let exog = Mat::from_fn(observations, 1, |row, _| (4.0 * angle(row)).cos());
    let instruments = Mat::from_fn(observations, 3, |row, column| {
        ((column + 1) as f64 * angle(row)).sin()
    });
    let endogenous = Mat::from_fn(observations, 3, |row, column| {
        signal_scale[column] * instruments[(row, column)]
            + noise_scale[column] * ((column + 1) as f64 * angle(row)).cos()
            + 0.3 * (column + 1) as f64 * exog[(row, 0)]
    });
    let response = Col::from_fn(observations, |row| {
        1.0 + 0.7 * exog[(row, 0)] + 2.0 * endogenous[(row, 0)] - 0.5 * endogenous[(row, 1)]
            + 0.4 * endogenous[(row, 2)]
            + (5.0 * angle(row)).cos()
    });
    for order in [[0, 1, 2], [2, 0, 1]] {
        let estimator = IV2SLS {
            endog: response.clone(),
            exog: exog.clone(),
            endog_reg: Mat::from_fn(observations, 3, |row, column| {
                endogenous[(row, order[column])]
            }),
            instruments: instruments.clone(),
            options: yss_sci_contract::regression::OlsOptions::default(),
            small: false,
            endog_names: None,
            z_var_names: None,
        };
        let (_, summary) = estimator.first_stage(false).unwrap();
        assert_eq!(summary.shea_partial_r2.len(), order.len());
        for (&index, &actual) in order.iter().zip(&summary.shea_partial_r2) {
            let expected = signal_scale[index].powi(2)
                / (signal_scale[index].powi(2) + noise_scale[index].powi(2));
            assert!(
                approx_eq(actual, expected, 1e-10, 1e-10),
                "Shea partial R² for endogenous {index}: {actual} != {expected}"
            );
        }
    }
    // A constant column remains a valid one-column design without an added intercept.
    let estimator = IV2SLS {
        endog: Col::from_iter((1..=16).map(f64::from)),
        exog: Mat::zeros(16, 0),
        endog_reg: Mat::from_fn(16, 1, |_, _| 1.0),
        instruments: Mat::from_fn(16, 1, |_, _| 1.0),
        options: yss_sci_contract::regression::OlsOptions {
            constant: false,
            covariance: yss_sci_contract::regression::OlsCovariance::NonRobust,
        },
        small: false,
        endog_names: None,
        z_var_names: None,
    };
    let fit = estimator.fit().unwrap();
    let (equations, first_stage) = estimator.first_stage(false).unwrap();
    let (hausman, endogenous) = estimator.endogeneity(&fit.betas).unwrap();
    assert!(equations.iter().all(|equation| {
        equation
            .betas
            .iter()
            .chain(&equation.stds)
            .chain(&equation.tvalues)
            .chain(&equation.pvalues)
            .chain(&equation.conf_int_left)
            .chain(&equation.conf_int_right)
            .chain([&equation.r2, &equation.r2_adjusted])
            .all(|value| value.is_finite())
    }));
    assert!(
        [
            first_stage.r2,
            first_stage.partial_r2,
            first_stage.f_stat,
            first_stage.f_p_value,
            Some(first_stage.min_eigenvalue),
        ]
        .into_iter()
        .flatten()
        .all(f64::is_finite)
    );
    let endogenous = endogenous.unwrap();
    assert!(
        [
            endogenous.durbin_stat,
            endogenous.durbin_p_value,
            endogenous.wu_stat,
            endogenous.wu_p_value,
        ]
        .into_iter()
        .all(f64::is_finite)
    );
    assert!(
        first_stage.r2_adjusted.is_none() && hausman.is_none(),
        "undefined diagnostics must stay typed None: adjusted R²={:?}, Hausman={hausman:?}",
        first_stage.r2_adjusted,
    );

    let short = IV2SLS {
        endog: Col::from_iter([1.0, 2.0]),
        exog: Mat::zeros(2, 0),
        endog_reg: Mat::from_fn(2, 1, |_, _| 1.0),
        instruments: Mat::from_fn(2, 1, |_, _| 1.0),
        ..estimator
    };
    let fit = short.fit().unwrap();
    let (hausman, endogenous) = short.endogeneity(&fit.betas).unwrap();
    assert!(
        endogenous.is_none(),
        "zero denominator degrees of freedom must be unavailable: {endogenous:?}, Hausman={hausman:?}"
    );
    let hausman = hausman.unwrap();
    assert!(hausman.stat.is_finite() && hausman.p_value.is_finite());
    assert!(hausman.df > 0);
}

#[test]
fn iv_first_stage_rejects_saturated_instrument_inference_without_invalidating_fit() {
    let n = 6;
    let mut state = 1_u64;
    let mut random = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 32) as u32 as f64 / u32::MAX as f64 - 0.5
    };
    let instruments = Mat::from_fn(n, n - 1, |_, _| random());
    let endogenous = Mat::from_fn(n, 2, |_, _| random());
    let response = Col::from_fn(n, |i| {
        1.0 + endogenous[(i, 0)] + 0.7 * endogenous[(i, 1)] + random()
    });
    let model = IV2SLS {
        endog: response,
        exog: Mat::zeros(n, 0),
        endog_reg: endogenous,
        instruments,
        options: yss_sci_contract::regression::OlsOptions::default(),
        small: false,
        endog_names: None,
        z_var_names: None,
    };
    // Structural residual degrees are positive; only first-stage inference is unavailable.
    assert_eq!(model.fit().unwrap().statistics.df_residual, 3);
    for for_liml in [false, true] {
        let result = std::panic::catch_unwind(|| model.first_stage(for_liml));
        assert!(matches!(result, Ok(Err(_))), "{result:?}");
    }
}

#[test]
fn iv_first_stage_minimum_eigenvalue_preserves_generalized_problem_and_column_units() {
    let signal = |row: usize, bit: usize| if row & (1 << bit) == 0 { -1.0 } else { 1.0 };
    let n = 64;
    let instruments = Mat::from_fn(n, 2, signal);
    let endogenous = Mat::from_fn(n, 2, |row, col| {
        if col == 0 {
            2.0 * signal(row, 0) + signal(row, 2)
        } else {
            signal(row, 1) + signal(row, 2) + signal(row, 3)
        }
    });
    for constant in [true, false] {
        // det(diag(4,1) - lambda*[[1,1],[1,2]]) = lambda² - 9lambda + 4.
        let expected = (9.0 - 65.0_f64.sqrt()) / 2.0 * (n - 2 - usize::from(constant)) as f64 / 2.0;
        for (order, scales) in [([0, 1], [1., 1.]), ([1, 0], [1., 1.]), ([0, 1], [1., 100.])] {
            let model = IV2SLS {
                endog: Col::from_fn(n, |row| signal(row, 4)),
                exog: Mat::zeros(n, 0),
                endog_reg: Mat::from_fn(n, 2, |row, col| {
                    endogenous[(row, order[col])] * scales[col]
                }),
                instruments: instruments.clone(),
                options: yss_sci_contract::regression::OlsOptions {
                    constant,
                    ..Default::default()
                },
                small: false,
                endog_names: None,
                z_var_names: None,
            };
            for for_liml in [false, true] {
                let (_, result) = model.first_stage(for_liml).unwrap();
                assert!(
                    approx_eq(result.min_eigenvalue, expected, 1e-10, 1e-10),
                    "{order:?} {scales:?}: {} != {expected}",
                    result.min_eigenvalue
                );
            }
        }
    }
}

#[test]
fn test_ols_golden() {
    let (exog, endog, _weights) = load_iris();
    let ols = OLS {
        endog: endog.clone(),
        exog: exog.clone(),
        config: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
            true,
            "nonrobust",
            (None).as_ref(),
        )
        .expect("valid OLS covariance options"),
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
    let (aic, bic) = compute_aic_bic(o.num_observation, o.betas.nrows(), o.ss_residual);
    assert!(
        approx_eq(aic, 81.99955266474048, TOL, TOL_REL),
        "AIC: got {}",
        aic
    );
    assert!(
        approx_eq(bic, 94.042_093_841_125_5, TOL, TOL_REL),
        "BIC: got {}",
        bic
    );

    // Breusch-Pagan 四种变体
    let fitted: Col<f64> = exog
        .row_iter()
        .map(|row| row.iter().zip(o.betas.iter()).map(|(x, b)| x * b).sum())
        .collect();
    let resid = &endog - &fitted;
    let bp_s = diagnostics::breusch_pagan::breusch_pagan_stata(&resid, &fitted).unwrap();
    let bp_k = diagnostics::breusch_pagan::breusch_pagan_koenker(&resid, &fitted).unwrap();
    let bp_sr = diagnostics::breusch_pagan::breusch_pagan_stata_rhs(&exog, &resid).unwrap();
    let bp_kr = diagnostics::breusch_pagan::breusch_pagan_koenker_rhs(&exog, &resid).unwrap();
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
    let im = diagnostics::im_test::im_test(&exog, &resid).unwrap();
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
    let nt = diagnostics::normality::normality_tests(&resid).unwrap();
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
            covariance: Default::default(),
        },
    };
    let w = wls.fit().unwrap();

    // 模型摘要
    assert_eq!(w.num_observation, 150);
    assert!(
        approx_eq(w.ss_model, 275.1579690774291, TOL, TOL_REL),
        "ss_model: got {}",
        w.ss_model
    );
    assert!(
        approx_eq(w.ss_residual, 42.98617674226099, TOL, TOL_REL),
        "ss_residual: got {}",
        w.ss_residual
    );
    assert!(
        approx_eq(w.ss_total, 318.14414581969004, TOL, TOL_REL),
        "ss_total: got {}",
        w.ss_total
    );
    assert_eq!(w.df_model, 3);
    assert_eq!(w.df_residual, 146);
    assert_eq!(w.df_total, 149);
    assert!(
        approx_eq(w.ms_model, 91.71932302580969, TOL, TOL_REL),
        "ms_model: got {}",
        w.ms_model
    );
    assert!(
        approx_eq(w.ms_residual, 0.294425868097678, TOL, TOL_REL),
        "ms_residual: got {}",
        w.ms_residual
    );
    assert!(
        approx_eq(w.ms_total, 2.1351956095281213, TOL, TOL_REL),
        "ms_total: got {}",
        w.ms_total
    );
    assert!(
        approx_eq(w.r2, 0.8648845898719644, TOL, TOL_REL),
        "r2: got {}",
        w.r2
    );
    assert!(
        approx_eq(w.r2_adjusted, 0.8621082458282376, TOL, TOL_REL),
        "r2_adjusted: got {}",
        w.r2_adjusted
    );
    assert!(
        approx_eq(w.fvalue, 311.51924122163445, TOL, TOL_REL),
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
    let (aic_w, bic_w) = compute_aic_bic(n, w.betas.nrows(), ss_residual_for_ic);
    assert!(
        approx_eq(aic_w, 78.75022376256308, TOL, TOL_REL),
        "AIC: got {}",
        aic_w
    );
    assert!(
        approx_eq(bic_w, 90.792_764_938_948_1, TOL, TOL_REL),
        "BIC: got {}",
        bic_w
    );

    // Breusch-Pagan 加权四种变体
    let fitted_w: Col<f64> = exog
        .row_iter()
        .map(|row| row.iter().zip(w.betas.iter()).map(|(x, b)| x * b).sum())
        .collect();
    let resid_w = &endog - &fitted_w;
    let w_norm: Col<f64> = Col::from_fn(n, |i| weights[i] * n as f64 / sum_w);
    let bp_ws =
        diagnostics::weighted::breusch_pagan_stata_weighted(&resid_w, &fitted_w, &w_norm).unwrap();
    let bp_wk = diagnostics::weighted::breusch_pagan_koenker_weighted(&resid_w, &fitted_w, &w_norm)
        .unwrap();
    let bp_wsr =
        diagnostics::weighted::breusch_pagan_stata_rhs_weighted(&exog, &resid_w, &w_norm).unwrap();
    let bp_wkr =
        diagnostics::weighted::breusch_pagan_koenker_rhs_weighted(&exog, &resid_w, &w_norm)
            .unwrap();
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
    let im_w = diagnostics::im_test::im_test_weighted(&exog, &resid_w, &w_norm).unwrap();
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
    let wresid: Col<f64> = resid_w
        .iter()
        .zip(weights.iter())
        .map(|(r, w)| r * w.sqrt())
        .collect();
    let nt = diagnostics::normality::normality_tests(&wresid).unwrap();
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
        config: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
            true,
            "nonrobust",
            (None).as_ref(),
        )
        .expect("valid OLS covariance options"),
    };
    let o = ols.fit().unwrap();
    let fitted: Col<f64> = exog
        .row_iter()
        .map(|row| row.iter().zip(o.betas.iter()).map(|(x, b)| x * b).sum())
        .collect();
    let resid = &endog - &fitted;

    let white = diagnostics::white::white_test(&exog, &resid).unwrap();
    assert_eq!(white.df, 9);
    assert!(white.lm_stat > 0.0);
    assert!((0.0..=1.0).contains(&white.p_value));

    let reset = diagnostics::reset::reset_test(&endog, &exog, &fitted, None).unwrap();
    assert_eq!(reset.df1, 3);
    assert_eq!(reset.df2, 143);
    assert!(reset.f_stat.is_finite());
    assert!((0.0..=1.0).contains(&reset.p_value));

    let reset_rhs = diagnostics::reset::reset_test_rhs(&endog, &exog, None).unwrap();
    assert_eq!(reset_rhs.df1, 9);
    assert_eq!(reset_rhs.df2, 137);
    assert!(reset_rhs.f_stat.is_finite());
    assert!((0.0..=1.0).contains(&reset_rhs.p_value));

    let vif = diagnostics::vif::vif_centered(&exog, true).unwrap();
    assert_eq!(vif.len(), 4);
    assert!(vif[0].vif.is_none());
    assert!(vif[0].tolerance.is_none());
    assert!(vif.iter().skip(1).all(|entry| {
        entry
            .vif
            .is_some_and(|value| value.is_finite() && value >= 1.0)
    }));

    let leverage = diagnostics::leverage::leverage(&exog).unwrap();
    assert_eq!(leverage.len(), exog.nrows());
    assert!(leverage.iter().all(|v| *v >= 0.0 && *v <= 1.0));
    assert!(approx_eq(
        leverage.iter().sum::<f64>(),
        exog.ncols() as f64,
        1e-8,
        1e-8
    ));
}
