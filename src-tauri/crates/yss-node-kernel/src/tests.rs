use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_data_contract::TabularScalar;
use yss_data_contract::ValueType;

use crate::{
    KernelControl, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry, RuntimeValue,
};

#[test]
fn semantic_annotations_only_contain_materialized_scalar_values() {
    use yss_data_contract::{ColumnSemantic, ConversionMetadata, SemanticType};
    let metadata = ConversionMetadata {
        semantic: ColumnSemantic::new(SemanticType::Identifier),
        temporal: None,
        dummy_base_level: None,
    };
    let value = RuntimeValue::Scalar(TabularScalar::String("001".into()));
    for value in [
        value.clone(),
        RuntimeValue::List(std::sync::Arc::from([
            value,
            RuntimeValue::Scalar(TabularScalar::Null),
        ])),
    ] {
        let annotated = value.clone().with_metadata(metadata.clone()).unwrap();
        assert_eq!(annotated.metadata(), Some(&metadata));
        assert_eq!(annotated.unannotated(), &value);
        assert_eq!(
            annotated.with_metadata(metadata.clone()),
            Err(crate::RuntimeValueError::Unrepresentable),
        );
    }
    for value in [
        RuntimeValue::Resource("dataset".into()),
        RuntimeValue::List(std::sync::Arc::from([RuntimeValue::List(
            std::sync::Arc::from([]),
        )])),
        RuntimeValue::Record(std::sync::Arc::new(BTreeMap::new())),
    ] {
        assert_eq!(
            value.with_metadata(metadata.clone()),
            Err(crate::RuntimeValueError::Unrepresentable)
        );
    }
    assert_eq!(
        RuntimeValue::float64(f64::NAN),
        Err(crate::RuntimeValueError::NonFinite),
    );
}

fn compare(
    operation: &str,
    inputs: &[RuntimeValue],
) -> Result<Vec<RuntimeValue>, crate::KernelError> {
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    KernelRegistry::default().execute(
        &KernelId::new(format!("yssbi.compare.{operation}").into()).unwrap(),
        &KernelInvocation {
            relations: &crate::tests::relations(),
            inputs,
            input_keys: &["left", "right"],
            parameters: BTreeMap::from([(
                crate::KernelParameterKey::new("mode".into()).unwrap(),
                std::borrow::Cow::Owned(RuntimeValue::Scalar(TabularScalar::String(
                    "exact".into(),
                ))),
            )]),
            outputs: &[KernelOutputSpec {
                data_type: if inputs.iter().any(|v| {
                    matches!(
                        v.unannotated(),
                        RuntimeValue::List(_) | RuntimeValue::Series(_)
                    )
                }) {
                    ValueType::DataSeries(Box::new(ValueType::Scalar(
                        yss_data_contract::SemanticType::Binary,
                    )))
                } else {
                    ValueType::Scalar(yss_data_contract::SemanticType::Binary)
                },
                fields: None,
            }],
            control: &control,
        },
    )
}

