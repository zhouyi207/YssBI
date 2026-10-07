use super::*;
use arrow::array::{Date32Array, UInt64Array};

#[test]
fn alignment_preserves_column_order_types_metadata_and_nulls() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("label", DataType::Utf8, true),
            Field::new("date", DataType::Date32, false),
            Field::new("entity", DataType::Int64, false)
                .with_metadata([("column_id".into(), "entity-id".into())].into()),
            Field::new("count", DataType::Int64, false),
        ],
        [("source".into(), "observations".into())].into(),
    ));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec![
                Some("end"),
                Some("start"),
                None,
                Some("other"),
            ])),
            Arc::new(Date32Array::from(vec![2, 0, 0, 1])),
            Arc::new(Int64Array::from(vec![10, 10, 20, 20])),
            Arc::new(Int64Array::from(vec![30, 10, 100, 150])),
        ],
    )
    .unwrap();
    let source = runtime.batch_relation(binding(), batch.clone()).unwrap();
    let aligned = source
        .align_grid("date", Some("entity"), 1, 64 * 1024)
        .unwrap();
    let mut batches = Vec::new();
    aligned
        .visit_batches(&control(), &mut |batch| {
            batches.push(batch);
            Ok(())
        })
        .unwrap();
    let output = arrow::compute::concat_batches(&aligned.schema(), &batches).unwrap();
    assert_eq!(output.num_rows(), 5);
    assert_eq!(output.schema().metadata(), schema.metadata());
    for (actual, original) in output.schema().fields().iter().zip(schema.fields()) {
        assert_eq!(actual.name(), original.name());
        assert_eq!(actual.data_type(), original.data_type());
        assert_eq!(actual.metadata(), original.metadata());
    }
    assert_eq!(
        output
            .column(3)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [Some(10), None, Some(30), Some(100), Some(150)],
    );
    assert_eq!(
        output
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [Some("start"), None, Some("end"), None, Some("other")],
    );
    let temporal = runtime
        .batch_relation(binding(), batch.slice(0, 2))
        .unwrap()
        .align_grid("date", None, 1, 64 * 1024)
        .unwrap();
    assert_eq!(temporal.schema().field(1).data_type(), &DataType::Date32);
    let page = temporal.page(0, 10, &control()).unwrap();
    assert_eq!(page.row_count, 3);
    assert_eq!(
        page.data.columns()[0].values()[1],
        yss_data_contract::TabularScalar::Null
    );
    assert_eq!(
        page.data.columns()[2].values()[1],
        yss_data_contract::TabularScalar::Null
    );
}

#[test]
fn alignment_rejects_invalid_grids_and_bounds_each_entity_span() {
    let runtime = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let table = |times: Vec<i64>, entities: Vec<i64>| {
        runtime
            .batch_relation(
                binding(),
                RecordBatch::try_new(
                    Arc::new(Schema::new(vec![
                        Field::new("time", DataType::Int64, false),
                        Field::new("entity", DataType::Int64, false),
                    ])),
                    vec![
                        Arc::new(Int64Array::from(times)),
                        Arc::new(Int64Array::from(entities)),
                    ],
                )
                .unwrap(),
            )
            .unwrap()
    };
    let consume = |relation: Result<RelationHandle, RelationError>| {
        relation.and_then(|relation| relation.visit_batches(&control(), &mut |_| Ok(())))
    };
    let off_grid = table(vec![0, 1, 2], vec![1, 1, 1]);
    for entity in [None, Some("entity")] {
        assert_eq!(
            consume(off_grid.align_grid("time", entity, 2, 64 * 1024)),
            Err(RelationError::InvalidInput)
        );
        assert_eq!(
            consume(off_grid.align_grid("time", entity, 1, 1)),
            Err(RelationError::MemoryLimitExceeded)
        );
    }
    let oversized = table(vec![0, 1_000_000], vec![1, 1]);
    assert_eq!(
        consume(oversized.align_grid("time", None, 1, 64 * 1024)),
        Err(RelationError::MemoryLimitExceeded)
    );
    let unsigned = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![Field::new(
                    "time",
                    DataType::UInt64,
                    false,
                )])),
                vec![Arc::new(UInt64Array::from(vec![0, u64::MAX]))],
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        consume(unsigned.align_grid("time", None, 1, 64 * 1024)),
        Err(RelationError::InvalidInput)
    );
    let missing = runtime
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![Field::new(
                    "time",
                    DataType::Date32,
                    true,
                )])),
                vec![Arc::new(Date32Array::from(vec![Some(0), None]))],
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        consume(missing.align_grid("time", None, 1, 64 * 1024)),
        Err(RelationError::InvalidInput)
    );
    let sparse = table((0..200).collect(), (0..200).collect());
    let aligned = sparse
        .align_grid("time", Some("entity"), 1, 64 * 1024)
        .unwrap();
    assert_eq!(aligned.page(0, 201, &control()).unwrap().row_count, 200);
}
