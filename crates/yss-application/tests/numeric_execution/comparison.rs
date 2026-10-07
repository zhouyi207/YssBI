use super::*;
use fixture::{columns, connect, node};
use serde_json::json;

#[test]
fn fitted_plus_residuals_compares_with_the_original_database_series() {
    let mut document = GraphDocument::default();
    let source = columns(
        &mut document,
        &[
            ("x", json!([1, 2, 3, 4, 5])),
            ("price", json!([3, 7, 8, 13, 14])),
        ],
    );
    let fit = node(&mut document, "yssbi.statistics.linear.fit", json!({}));
    connect(&mut document, source["price"], "series", fit, "y", None);
    connect(&mut document, source["x"], "series", fit, "x", Some(0));
    let sum = node(&mut document, "yssbi.numeric.add", json!({}));
    connect(&mut document, fit, "fitted", sum, "operands", Some(0));
    connect(&mut document, fit, "residuals", sum, "operands", Some(1));
    let equal = node(
        &mut document,
        "yssbi.logic.equal",
        json!({"mode":"tolerance"}),
    );
    for (left, right) in [(sum, source["price"]), (source["price"], sum)] {
        document
            .connections
            .retain(|_, edge| edge.input.node_id != equal);
        for (from, input) in [(left, "left"), (right, "right")] {
            connect(
                &mut document,
                from,
                if from == sum { "result" } else { "series" },
                equal,
                input,
                None,
            );
        }
        assert_eq!(
            execute(&document, "yssbi.logic.equal").unwrap(),
            RuntimeValue::List(vec![RuntimeValue::Scalar(TabularScalar::Bool(true)); 5].into())
        );
    }
}

#[test]
fn mixed_comparisons_preserve_values_and_enforce_shape_and_resource_limits() {
    use arrow::{
        array::{Int64Array, StringArray},
        datatypes::{DataType, Field, Schema},
        record_batch::RecordBatch,
    };
    use std::borrow::Cow;
    use yss_data_contract::{SemanticType, ValueType};
    use yss_node_kernel::{
        KernelControl, KernelError, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    };

    let relations: Arc<dyn yss_relational_contract::RelationFactory> =
        yss_database_runtime::dataset_query_engine().unwrap();
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let relation_control = yss_relational_contract::RelationControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("number", DataType::Int64, true),
            Field::new("text", DataType::Utf8, true),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![
                Some(9_007_199_254_740_993),
                Some(3),
                None,
            ])),
            Arc::new(StringArray::from(vec![Some("001"), Some("1"), None])),
        ],
    )
    .unwrap();
    let frame = relations
        .clone()
        .materialize(batch, &relation_control)
        .unwrap();
    let database = RuntimeValue::Series(frame.select_series("number").unwrap());
    let memory = RuntimeValue::List(Arc::from([
        RuntimeValue::float64(9_007_199_254_740_992.).unwrap(),
        RuntimeValue::Scalar(TabularScalar::Integer(4)),
        RuntimeValue::Scalar(TabularScalar::Null),
    ]));
    let kernels = KernelRegistry::default();
    let outputs = [KernelOutputSpec {
        data_type: ValueType::DataSeries(Box::new(ValueType::Scalar(SemanticType::Binary))),
        fields: None,
    }];
    let run = |name: &str, inputs: &[RuntimeValue], control: &KernelControl| {
        kernels.execute(
            &KernelId::new(format!("yssbi.compare.{name}").into()).unwrap(),
            &KernelInvocation {
                relations: &relations,
                inputs,
                input_keys: &["left", "right"],
                parameters: BTreeMap::from([(
                    yss_node_kernel::KernelParameterKey::new("mode".into()).unwrap(),
                    Cow::Owned(RuntimeValue::Scalar(TabularScalar::String("exact".into()))),
                )]),
                outputs: &outputs,
                control,
            },
        )
    };
    let expected = |values: [Option<bool>; 3]| {
        vec![RuntimeValue::List(
            values
                .map(|value| {
                    RuntimeValue::Scalar(
                        value
                            .map(TabularScalar::Bool)
                            .unwrap_or(TabularScalar::Null),
                    )
                })
                .into(),
        )]
    };
    for inputs in [
        [memory.clone(), database.clone()],
        [database.clone(), memory.clone()],
    ] {
        assert_eq!(
            run("equal", &inputs, &control).unwrap(),
            expected([Some(false), Some(false), None])
        );
    }
    assert_eq!(
        run("greater", &[memory.clone(), database.clone()], &control).unwrap(),
        expected([Some(false), Some(true), None])
    );
    assert_eq!(
        run("greater", &[database.clone(), memory.clone()], &control).unwrap(),
        expected([Some(true), Some(false), None])
    );
    assert_eq!(
        run(
            "equal",
            &[
                RuntimeValue::Series(frame.select_series("text").unwrap()),
                RuntimeValue::List(Arc::from([
                    RuntimeValue::Scalar(TabularScalar::String("001".into())),
                    RuntimeValue::Scalar(TabularScalar::String("01".into())),
                    RuntimeValue::Scalar(TabularScalar::Null),
                ])),
            ],
            &control
        )
        .unwrap(),
        expected([Some(true), Some(false), None])
    );
    assert!(matches!(
        run(
            "equal",
            &[
                database.clone(),
                RuntimeValue::List(Arc::from([RuntimeValue::Scalar(TabularScalar::Integer(3))])),
            ],
            &control
        ),
        Err(KernelError::ShapeMismatch)
    ));
    let empty = RuntimeValue::List(Arc::from([]));
    assert_eq!(
        run(
            "equal",
            &[
                empty.clone(),
                RuntimeValue::Series(frame.limit(0, 0).unwrap().select_series("number").unwrap()),
            ],
            &control
        )
        .unwrap(),
        vec![empty]
    );

    let inputs = [memory, database];
    let mut constrained = KernelControl::new(Arc::new(AtomicBool::new(false)), control.deadline);
    constrained.max_input_bytes = 1;
    assert!(matches!(
        run("equal", &inputs, &constrained),
        Err(KernelError::BudgetExceeded)
    ));
    constrained.max_input_bytes = control.max_input_bytes;
    constrained
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        run("equal", &inputs, &constrained),
        Err(KernelError::Cancelled)
    ));
    constrained
        .cancellation
        .store(false, std::sync::atomic::Ordering::Release);
    constrained.deadline = Instant::now();
    assert!(matches!(
        run("equal", &inputs, &constrained),
        Err(KernelError::DeadlineExceeded)
    ));
}
