use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_data_contract::ValueType;

use crate::{
    KernelControl, KernelField, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    RuntimeValue,
};

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
                data_type: ValueType::Scalar(yss_data_contract::SemanticType::Binary),
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
fn equality_compares_nested_numeric_values_without_coercing_other_semantics() {
    let record = |value| {
        RuntimeValue::Record(BTreeMap::from([(
            "values".into(),
            RuntimeValue::List(vec![value].into_boxed_slice()),
        )]))
    };
    assert_eq!(
        compare(
            "equal",
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
            "equal",
            &[RuntimeValue::Bool(true), RuntimeValue::Integer(1)]
        )
        .unwrap(),
        vec![RuntimeValue::Bool(false)]
    );
    assert_eq!(
        compare("equal", &[RuntimeValue::Null, RuntimeValue::Null]).unwrap(),
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
