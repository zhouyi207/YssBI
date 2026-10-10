use super::*;
use std::time::{Duration, Instant};
use yss_sci_contract::association::*;
use yss_sci_contract::execution::ScientificCancellationToken;

fn control() -> Control {
    Control {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/reference.json")).unwrap()
}
fn numbers(value: &serde_json::Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}
fn matrix_values(value: &serde_json::Value) -> Vec<Vec<f64>> {
    serde_json::from_value(value.clone()).unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
fn ci_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-7 * expected.abs().max(1.0),
        "CI {actual} != {expected}"
    );
}

#[test]
fn kappa_retains_interval_width_at_tiny_confidence() {
    let confidence = 1e-20;
    let result = kappa(
        &[vec![0, 0, 1, 1], vec![0, 1, 0, 1]],
        2,
        KappaOptions {
            method: KappaMethod::Cohen,
            weighting: KappaWeighting::None,
            confidence_level: confidence,
        },
        &control(),
    )
    .unwrap();
    assert_eq!(result.coefficient, 0.0);
    let standard_error = result.standard_error.unwrap();
    assert!(standard_error > 0.0);
    // The Gaussian central expansion has relative error O(confidence^2).
    let margin = (std::f64::consts::PI / 2.0).sqrt() * confidence * standard_error;
    let interval = result.confidence_interval.unwrap();
    assert_eq!(interval.level, confidence);
    for (actual, expected) in [(interval.lower, -margin), (interval.upper, margin)] {
        assert!(
            (actual / expected - 1.0).abs() < 1e-13,
            "{actual} != {expected}"
        );
    }
}

#[test]
fn pearson_retains_finite_interval_at_near_one_confidence() {
    let confidence = f64::from_bits(1.0_f64.to_bits() - 1);
    let result = pearson(
        &[-1.0, 0.0, 1.0, 0.0],
        &[0.0, -1.0, 0.0, 1.0],
        CorrelationOptions {
            confidence_level: confidence,
            ..Default::default()
        },
        &control(),
    )
    .unwrap();
    assert_eq!(result.coefficient, 0.0);
    // Independent 110-digit Gaussian tail inversion and Fisher transform; n - 3 = 1.
    let bound = 0.999_999_874_577_153_2;
    let interval = result.confidence_interval.unwrap();
    assert_eq!(interval.level, confidence);
    for (actual, expected) in [(interval.lower, -bound), (interval.upper, bound)] {
        assert!(actual.is_finite() && actual.abs() < 1.0);
        assert!((actual - expected).abs() <= 2.0 * f64::EPSILON);
    }
}

#[test]
fn correlations_match_scipy_inference_and_exact_tie_permutations() {
    let fixture = fixture();
    let control = control();
    let input = &fixture["pearson"];
    let x = numbers(&input["x"]);
    let y = numbers(&input["y"]);
    for scale in [1.0, 1e307] {
        let sx = x.iter().map(|x| x * scale).collect::<Vec<_>>();
        let sy = y.iter().map(|y| y * scale).collect::<Vec<_>>();
        let result = pearson(&sx, &sy, CorrelationOptions::default(), &control).unwrap();
        close(result.coefficient, input["coefficient"].as_f64().unwrap());
        close(
            result.inference.p_value.unwrap(),
            input["p_value"].as_f64().unwrap(),
        );
        let ci = result.confidence_interval.unwrap();
        ci_close(ci.lower, input["lower"].as_f64().unwrap());
        ci_close(ci.upper, input["upper"].as_f64().unwrap());
    }
    let input = &fixture["rank_asymptotic"];
    let x = numbers(&input["x"]);
    let y = numbers(&input["y"]);
    let options = RankCorrelationOptions {
        inference: RankInference::Asymptotic,
        ..Default::default()
    };
    let s = spearman(&x, &y, options, &control).unwrap();
    let k = kendall(&x, &y, options, &control).unwrap();
    close(
        s.coefficient,
        input["spearman_coefficient"].as_f64().unwrap(),
    );
    close(
        s.inference.p_value.unwrap(),
        input["spearman_p"].as_f64().unwrap(),
    );
    close(
        k.coefficient,
        input["kendall_coefficient"].as_f64().unwrap(),
    );
    close(
        k.inference.p_value.unwrap(),
        input["kendall_p"].as_f64().unwrap(),
    );
    let input = &fixture["rank_exact"];
    let x = numbers(&input["x"]);
    let y = numbers(&input["y"]);
    for (alternative, suffix) in [
        (Alternative::TwoSided, "two-sided"),
        (Alternative::Greater, "greater"),
        (Alternative::Less, "less"),
    ] {
        let options = RankCorrelationOptions {
            alternative,
            inference: RankInference::PermutationExact,
        };
        for (method, result) in [
            ("spearman", spearman(&x, &y, options, &control).unwrap()),
            ("kendall", kendall(&x, &y, options, &control).unwrap()),
        ] {
            close(
                result.coefficient,
                input[format!("{method}_coefficient")].as_f64().unwrap(),
            );
            close(
                result.inference.p_value.unwrap(),
                input[format!("{method}_{suffix}")].as_f64().unwrap(),
            );
            assert_eq!(result.inference.method, "permutation_exact");
        }
    }
}

