use std::collections::BTreeSet;
use std::io::Cursor;
use std::sync::Arc;

use crate::DatasetStoreError;
use arrow::array::{Array, Int64Array, new_null_array};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::ipc::{reader::FileReader, writer::FileWriter};
use arrow::record_batch::{RecordBatch, RecordBatchOptions};
use sqlx::{Row, SqliteConnection};
use yss_relational_contract::{DatasetColumnPatch, DatasetOverlay};

pub(crate) const MAX_DELTA_BYTES: usize = 16 * 1024 * 1024;

pub(crate) struct EncodedOverlay {
    pub columns: Vec<(Box<str>, Vec<u8>)>,
    pub inserted: Option<Vec<u8>>,
    pub deleted: Box<[i64]>,
}

fn encode(batch: &RecordBatch) -> Result<Vec<u8>, DatasetStoreError> {
    let mut bytes = Vec::new();
    let batch = yss_tabular_arrow::normalize_batch_categories(batch)
        .map_err(|_| DatasetStoreError::InvalidValue)?;
    {
        let mut writer = FileWriter::try_new(&mut bytes, &batch.schema())?;
        writer.write(&batch)?;
        writer.finish()?;
    }
    Ok(bytes)
}

fn decode(bytes: Vec<u8>) -> Result<RecordBatch, DatasetStoreError> {
    let mut reader = FileReader::try_new(Cursor::new(bytes), None)?;
    let batch = reader.next().ok_or(DatasetStoreError::CorruptCatalog)??;
    if reader.next().is_some() {
        return Err(DatasetStoreError::CorruptCatalog);
    }
    Ok(batch)
}

pub(crate) fn align_batch(
    batch: &RecordBatch,
    target: SchemaRef,
) -> Result<RecordBatch, DatasetStoreError> {
    let arrays = target
        .fields()
        .iter()
        .map(|field| {
            let id = yss_tabular_arrow::column_identity(field)
                .map_err(|_| DatasetStoreError::InvalidSchema)?;
            match batch
                .schema()
                .fields()
                .iter()
                .position(|source| yss_tabular_arrow::column_identity(source).ok() == Some(id))
            {
                Some(index) if batch.column(index).data_type() == field.data_type() => {
                    Ok(batch.column(index).clone())
                }
                Some(_) => Err(DatasetStoreError::InvalidSchema),
                None => Ok(new_null_array(field.data_type(), batch.num_rows())),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecordBatch::try_new_with_options(
        target,
        arrays,
        &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
    )?)
}

pub(crate) fn encode_overlay(
    schema: &Schema,
    overlay: &DatasetOverlay,
) -> Result<EncodedOverlay, DatasetStoreError> {
    let mut total = overlay
        .deleted
        .len()
        .checked_mul(8)
        .ok_or(DatasetStoreError::DeltaLimit)?;
    let mut columns = Vec::new();
    for patch in &overlay.columns {
        let Some(field) = schema.fields().iter().find(|field| {
            yss_tabular_arrow::column_identity(field).ok() == Some(patch.column_id.as_ref())
        }) else {
            continue;
        };
        if patch.row_ids.len() != patch.values.len()
            || patch.row_ids.iter().collect::<BTreeSet<_>>().len() != patch.row_ids.len()
        {
            return Err(DatasetStoreError::InvalidValue);
        }
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(vec![
                Field::new("row_id", DataType::Int64, false),
                field.as_ref().clone().with_name("value"),
            ])),
            vec![
                Arc::new(Int64Array::from(patch.row_ids.to_vec())),
                patch.values.clone(),
            ],
        )?;
        let bytes = encode(&batch)?;
        total = total
            .checked_add(bytes.len())
            .ok_or(DatasetStoreError::DeltaLimit)?;
        if total > MAX_DELTA_BYTES {
            return Err(DatasetStoreError::DeltaLimit);
        }
        columns.push((patch.column_id.clone(), bytes));
    }
    let inserted = if overlay.inserted.is_empty() {
        None
    } else {
        let schema = Arc::new(schema.clone());
        let batches = overlay
            .inserted
            .iter()
            .map(|batch| align_batch(batch, schema.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let batch = arrow::compute::concat_batches(&schema, &batches)?;
        let bytes = encode(&batch)?;
        total = total
            .checked_add(bytes.len())
            .ok_or(DatasetStoreError::DeltaLimit)?;
        Some(bytes)
    };
    if total > MAX_DELTA_BYTES {
        return Err(DatasetStoreError::DeltaLimit);
    }
    Ok(EncodedOverlay {
        columns,
        inserted,
        deleted: overlay.deleted.clone(),
    })
}

pub(crate) async fn load_overlay(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> Result<DatasetOverlay, DatasetStoreError> {
    let size: i64 = sqlx::query_scalar("SELECT coalesce((SELECT sum(length(data_ipc)) FROM column_edits WHERE snapshot_id=?),0)+coalesce((SELECT length(data_ipc) FROM inserted_rows WHERE snapshot_id=?),0)+8*(SELECT count(*) FROM deleted_rows WHERE snapshot_id=?)").bind(snapshot).bind(snapshot).bind(snapshot).fetch_one(&mut *connection).await?;
    if size < 0 || size as u64 > MAX_DELTA_BYTES as u64 {
        return Err(DatasetStoreError::CorruptCatalog);
    }
    let mut columns = Vec::new();
    for row in sqlx::query("SELECT column_id,data_ipc FROM column_edits WHERE snapshot_id=?")
        .bind(snapshot)
        .fetch_all(&mut *connection)
        .await?
    {
        let column_id: String = row.try_get("column_id")?;
        let batch = decode(row.try_get("data_ipc")?)?;
        if batch.num_columns() != 2
            || yss_tabular_arrow::column_identity(batch.schema().field(1)).ok()
                != Some(column_id.as_str())
        {
            return Err(DatasetStoreError::CorruptCatalog);
        }
        let row_ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or(DatasetStoreError::CorruptCatalog)?;
        if row_ids.null_count() != 0
            || row_ids.values().iter().collect::<BTreeSet<_>>().len() != row_ids.len()
        {
            return Err(DatasetStoreError::CorruptCatalog);
        }
        columns.push(DatasetColumnPatch {
            column_id: column_id.into(),
            row_ids: row_ids.values().to_vec().into_boxed_slice(),
            values: batch.column(1).clone(),
        });
    }
    let inserted =
        sqlx::query_scalar::<_, Vec<u8>>("SELECT data_ipc FROM inserted_rows WHERE snapshot_id=?")
            .bind(snapshot)
            .fetch_optional(&mut *connection)
            .await?
            .map(decode)
            .transpose()?
            .into_iter()
            .collect();
    let deleted = sqlx::query_scalar::<_, i64>(
        "SELECT row_id FROM deleted_rows WHERE snapshot_id=? ORDER BY row_id",
    )
    .bind(snapshot)
    .fetch_all(&mut *connection)
    .await?
    .into_boxed_slice();
    Ok(DatasetOverlay {
        columns: columns.into_boxed_slice(),
        inserted,
        deleted,
    })
}
