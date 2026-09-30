use super::*;
use std::time::{Duration, Instant};
use yss_sci_contract::execution::ScientificCancellationToken;

fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn gini_and_dagum_match_pairwise_definitions_and_decomposition() {
    let control = control();
    for (values, labels, expected) in [
        (
            vec![1., 2., 3., 4.],
            vec![0, 0, 1, 1],
            [0.25, 0.05, 0.20, 0.0],
        ),
        (
            vec![1., 2., 3., 4.],
            vec![0, 1, 1, 0],
            [0.25, 0.10, 0.0, 0.15],
        ),
        (vec![0., 0., 2., 2.], vec![0, 0, 1, 1], [0.5, 0.0, 0.5, 0.0]),
        (vec![7., 7.], vec![0, 1], [0.0; 4]),
        (vec![1., 2., 3., 4.], vec![0; 4], [0.25, 0.25, 0.0, 0.0]),
    ] {
        for scale in [1.0, 1e-300, 1e307] {
            let scaled = values.iter().map(|x| x * scale).collect::<Vec<_>>();
            let result = dagum_gini(&scaled, &labels, &control).unwrap();
            close(gini(&scaled, &control).unwrap().gini, expected[0]);
            for (actual, expected) in [
                result.gini,
                result.within,
                result.between,
                result.transvariation,
            ]
            .into_iter()
            .zip(expected)
            {
                close(actual, expected);
            }
            close(
                result.within + result.between + result.transvariation,
                result.gini,
            );
            if result.gini == 0.0 {
                assert_eq!(result.within_share, None);
                assert_eq!(result.pairs[0].economic_distance, None);
            } else {
                close(
                    result.within_share.unwrap()
                        + result.between_share.unwrap()
                        + result.transvariation_share.unwrap(),
                    1.0,
                );
            }
        }
    }
    close(
        gini(&[f64::MAX, f64::MAX / 2.0], &control).unwrap().gini,
        1.0 / 6.0,
    );
    close(gini(&[7.], &control).unwrap().gini, 0.0);
    let mixed_scale = dagum_gini(&[1e-300, 2e-300, 1e300], &[0, 1, 2], &control).unwrap();
    assert_eq!(mixed_scale.groups[0].mean, 1e-300);
    assert_eq!(mixed_scale.groups[0].gini, Some(0.0));
    close(mixed_scale.pairs[0].gini.unwrap(), 1.0 / 3.0);
    // Unequal group sizes: the oracle sums raw pair differences rather than sorted gaps.
    let values = [0., 1., 1., 4., 5.];
    let labels = [0, 0, 0, 1, 2];
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let expected = values
        .iter()
        .flat_map(|a| values.iter().map(move |b| (a - b).abs()))
        .sum::<f64>()
        / (2.0 * 25.0 * mean);
    let result = dagum_gini(&values, &labels, &control).unwrap();
    close(result.gini, expected);
    for pair in &result.pairs {
        let a = values
            .iter()
            .zip(labels)
            .filter_map(|(&x, g)| (g == pair.group_a).then_some(x))
            .collect::<Vec<_>>();
        let b = values
            .iter()
            .zip(labels)
            .filter_map(|(&x, g)| (g == pair.group_b).then_some(x))
            .collect::<Vec<_>>();
        let difference = a
            .iter()
            .flat_map(|x| b.iter().map(move |y| (x - y).abs()))
            .sum::<f64>()
            / (a.len() * b.len()) as f64;
        let am = a.iter().sum::<f64>() / a.len() as f64;
        let bm = b.iter().sum::<f64>() / b.len() as f64;
        let mass = (a.len() * b.len()) as f64 / 25.0;
        close(pair.gini.unwrap(), difference / (am + bm));
        close(pair.between_contribution, mass * (am - bm).abs() / mean);
        close(
            pair.transvariation_contribution,
            mass * (difference - (am - bm).abs()) / mean,
        );
    }
    let zeros = dagum_gini(&[0., 0., 1.], &[0, 1, 2], &control).unwrap();
    assert_eq!(zeros.groups[0].gini, None);
    assert_eq!(zeros.pairs[0].gini, None);
    close(zeros.groups[0].within_contribution, 0.0);
}

