use super::*;

#[test]
fn survival_admission_distinguishes_observations_from_options() {
    let error = run(
        "yssbi.statistics.survival.kaplan_meier",
        &[("time", series(&[0., 2.])), ("event", series(&[1., 0.]))],
        &[],
        1,
    )
    .unwrap_err();
    assert!(
        matches!(error, KernelError::InvalidNumericInput),
        "{error:?}"
    );
    for (predicted, bins, parameter_error) in
        [(vec![0.2, 1.2], 2, false), (vec![0.2, 0.8], 1, true)]
    {
        let error = run(
            "yssbi.statistics.plot.calibration",
            &[
                ("time", series(&[1., 2.])),
                ("event", series(&[1., 0.])),
                ("predicted_risk", series(&predicted)),
            ],
            &[
                ("survival_horizon", number(1.)),
                ("calibration_bins", int(bins)),
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

#[test]
fn survival_adapters_preserve_binary_events_exact_groups_and_cause_codes() {
    let inputs = [
        ("time", series(&[1.0, 2.0, 3.0, 4.0])),
        (
            "event",
            RuntimeValue::List(vec![flag(true), flag(false), flag(true), flag(false)].into()),
        ),
        (
            "groups",
            RuntimeValue::List(
                vec![
                    int(9_007_199_254_740_992),
                    int(9_007_199_254_740_993),
                    int(9_007_199_254_740_992),
                    int(9_007_199_254_740_993),
                ]
                .into(),
            ),
        ),
    ];
    let r = run("yssbi.statistics.survival.kaplan_meier", &inputs, &[], 1)
        .unwrap()
        .remove(0);
    let RuntimeValue::List(curves) = field(&r, "curves").unwrap() else {
        panic!("curves")
    };
    assert_eq!(curves.len(), 2);
    assert_eq!(
        field(&curves[0], "group").unwrap(),
        &int(9_007_199_254_740_992)
    );
    assert_eq!(
        field(&curves[1], "group").unwrap(),
        &int(9_007_199_254_740_993)
    );
    let result = run(
        "yssbi.statistics.survival.competing_risks",
        &[
            inputs[0].clone(),
            (
                "status",
                RuntimeValue::List(
                    vec![
                        int(9_007_199_254_740_992),
                        int(0),
                        int(9_007_199_254_740_993),
                        int(0),
                    ]
                    .into(),
                ),
            ),
        ],
        &[],
        1,
    )
    .unwrap()
    .remove(0);
    assert_eq!(
        field(&result, "causes").unwrap(),
        &RuntimeValue::List(vec![int(9_007_199_254_740_992), int(9_007_199_254_740_993)].into())
    );
    assert!(matches!(
        run(
            "yssbi.statistics.survival.logrank",
            &[
                ("time", series(&[1.0, 2.0])),
                ("event", series(&[1.0])),
                ("groups", series(&[0.0, 1.0]))
            ],
            &[],
            1
        ),
        Err(KernelError::ShapeMismatch)
    ));
}

#[test]
fn survival_plot_workspace_and_missing_observations_fail_before_computation() {
    let mut inputs = vec![
        ("time", series(&[1.0, 2.0, 3.0])),
        ("event", series(&[1.0, 1.0, 0.0])),
        ("predicted_risk", series(&[0.2, 0.4, 0.6])),
    ];
    let mut params = vec![
        ("survival_horizon", number(1.0)),
        ("decision_threshold_min", number(0.01)),
        ("decision_threshold_max", number(0.99)),
        ("decision_points", int(i64::MAX)),
    ];
    assert!(matches!(
        run("yssbi.statistics.plot.decision_curve", &inputs, &params, 1),
        Err(KernelError::BudgetExceeded)
    ));
    params[3].1 = int(99);
    inputs[1].1 =
        RuntimeValue::List(vec![number(1.0), TabularScalar::Null.into(), number(0.0)].into());
    assert!(run("yssbi.statistics.plot.decision_curve", &inputs, &params, 1).is_err());
    assert!(matches!(
        run(
            "yssbi.statistics.plot.nomogram",
            &[("model", series(&[1.0]))],
            &[
                ("survival_horizon", number(1.0)),
                ("nomogram_ticks", int(5))
            ],
            1
        ),
        Err(KernelError::InvalidNumericInput)
    ));
}
