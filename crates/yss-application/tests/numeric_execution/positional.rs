use super::*;
use arrow::{
    array::{ArrayRef, BooleanArray, Date32Array, Int32Array, Int64Array, StringArray},
    datatypes::{DataType, Field, Schema},
    record_batch::RecordBatch,
};
use std::borrow::Cow;
use yss_data_contract::{SemanticType, ValueType};
use yss_node_kernel::{
    KernelControl, KernelError, KernelField, KernelId, KernelInvocation, KernelOutputSpec,
    KernelParameterKey, KernelRegistry,
};
use yss_relational_contract::{RelationFactory, RelationHandle};

struct Harness {
    relations: Arc<dyn RelationFactory>,
    control: KernelControl,
    kernels: KernelRegistry,
}
impl Harness {
    fn new() -> Self {
        Self {
            relations: yss_database_runtime::dataset_query_engine().unwrap(),
            control: KernelControl::new(
                Arc::new(AtomicBool::new(false)),
                Instant::now() + Duration::from_secs(30),
            ),
            kernels: KernelRegistry::default(),
        }
    }
    fn frame(&self, columns: &[(&str, ArrayRef)]) -> RelationHandle {
        let fields = columns
            .iter()
            .map(|(name, array)| Field::new(*name, array.data_type().clone(), true))
            .collect::<Vec<_>>();
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(fields)),
            columns.iter().map(|(_, a)| a.clone()).collect(),
        )
        .unwrap();
        self.relations
            .clone()
            .materialize(
                batch,
                &yss_relational_contract::RelationControl {
                    cancellation: self.control.cancellation.clone(),
                    deadline: self.control.deadline,
                    max_input_bytes: self.control.max_input_bytes,
                },
            )
            .unwrap()
    }
    fn run(
        &self,
        id: &str,
        inputs: &[(&str, RuntimeValue)],
        parameters: &[(&str, RuntimeValue)],
        output: KernelOutputSpec,
    ) -> Result<RuntimeValue, KernelError> {
        let values = inputs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
        self.kernels
            .execute(
                &KernelId::new(id.into()).unwrap(),
                &KernelInvocation {
                    relations: &self.relations,
                    inputs: &values,
                    input_keys: &inputs.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
                    parameters: parameters
                        .iter()
                        .map(|(k, v)| {
                            (
                                KernelParameterKey::new((*k).into()).unwrap(),
                                Cow::Borrowed(v),
                            )
                        })
                        .collect(),
                    outputs: &[output],
                    control: &self.control,
                },
            )
            .map(|mut values| values.remove(0))
    }
}
fn output(data_type: ValueType) -> KernelOutputSpec {
    KernelOutputSpec {
        data_type,
        fields: None,
    }
}
fn series(kind: SemanticType) -> KernelOutputSpec {
    output(ValueType::DataSeries(Box::new(ValueType::Scalar(kind))))
}
fn integer(value: i64) -> RuntimeValue {
    TabularScalar::Integer(value).into()
}
fn text(value: &str) -> RuntimeValue {
    TabularScalar::String(value.into()).into()
}
fn flag(value: bool) -> RuntimeValue {
    TabularScalar::Bool(value).into()
}
fn numbers(values: &[i64]) -> RuntimeValue {
    RuntimeValue::List(values.iter().map(|v| integer(*v)).collect())
}
fn booleans(values: &[bool]) -> RuntimeValue {
    RuntimeValue::List(values.iter().map(|v| flag(*v)).collect())
}
fn selected(frame: &RelationHandle, column: &str) -> RuntimeValue {
    RuntimeValue::Series(frame.select_series(column).unwrap())
}
fn table(value: RuntimeValue) -> RelationHandle {
    let RuntimeValue::Relation(frame) = value else {
        panic!("expected table")
    };
    frame
}