#[test]
fn partial_correlation_matches_independent_least_squares_and_rejects_rank_loss() {
    let fixture = fixture();
    let input = &fixture["partial"];
    let control = control();
    let x = numbers(&input["x"]);
    let y = numbers(&input["y"]);
    let controls = matrix_values(&input["controls"]);
    let result = partial(&x, &y, &controls, CorrelationOptions::default(), &control).unwrap();
    close(result.coefficient, input["coefficient"].as_f64().unwrap());
    close(
        result.inference.p_value.unwrap(),
        input["p_value"].as_f64().unwrap(),
    );
    let ci = result.confidence_interval.unwrap();
    ci_close(ci.lower, input["lower"].as_f64().unwrap());
    ci_close(ci.upper, input["upper"].as_f64().unwrap());
    assert!(
        partial(
            &x,
            &y,
            &[controls[0].clone(), controls[0].clone()],
            CorrelationOptions::default(),
            &control
        )
        .is_err()
    );
    assert!(
        partial(
            &x,
            &y,
            std::slice::from_ref(&x),
            CorrelationOptions::default(),
            &control
        )
        .is_err()
    );
}

#[test]
fn kappa_weights_and_fleiss_match_reference_estimators() {
    let fixture = fixture();
    let control = control();
    let input = &fixture["cohen_kappa"];
    let ratings: Vec<Vec<usize>> = serde_json::from_value(input["ratings"].clone()).unwrap();
    for (weighting, key) in [
        (KappaWeighting::None, "none"),
        (KappaWeighting::Linear, "linear"),
        (KappaWeighting::Quadratic, "quadratic"),
    ] {
        let result = kappa(
            &ratings,
            3,
            KappaOptions {
                method: KappaMethod::Cohen,
                weighting,
                confidence_level: 0.95,
            },
            &control,
        )
        .unwrap();
        let reference = &input["results"][key];
        close(
            result.coefficient,
            reference["coefficient"].as_f64().unwrap(),
        );
        close(
            result.standard_error.unwrap(),
            reference["standard_error"].as_f64().unwrap(),
        );
        close(
            result.inference.unwrap().p_value.unwrap(),
            reference["p_value"].as_f64().unwrap(),
        );
        let ci = result.confidence_interval.unwrap();
        ci_close(ci.lower, reference["lower"].as_f64().unwrap());
        ci_close(ci.upper, reference["upper"].as_f64().unwrap());
    }
    let input = &fixture["fleiss_kappa"];
    let ratings: Vec<Vec<usize>> = serde_json::from_value(input["ratings"].clone()).unwrap();
    let result = kappa(
        &ratings,
        3,
        KappaOptions {
            method: KappaMethod::Fleiss,
            weighting: KappaWeighting::None,
            confidence_level: 0.95,
        },
        &control,
    )
    .unwrap();
    close(result.coefficient, input["coefficient"].as_f64().unwrap());
    close(
        result.inference.unwrap().p_value.unwrap(),
        input["p_value"].as_f64().unwrap(),
    );
    assert!(result.contingency_table.is_none());
}

