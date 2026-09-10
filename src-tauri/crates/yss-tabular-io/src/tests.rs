use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use arrow::array::{ArrayRef, Decimal128Array, StringArray, TimestampNanosecondArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatchReader;
use yss_tabular_arrow::{array_to_json, with_column_metadata};

use super::*;

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn create() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
                "yssbi-arrow-io-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn batches_round_trip_exact_schema_and_values_through_ipc_and_parquet() {
    let directory = TestDirectory::create();
    let schema = Arc::new(Schema::new(vec![
        with_column_metadata(Field::new("id", DataType::UInt64, false), "id-0", None).unwrap(),
        with_column_metadata(
            Field::new("amount", DataType::Decimal128(38, 12), true),
            "id-1",
            None,
        )
        .unwrap(),
        with_column_metadata(
            Field::new(
                "at",
                DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                true,
            ),
            "id-2",
            None,
        )
        .unwrap(),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from(vec![u64::MAX, 1])) as ArrayRef,
            Arc::new(
                Decimal128Array::from(vec![Some(12345678901234567890123456789012345678), None])
                    .with_precision_and_scale(38, 12)
                    .unwrap(),
            ),
            Arc::new(TimestampNanosecondArray::from(vec![Some(-1), None]).with_timezone("UTC")),
        ],
    )
    .unwrap();
    let ipc = directory.path().join("nested/data.arrow");
    let parquet = directory.path().join("nested/data.parquet");
    write_ipc_batches(&ipc, &schema, [Ok(batch.clone()), Ok(batch.clone())]).unwrap();
    // Julia's IPC reader requires the first message immediately after the magic header.
    assert_eq!(
        &std::fs::read(&ipc).unwrap()[..12],
        b"ARROW1\0\0\xff\xff\xff\xff"
    );
    let reader = read_ipc_batches(&ipc, None).unwrap();
    assert_eq!(reader.schema(), schema);
    write_parquet_batches(&parquet, schema.clone(), reader).unwrap();
    let reader = read_parquet_batches(&parquet, 1, None).unwrap();
    assert_eq!(reader.schema(), schema);
    let pages = reader.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(pages.len(), 4);
    for (index, page) in pages.iter().enumerate() {
        for column in 0..3 {
            let expected = array_to_json(batch.column(column).as_ref()).unwrap();
            assert_eq!(
                array_to_json(page.column(column).as_ref()).unwrap(),
                vec![expected[index % 2].clone()]
            );
        }
    }
    let projected = read_parquet_batches(&parquet, 2, Some(&[1])).unwrap();
    assert_eq!(projected.schema().fields().len(), 1);
    assert_eq!(projected.schema().field(0), schema.field(1));
    assert!(read_parquet_batches(&parquet, 2, Some(&[3])).is_err());
}

#[test]
fn csv_stream_has_one_header_and_propagates_late_batch_failure() {
    let directory = TestDirectory::create();
    let csv = directory.path().join("data.csv");
    let schema = Arc::new(Schema::new(vec![Field::new("label", DataType::Utf8, true)]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(StringArray::from(vec!["a", "b"]))],
    )
    .unwrap();
    write_csv_batches(&csv, [Ok(batch.clone()), Ok(batch.clone())]).unwrap();
    assert_eq!(fs::read_to_string(&csv).unwrap(), "label\na\nb\na\nb\n");
    let reader = read_csv_batches(&csv, b',', true, 2, 1).unwrap();
    assert_eq!(reader.collect::<Result<Vec<_>, _>>().unwrap().len(), 4);
    let failure = write_ipc_batches(
        &directory.path().join("failed.arrow"),
        &schema,
        [
            Ok(batch),
            Err(ArrowError::ComputeError("source failed".into())),
        ],
    )
    .unwrap_err();
    assert_eq!(failure.phase(), TabularIoPhase::Encode);
    assert_eq!(failure.format(), TabularIoFormat::ArrowIpc);
    assert_eq!(failure.operation(), TabularIoOperation::Write);
}

#[test]
fn current_directory_output_has_no_parent_to_create() {
    assert_eq!(output_parent(Path::new("data.csv")), None);
    assert_eq!(
        output_parent(Path::new("nested/data.csv")),
        Some(Path::new("nested"))
    );
}

#[test]
fn csv_wide_multiline_values_are_batched_by_bytes_and_oversized_records_fail() {
    use std::io::Write;
    let directory = TestDirectory::create();
    let path = directory.path().join("wide.csv");
    let label = format!("{}\n\"quoted\"", "x".repeat(4096));
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("label", DataType::Utf8, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt64Array::from(vec![u64::MAX; 5000])),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                label.as_str(),
                5000,
            ))),
        ],
    )
    .unwrap();
    write_csv_batches(&path, [Ok(batch)]).unwrap();
    let reader = read_csv_batches(&path, b',', true, 2, 50_000).unwrap();
    assert_eq!(reader.schema().field(0).data_type(), &DataType::Utf8);
    let mut count = 0;
    let mut batches = 0;
    for batch in reader {
        let batch = batch.unwrap();
        assert!(batch.get_array_memory_size() <= MAX_CSV_BATCH_BYTES);
        assert!(batch.num_rows() < 5000);
        assert_eq!(
            array_to_json(batch.column(0).as_ref()).unwrap()[0],
            serde_json::json!(u64::MAX.to_string())
        );
        assert_eq!(
            array_to_json(batch.column(1).as_ref()).unwrap()[0],
            serde_json::json!(label)
        );
        count += batch.num_rows();
        batches += 1;
    }
    assert_eq!(count, 5000);
    assert!(batches > 1);
    let oversized = directory.path().join("oversized.csv");
    let mut file = File::create(&oversized).unwrap();
    file.write_all(b"label\n").unwrap();
    file.write_all(&vec![b'x'; MAX_CSV_BATCH_BYTES + 1])
        .unwrap();
    drop(file);
    let mut reader = read_csv_batches(&oversized, b',', true, 1, 50_000).unwrap();
    assert!(reader.next().unwrap().is_err());
    assert!(reader.next().is_none());
}

#[test]
fn ipc_rekeys_independent_dictionaries_against_the_persisted_category_domain() {
    use arrow::array::{DictionaryArray, Int8Array};
    use arrow::datatypes::Int8Type;
    use yss_tabular_arrow::CategoryDomain;
    let directory = TestDirectory::create();
    let path = directory.path().join("categories.arrow");
    let domain = CategoryDomain {
        labels: vec!["high".into(), "low".into()],
        ordered: true,
    };
    let field = with_column_metadata(
        Field::new(
            "grade",
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        "grade-id",
        Some(&domain),
    )
    .unwrap();
    let schema = Arc::new(Schema::new(vec![field]));
    let batches = [["high", "low"], ["low", "high"]].map(|labels| {
        RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(
                DictionaryArray::<Int8Type>::try_new(
                    Int8Array::from(vec![Some(0), None]),
                    Arc::new(StringArray::from(labels.to_vec())),
                )
                .unwrap(),
            )],
        )
        .unwrap()
    });
    write_ipc_batches(&path, &schema, batches.into_iter().map(Ok)).unwrap();
    let mut reader = read_ipc_batches(&path, None).unwrap();
    assert_eq!(
        CategoryDomain::from_field(reader.schema().field(0)).unwrap(),
        Some(domain)
    );
    for expected in ["high", "low"] {
        let batch = reader.next().unwrap().unwrap();
        assert_eq!(
            array_to_json(batch.column(0).as_ref()).unwrap(),
            vec![
                serde_json::Value::String(expected.into()),
                serde_json::Value::Null
            ]
        );
    }
}