#[test]
fn comparison_kernels_preserve_exact_mixed_numeric_order() {
    use std::cmp::Ordering::{Equal, Greater, Less};
    for (a, b, order) in [
        (
            RuntimeValue::Scalar(TabularScalar::Integer(1)),
            RuntimeValue::float64(1.0).unwrap(),
            Equal,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(1)),
            RuntimeValue::Scalar(TabularScalar::Unsigned(1)),
            Equal,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(9_007_199_254_740_993)),
            RuntimeValue::Scalar(TabularScalar::Integer(9_007_199_254_740_994)),
            Less,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(9_007_199_254_740_993)),
            RuntimeValue::float64(9_007_199_254_740_992.0).unwrap(),
            Greater,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Unsigned(u64::MAX)),
            RuntimeValue::float64(18_446_744_073_709_551_616.0).unwrap(),
            Less,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(i64::MAX)),
            RuntimeValue::float64(9_223_372_036_854_775_808.0).unwrap(),
            Less,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(i64::MIN)),
            RuntimeValue::float64(-9_223_372_036_854_775_808.0).unwrap(),
            Equal,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(-1)),
            RuntimeValue::Scalar(TabularScalar::Unsigned(u64::MAX)),
            Less,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
            RuntimeValue::float64(-0.5).unwrap(),
            Greater,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
            RuntimeValue::float64(0.5).unwrap(),
            Less,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
            RuntimeValue::float64(-0.0).unwrap(),
            Equal,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(-1)),
            RuntimeValue::float64(-1.5).unwrap(),
            Greater,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Unsigned(u64::MAX)),
            RuntimeValue::float64(f64::MAX).unwrap(),
            Less,
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Integer(i64::MIN)),
            RuntimeValue::float64(-f64::MAX).unwrap(),
            Greater,
        ),
    ] {
        for (left, right, expected) in [(a.clone(), b.clone(), order), (b, a, order.reverse())] {
            for (operation, expected) in [
                ("equal", expected.is_eq()),
                ("not_equal", !expected.is_eq()),
                ("less", expected.is_lt()),
                ("less_equal", expected.is_le()),
                ("greater", expected.is_gt()),
                ("greater_equal", expected.is_ge()),
            ] {
                assert_eq!(
                    compare(operation, &[left.clone(), right.clone()]).unwrap(),
                    vec![RuntimeValue::Scalar(TabularScalar::Bool(expected))],
                    "{operation}: {left:?}, {right:?}"
                );
            }
        }
    }
    assert!(RuntimeValue::float64(f64::NAN).is_err());
    assert!(compare("equal", &[]).is_err());
}

#[test]
fn boolean_kernels_share_three_valued_logic_and_broadcast_shape() {
    let evaluate = |operation: &str, inputs: &[RuntimeValue]| {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let scalar = ValueType::Scalar(yss_data_contract::SemanticType::Binary);
        let data_type = if inputs.iter().any(|v| matches!(v, RuntimeValue::List(_))) {
            ValueType::DataSeries(Box::new(scalar))
        } else {
            scalar
        };
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.logic.{operation}").into()).unwrap(),
            &KernelInvocation {
                relations: &crate::tests::relations(),
                inputs,
                input_keys: if operation == "not" {
                    &["input"]
                } else {
                    &["left", "right"]
                },
                parameters: BTreeMap::new(),
                outputs: &[KernelOutputSpec {
                    data_type,
                    fields: None,
                }],
                control: &control,
            },
        )
    };
    for (left, right, and, or) in [
        (
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Null),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Null),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
        ),
        (
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Null),
        ),
    ] {
        assert_eq!(
            evaluate("and", &[left.clone(), right.clone()]).unwrap(),
            vec![and]
        );
        assert_eq!(evaluate("or", &[left, right]).unwrap(), vec![or]);
    }
    let values = RuntimeValue::List(std::sync::Arc::from([
        RuntimeValue::Scalar(TabularScalar::Bool(false)),
        RuntimeValue::Scalar(TabularScalar::Bool(true)),
        RuntimeValue::Scalar(TabularScalar::Null),
    ]));
    assert_eq!(
        evaluate("not", std::slice::from_ref(&values)).unwrap(),
        vec![RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Null)
        ]))]
    );
    assert_eq!(
        evaluate("not", &[RuntimeValue::Scalar(TabularScalar::Null)]).unwrap(),
        vec![RuntimeValue::Scalar(TabularScalar::Null)]
    );
    for inputs in [
        [
            values.clone(),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
        ],
        [
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            values.clone(),
        ],
    ] {
        assert_eq!(
            evaluate("and", &inputs).unwrap(),
            vec![RuntimeValue::List(std::sync::Arc::from([
                RuntimeValue::Scalar(TabularScalar::Bool(false)),
                RuntimeValue::Scalar(TabularScalar::Bool(false)),
                RuntimeValue::Scalar(TabularScalar::Bool(false))
            ]))]
        );
    }
    assert_eq!(
        evaluate(
            "or",
            &[
                values.clone(),
                RuntimeValue::Scalar(TabularScalar::Bool(true))
            ]
        )
        .unwrap(),
        vec![RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(true))
        ]))]
    );
    assert!(
        evaluate(
            "and",
            &[values, RuntimeValue::List(std::sync::Arc::from([]))]
        )
        .is_err()
    );
    assert!(evaluate("not", &[RuntimeValue::Scalar(TabularScalar::Integer(1))]).is_err());
    assert!(
        evaluate(
            "and",
            &[
                RuntimeValue::Scalar(TabularScalar::Bool(false)),
                RuntimeValue::Scalar(TabularScalar::Integer(1))
            ]
        )
        .is_err()
    );
    assert!(
        evaluate(
            "not",
            &[
                RuntimeValue::Scalar(TabularScalar::Bool(true)),
                RuntimeValue::Scalar(TabularScalar::Bool(false))
            ]
        )
        .is_err()
    );
    assert!(evaluate("and", &[RuntimeValue::Scalar(TabularScalar::Bool(true))]).is_err());
}