#[test]
fn icc_definitions_and_intervals_match_anova_reference() {
    let fixture = fixture();
    let input = &fixture["icc"];
    let ratings = matrix_values(&input["ratings"]);
    let control = control();
    for (kind, key) in [
        (IccType::Icc1, "ICC1"),
        (IccType::Icc2, "ICC2"),
        (IccType::Icc3, "ICC3"),
        (IccType::Icc1k, "ICC1k"),
        (IccType::Icc2k, "ICC2k"),
        (IccType::Icc3k, "ICC3k"),
    ] {
        let result = icc(&ratings, kind, 0.95, &control).unwrap();
        let reference = &input["results"][key];
        close(
            result.coefficient,
            reference["coefficient"].as_f64().unwrap(),
        );
        close(
            result.inference.p_value.unwrap(),
            reference["p_value"].as_f64().unwrap(),
        );
        let ci = result.confidence_interval.unwrap();
        ci_close(ci.lower, reference["lower"].as_f64().unwrap());
        ci_close(ci.upper, reference["upper"].as_f64().unwrap());
    }
}

#[test]
fn bland_altman_limits_use_complete_data_and_bound_display_points() {
    let fixture = fixture();
    let input = &fixture["bland_altman"];
    let x = numbers(&input["x"]);
    let y = numbers(&input["y"]);
    let control = control();
    let result = bland_altman(&x, &y, 0.95, 0.95, &control).unwrap();
    for (actual, key) in [
        (result.bias, "bias"),
        (result.standard_deviation, "sd"),
        (result.lower_limit, "lower"),
        (result.upper_limit, "upper"),
        (result.bias_confidence_interval.lower, "bias_lower"),
        (result.bias_confidence_interval.upper, "bias_upper"),
        (
            result.lower_limit_confidence_interval.lower,
            "limit_lower_ci",
        ),
        (
            result.upper_limit_confidence_interval.upper,
            "limit_upper_ci",
        ),
    ] {
        ci_close(actual, input[key].as_f64().unwrap());
    }
    let n = 5001;
    let x = (0..n).map(|i| i as f64).collect::<Vec<_>>();
    let y = x.iter().map(|x| x - 2.0).collect::<Vec<_>>();
    let result = bland_altman(&x, &y, 0.95, 0.95, &control).unwrap();
    close(result.bias, 2.0);
    close(result.standard_deviation, 0.0);
    assert!(result.sampled);
    assert_eq!(result.points.len(), MAX_BLAND_ALTMAN_POINTS);
    assert_eq!(result.points.first().unwrap().observation, 0);
    assert_eq!(result.points.last().unwrap().observation, n - 1);
}

#[test]
fn concordance_ridit_and_rwg_keep_their_distinct_definitions() {
    let control = control();
    let ratings = vec![
        vec![1., 1., 2., 3.],
        vec![1., 2., 2., 3.],
        vec![1., 1., 3., 4.],
    ];
    let w = kendall_w(&ratings, &control).unwrap();
    // Rank sums [4,5.5,8.5,12] give S=37.5 and tie-adjusted rank variance=40.5.
    close(w.coefficient, 25.0 / 27.0);
    let fixture = fixture();
    let input = &fixture["ridit"];
    let sample: Vec<usize> = serde_json::from_value(input["sample"].clone()).unwrap();
    let reference: Vec<usize> = serde_json::from_value(input["reference"].clone()).unwrap();
    let result = ridit(
        &sample,
        &reference,
        5,
        Alternative::TwoSided,
        false,
        &control,
    )
    .unwrap();
    close(result.mean_ridit, input["mean_ridit"].as_f64().unwrap());
    close(
        result.inference.p_value.unwrap(),
        input["p_value"].as_f64().unwrap(),
    );
    close(
        sum(result
            .categories
            .iter()
            .map(|row| row.reference_proportion * row.ridit)),
        0.5,
    );
    let result = rwg(
        &[vec![3., 3., 3., 3.], vec![1., 2., 3., 4.]],
        AgreementNull::Uniform { scale_points: 5 },
        &control,
    )
    .unwrap();
    close(result.expected_variance, 2.0);
    close(result.mean_observed_variance, 5.0 / 6.0);
    close(result.rwg_j, 14.0 / 19.0);
    let result = rwg(
        &[vec![1., 5.]],
        AgreementNull::Uniform { scale_points: 5 },
        &control,
    )
    .unwrap();
    close(result.rwg_j, 0.0);
    close(result.items[0].raw_rwg, -3.0);
    assert!(result.variance_truncated);
}

