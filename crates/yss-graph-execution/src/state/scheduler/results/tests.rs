use super::*;
use arrow_array::{ArrayRef, Float64Array, RecordBatch};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use yss_database_engine::DataFusionRuntime;
use yss_relational_contract::{
    NumericOperation, NumericType, RelationError, RelationHandle, SeriesOperand,
};

struct CountingFactory {
    engine: Arc<DataFusionRuntime>,
    snapshots: AtomicUsize,
}

impl RelationFactory for CountingFactory {
    fn snapshot(
        self: Arc<Self>,
        relation: &RelationHandle,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        self.snapshots.fetch_add(1, Ordering::Relaxed);
        self.engine.clone().snapshot(relation, control)
    }

    fn materialize(
        self: Arc<Self>,
        data: RecordBatch,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        self.engine.clone().materialize(data, control)
    }
}

fn fixture() -> (
    Arc<CountingFactory>,
    RelationHandle,
    KernelControl,
    RelationControl,
) {
    let factory = Arc::new(CountingFactory {
        engine: DataFusionRuntime::unbounded(32).unwrap(),
        snapshots: AtomicUsize::new(0),
    });
    let cancellation = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(30);
    let kernel_control = KernelControl::new(cancellation.clone(), deadline);
    let relation_control = RelationControl {
        cancellation,
        deadline,
        max_input_bytes: usize::MAX,
    };
    let column = Arc::new(Float64Array::from(vec![1., 2., 3.])) as ArrayRef;
    let batch = RecordBatch::try_from_iter([("value", column)]).unwrap();
    let relation = factory
        .clone()
        .materialize(batch, &relation_control)
        .unwrap();
    (factory, relation, kernel_control, relation_control)
}

fn numbers(value: &RuntimeValue, control: &RelationControl) -> Vec<f64> {
    let RuntimeValue::Series(series) = value else {
        panic!("expected series")
    };
    series
        .relation()
        .numeric_columns(std::slice::from_ref(series), control)
        .unwrap()
        .remove(0)
}

#[test]
fn repeated_series_boundaries_share_one_snapshot_and_leave_internal_values_lazy() {
    let (factory, relation, kernel_control, relation_control) = fixture();
    let series = relation.select_series("value").unwrap();
    let original = RuntimeValue::Series(series.clone());
    let mut values = vec![original.clone(), original.clone(), original.clone()];
    let adapter: Arc<dyn RelationFactory> = factory.clone();
    stabilize_outputs(&mut values, &[true, true, false], &adapter, &kernel_control).unwrap();
    assert_eq!(factory.snapshots.load(Ordering::Relaxed), 1);
    assert_eq!(values[0], values[1]);
    assert_eq!(values[2], original);
    assert_eq!(numbers(&values[0], &relation_control), [1., 2., 3.]);
}

#[test]
fn same_named_distinct_expressions_preserve_their_values_and_snapshot_each_once() {
    let (factory, relation, kernel_control, relation_control) = fixture();
    let source = relation.select_series("value").unwrap();
    let original = relation
        .numeric_series(
            NumericOperation::Add,
            &[
                SeriesOperand::Series(source.clone()),
                SeriesOperand::Scalar(yss_data_contract::TabularScalar::Integer(0)),
            ],
            NumericType::Float64,
        )
        .unwrap();
    let different = relation
        .numeric_series(
            NumericOperation::Add,
            &[
                SeriesOperand::Series(source),
                SeriesOperand::Scalar(yss_data_contract::TabularScalar::Integer(10)),
            ],
            NumericType::Float64,
        )
        .unwrap();
    assert_eq!(original.column(), different.column());
    assert_ne!(original, different);
    let mut values = vec![
        RuntimeValue::Series(original),
        RuntimeValue::Series(different.clone()),
        RuntimeValue::Series(different),
    ];
    let adapter: Arc<dyn RelationFactory> = factory.clone();
    stabilize_outputs(&mut values, &[true, true, true], &adapter, &kernel_control).unwrap();
    assert_eq!(factory.snapshots.load(Ordering::Relaxed), 2);
    assert_eq!(numbers(&values[0], &relation_control), [1., 2., 3.]);
    assert_eq!(numbers(&values[1], &relation_control), [11., 12., 13.]);
    assert_eq!(values[1], values[2]);
}
