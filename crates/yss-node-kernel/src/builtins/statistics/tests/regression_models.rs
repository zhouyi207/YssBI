use super::*;

#[test]
fn robust_regression_distinguishes_undefined_data_from_invalid_tuning() {
    for (response, predictors, tuning, parameter_error) in [
        (vec![1., 3.], vec![0., 1.], 1.345, false),
        (vec![1., 3., 2.], vec![1., 1., 1.], 1.345, false),
        (vec![1., 3., 2.], vec![0., 1., 2.], 0.0, true),
    ] {
        let error = run(
            "yssbi.statistics.regression.robust",
            &[("y", series(&response)), ("x", series(&predictors))],
            &[
                ("constant", flag(true)),
                ("robust_loss", string("huber")),
                ("tuning", number(tuning)),
                ("max_iterations", int(500)),
                ("tolerance", number(1e-8)),
            ],
            1,
        )
        .unwrap_err();
        if parameter_error {
            assert!(matches!(error, KernelError::InvalidParameter), "{error:?}");
        } else {
            assert!(
                matches!(error, KernelError::InvalidNumericInput),
                "{error:?}"
            );
        }
    }
}
