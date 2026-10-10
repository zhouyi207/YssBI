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

#[test]
fn ordinal_rank_preparation_admits_declared_domain_before_indexing() {
    use yss_data_contract::{ColumnSemantic, ConversionMetadata, SemanticType, SemanticValue};
    let metadata = Arc::new(ConversionMetadata {
        semantic: ColumnSemantic {
            values: (0..16_384)
                .map(|value| SemanticValue {
                    value: value.to_string(),
                    label: value.to_string(),
                })
                .collect(),
            ..ColumnSemantic::new(SemanticType::Ordinal)
        },
        temporal: None,
        dummy_base_level: None,
    });
    let x = [0, 3, 6, 9, 12, 15, 18, 21];
    let inputs = [
        RuntimeValue::List(x.into_iter().map(int).collect())
            .with_metadata(metadata.clone())
            .unwrap(),
        RuntimeValue::List(x.into_iter().rev().map(int).collect())
            .with_metadata(metadata)
            .unwrap(),
    ];
    let parameters = [
        ("alternative", string("two_sided")),
        ("p_value_method", string("asymptotic")),
    ];
    let registry = KernelRegistry::default();
    let relations = crate::tests::relations();
    let invoke = |control: &KernelControl| {
        registry.execute(
            &KernelId::new("yssbi.statistics.association.spearman".into()).unwrap(),
            &KernelInvocation {
                relations: &relations,
                inputs: &inputs,
                input_keys: &["x", "y"],
                parameters: parameters
                    .iter()
                    .map(|(key, value)| {
                        (
                            KernelParameterKey::new((*key).into()).unwrap(),
                            Cow::Borrowed(value),
                        )
                    })
                    .collect(),
                outputs: &[KernelOutputSpec {
                    data_type: ValueType::Struct("statistics.report".into()),
                    fields: None,
                }],
                control,
            },
        )
    };
    let mut control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    control.max_input_bytes = 64 * 1024;
    let error = invoke(&control).err();
    assert!(
        matches!(error, Some(KernelError::BudgetExceeded)),
        "{error:?}"
    );
    control.max_input_bytes = usize::MAX;
    let output = invoke(&control).unwrap();
    let coefficient =
        crate::builtins::numeric_input(Some(field(&output[0], "coefficient").unwrap())).unwrap();
    assert!((coefficient + 1.).abs() < 1e-12);
}

#[test]
fn weighted_kappa_compares_declared_codes_independently_of_display_labels() {
    use yss_data_contract::{ColumnSemantic, ConversionMetadata, SemanticType, SemanticValue};
    let ratings = |labels: [&str; 3]| {
        RuntimeValue::List(
            ["low", "high", "low", "high"]
                .into_iter()
                .map(string)
                .collect(),
        )
        .with_metadata(ConversionMetadata {
            semantic: ColumnSemantic {
                values: ["low", "unused", "high"]
                    .into_iter()
                    .zip(labels)
                    .map(|(value, label)| SemanticValue {
                        value: value.into(),
                        label: label.into(),
                    })
                    .collect(),
                ..ColumnSemantic::new(SemanticType::Ordinal)
            },
            temporal: None,
            dummy_base_level: None,
        })
        .unwrap()
    };
    let output = run(
        "yssbi.statistics.test.kappa",
        &[
            ("ratings", ratings(["Low", "Unused", "High"])),
            ("ratings", ratings(["低", "未观测", "高"])),
        ],
        &[
            ("kappa_method", string("cohen")),
            ("kappa_weighting", string("linear")),
            ("confidence_level", number(0.95)),
        ],
        1,
    )
    .unwrap();
    assert_eq!(
        field(&output[0], "categories").unwrap(),
        &RuntimeValue::List(["low", "unused", "high"].into_iter().map(string).collect())
    );
    assert_eq!(
        crate::builtins::numeric_input(Some(field(&output[0], "coefficient").unwrap())).unwrap(),
        1.
    );
}
