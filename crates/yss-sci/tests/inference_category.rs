use std::time::{Duration, Instant};
use yss_sci::inference::{comparisons, intervals};
use yss_sci_contract::{execution::*, inference::*};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/inference_reference.json")).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * (1. + b.abs()), "{a} != {b}");
}

#[test]
fn coefficient_plot_preserves_confidence_before_cdf_rounding() {
    // Independent 110-digit density/Beta references, including the exact Cauchy tail.
    for (df, confidence, expected) in [
        (64.0, 1e-20, 1.2582192702251038e-20),
        (
            1.0,
            f64::from_bits(1.0_f64.to_bits() - 1),
            5734161139222659.0,
        ),
    ] {
        let result = yss_sci::visualization::coefficients(
            &["coefficient".into()],
            &[0.0],
            &[1.0],
            df,
            confidence,
            &control(),
        )
        .unwrap();
        let row = &result.data[0];
        assert!(row.lower < 0.0 && row.upper > 0.0);
        assert!((row.upper / expected - 1.0).abs() < 1e-12);
        assert!((row.lower / expected + 1.0).abs() < 1e-12);
        assert_eq!(result.confidence_level, confidence);
    }
}

#[test]
fn coefficient_plot_retains_accurate_student_width_at_large_degrees() {
    let result = yss_sci::visualization::coefficients(
        &["coefficient".into()],
        &[0.0],
        &[1.0],
        1_000_000.0,
        0.95,
        &control(),
    )
    .unwrap();
    // Independent 110-digit inversion of the full Student-t density series.
    let expected = 1.9599663568141066;
    assert!((result.data[0].upper - expected).abs() < 1e-12);
    assert!((result.data[0].lower + expected).abs() < 1e-12);
}

#[test]
fn confidence_intervals_match_quantiles_keep_all_rows_and_check_inputs() {
    let f = fixture();
    let data = (0..640).map(|i| i as f64 / 10.).collect::<Vec<_>>();
    for (df, key) in [(None, "normal_q"), (Some(12.5), "t_q")] {
        let options = IntervalOptions {
            confidence_level: 0.95,
            degrees_of_freedom: df,
        };
        let result = intervals::calculate(&data, &vec![0.5; 640], options, &control()).unwrap();
        assert_eq!(result.rows.len(), 640);
        close(
            result.rows[639].upper,
            63.9 + 0.5 * f[key].as_f64().unwrap(),
        );
        close(
            intervals::calculate(&[2.], &[0.], options, &control())
                .unwrap()
                .rows[0]
                .lower,
            2.,
        );
        assert!(intervals::calculate(&[2.], &[-1.], options, &control()).is_err());
        assert!(intervals::calculate(&[2.], &[], options, &control()).is_err());
        assert!(
            intervals::calculate(
                &[2.],
                &[1.],
                IntervalOptions {
                    confidence_level: 1.,
                    ..options
                },
                &control()
            )
            .is_err()
        );
        let c = control();
        c.cancellation.cancel();
        assert!(matches!(
            intervals::calculate(&[2.], &[1.], options, &c),
            Err(ScientificComputationError::Cancelled)
        ));
    }
}
#[test]
fn student_confidence_preserves_tiny_width_and_knapp_hartung_inference() {
    let confidence = 1e-20;
    check_student_confidence(confidence);
    // Exact half-integer Gamma recurrence, evaluated with 100-digit Decimal factorials.
    check_student_width(confidence, 64.0, 1.2582192702251038e-20);
    let result = yss_sci::meta::model::fit(
        &[-1.0, 0.0, 1.0],
        &[1.0; 3],
        &[],
        yss_sci_contract::meta::MetaOptions {
            estimator: yss_sci_contract::meta::MetaEstimator::Fixed,
            inference: yss_sci_contract::meta::MetaInference::KnappHartung,
            confidence_level: confidence,
        },
        &control(),
    )
    .unwrap();
    let [lower, upper] = result.summary.coefficients[0].confidence_interval.unwrap();
    let expected = std::f64::consts::SQRT_2 * confidence / 3.0f64.sqrt();
    assert!(lower < 0.0 && upper > 0.0);
    assert!((upper / expected - 1.0).abs() < 1e-12);
    assert!((lower / expected + 1.0).abs() < 1e-12);
}

#[test]
fn student_confidence_retains_center_precision_and_nonlinear_width() {
    // The first case keeps a representable tail but loses precision in inverse Beta.
    // The second needs the nonlinear terms of the central quantile.
    for confidence in [1e-10, 1e-3] {
        check_student_confidence(confidence);
    }
}

#[test]
fn student_confidence_large_degrees_preserves_accuracy_and_finite_limit() {
    // Independent 110-digit integration of the Student density.
    check_student_width(0.95, 1e6, 1.9599663568141066);
    check_student_width(0.95, 1e16, 1.959963984540054);
}

