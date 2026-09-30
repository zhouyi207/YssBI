use super::*;
use crate::{KernelControl, KernelOutputSpec, KernelRegistry};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use yss_data_contract::{ColumnSemantic, ConversionMetadata, SemanticValue, ValueType};

fn control() -> KernelControl {
    KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    )
}
fn float(v: f64) -> RuntimeValue {
    RuntimeValue::float64(v).unwrap()
}
fn invoke(
    id: &str,
    inputs: &[(&str, RuntimeValue)],
    params: &[(&str, RuntimeValue)],
    control: &KernelControl,
) -> Result<Vec<RuntimeValue>, KernelError> {
    KernelRegistry::default().execute(
        &KernelId::new(id.into()).unwrap(),
        &KernelInvocation {
            relations: &crate::tests::relations(),
            inputs: &inputs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
            input_keys: &inputs.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            parameters: params
                .iter()
                .map(|(k, v)| {
                    (
                        KernelParameterKey::new((*k).into()).unwrap(),
                        Cow::Borrowed(v),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            outputs: &[KernelOutputSpec {
                data_type: ValueType::Struct("statistics.report".into()),
                fields: None,
            }],
            control,
        },
    )
}
#[test]
fn ordinal_regression_keeps_declared_order_gaps_and_exact_wide_labels() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../yss-sci/src/regression/models/fixtures/reference.json"
    ))
    .unwrap();
    let data = &fixture["cases"]["ordinal"];
    let labels = [u64::MAX - 1, u64::MAX, u64::MAX - 2];
    let declared = [labels[0], u64::MAX - 4, labels[1], labels[2]];
    let response = RuntimeValue::List(
        data["y"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| TabularScalar::Unsigned(labels[v.as_u64().unwrap() as usize]).into())
            .collect(),
    )
    .with_metadata(ConversionMetadata {
        semantic: ColumnSemantic {
            values: declared
                .iter()
                .map(|v| SemanticValue {
                    value: v.to_string(),
                    label: v.to_string(),
                })
                .collect(),
            ..ColumnSemantic::new(SemanticType::Ordinal)
        },
        temporal: None,
        dummy_base_level: None,
    })
    .unwrap();
    let mut inputs = vec![("response", response)];
    for x in data["x"].as_array().unwrap() {
        inputs.push((
            "predictors",
            RuntimeValue::List(
                x.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| float(v.as_f64().unwrap()))
                    .collect(),
            ),
        ));
    }
    let output = invoke(
        "yssbi.statistics.regression.logit.ordinal",
        &inputs,
        &[
            ("max_iterations", TabularScalar::Integer(500).into()),
            ("tolerance", float(1e-7)),
        ],
        &control(),
    )
    .unwrap();
    let RuntimeValue::Record(fields) = &output[0] else {
        panic!("record");
    };
    let RuntimeValue::List(actual) = &fields["categories"] else {
        panic!("labels");
    };
    assert_eq!(
        actual
            .iter()
            .map(|v| v.tabular_scalar().unwrap())
            .collect::<Vec<_>>(),
        labels
            .iter()
            .copied()
            .map(TabularScalar::Unsigned)
            .collect::<Vec<_>>()
    );
    let RuntimeValue::List(predicted) = &fields["fitted_categories"] else {
        panic!("predicted labels");
    };
    assert!(predicted.iter().all(|v| {
        labels
            .iter()
            .any(|&label| v == &RuntimeValue::Scalar(TabularScalar::Unsigned(label)))
    }));
}
#[test]
fn regression_admission_checks_shapes_budget_and_cancellation_before_estimation() {
    let inputs = [
        (
            "response",
            RuntimeValue::List([1., 2., 3., 4.].into_iter().map(float).collect()),
        ),
        (
            "predictors",
            RuntimeValue::List([0., 1., 2., 3.].into_iter().map(float).collect()),
        ),
    ];
    let params = [
        ("constant", TabularScalar::Bool(true).into()),
        ("standardize", TabularScalar::Bool(true).into()),
        ("lambda", float(1.0)),
    ];
    let mut small = control();
    small.max_input_bytes = 16;
    assert!(matches!(
        invoke(
            "yssbi.statistics.regression.ridge",
            &inputs,
            &params,
            &small
        ),
        Err(KernelError::BudgetExceeded)
    ));
    small
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        invoke(
            "yssbi.statistics.regression.ridge",
            &inputs,
            &params,
            &small
        ),
        Err(KernelError::Cancelled)
    ));
    let mismatch = [
        inputs[0].clone(),
        ("predictors", RuntimeValue::List(vec![float(1.0)].into())),
    ];
    assert!(matches!(
        invoke(
            "yssbi.statistics.regression.ridge",
            &mismatch,
            &params,
            &control()
        ),
        Err(KernelError::ShapeMismatch)
    ));
}
