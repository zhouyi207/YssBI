use arrow::array::{Array, Date32Array, Int64Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use std::sync::Arc;
use yss_sci_runtime::preprocessing::{PreparationError, align_batches};

#[test]
fn streamed_alignment_preserves_column_order_types_metadata_and_nulls() {
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
    let aligned = align_batches(
        &[batch.slice(0, 2), batch.slice(2, 2)],
        "date",
        Some("entity"),
        1,
        64 * 1024,
    )
    .unwrap();
    assert_eq!(aligned.num_rows(), 5);
    assert_eq!(aligned.schema().metadata(), schema.metadata());
    for (actual, original) in aligned.schema().fields().iter().zip(schema.fields()) {
        assert_eq!(actual.name(), original.name());
        assert_eq!(actual.data_type(), original.data_type());
        assert_eq!(actual.metadata(), original.metadata());
    }
    let count = aligned
        .column(3)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    assert_eq!(
        count.iter().collect::<Vec<_>>(),
        [Some(10), None, Some(30), Some(100), Some(150)]
    );
    let labels = aligned
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(
        labels.iter().collect::<Vec<_>>(),
        [Some("start"), None, Some("end"), None, Some("other")]
    );
    assert_eq!(
        aligned
            .column(2)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .values()
            .as_ref(),
        &[10, 10, 10, 20, 20]
    );

    let temporal = align_batches(&[batch.slice(0, 2)], "date", None, 1, 64 * 1024).unwrap();
    assert_eq!(temporal.num_rows(), 3);
    assert_eq!(temporal.column(1).data_type(), &DataType::Date32);
    assert!(temporal.column(0).is_null(1));
    assert!(temporal.column(2).is_null(1));
    assert_eq!(temporal.schema().metadata(), schema.metadata());
}

#[test]
fn alignment_rejects_duplicate_off_grid_and_oversized_inputs_without_dropping_rows() {
    let make = |times: Vec<i64>, entities: Vec<i64>| {
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
        .unwrap()
    };
    let duplicate = make(vec![0, 0], vec![1, 1]);
    for entity in [None, Some("entity")] {
        assert!(matches!(
            align_batches(
                std::slice::from_ref(&duplicate),
                "time",
                entity,
                1,
                64 * 1024
            ),
            Err(PreparationError::DuplicateTime)
        ));
    }
    let off_grid = make(vec![0, 1, 2], vec![1, 1, 1]);
    for entity in [None, Some("entity")] {
        assert!(matches!(
            align_batches(
                std::slice::from_ref(&off_grid),
                "time",
                entity,
                2,
                64 * 1024
            ),
            Err(PreparationError::Interval)
        ));
        assert!(matches!(
            align_batches(std::slice::from_ref(&off_grid), "time", entity, 1, 1),
            Err(PreparationError::MemoryLimit)
        ));
    }
    let huge_grid = make(vec![0, i64::MAX], vec![1, 1]);
    assert!(matches!(
        align_batches(&[huge_grid], "time", None, 1, 64 * 1024),
        Err(PreparationError::MemoryLimit)
    ));
    let unsigned_overflow = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new(
            "time",
            DataType::UInt64,
            false,
        )])),
        vec![Arc::new(UInt64Array::from(vec![0, u64::MAX]))],
    )
    .unwrap();
    assert!(matches!(
        align_batches(&[unsigned_overflow], "time", None, 1, 64 * 1024),
        Err(PreparationError::Overflow)
    ));
    // Many observed times do not imply every entity spans the entire time grid.
    let sparse = make((0..200).collect(), (0..200).collect());
    assert_eq!(
        align_batches(&[sparse], "time", Some("entity"), 1, 64 * 1024)
            .unwrap()
            .num_rows(),
        200
    );
}