#[test]
fn positional_computation_flows_through_masks_plots_and_table_outputs() {
    let mut h = Harness::new();
    let source = h.frame(&[
        ("x", Arc::new(Int32Array::from(vec![4, 2, 8, 6]))),
        (
            "flag",
            Arc::new(BooleanArray::from(vec![true, true, false, true])),
        ),
    ]);
    let other = h.frame(&[("y", Arc::new(Int64Array::from(vec![1, 2, 3, 4])))]);
    let x = selected(&source, "x");
    let y = selected(&other, "y");
    let difference = h
        .run(
            "yssbi.numeric.subtract",
            &[("left", x.clone()), ("right", y.clone())],
            &[],
            series(SemanticType::Numeric),
        )
        .unwrap();
    assert_eq!(observed_values(difference.clone()), numbers(&[3, 0, 5, 2]));
    let exact = [("mode", text("exact"))];
    let equal = h
        .run(
            "yssbi.compare.equal",
            &[("left", x.clone()), ("right", y.clone())],
            &exact,
            series(SemanticType::Binary),
        )
        .unwrap();
    assert_eq!(
        observed_values(equal),
        booleans(&[false, true, false, false])
    );
    let greater = h
        .run(
            "yssbi.compare.greater",
            &[("left", difference.clone()), ("right", y.clone())],
            &exact,
            series(SemanticType::Binary),
        )
        .unwrap();
    let mask = h
        .run(
            "yssbi.logic.and",
            &[("left", greater), ("right", selected(&source, "flag"))],
            &[],
            series(SemanticType::Binary),
        )
        .unwrap();
    assert_eq!(
        observed_values(mask.clone()),
        booleans(&[true, false, false, false])
    );
    let chosen = h
        .run(
            "yssbi.dataframe.series.choose",
            &[
                ("condition", mask.clone()),
                ("when_true", x.clone()),
                ("when_false", y.clone()),
            ],
            &[],
            series(SemanticType::Numeric),
        )
        .unwrap();
    assert_eq!(observed_values(chosen), numbers(&[4, 2, 3, 4]));
    let filtered = table(
        h.run(
            "yssbi.dataframe.filter.mask",
            &[
                ("source", RuntimeValue::Relation(source.clone())),
                ("mask", mask),
            ],
            &[("drop_matches", flag(false))],
            output(ValueType::DataFrame),
        )
        .unwrap(),
    );
    assert_eq!(filtered.schema().fields().len(), 2);
    assert_eq!(filtered.schema().field(0).data_type(), &DataType::Int32);
    assert_eq!(observed_values(selected(&filtered, "x")), numbers(&[4]));
    let extended = table(
        h.run(
            "yssbi.dataframe.set_column",
            &[
                ("source", RuntimeValue::Relation(source.clone())),
                ("series", difference.clone()),
            ],
            &[("name", text("difference"))],
            output(ValueType::DataFrame),
        )
        .unwrap(),
    );
    assert_eq!(extended.schema().fields().len(), 3);
    assert_eq!(extended.schema().field(0).data_type(), &DataType::Int32);
    assert_eq!(
        observed_values(selected(&extended, "difference")),
        numbers(&[3, 0, 5, 2])
    );
    let mut combined_output = output(ValueType::DataFrame);
    combined_output.fields = Some(
        ["x", "difference"]
            .map(|name| KernelField {
                name: name.into(),
                data_type: ValueType::Scalar(SemanticType::Numeric),
            })
            .into(),
    );
    let combined = table(
        h.run(
            "yssbi.dataframe.combine",
            &[("series", x.clone()), ("series", difference.clone())],
            &[],
            combined_output,
        )
        .unwrap(),
    );
    assert_eq!(
        observed_values(selected(&combined, "x")),
        numbers(&[4, 2, 8, 6])
    );
    assert_eq!(
        observed_values(selected(&combined, "difference")),
        numbers(&[3, 0, 5, 2])
    );
    let lag = h
        .run(
            "yssbi.dataframe.timeseries.lag",
            &[
                ("series", difference.clone()),
                ("context", RuntimeValue::Relation(source.clone())),
            ],
            &[
                ("window", integer(1)),
                ("direction", text("lag")),
                ("partition_by", RuntimeValue::List(Arc::from([]))),
                ("order_by", RuntimeValue::List([text("x")].into())),
                ("descending", flag(false)),
                ("nulls_first", flag(false)),
            ],
            series(SemanticType::Numeric),
        )
        .unwrap();
    assert_eq!(
        observed_values(lag),
        RuntimeValue::List(
            [
                integer(0),
                TabularScalar::Null.into(),
                integer(2),
                integer(3)
            ]
            .into()
        )
    );
    let plot = |inputs: &[(&str, RuntimeValue)]| {
        h.run(
            "yssbi.plot.scatter.view",
            inputs,
            &[],
            output(ValueType::Struct("plot.data".into())),
        )
        .unwrap()
    };
    assert_eq!(
        plot(&[("x", x.clone()), ("y", difference.clone())]),
        plot(&[("x", numbers(&[4, 2, 8, 6])), ("y", numbers(&[3, 0, 5, 2]))])
    );
    assert!(matches!(
        h.run(
            "yssbi.numeric.add",
            &[("operands", x.clone()), ("operands", numbers(&[1, 2]))],
            &[],
            series(SemanticType::Numeric)
        ),
        Err(KernelError::ShapeMismatch)
    ));
    assert!(matches!(
        h.run(
            "yssbi.dataframe.filter.mask",
            &[
                ("source", RuntimeValue::Relation(source)),
                ("mask", booleans(&[true]))
            ],
            &[("drop_matches", flag(false))],
            output(ValueType::DataFrame)
        ),
        Err(KernelError::ShapeMismatch)
    ));
    h.control.max_input_bytes = 1;
    assert!(matches!(
        h.run(
            "yssbi.numeric.add",
            &[("operands", x), ("operands", difference)],
            &[],
            series(SemanticType::Numeric)
        ),
        Err(KernelError::BudgetExceeded)
    ));
}