#[test]
fn association_invalid_inputs_and_execution_control_fail_explicitly() {
    let control = control();
    let options = CorrelationOptions::default();
    assert_eq!(
        pearson(&[1., 1., 1.], &[1., 2., 3.], options, &control),
        Err(invalid(Violation::DataOutOfRange)),
    );
    assert_eq!(
        pearson(&[1., 2., 3.], &[1., 2.], options, &control),
        Err(invalid(Violation::ShapeMismatch)),
    );
    assert_eq!(
        kendall(
            &[1., 1., 1.],
            &[1., 2., 3.],
            RankCorrelationOptions::default(),
            &control
        ),
        Err(invalid(Violation::DataOutOfRange)),
    );
    assert_eq!(
        kappa(
            &[vec![0, 0], vec![0, 0]],
            2,
            KappaOptions {
                method: KappaMethod::Cohen,
                weighting: KappaWeighting::None,
                confidence_level: 0.95
            },
            &control
        ),
        Err(invalid(Violation::DataOutOfRange)),
    );
    assert_eq!(
        icc(&[vec![1., 1.], vec![1., 1.]], IccType::Icc2, 0.95, &control),
        Err(invalid(Violation::DataOutOfRange)),
    );
    assert_eq!(
        rwg(
            &[vec![1., 1.5]],
            AgreementNull::Uniform { scale_points: 5 },
            &control
        ),
        Err(invalid(Violation::DataOutOfRange)),
    );
    let ordered = (0..10).map(|i| i as f64).collect::<Vec<_>>();
    assert_eq!(
        spearman(
            &ordered,
            &ordered,
            RankCorrelationOptions {
                inference: RankInference::PermutationExact,
                ..Default::default()
            },
            &control
        ),
        Err(invalid(Violation::ParameterOutOfRange)),
    );
    assert_eq!(
        rwg(
            &[vec![1., 2.]],
            AgreementNull::SpecifiedVariance { variance: 0.0 },
            &control,
        ),
        Err(invalid(Violation::ParameterOutOfRange)),
    );
    assert_eq!(
        icc(&[vec![1., 2.]], IccType::Icc2, 0.95, &control),
        Err(invalid(Violation::ShapeMismatch)),
    );
    assert_eq!(
        partial(&[1., 2., 3.], &[3., 1., 2.], &[], options, &control),
        Err(invalid(Violation::ShapeMismatch)),
    );
    assert_eq!(
        partial(
            &[1., 2., 3.],
            &[3., 1., 2.],
            &[vec![2., 3., 1.]],
            options,
            &control,
        ),
        Err(invalid(Violation::EmptyInput)),
    );
    assert_eq!(
        ridit(&[2], &[0, 1], 2, Alternative::TwoSided, false, &control),
        Err(invalid(Violation::DataOutOfRange)),
    );
    assert_eq!(
        ridit(&[], &[0, 1], 2, Alternative::TwoSided, false, &control),
        Err(invalid(Violation::EmptyInput)),
    );
    let expired = Control {
        deadline: Instant::now(),
        ..control
    };
    assert_eq!(
        pearson(&[1., 2., 3.], &[1., 2., 3.], options, &expired),
        Err(Error::DeadlineExceeded)
    );
    expired.cancellation.cancel();
    assert_eq!(
        kendall(
            &[1., 2., 3.],
            &[1., 2., 3.],
            RankCorrelationOptions::default(),
            &expired
        ),
        Err(Error::Cancelled)
    );
}
