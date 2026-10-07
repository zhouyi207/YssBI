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