#[test]
fn power_and_logarithm_execute_scalars_broadcasts_and_domain_checks() {
    let evaluate = |operation: &str, inputs: &[RuntimeValue]| {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let scalar = ValueType::Scalar(yss_data_contract::SemanticType::Numeric);
        let data_type = if inputs.iter().any(|v| matches!(v, RuntimeValue::List(_))) {
            ValueType::DataSeries(Box::new(scalar))
        } else {
            scalar
        };
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.numeric.{operation}").into()).unwrap(),
            &KernelInvocation {
                relations: &crate::tests::relations(),
                inputs,
                input_keys: &["left", "right"],
                parameters: BTreeMap::new(),
                outputs: &[KernelOutputSpec {
                    data_type,
                    fields: None,
                }],
                control: &control,
            },
        )
    };
    for (op, left, right, expected) in [
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
            RuntimeValue::Scalar(TabularScalar::Integer(3)),
            8.,
        ),
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(9)),
            RuntimeValue::float64(0.5).unwrap(),
            3.,
        ),
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(-2)),
            RuntimeValue::Scalar(TabularScalar::Integer(3)),
            -8.,
        ),
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
            RuntimeValue::Scalar(TabularScalar::Integer(-1)),
            0.5,
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(8)),
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
            3.,
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(100)),
            RuntimeValue::Scalar(TabularScalar::Integer(10)),
            2.,
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(4)),
            RuntimeValue::float64(0.5).unwrap(),
            -2.,
        ),
    ] {
        assert_eq!(
            evaluate(op, &[left, right]).unwrap(),
            vec![RuntimeValue::float64(expected).unwrap()]
        );
    }
    let list = |values: &[i64]| {
        RuntimeValue::List(
            values
                .iter()
                .map(|v| RuntimeValue::Scalar(TabularScalar::Integer(*v)))
                .collect(),
        )
    };
    assert_eq!(
        evaluate(
            "power",
            &[
                RuntimeValue::Scalar(TabularScalar::Integer(2)),
                list(&[1, 2, 3])
            ]
        )
        .unwrap(),
        vec![RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::float64(2.).unwrap(),
            RuntimeValue::float64(4.).unwrap(),
            RuntimeValue::float64(8.).unwrap()
        ]))]
    );
    assert_eq!(
        evaluate("power", &[list(&[2, 3]), list(&[3, 2])]).unwrap(),
        vec![RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::float64(8.).unwrap(),
            RuntimeValue::float64(9.).unwrap()
        ]))]
    );
    assert_eq!(
        evaluate(
            "log",
            &[
                list(&[2, 4, 8]),
                RuntimeValue::Scalar(TabularScalar::Integer(2))
            ]
        )
        .unwrap(),
        vec![RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::float64(1.).unwrap(),
            RuntimeValue::float64(2.).unwrap(),
            RuntimeValue::float64(3.).unwrap()
        ]))]
    );
    for (op, left, right) in [
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
        ),
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
            RuntimeValue::Scalar(TabularScalar::Integer(-1)),
        ),
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(-2)),
            RuntimeValue::float64(0.5).unwrap(),
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(-1)),
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
            RuntimeValue::Scalar(TabularScalar::Integer(1)),
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
            RuntimeValue::Scalar(TabularScalar::Integer(0)),
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
            RuntimeValue::Scalar(TabularScalar::Integer(-2)),
        ),
        (
            "power",
            RuntimeValue::Scalar(TabularScalar::Integer(9_007_199_254_740_993)),
            RuntimeValue::Scalar(TabularScalar::Integer(1)),
        ),
        (
            "log",
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Integer(2)),
        ),
        ("power", list(&[1, 2]), list(&[1])),
    ] {
        assert!(evaluate(op, &[left, right]).is_err(), "{op}");
    }
    assert!(matches!(
        evaluate(
            "power",
            &[
                RuntimeValue::float64(f64::MAX).unwrap(),
                RuntimeValue::Scalar(TabularScalar::Integer(2))
            ]
        ),
        Err(crate::KernelError::NonFiniteResult)
    ));
}

