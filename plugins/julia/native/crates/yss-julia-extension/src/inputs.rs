use std::sync::Arc;

use arrow::array::{Array, ArrayRef, Float64Array, StringArray};
use arrow::datatypes::DataType;
use yss_plugin_protocol::PluginFailure;
use yss_sci_contract::{StatisticalInput, StatisticalScalar};

const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;

fn invalid() -> PluginFailure {
    PluginFailure::new("plugin_snapshot_invalid")
}

pub fn read_inputs(path: &str) -> Result<Arc<[StatisticalInput]>, PluginFailure> {
    let file = std::fs::File::open(path).map_err(|_| invalid())?;
    let reader = arrow::ipc::reader::FileReader::try_new(file, None).map_err(|_| invalid())?;
    let schema = reader.schema();
    let mut columns = vec![Vec::new(); schema.fields().len()];
    let mut bytes = 0usize;
    for batch in reader {
        let batch = batch.map_err(|_| invalid())?;
        if batch.get_array_memory_size() > MAX_INPUT_BYTES {
            return Err(invalid());
        }
        for (array, values) in batch.columns().iter().zip(&mut columns) {
            let array = numeric_or_category(array)?;
            for row in 0..array.len() {
                let value = if array.is_null(row) {
                    None
                } else if let Some(strings) = array.as_any().downcast_ref::<StringArray>() {
                    let value = strings.value(row);
                    bytes = bytes.checked_add(value.len()).ok_or_else(invalid)?;
                    Some(StatisticalScalar::Category(value.into()))
                } else {
                    let value = array
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .ok_or_else(invalid)?
                        .value(row);
                    if !value.is_finite() {
                        return Err(invalid());
                    }
                    Some(StatisticalScalar::Numeric(value))
                };
                // Include Vec capacity growth and enum storage as well as category text.
                bytes = bytes
                    .checked_add(2 * std::mem::size_of::<Option<StatisticalScalar>>())
                    .ok_or_else(invalid)?;
                if bytes > MAX_INPUT_BYTES {
                    return Err(invalid());
                }
                values.push(value);
            }
        }
    }
    schema
        .fields()
        .iter()
        .zip(columns)
        .map(|(field, values)| {
            StatisticalInput::try_new(field.name().as_str().into(), values.into(), None)
                .map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Arc::from)
}

fn numeric_or_category(array: &ArrayRef) -> Result<ArrayRef, PluginFailure> {
    if array.null_count() == array.len() {
        return Ok(Arc::new(Float64Array::from(vec![None; array.len()])));
    }
    let cast = |array: &dyn Array, target: &DataType| {
        arrow::compute::cast(array, target).map_err(|_| invalid())
    };
    match array.data_type() {
        DataType::Boolean | DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => {
            cast(array.as_ref(), &DataType::Utf8)
        }
        DataType::Dictionary(_, values)
            if matches!(
                values.as_ref(),
                DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
            ) =>
        {
            cast(array.as_ref(), &DataType::Utf8)
        }
        DataType::Date32 | DataType::Time32(_) => cast(
            cast(array.as_ref(), &DataType::Int32)?.as_ref(),
            &DataType::Float64,
        ),
        DataType::Date64
        | DataType::Timestamp(..)
        | DataType::Time64(_)
        | DataType::Duration(_) => cast(
            cast(array.as_ref(), &DataType::Int64)?.as_ref(),
            &DataType::Float64,
        ),
        data_type if data_type.is_numeric() => cast(array.as_ref(), &DataType::Float64),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
#[test]
fn arrow_snapshot_preserves_batch_alignment_labels_temporal_values_and_missingness() {
    use arrow::array::{
        BooleanArray, Date32Array, Decimal128Array, DictionaryArray, Int8Array,
        TimestampNanosecondArray, UInt64Array,
    };
    use arrow::datatypes::Int8Type;
    use arrow::record_batch::RecordBatch;
    let path =
        std::env::temp_dir().join(format!("yss-plugin-input-{}.arrow", uuid::Uuid::new_v4()));
    struct RemoveFile(std::path::PathBuf);
    impl Drop for RemoveFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = RemoveFile(path);
    let dictionary = DictionaryArray::<Int8Type>::try_new(
        Int8Array::from(vec![Some(0), None, Some(1)]),
        Arc::new(StringArray::from(vec!["B", "A"])),
    )
    .unwrap();
    let batch = RecordBatch::try_from_iter(vec![
        (
            "wide",
            Arc::new(UInt64Array::from(vec![Some(u64::MAX), None, Some(3)])) as ArrayRef,
        ),
        ("group", Arc::new(dictionary)),
        (
            "flag",
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])),
        ),
        (
            "day",
            Arc::new(Date32Array::from(vec![Some(10), None, Some(12)])),
        ),
        (
            "time",
            Arc::new(TimestampNanosecondArray::from(vec![
                Some(1000),
                None,
                Some(3000),
            ])),
        ),
        (
            "amount",
            Arc::new(
                Decimal128Array::from(vec![Some(325), None, Some(400)])
                    .with_precision_and_scale(10, 2)
                    .unwrap(),
            ),
        ),
    ])
    .unwrap();
    let mut writer = arrow::ipc::writer::FileWriter::try_new(
        std::fs::File::create(&file.0).unwrap(),
        &batch.schema(),
    )
    .unwrap();
    writer.write(&batch.slice(0, 1)).unwrap();
    writer.write(&batch.slice(1, 2)).unwrap();
    writer.finish().unwrap();
    drop(writer);
    let values = read_inputs(file.0.to_str().unwrap()).unwrap();
    assert_eq!(values.len(), 6);
    assert!(
        values
            .iter()
            .all(|column| column.values().len() == 3 && column.values()[1].is_none())
    );
    assert_eq!(
        values[0].values()[0],
        Some(StatisticalScalar::Numeric(u64::MAX as f64))
    );
    assert_eq!(
        values[1].values()[0],
        Some(StatisticalScalar::Category("B".into()))
    );
    assert_eq!(
        values[1].values()[2],
        Some(StatisticalScalar::Category("A".into()))
    );
    assert_eq!(
        values[2].values()[2],
        Some(StatisticalScalar::Category("false".into()))
    );
    assert_eq!(values[3].values()[0], Some(StatisticalScalar::Numeric(10.)));
    assert_eq!(
        values[4].values()[2],
        Some(StatisticalScalar::Numeric(3000.))
    );
    assert_eq!(
        values[5].values()[0],
        Some(StatisticalScalar::Numeric(3.25))
    );
    let invalid_batch = RecordBatch::try_from_iter(vec![(
        "bad",
        Arc::new(Float64Array::from(vec![1., f64::NAN])) as ArrayRef,
    )])
    .unwrap();
    let mut writer = arrow::ipc::writer::FileWriter::try_new(
        std::fs::File::create(&file.0).unwrap(),
        &invalid_batch.schema(),
    )
    .unwrap();
    writer.write(&invalid_batch).unwrap();
    writer.finish().unwrap();
    drop(writer);
    assert_eq!(
        read_inputs(file.0.to_str().unwrap()).unwrap_err().code,
        "plugin_snapshot_invalid"
    );
}
