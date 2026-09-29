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
