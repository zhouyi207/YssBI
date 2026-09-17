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