#[test]
fn student_confidence_tiny_degrees_keeps_a_representable_nonlinear_width() {
    // In hyperbolic coordinates, central probability tends to df * u.
    // At confidence=df, t/sqrt(df) tends to sinh(1); the correction is below rounding.
    for df in [1e-24_f64, f64::from_bits(1)] {
        check_student_width(df, df, df.sqrt() * 1.0_f64.sinh());
    }
}

fn check_student_confidence(confidence: f64) {
    check_student_width(
        confidence,
        1.0,
        (std::f64::consts::FRAC_PI_2 * confidence).tan(),
    );
    check_student_width(
        confidence,
        2.0,
        std::f64::consts::SQRT_2 * confidence / (1.0 - confidence * confidence).sqrt(),
    );
}

fn check_student_width(confidence: f64, df: f64, critical: f64) {
    let result = intervals::calculate(
        &[0.0],
        &[1.0],
        IntervalOptions {
            confidence_level: confidence,
            degrees_of_freedom: Some(df),
        },
        &control(),
    )
    .unwrap();
    assert_eq!(result.summary.reference_distribution, "student_t");
    assert_eq!(result.summary.degrees_of_freedom, Some(df));
    assert!(result.rows[0].upper > 0.0 && result.rows[0].lower < 0.0);
    assert!((result.rows[0].upper / critical - 1.0).abs() < 1e-12);
    assert!((result.rows[0].lower / critical + 1.0).abs() < 1e-12);
}

#[test]
fn normal_confidence_edges_keep_arima_intervals_finite_near_one() {
    // Independent 140-digit Gaussian integration at the largest f64 below one.
    check_normal_confidence_edges(f64::from_bits(1.0f64.to_bits() - 1), 8.292361075813595);
}

#[test]
fn normal_confidence_edges_preserve_tiny_positive_width_and_meta_inference() {
    let confidence = 1e-20;
    // Independent Gaussian integration; the first-order width is sqrt(pi / 2) * confidence.
    let critical = 1.2533141373155002e-20;
    check_normal_confidence_edges(confidence, critical);
    let result = yss_sci::meta::model::fit(
        &[0.0; 3],
        &[1.0; 3],
        &[],
        yss_sci_contract::meta::MetaOptions {
            estimator: yss_sci_contract::meta::MetaEstimator::Fixed,
            inference: yss_sci_contract::meta::MetaInference::Wald,
            confidence_level: confidence,
        },
        &control(),
    )
    .unwrap();
    let [lower, upper] = result.summary.coefficients[0].confidence_interval.unwrap();
    let expected = critical / 3.0f64.sqrt();
    assert!(lower < 0.0 && upper > 0.0);
    assert!((upper / expected - 1.0).abs() < 1e-12);
    assert!((lower / expected + 1.0).abs() < 1e-12);
}

fn check_normal_confidence_edges(confidence: f64, critical: f64) {
    let result = intervals::calculate(
        &[0.0],
        &[1.0],
        IntervalOptions {
            confidence_level: confidence,
            degrees_of_freedom: None,
        },
        &control(),
    )
    .unwrap();
    assert!(result.rows[0].lower.is_finite() && result.rows[0].lower < 0.0);
    assert!(result.rows[0].upper.is_finite() && result.rows[0].upper > 0.0);
    assert!((result.rows[0].upper / critical - 1.0).abs() < 1e-12);
    assert!((result.rows[0].lower / critical + 1.0).abs() < 1e-12);
    let forecast = yss_sci::time_series::forecast::arima(
        &[-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0],
        yss_sci_contract::time_series::forecast::ArimaOptions {
            p: 0,
            d: 0,
            q: 0,
            seasonal_p: 0,
            seasonal_d: 0,
            seasonal_q: 0,
            period: 1,
            constant: false,
            horizon: 3,
            confidence,
            iteration: yss_sci_contract::regression::models::IterationOptions::default(),
        },
        &control(),
    )
    .unwrap();
    assert_eq!(forecast.innovation_variance, 1.0);
    assert_eq!(forecast.forecasts, vec![0.0; 3]);
    for (&lower, &upper) in forecast
        .lower
        .as_ref()
        .unwrap()
        .iter()
        .zip(forecast.upper.as_ref().unwrap())
    {
        assert!(lower.is_finite() && lower < 0.0);
        assert!(upper.is_finite() && upper > 0.0);
        assert!((upper / critical - 1.0).abs() < 1e-12);
        assert!((lower / critical + 1.0).abs() < 1e-12);
    }
}