#[test]
fn unary_arithmetic_preserves_shape_and_rejects_invalid_values() {
    let evaluate = |operation: &str, inputs: &[RuntimeValue]| {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let scalar = ValueType::Scalar(yss_data_contract::SemanticType::Numeric);
        let data_type = if inputs.iter().any(|v| matches!(v, RuntimeValue::List(_))) {
            ValueType::DataSeries(Box::new(scalar))
        } else {
            scalar
        };
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.numeric.{operation}").into()).unwrap(),
            &KernelInvocation {
                relations: &crate::tests::relations(),
                inputs,
                input_keys: &["input"],
                parameters: BTreeMap::new(),
                outputs: &[KernelOutputSpec {
                    data_type,
                    fields: None,
                }],
                control: &control,
            },
        )
    };
    for (op, values, expected) in [
        ("ln", [1., std::f64::consts::E], [0., 1.]),
        ("log2", [1., 8.], [0., 3.]),
        ("log10", [1., 100.], [0., 2.]),
        ("square", [-3., 0.], [9., 0.]),
        ("sqrt", [0., 9.], [0., 3.]),
    ] {
        for (value, result) in values.into_iter().zip(expected) {
            assert_eq!(
                evaluate(op, &[RuntimeValue::float64(value).unwrap()]).unwrap(),
                vec![RuntimeValue::float64(result).unwrap()]
            );
        }
        assert_eq!(
            evaluate(
                op,
                &[RuntimeValue::List(
                    values
                        .map(|value| RuntimeValue::float64(value).unwrap())
                        .into()
                )]
            )
            .unwrap(),
            vec![RuntimeValue::List(
                expected
                    .map(|value| RuntimeValue::float64(value).unwrap())
                    .into()
            )]
        );
        assert_eq!(
            evaluate(op, &[RuntimeValue::List(std::sync::Arc::from([]))]).unwrap(),
            vec![RuntimeValue::List(std::sync::Arc::from([]))]
        );
        for value in [
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Integer(9_007_199_254_740_993)),
        ] {
            assert!(evaluate(op, &[value]).is_err());
        }
        assert!(evaluate(op, &[]).is_err());
        assert!(
            evaluate(
                op,
                &[
                    RuntimeValue::Scalar(TabularScalar::Integer(1)),
                    RuntimeValue::Scalar(TabularScalar::Integer(2))
                ]
            )
            .is_err()
        );
    }
    for op in ["ln", "log2", "log10"] {
        assert!(evaluate(op, &[RuntimeValue::Scalar(TabularScalar::Integer(0))]).is_err());
        assert!(evaluate(op, &[RuntimeValue::Scalar(TabularScalar::Integer(-1))]).is_err());
    }
    assert!(evaluate("sqrt", &[RuntimeValue::Scalar(TabularScalar::Integer(-1))]).is_err());
    assert!(matches!(
        evaluate("square", &[RuntimeValue::float64(f64::MAX).unwrap()]),
        Err(crate::KernelError::NonFiniteResult)
    ));
}