#[test]
fn positional_transforms_preserve_null_text_and_temporal_values() {
    let h = Harness::new();
    let source = h.frame(&[
        (
            "text",
            Arc::new(StringArray::from(vec![Some("001"), None, Some("x")])),
        ),
        (
            "date",
            Arc::new(Date32Array::from(vec![Some(4), None, Some(8)])),
        ),
    ]);
    let other = h.frame(&[("date", Arc::new(Date32Array::from(vec![1, 2, 3])))]);
    let replacements = RuntimeValue::List([text("a"), text("b"), text("c")].into());
    let filled = h
        .run(
            "yssbi.dataframe.series.fill_null",
            &[
                ("series", selected(&source, "text")),
                ("replacement", replacements.clone()),
            ],
            &[],
            series(SemanticType::Text),
        )
        .unwrap();
    assert_eq!(
        observed_values(filled.clone()),
        RuntimeValue::List([text("001"), text("b"), text("x")].into())
    );
    let joined = h
        .run(
            "yssbi.dataframe.series.text.concatenate",
            &[("parts", filled), ("parts", replacements)],
            &[("separator", text("-"))],
            series(SemanticType::Text),
        )
        .unwrap();
    assert_eq!(
        observed_values(joined),
        RuntimeValue::List([text("001-a"), text("b-b"), text("x-c")].into())
    );
    let dates = h
        .run(
            "yssbi.dataframe.series.datetime.difference",
            &[
                ("series", selected(&source, "date")),
                ("other", selected(&other, "date")),
            ],
            &[("unit", text("day"))],
            series(SemanticType::Numeric),
        )
        .unwrap();
    assert_eq!(
        observed_values(dates),
        RuntimeValue::List(
            [
                RuntimeValue::float64(3.).unwrap(),
                TabularScalar::Null.into(),
                RuntimeValue::float64(5.).unwrap()
            ]
            .into()
        )
    );
}

#[test]
fn independent_sample_tests_accept_different_lengths_and_mixed_storage() {
    let h = Harness::new();
    let source = h.frame(&[("x", Arc::new(Int64Array::from(vec![1, 3, 5, 7])))]);
    let other = h.frame(&[("y", Arc::new(Int64Array::from(vec![2, 4, 6, 8, 9, 10])))]);
    let first = numbers(&[1, 3, 5, 7]);
    let second = numbers(&[2, 4, 6, 8, 9, 10]);
    let output = || output(ValueType::Struct("statistics.report".into()));
    for (id, keys, params) in [
        (
            "mann_whitney",
            ["group1", "group2"],
            vec![("alternative", text("two_sided"))],
        ),
        ("kruskal_wallis", ["groups", "groups"], vec![]),
        ("levene", ["groups", "groups"], vec![]),
    ] {
        let id = format!("yssbi.statistics.test.{id}");
        let expected = h
            .run(
                &id,
                &[(keys[0], first.clone()), (keys[1], second.clone())],
                &params,
                output(),
            )
            .unwrap();
        for inputs in [
            [(keys[0], selected(&source, "x")), (keys[1], second.clone())],
            [
                (keys[0], selected(&source, "x")),
                (keys[1], selected(&other, "y")),
            ],
        ] {
            assert_eq!(h.run(&id, &inputs, &params, output()).unwrap(), expected);
        }
    }
}
