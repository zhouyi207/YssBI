//! Numerical time-series model regressions.
use yss_sci::time_series::unit_root::{AdfRegression, adf_test};
use yss_sci::time_series::var::var_varsoc;
use yss_sci::time_series::vec::{VECConfig, VecTrendSpec, vec_estimate, vec_vecrank_stats};
use yss_sci_linalg::Mat;

#[test]
fn test_var_varsoc_shape_and_lr() {
    // T、K 足够大，两列独立非周期模式，避免任意阶 Z'Z 接近奇异
    let t = 80usize;
    let y = Mat::from_fn(t, 2, |i, j| {
        let i = i as f64;
        if j == 0 {
            0.02 * i + (0.11 * i + 0.3).sin() * 3.0
        } else {
            -0.015 * i + (0.07 * i * i * 0.001 + 1.2).cos() * 2.5 + 5.0
        }
    });
    let r = var_varsoc(y, 3, Some(vec!["a".into(), "b".into()])).unwrap();
    assert_eq!(r.maxlag, 3);
    assert_eq!(r.num_observation, t - r.maxlag);
    // Stata 表：Lag 0 … maxlag
    assert_eq!(r.rows.len(), 4);
    assert_eq!(r.rows[0].lag, 0);
    assert!(r.rows[0].lr.is_none());
    assert_eq!(r.rows[1].lag, 1);
    assert!(r.rows[1].lr.is_some());
    assert_eq!(r.rows[1].lr_df, Some(4));
    assert!(r.rows[1].lr_p.unwrap() >= 0.0 && r.rows[1].lr_p.unwrap() <= 1.0);
}

#[test]
fn test_adf_preserves_drift_and_matches_mackinnon_reference() {
    let y: Vec<f64> = (0..100)
        .map(|i| i as f64 + (i as f64 * 0.1).sin())
        .collect();
    let result = adf_test(&y, 0, true, false).unwrap();

    assert_eq!(result.lags, 0);
    assert_eq!(result.regression, AdfRegression::Drift);
    assert!(result.num_obs > 0);
    assert!(result.test_statistic.is_finite());
    assert!(result.p_value >= 0.0 && result.p_value <= 1.0);
    assert!(!result.regression_table.is_empty());
    assert!(result.use_t_distribution);

    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/panel_category_reference.json")).unwrap();
    let data = &fixture["nonstationary"];
    let response: Vec<f64> = serde_json::from_value(data["response"].clone()).unwrap();
    let entity: Vec<f64> = serde_json::from_value(data["entity"].clone()).unwrap();
    for case in data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["method"] == "unit_root" && case["regression"] != "constant")
    {
        let trend = case["regression"] == "trend";
        let lags = case["lags"].as_u64().unwrap() as usize;
        for (index, expected) in case["entity_tests"].as_array().unwrap().iter().enumerate() {
            let values = response
                .iter()
                .zip(&entity)
                .filter_map(|(&value, &group)| (group == index as f64).then_some(value))
                .collect::<Vec<_>>();
            let result = adf_test(&values, lags, trend, trend).unwrap();
            assert_eq!(
                result.num_obs,
                expected["observations"].as_u64().unwrap() as usize
            );
            assert!(!result.use_t_distribution);
            for (name, actual) in [
                ("statistic", result.test_statistic),
                ("p_value", result.p_value),
            ] {
                let reference = expected[name].as_f64().unwrap();
                assert!(
                    (actual - reference).abs() < 1e-7 * (1.0 + reference.abs()),
                    "{} entity {index} {name}: {actual} != {reference}",
                    case["regression"]
                );
            }
        }
    }
}

#[test]
fn adf_requires_residual_degrees_and_valid_deterministic_terms() {
    let values = [0., 1., 4., 2., 5., 3.];
    for (sample, lags, constant, trend, statistic, standard_error) in [
        (
            &values[..4],
            0,
            true,
            false,
            -1.3121597027036949,
            0.7327907262791404,
        ),
        (
            &values[..5],
            1,
            false,
            false,
            0.47905316946566634,
            0.9310227300423423,
        ),
        (
            &values[..5],
            0,
            true,
            true,
            -2.44476765539977,
            0.7136387162233372,
        ),
        (
            &values[..6],
            1,
            true,
            false,
            -3.0542361089076304,
            0.35485633404071315,
        ),
    ] {
        // Independent exact-rational least squares; each regression has one residual degree.
        let result = adf_test(sample, lags, constant, trend).unwrap();
        assert!((result.test_statistic - statistic).abs() < 1e-11);
        assert!((result.std_err_lagged - standard_error).abs() < 1e-11);
        if result.use_t_distribution {
            let expected = 0.5 + statistic.atan() / std::f64::consts::PI;
            assert!((result.p_value - expected).abs() < 1e-12);
            assert!((result.critical_value_5pct + 6.313751514675044).abs() < 1e-11);
        }
    }
    let original = adf_test(&values[..4], 0, true, false).unwrap();
    let scaled = values[..4]
        .iter()
        .map(|value| value * 1e-20)
        .collect::<Vec<_>>();
    let rescaled = adf_test(&scaled, 0, true, false).unwrap();
    for (original, rescaled) in original
        .regression_table
        .iter()
        .zip(&rescaled.regression_table)
    {
        assert!((original.t - rescaled.t).abs() < 1e-11);
        assert!((original.p_value - rescaled.p_value).abs() < 1e-12);
    }
    for (sample, lags, constant, trend) in [
        (&values[..4], 0, true, true),
        (&values[..5], 1, true, false),
        (&values[..6], 2, false, false),
        (&values[..6], 0, false, true),
    ] {
        let outcome = std::panic::catch_unwind(|| adf_test(sample, lags, constant, trend));
        assert!(
            matches!(outcome, Ok(Err(_))),
            "invalid ADF specification must fail without panicking: {outcome:?}"
        );
    }
    let undefined = std::panic::catch_unwind(|| adf_test(&[1., 2., 4., 8.], 0, false, false));
    assert!(matches!(undefined, Ok(Err(_))), "{undefined:?}");
}

#[test]
fn test_vec_estimate_rejects_invalid_config() {
    let n = 80usize;
    let y = Mat::from_fn(n, 2, |i, j| {
        let t = i as f64;
        let base = 0.05 * t + (0.1 * t).sin();
        if j == 0 {
            base
        } else {
            base + 0.2 + (0.13 * t).cos() * 0.01
        }
    });
    let config = VECConfig {
        trend_spec: VecTrendSpec::Constant,
        lags: 0,
        rank: 1,
    };

    let err = vec_estimate(&y, &config, Some(vec!["y1".into(), "y2".into()]), None).unwrap_err();
    assert!(err.contains("lags must be >= 1"));

    for (observations, lags) in [(2, 3), (0, 1)] {
        let sample = Mat::zeros(observations, 2);
        let config = VECConfig {
            trend_spec: VecTrendSpec::Constant,
            lags,
            rank: 1,
        };
        assert!(vec_estimate(&sample, &config, None, None).is_err());
        assert!(
            vec_vecrank_stats(&sample, lags, VecTrendSpec::Constant, None, true, None).is_err()
        );
    }
}
