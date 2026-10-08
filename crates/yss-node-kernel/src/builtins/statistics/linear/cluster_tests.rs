use crate::{
    KernelControl, KernelError, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    LinearRegressionValue, RuntimeValue,
};
use arrow_array::{ArrayRef, Float64Array, RecordBatch};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use yss_data_contract::{TabularScalar, ValueType};
use yss_database_engine::DataFusionRuntime;
use yss_relational_contract::{RelationControl, RelationFactory};

const ROWS: usize = 24;

fn fit_clustered(labels: &[TabularScalar]) -> Result<Arc<LinearRegressionValue>, KernelError> {
    let factory: Arc<dyn RelationFactory> = DataFusionRuntime::unbounded(32).unwrap();
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let relation_control = RelationControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let x = (0..ROWS).map(|row| row as f64).collect::<Vec<_>>();
    let y = x
        .iter()
        .enumerate()
        .map(|(row, x)| 2. + 0.4 * x + (row % 5) as f64 * 0.07 + (row / 6) as f64 * 0.2)
        .collect::<Vec<_>>();
    let source = factory
        .clone()
        .materialize(
            RecordBatch::try_from_iter([
                ("response", Arc::new(Float64Array::from(y)) as ArrayRef),
                ("predictor", Arc::new(Float64Array::from(x)) as ArrayRef),
            ])
            .unwrap(),
            &relation_control,
        )
        .unwrap();
    let (field, values) = yss_database_arrow::materialized_column("cluster", labels, None).unwrap();
    let clusters = factory
        .clone()
        .literal_series(field, values, &relation_control)
        .unwrap();
    let numeric = ValueType::DataSeries(Box::new(ValueType::number()));
    let outputs = KernelRegistry::default().execute(
        &KernelId::new("yssbi.statistics.linear.fit".into()).unwrap(),
        &KernelInvocation {
            relations: &factory,
            inputs: &[
                RuntimeValue::Series(source.select_series("response").unwrap()),
                RuntimeValue::Series(source.select_series("predictor").unwrap()),
                RuntimeValue::Series(clusters),
            ],
            input_keys: &["y", "x", "clusters"],
            parameters: BTreeMap::from([
                (
                    crate::KernelParameterKey::new("method".into()).unwrap(),
                    Cow::Owned(TabularScalar::String("OLS".into()).into()),
                ),
                (
                    crate::KernelParameterKey::new("constant".into()).unwrap(),
                    Cow::Owned(TabularScalar::Bool(true).into()),
                ),
                (
                    crate::KernelParameterKey::new("covariance".into()).unwrap(),
                    Cow::Owned(TabularScalar::String("cluster".into()).into()),
                ),
            ]),
            outputs: &[
                KernelOutputSpec {
                    data_type: ValueType::Struct("statistics.model.linear".into()),
                    fields: None,
                },
                KernelOutputSpec {
                    data_type: numeric.clone(),
                    fields: None,
                },
                KernelOutputSpec {
                    data_type: numeric,
                    fields: None,
                },
            ],
            control: &control,
        },
    )?;
    let Some(RuntimeValue::LinearRegression(model)) = outputs.into_iter().next() else {
        panic!("linear fitting must return its retained model");
    };
    Ok(model)
}

#[test]
fn clustered_linear_fit_preserves_exact_database_labels() {
    let codes = (0..ROWS).map(|row| (row / 6) as u64).collect::<Vec<_>>();
    let baseline = fit_clustered(
        &codes
            .iter()
            .map(|code| TabularScalar::Unsigned(*code))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for labels in [
        codes
            .iter()
            .map(|code| TabularScalar::Unsigned(u64::MAX - code))
            .collect::<Vec<_>>(),
        codes
            .iter()
            .map(|code| TabularScalar::String(format!("group-{code}").into()))
            .collect(),
    ] {
        let actual = fit_clustered(&labels).unwrap();
        for (actual, expected) in actual.coefficients.iter().zip(&baseline.coefficients) {
            assert!((actual - expected).abs() < 1e-12);
        }
        for (actual, expected) in actual
            .report
            .cov_beta
            .iter()
            .flatten()
            .zip(baseline.report.cov_beta.iter().flatten())
        {
            assert!((actual - expected).abs() < 1e-12);
        }
    }
}

#[test]
fn clustered_linear_fit_rejects_missing_or_misaligned_labels() {
    let mut labels = (0..ROWS)
        .map(|row| TabularScalar::Unsigned((row / 6) as u64))
        .collect::<Vec<_>>();
    labels[ROWS - 1] = TabularScalar::Null;
    assert!(matches!(
        fit_clustered(&labels),
        Err(KernelError::InvalidNumericInput)
    ));
    labels.pop();
    assert!(matches!(
        fit_clustered(&labels),
        Err(KernelError::ShapeMismatch)
    ));
}
