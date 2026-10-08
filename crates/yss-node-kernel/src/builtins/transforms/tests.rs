use crate::{
    KernelControl, KernelError, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    RuntimeValue,
};
use arrow_array::{ArrayRef, Float64Array, RecordBatch};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use yss_data_contract::{SemanticType, TabularScalar, ValueType};
use yss_database_engine::DataFusionRuntime;
use yss_relational_contract::RelationFactory;

fn numeric_output(series: bool) -> KernelOutputSpec {
    let numeric = ValueType::Scalar(SemanticType::Numeric);
    KernelOutputSpec {
        data_type: if series {
            ValueType::DataSeries(Box::new(numeric))
        } else {
            numeric
        },
        fields: None,
    }
}

fn standardize(
    values: Vec<Option<f64>>,
    registry: &KernelRegistry,
    factory: &Arc<dyn RelationFactory>,
    control: &KernelControl,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let array = Arc::new(Float64Array::from(values)) as ArrayRef;
    let batch = RecordBatch::try_from_iter([("value", array)]).unwrap();
    let source = factory
        .clone()
        .materialize(
            batch,
            &yss_relational_contract::RelationControl {
                cancellation: control.cancellation.clone(),
                deadline: control.deadline,
                max_input_bytes: control.max_input_bytes,
            },
        )
        .unwrap();
    registry.execute(
        &KernelId::new("yssbi.dataframe.series.standardize".into()).unwrap(),
        &KernelInvocation {
            relations: factory,
            inputs: &[RuntimeValue::Series(source.select_series("value").unwrap())],
            input_keys: &["series"],
            parameters: BTreeMap::new(),
            outputs: &[
                numeric_output(true),
                numeric_output(false),
                numeric_output(false),
            ],
            control,
        },
    )
}

fn fixture() -> (KernelRegistry, Arc<dyn RelationFactory>, KernelControl) {
    (
        KernelRegistry::default(),
        DataFusionRuntime::unbounded(32).unwrap(),
        KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        ),
    )
}

#[test]
fn standardize_outputs_match_returned_statistics_and_preserve_nulls_on_roundtrip() {
    let (registry, factory, control) = fixture();
    let outputs = standardize(
        vec![Some(1.), None, Some(3.), Some(5.)],
        &registry,
        &factory,
        &control,
    )
    .unwrap();
    let [RuntimeValue::Series(series), mean, deviation] = outputs.as_slice() else {
        panic!("standardization must return a series and its statistics");
    };
    assert_eq!(mean, &RuntimeValue::float64(3.).unwrap());
    assert_eq!(deviation, &RuntimeValue::float64(2.).unwrap());
    assert!(series.plan().field().is_nullable());
    assert_eq!(
        yss_database_arrow::column_semantic(series.plan().field())
            .unwrap()
            .kind,
        SemanticType::Numeric,
    );
    let relation_control = KernelInvocation {
        relations: &factory,
        inputs: &[],
        input_keys: &[],
        parameters: BTreeMap::new(),
        outputs: &[],
        control: &control,
    }
    .relation_control();
    let page = series
        .as_relation()
        .unwrap()
        .page(0, 10, &relation_control)
        .unwrap();
    assert_eq!(
        page.data.columns()[0].values(),
        &[
            TabularScalar::Float64((-1.).try_into().unwrap()),
            TabularScalar::Null,
            TabularScalar::Float64(0_f64.try_into().unwrap()),
            TabularScalar::Float64(1_f64.try_into().unwrap()),
        ]
    );
    let restored = registry
        .execute(
            &KernelId::new("yssbi.dataframe.series.inverse_standardize".into()).unwrap(),
            &KernelInvocation {
                relations: &factory,
                inputs: &outputs,
                input_keys: &["standardized", "mean", "standard_deviation"],
                parameters: BTreeMap::new(),
                outputs: &[numeric_output(true)],
                control: &control,
            },
        )
        .unwrap();
    let [RuntimeValue::Series(series)] = restored.as_slice() else {
        panic!("inverse standardization must return a series");
    };
    let page = series
        .as_relation()
        .unwrap()
        .page(0, 10, &relation_control)
        .unwrap();
    assert_eq!(
        page.data.columns()[0].values(),
        &[
            TabularScalar::Float64(1_f64.try_into().unwrap()),
            TabularScalar::Null,
            TabularScalar::Float64(3_f64.try_into().unwrap()),
            TabularScalar::Float64(5_f64.try_into().unwrap()),
        ]
    );
}

#[test]
fn standardize_rejects_missing_or_zero_sample_deviation() {
    let (registry, factory, control) = fixture();
    for values in [
        vec![],
        vec![None, None],
        vec![Some(4.)],
        vec![Some(4.), Some(4.)],
    ] {
        assert!(matches!(
            standardize(values, &registry, &factory, &control),
            Err(KernelError::InvalidNumericInput),
        ));
    }
}
