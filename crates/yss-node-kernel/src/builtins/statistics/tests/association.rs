use super::*;

#[test]
fn association_failures_distinguish_parameters_from_observations() {
    let parameters = |confidence| {
        [
            ("alternative", string("two_sided")),
            ("confidence_level", number(confidence)),
        ]
    };
    let error = run(
        "yssbi.statistics.association.pearson",
        &[("x", series(&[1., 2., 3.])), ("y", series(&[3., 1., 2.]))],
        &parameters(1.0),
        1,
    )
    .unwrap_err();
    assert!(matches!(error, KernelError::InvalidParameter), "{error:?}");
    let error = run(
        "yssbi.statistics.association.pearson",
        &[("x", series(&[1., 1., 1.])), ("y", series(&[3., 1., 2.]))],
        &parameters(0.95),
        1,
    )
    .unwrap_err();
    assert!(
        matches!(error, KernelError::InvalidNumericInput),
        "{error:?}"
    );
    let error = run(
        "yssbi.statistics.association.pearson",
        &[("x", series(&[1., 2., 3.])), ("y", series(&[3., 1.]))],
        &parameters(0.95),
        1,
    )
    .unwrap_err();
    assert!(matches!(error, KernelError::ShapeMismatch), "{error:?}");
}

#[test]
fn association_category_union_preserves_exact_numeric_identity_and_text_labels() {
    let result = run(
        "yssbi.statistics.test.kappa",
        &[
            (
                "ratings",
                RuntimeValue::List(vec![int(1), int(2), int(1), int(2)].into()),
            ),
            ("ratings", series(&[1., 2., 2., 1.])),
            (
                "ratings",
                RuntimeValue::List(vec![string("1"), string("2"), string("1"), string("2")].into()),
            ),
        ],
        &[
            ("kappa_method", string("fleiss")),
            ("confidence_level", number(0.95)),
        ],
        1,
    )
    .unwrap();
    assert_eq!(
        field(&result[0], "categories").unwrap(),
        &RuntimeValue::List(vec![int(1), int(2), string("1"), string("2")].into()),
    );
    let coefficient =
        crate::builtins::numeric_input(Some(field(&result[0], "coefficient").unwrap())).unwrap();
    assert!((coefficient + 2. / 13.).abs() < 1e-12);
}