#[test]
fn comparisons_broadcast_both_sides_preserve_nulls_and_reject_misalignment() {
    let list = RuntimeValue::List(std::sync::Arc::from([
        RuntimeValue::Scalar(TabularScalar::String("001".into())),
        RuntimeValue::Scalar(TabularScalar::String("1".into())),
        RuntimeValue::Scalar(TabularScalar::Null),
    ]));
    for inputs in [
        [
            list.clone(),
            RuntimeValue::Scalar(TabularScalar::String("1".into())),
        ],
        [
            RuntimeValue::Scalar(TabularScalar::String("1".into())),
            list.clone(),
        ],
    ] {
        assert_eq!(
            compare("equal", &inputs).unwrap(),
            vec![RuntimeValue::List(std::sync::Arc::from([
                RuntimeValue::Scalar(TabularScalar::Bool(false)),
                RuntimeValue::Scalar(TabularScalar::Bool(true)),
                RuntimeValue::Scalar(TabularScalar::Null)
            ]))]
        );
        assert_eq!(
            compare("not_equal", &inputs).unwrap(),
            vec![RuntimeValue::List(std::sync::Arc::from([
                RuntimeValue::Scalar(TabularScalar::Bool(true)),
                RuntimeValue::Scalar(TabularScalar::Bool(false)),
                RuntimeValue::Scalar(TabularScalar::Null)
            ]))]
        );
    }
    assert_eq!(
        compare(
            "less",
            &[
                RuntimeValue::Scalar(TabularScalar::String("1".into())),
                list.clone()
            ]
        )
        .unwrap(),
        vec![RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Bool(false)),
            RuntimeValue::Scalar(TabularScalar::Null)
        ]))]
    );
    assert_eq!(
        compare("equal", &[list.clone(), list.clone()]).unwrap(),
        vec![RuntimeValue::List(std::sync::Arc::from([
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Bool(true)),
            RuntimeValue::Scalar(TabularScalar::Null)
        ]))]
    );
    assert!(
        compare(
            "equal",
            &[list, RuntimeValue::List(std::sync::Arc::from([]))]
        )
        .is_err()
    );
    assert_eq!(
        compare(
            "equal",
            &[
                RuntimeValue::Scalar(TabularScalar::Null),
                RuntimeValue::Scalar(TabularScalar::Null)
            ]
        )
        .unwrap(),
        vec![RuntimeValue::Scalar(TabularScalar::Null)]
    );
}

pub(crate) fn relations() -> std::sync::Arc<dyn yss_relational_contract::RelationFactory> {
    struct UnusedRelations;
    impl yss_relational_contract::RelationFactory for UnusedRelations {
        fn materialize(
            self: std::sync::Arc<Self>,
            _: arrow_array::RecordBatch,
            _: &yss_relational_contract::RelationControl,
        ) -> Result<yss_relational_contract::RelationHandle, yss_relational_contract::RelationError>
        {
            panic!("this unit test must not materialize a relation")
        }
    }
    std::sync::Arc::new(UnusedRelations)
}

#[test]
fn invocation_rejects_wrong_input_order_before_dispatch_and_wrong_output_carriers() {
    use crate::{KernelContract, KernelError, KernelInputSpec, KernelRegistryBuilder};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let id = KernelId::new("test.table".into()).unwrap();
    let mut builder = KernelRegistryBuilder::new();
    builder
        .register(
            id.clone(),
            std::num::NonZeroU32::new(1).unwrap(),
            KernelContract::new(
                [
                    KernelInputSpec::fixed("left"),
                    KernelInputSpec::fixed("right"),
                ],
                [],
                1..=1,
            )
            .unwrap(),
            move |_| {
                called.fetch_add(1, Ordering::Relaxed);
                Ok(vec![RuntimeValue::Record(Arc::new(BTreeMap::new()))])
            },
        )
        .unwrap();
    let registry = builder.freeze();
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(10),
    );
    let output = [KernelOutputSpec {
        data_type: ValueType::DataFrame,
        fields: None,
    }];
    let relations = relations();
    let mut invocation = KernelInvocation {
        relations: &relations,
        inputs: &[
            RuntimeValue::Scalar(TabularScalar::Null),
            RuntimeValue::Scalar(TabularScalar::Null),
        ],
        input_keys: &["right", "left"],
        parameters: BTreeMap::new(),
        outputs: &output,
        control: &control,
    };
    assert!(matches!(
        registry.execute(&id, &invocation),
        Err(KernelError::InputLayoutMismatch)
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    invocation.input_keys = &["left", "right"];
    assert!(matches!(
        registry.execute(&id, &invocation),
        Err(KernelError::OutputContractMismatch)
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
