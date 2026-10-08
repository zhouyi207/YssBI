use super::*;

#[test]
fn cluster_robust_rejects_undefined_data_as_numeric_input() {
    for (response, predictors, groups) in [
        (
            vec![1., 3., 2., 5.],
            vec![0., 1., 2., 3.],
            vec![9., 9., 9., 9.],
        ),
        (vec![1., 3.], vec![0., 1.], vec![0., 1.]),
    ] {
        let error = run(
            "yssbi.statistics.inference.cluster_robust",
            &[
                ("y", series(&response)),
                ("clusters", series(&groups)),
                ("x", series(&predictors)),
            ],
            &[("constant", flag(true))],
            1,
        )
        .unwrap_err();
        assert!(
            matches!(error, KernelError::InvalidNumericInput),
            "{error:?}"
        );
    }
}
