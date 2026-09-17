use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_data_contract::ValueType;

use crate::{
    KernelControl, KernelField, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    RuntimeValue,
};

#[test]
fn semantic_annotations_only_contain_materialized_scalar_values() {
    use yss_data_contract::{ColumnSemantic, ConversionMetadata, SemanticType};
    let metadata = ConversionMetadata {
        semantic: ColumnSemantic::new(SemanticType::Identifier),
        temporal: None,
    };
    let value = RuntimeValue::String("001".into());
    for value in [
        value.clone(),
        RuntimeValue::List(Box::new([value, RuntimeValue::Null])),
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
        RuntimeValue::List(Box::new([RuntimeValue::List(Box::new([]))])),
        RuntimeValue::Record(BTreeMap::new()),
    ] {
        assert_eq!(
            value.with_metadata(metadata.clone()),
            Err(crate::RuntimeValueError::Unrepresentable)
        );
    }
    assert_eq!(
        RuntimeValue::Decimal(f64::NAN).with_metadata(metadata),
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
            inputs,
            input_groups: &[None, None],
            parameters: BTreeMap::new(),
            outputs: &[KernelOutputSpec {
                data_type: if operation != "whole_equal"
                    && inputs.iter().any(|v| {
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
    use RuntimeValue::{Decimal as D, Integer as I, Unsigned as U};
    use std::cmp::Ordering::{Equal, Greater, Less};
    for (a, b, order) in [
        (I(1), D(1.0), Equal),
        (I(1), U(1), Equal),
        (I(9_007_199_254_740_993), I(9_007_199_254_740_994), Less),
        (
            I(9_007_199_254_740_993),
            D(9_007_199_254_740_992.0),
            Greater,
        ),
        (U(u64::MAX), D(18_446_744_073_709_551_616.0), Less),
        (I(i64::MAX), D(9_223_372_036_854_775_808.0), Less),
        (I(i64::MIN), D(-9_223_372_036_854_775_808.0), Equal),
        (I(-1), U(u64::MAX), Less),
        (I(0), D(-0.5), Greater),
        (I(0), D(0.5), Less),
        (I(0), D(-0.0), Equal),
        (I(-1), D(-1.5), Greater),
        (U(u64::MAX), D(f64::MAX), Less),
        (I(i64::MIN), D(-f64::MAX), Greater),
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
                    vec![RuntimeValue::Bool(expected)],
                    "{operation}: {left:?}, {right:?}"
                );
            }
        }
    }
    assert!(compare("less", &[I(0), D(f64::NAN)]).is_err());
    assert!(compare("equal", &[]).is_err());
}

#[test]
fn boolean_kernels_share_three_valued_logic_and_broadcast_shape() {
    use RuntimeValue::{Bool as B, List as L, Null as N};
    let evaluate = |operation: &str, inputs: &[RuntimeValue]| {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let scalar = ValueType::Scalar(yss_data_contract::SemanticType::Binary);
        let data_type = if inputs.iter().any(|v| matches!(v, L(_))) {
            ValueType::DataSeries(Box::new(scalar))
        } else {
            scalar
        };
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.logic.{operation}").into()).unwrap(),
            &KernelInvocation {
                inputs,
                input_groups: &vec![None; inputs.len()],
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
        (B(false), B(false), B(false), B(false)),
        (B(false), B(true), B(false), B(true)),
        (B(false), N, B(false), N),
        (B(true), B(false), B(false), B(true)),
        (B(true), B(true), B(true), B(true)),
        (B(true), N, N, B(true)),
        (N, B(false), B(false), N),
        (N, B(true), N, B(true)),
        (N, N, N, N),
    ] {
        assert_eq!(
            evaluate("and", &[left.clone(), right.clone()]).unwrap(),
            vec![and]
        );
        assert_eq!(evaluate("or", &[left, right]).unwrap(), vec![or]);
    }
    let values = L(Box::new([B(false), B(true), N]));
    assert_eq!(
        evaluate("not", std::slice::from_ref(&values)).unwrap(),
        vec![L(Box::new([B(true), B(false), N]))]
    );
    assert_eq!(evaluate("not", &[N]).unwrap(), vec![N]);
    for inputs in [[values.clone(), B(false)], [B(false), values.clone()]] {
        assert_eq!(
            evaluate("and", &inputs).unwrap(),
            vec![L(Box::new([B(false), B(false), B(false)]))]
        );
    }
    assert_eq!(
        evaluate("or", &[values.clone(), B(true)]).unwrap(),
        vec![L(Box::new([B(true), B(true), B(true)]))]
    );
    assert!(evaluate("and", &[values, L(Box::new([]))]).is_err());
    assert!(evaluate("not", &[RuntimeValue::Integer(1)]).is_err());
    assert!(evaluate("and", &[B(false), RuntimeValue::Integer(1)]).is_err());
    assert!(evaluate("not", &[B(true), B(false)]).is_err());
    assert!(evaluate("and", &[B(true)]).is_err());
}

#[test]
fn power_and_logarithm_execute_scalars_broadcasts_and_domain_checks() {
    use RuntimeValue::{Decimal as D, Integer as I, List as L};
    let evaluate = |operation: &str, inputs: &[RuntimeValue]| {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let scalar = ValueType::Scalar(yss_data_contract::SemanticType::Numeric);
        let data_type = if inputs.iter().any(|v| matches!(v, L(_))) {
            ValueType::DataSeries(Box::new(scalar))
        } else {
            scalar
        };
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.numeric.{operation}").into()).unwrap(),
            &KernelInvocation {
                inputs,
                input_groups: &[None, None],
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
        ("power", I(2), I(3), 8.),
        ("power", I(9), D(0.5), 3.),
        ("power", I(-2), I(3), -8.),
        ("power", I(2), I(-1), 0.5),
        ("log", I(8), I(2), 3.),
        ("log", I(100), I(10), 2.),
        ("log", I(4), D(0.5), -2.),
    ] {
        assert_eq!(evaluate(op, &[left, right]).unwrap(), vec![D(expected)]);
    }
    let list = |values: &[i64]| L(values.iter().map(|v| I(*v)).collect());
    assert_eq!(
        evaluate("power", &[I(2), list(&[1, 2, 3])]).unwrap(),
        vec![L(Box::new([D(2.), D(4.), D(8.)]))]
    );
    assert_eq!(
        evaluate("power", &[list(&[2, 3]), list(&[3, 2])]).unwrap(),
        vec![L(Box::new([D(8.), D(9.)]))]
    );
    assert_eq!(
        evaluate("log", &[list(&[2, 4, 8]), I(2)]).unwrap(),
        vec![L(Box::new([D(1.), D(2.), D(3.)]))]
    );
    for (op, left, right) in [
        ("power", I(0), I(0)),
        ("power", I(0), I(-1)),
        ("power", I(-2), D(0.5)),
        ("log", I(0), I(2)),
        ("log", I(-1), I(2)),
        ("log", I(2), I(1)),
        ("log", I(2), I(0)),
        ("log", I(2), I(-2)),
        ("power", I(9_007_199_254_740_993), I(1)),
        ("power", D(f64::NAN), I(1)),
        ("log", RuntimeValue::Null, I(2)),
        ("power", list(&[1, 2]), list(&[1])),
    ] {
        assert!(evaluate(op, &[left, right]).is_err(), "{op}");
    }
    assert!(matches!(
        evaluate("power", &[D(f64::MAX), I(2)]),
        Err(crate::KernelError::NonFiniteResult)
    ));
}

#[test]
fn unary_arithmetic_preserves_shape_and_rejects_invalid_values() {
    use RuntimeValue::{Decimal as D, Integer as I, List as L};
    let evaluate = |operation: &str, inputs: &[RuntimeValue]| {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let scalar = ValueType::Scalar(yss_data_contract::SemanticType::Numeric);
        let data_type = if inputs.iter().any(|v| matches!(v, L(_))) {
            ValueType::DataSeries(Box::new(scalar))
        } else {
            scalar
        };
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.numeric.{operation}").into()).unwrap(),
            &KernelInvocation {
                inputs,
                input_groups: &[None],
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
            assert_eq!(evaluate(op, &[D(value)]).unwrap(), vec![D(result)]);
        }
        assert_eq!(
            evaluate(op, &[L(values.map(D).into())]).unwrap(),
            vec![L(expected.map(D).into())]
        );
        assert_eq!(
            evaluate(op, &[L(Box::new([]))]).unwrap(),
            vec![L(Box::new([]))]
        );
        for value in [
            RuntimeValue::Null,
            D(f64::NAN),
            D(f64::INFINITY),
            I(9_007_199_254_740_993),
        ] {
            assert!(evaluate(op, &[value]).is_err());
        }
        assert!(evaluate(op, &[]).is_err());
        assert!(evaluate(op, &[I(1), I(2)]).is_err());
    }
    for op in ["ln", "log2", "log10"] {
        assert!(evaluate(op, &[I(0)]).is_err());
        assert!(evaluate(op, &[I(-1)]).is_err());
    }
    assert!(evaluate("sqrt", &[I(-1)]).is_err());
    assert!(matches!(
        evaluate("square", &[D(f64::MAX)]),
        Err(crate::KernelError::NonFiniteResult)
    ));
}

#[test]
fn comparisons_broadcast_both_sides_preserve_nulls_and_reject_misalignment() {
    use RuntimeValue::{Bool as B, List as L, Null as N, String as S};
    let list = L(Box::new([S("001".into()), S("1".into()), N]));
    for inputs in [[list.clone(), S("1".into())], [S("1".into()), list.clone()]] {
        assert_eq!(
            compare("equal", &inputs).unwrap(),
            vec![L(Box::new([B(false), B(true), N]))]
        );
        assert_eq!(
            compare("not_equal", &inputs).unwrap(),
            vec![L(Box::new([B(true), B(false), N]))]
        );
    }
    assert_eq!(
        compare("less", &[S("1".into()), list.clone()]).unwrap(),
        vec![L(Box::new([B(false), B(false), N]))]
    );
    assert_eq!(
        compare("equal", &[list.clone(), list.clone()]).unwrap(),
        vec![L(Box::new([B(true), B(true), N]))]
    );
    assert!(compare("equal", &[list, L(Box::new([]))]).is_err());
    assert_eq!(compare("equal", &[N, N]).unwrap(), vec![N]);
}

#[test]
fn equality_compares_nested_numeric_values_without_coercing_other_semantics() {
    let record = |value| {
        RuntimeValue::Record(BTreeMap::from([(
            "values".into(),
            RuntimeValue::List(vec![value].into_boxed_slice()),
        )]))
    };
    assert_eq!(
        compare(
            "whole_equal",
            &[
                record(RuntimeValue::Integer(1)),
                record(RuntimeValue::Decimal(1.0))
            ]
        )
        .unwrap(),
        vec![RuntimeValue::Bool(true)]
    );
    assert_eq!(
        compare(
            "whole_equal",
            &[RuntimeValue::Bool(true), RuntimeValue::Integer(1)]
        )
        .unwrap(),
        vec![RuntimeValue::Bool(false)]
    );
    assert_eq!(
        compare("whole_equal", &[RuntimeValue::Null, RuntimeValue::Null]).unwrap(),
        vec![RuntimeValue::Bool(true)]
    );
}

#[test]
fn decomposition_follows_local_output_schema_order_without_graph_addresses() {
    let first = RuntimeValue::List(vec![RuntimeValue::Integer(11)].into_boxed_slice());
    let second = RuntimeValue::List(vec![RuntimeValue::Integer(22)].into_boxed_slice());
    let input = RuntimeValue::Record(BTreeMap::from([
        ("a".into(), first.clone()),
        ("b".into(), second.clone()),
    ]));
    let outputs = ["b", "a"].map(|name| KernelOutputSpec {
        data_type: ValueType::DataSeries(Box::new(ValueType::Scalar(
            yss_data_contract::SemanticType::Numeric,
        ))),
        fields: Some(
            vec![KernelField {
                name: name.into(),
                data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            }]
            .into_boxed_slice(),
        ),
    });
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let result = KernelRegistry::default()
        .execute(
            &KernelId::new("yssbi.dataframe.decompose".into()).unwrap(),
            &KernelInvocation {
                inputs: &[input],
                input_groups: &[None],
                parameters: BTreeMap::new(),
                outputs: &outputs,
                control: &control,
            },
        )
        .unwrap();
    assert_eq!(result, vec![second, first]);
}