#[test]
fn gini_inputs_and_group_limits_fail_explicitly_and_honor_control() {
    let control = control();
    for values in [
        vec![],
        vec![0., 0.],
        vec![-1., 2.],
        vec![f64::NAN],
        vec![f64::INFINITY],
    ] {
        assert!(matches!(
            gini(&values, &control),
            Err(Error::InvalidInput { .. })
        ));
        assert!(matches!(
            dagum_gini(&values, &vec![0; values.len()], &control),
            Err(Error::InvalidInput { .. })
        ));
    }
    assert_eq!(
        dagum_gini(&[1., 2.], &[0], &control),
        Err(Error::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch
        })
    );
    assert_eq!(
        dagum_gini(
            &vec![1.; MAX_DAGUM_GROUPS + 1],
            &(0..=MAX_DAGUM_GROUPS).collect::<Vec<_>>(),
            &control
        ),
        Err(Error::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange
        })
    );
    let expired = ScientificExecutionControl {
        deadline: Instant::now(),
        ..control
    };
    assert_eq!(gini(&[1.], &expired), Err(Error::DeadlineExceeded));
    assert_eq!(
        dagum_gini(&[1.], &[0], &expired),
        Err(Error::DeadlineExceeded)
    );
    expired.cancellation.cancel();
    assert_eq!(gini(&[1.], &expired), Err(Error::Cancelled));
    assert_eq!(dagum_gini(&[1.], &[0], &expired), Err(Error::Cancelled));
}

#[test]
fn theil_matches_population_expansion_and_zero_value_limit() {
    let control = control();
    // Three people with 1 and one with 3: half the income belongs to each group.
    let expected = 0.5 * (4.0_f64 / 3.0).ln();
    close(
        theil_t(&[1., 1., 1., 3.], None, &control).unwrap(),
        expected,
    );
    for weights in [[3., 1.], [0.75, 0.25], [300., 100.]] {
        close(
            theil_t(&[1., 3.], Some(&weights), &control).unwrap(),
            expected,
        );
    }
    close(theil_t(&[0., 2.], None, &control).unwrap(), 2.0_f64.ln());
    close(theil_t(&[7., 7.], Some(&[1., 4.]), &control).unwrap(), 0.0);
    close(theil_t(&[7.], None, &control).unwrap(), 0.0);
    close(theil_t(&[0., 7.], Some(&[0., 2.]), &control).unwrap(), 0.0);
}

#[test]
fn theil_preserves_scale_with_overflowing_products_and_extreme_weight_ratios() {
    let control = control();
    let expected = 0.5 * (4.0_f64 / 3.0).ln();
    for values in [[1e307, 3e307], [1e-307, 3e-307]] {
        close(
            theil_t(&values, Some(&[3e307, 1e307]), &control).unwrap(),
            expected,
        );
    }
    // Both income masses are one, although direct normalization of weights
    // would underflow the first population share to zero.
    let actual = theil_t(&[1e308, 1e-308], Some(&[1e-308, 1e308]), &control).unwrap();
    close(actual, 308.0 * 10.0_f64.ln() - 2.0_f64.ln());
}

#[test]
fn theil_rejects_undefined_inputs_and_honors_execution_control() {
    let control = control();
    for (values, weights, violation) in [
        (&[][..], None, ScientificInputViolation::EmptyInput),
        (
            &[1., 2.][..],
            Some(&[1.][..]),
            ScientificInputViolation::ShapeMismatch,
        ),
        (
            &[0., 0.][..],
            None,
            ScientificInputViolation::ParameterOutOfRange,
        ),
        (
            &[-1., 2.][..],
            None,
            ScientificInputViolation::ParameterOutOfRange,
        ),
        (
            &[1., 2.][..],
            Some(&[0., 0.][..]),
            ScientificInputViolation::ParameterOutOfRange,
        ),
        (
            &[1., 2.][..],
            Some(&[-1., 2.][..]),
            ScientificInputViolation::ParameterOutOfRange,
        ),
        (
            &[0., 2.][..],
            Some(&[1., 0.][..]),
            ScientificInputViolation::ParameterOutOfRange,
        ),
        (
            &[f64::NAN][..],
            None,
            ScientificInputViolation::NonFiniteInput,
        ),
        (
            &[f64::INFINITY][..],
            None,
            ScientificInputViolation::NonFiniteInput,
        ),
        (
            &[1.][..],
            Some(&[f64::INFINITY][..]),
            ScientificInputViolation::NonFiniteInput,
        ),
    ] {
        assert_eq!(
            theil_t(values, weights, &control),
            Err(Error::InvalidInput { violation })
        );
    }
    let expired = ScientificExecutionControl {
        deadline: Instant::now(),
        ..control
    };
    assert_eq!(theil_t(&[1.], None, &expired), Err(Error::DeadlineExceeded));
    expired.cancellation.cancel();
    assert_eq!(theil_t(&[1.], None, &expired), Err(Error::Cancelled));
}