#[test]
fn pairwise_confidence_preserves_tiny_width_for_a_single_comparison() {
    let confidence = 1e-20;
    for equal_variances in [true, false] {
        for adjustment in [
            ComparisonAdjustment::None,
            ComparisonAdjustment::Holm,
            ComparisonAdjustment::Bonferroni,
        ] {
            let result = comparisons::pairwise(
                &[-1., 1., -1., 1.],
                &[0, 0, 1, 1],
                PairwiseOptions {
                    equal_variances,
                    adjustment,
                    confidence_level: confidence,
                },
                &control(),
            )
            .unwrap();
            let row = &result.rows[0];
            assert_eq!(row.estimate, 0.0);
            assert_eq!(row.degrees_of_freedom, 2.0);
            assert_eq!(row.p_value, 1.0);
            assert!(row.lower < 0.0 && row.upper > 0.0);
            // For df=2, q=sqrt(2)*confidence to rounding; SE=sqrt(2).
            assert!((row.upper / (2.0 * confidence) - 1.0).abs() < 1e-12);
            assert!((row.lower / (2.0 * confidence) + 1.0).abs() < 1e-12);
            assert_eq!(
                result.summary.intervals_adjusted,
                adjustment == ComparisonAdjustment::Bonferroni
            );
        }
    }
}

#[test]
fn pairwise_statistics_preserve_response_units_and_distinct_group_scales() {
    for equal_variances in [true, false] {
        let options = PairwiseOptions {
            equal_variances,
            adjustment: ComparisonAdjustment::None,
            confidence_level: 0.95,
        };
        let df = if equal_variances { 2.0 } else { 25.0 / 17.0 };
        // Independent df-2 formula and 110-digit Student-density integration.
        let critical = if equal_variances {
            4.302652729749464
        } else {
            6.188114940769251
        };
        // +/-100 exposes Welch fourth powers; +/-200 exposes raw second moments.
        for scale in [1e-200, 1e-100, 1e100, 1e200] {
            let y = [-scale, scale, -2.0 * scale, 2.0 * scale];
            let result = comparisons::pairwise(&y, &[0, 0, 1, 1], options, &control()).unwrap();
            let row = &result.rows[0];
            assert_eq!(row.estimate, 0.0);
            assert_eq!(row.statistic, 0.0);
            assert_eq!(row.p_value, 1.0);
            assert!((row.degrees_of_freedom - df).abs() < 1e-12);
            assert!((row.standard_error / scale - 5.0_f64.sqrt()).abs() < 1e-12);
            let width = critical * 5.0_f64.sqrt() * scale;
            assert!((row.upper / width - 1.0).abs() < 1e-12);
            assert!((row.lower / width + 1.0).abs() < 1e-12);
            for (group, factor) in result.summary.groups.iter().zip([1.0, 2.0]) {
                assert_eq!(group.mean, 0.0);
                assert!(
                    (group.standard_deviation.unwrap() / scale - factor * std::f64::consts::SQRT_2)
                        .abs()
                        < 1e-12
                );
            }
        }
        let result = comparisons::pairwise(
            &[-1e200, 1e200, -1e-200, 1e-200],
            &[0, 0, 1, 1],
            options,
            &control(),
        )
        .unwrap();
        for (group, scale) in result.summary.groups.iter().zip([1e200, 1e-200]) {
            assert!(
                (group.standard_deviation.unwrap() / scale - std::f64::consts::SQRT_2).abs()
                    < 1e-12
            );
        }
        assert!((result.rows[0].standard_error / 1e200 - 1.0).abs() < 1e-12);
        assert!(
            (result.rows[0].degrees_of_freedom - if equal_variances { 2.0 } else { 1.0 }).abs()
                < 1e-12
        );
    }
}

#[test]
fn independent_group_comparisons_match_ols_welch_and_multipletests() {
    let f = fixture();
    let y: Vec<f64> = serde_json::from_value(f["y"].clone()).unwrap();
    let groups: Vec<usize> = serde_json::from_value(f["groups"].clone()).unwrap();
    for case in f["cases"].as_array().unwrap() {
        let options = PairwiseOptions {
            confidence_level: 0.9,
            equal_variances: case["pooled"] == true,
            adjustment: match case["adjustment"].as_str().unwrap() {
                "none" => ComparisonAdjustment::None,
                "holm" => ComparisonAdjustment::Holm,
                _ => ComparisonAdjustment::Bonferroni,
            },
        };
        let result = comparisons::pairwise(&y, &groups, options, &control()).unwrap();
        assert_eq!(result.rows.len(), 3);
        assert_eq!(
            result.summary.intervals_adjusted,
            options.adjustment == ComparisonAdjustment::Bonferroni
        );
        for (row, expected) in result.rows.iter().zip(case["rows"].as_array().unwrap()) {
            assert_eq!(row.group_a, expected["a"].as_u64().unwrap() as usize);
            assert_eq!(row.group_b, expected["b"].as_u64().unwrap() as usize);
            for (a, key) in [
                (row.estimate, "estimate"),
                (row.standard_error, "se"),
                (row.degrees_of_freedom, "df"),
                (row.statistic, "t"),
                (row.p_value, "p"),
                (row.adjusted_p_value, "adjusted_p"),
                (row.lower, "lower"),
                (row.upper, "upper"),
            ] {
                close(a, expected[key].as_f64().unwrap());
            }
        }
        assert!(comparisons::pairwise(&y, &groups[1..], options, &control()).is_err());
        assert!(comparisons::pairwise(&[1., 2.], &[0, 1], options, &control()).is_err());
    }
}
