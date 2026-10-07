use std::collections::BTreeSet;
use std::io::Cursor;
use std::sync::Arc;

use crate::{DatasetSnapshot, DatasetStoreError};
use arrow::array::{Array, Int64Array, new_null_array};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::ipc::{reader::FileReader, writer::FileWriter};
use arrow::record_batch::{RecordBatch, RecordBatchOptions};
use sqlx::{Row, SqliteConnection};
use yss_relational_contract::{DatasetColumnPatch, DatasetOverlay};

pub(crate) const MAX_DELTA_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct OverlayBlob {
    pub id: Box<str>,
    pub byte_size: usize,
    // Committed snapshots retain only the reference, not a second copy of the Arrow data.
    pub data: Option<Vec<u8>>,
}

impl OverlayBlob {
    fn new(data: Vec<u8>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string().into(),
            byte_size: data.len(),
            data: Some(data),
        }
    }

    fn stored(id: String, byte_size: usize) -> Self {
        Self {
            id: id.into(),
            byte_size,
            data: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct EncodedOverlay {
    pub columns: Vec<(Box<str>, OverlayBlob)>,
    pub inserted: Option<OverlayBlob>,
    pub deleted: Option<OverlayBlob>,
}

impl EncodedOverlay {
    pub fn blobs(&self) -> impl Iterator<Item = &OverlayBlob> {
        self.columns
            .iter()
            .map(|(_, blob)| blob)
            .chain(self.inserted.iter())
            .chain(self.deleted.iter())
    }

    pub fn into_persisted(mut self) -> Self {
        for blob in self
            .columns
            .iter_mut()
            .map(|(_, blob)| blob)
            .chain(self.inserted.iter_mut())
            .chain(self.deleted.iter_mut())
        {
            blob.data = None;
        }
        self
    }
}

fn encode(batch: &RecordBatch) -> Result<Vec<u8>, DatasetStoreError> {
    let mut bytes = Vec::new();
    let batch = yss_database_arrow::normalize_batch_categories(batch)
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
            let id = yss_database_arrow::column_identity(field)
                .map_err(|_| DatasetStoreError::InvalidSchema)?;
            match batch
                .schema()
                .fields()
                .iter()
                .position(|source| yss_database_arrow::column_identity(source).ok() == Some(id))
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
    previous: Option<&DatasetSnapshot>,
) -> Result<EncodedOverlay, DatasetStoreError> {
    let mut total = 0usize;
    let mut columns = Vec::new();
    for patch in &overlay.columns {
        let Some(field) = schema.fields().iter().find(|field| {
            yss_database_arrow::column_identity(field).ok() == Some(patch.column_id.as_ref())
        }) else {
            continue;
        };
        let reused = previous.and_then(|before| {
            let old = before
                .overlay
                .columns
                .iter()
                .find(|old| old.column_id == patch.column_id)?;
            let old_field = before.metadata.schema.fields().iter().find(|old| {
                yss_database_arrow::column_identity(old).ok() == Some(patch.column_id.as_ref())
            })?;
            if !Arc::ptr_eq(&old.values, &patch.values)
                || old.row_ids != patch.row_ids
                || old_field.as_ref().clone().with_name("value")
                    != field.as_ref().clone().with_name("value")
            {
                return None;
            }
            before
                .encoded_overlay
                .columns
                .iter()
                .find(|(id, _)| id == &patch.column_id)
                .map(|(_, blob)| blob.clone())
        });
        let blob = if let Some(blob) = reused {
            blob
        } else {
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
            OverlayBlob::new(encode(&batch)?)
        };
        total = total
            .checked_add(blob.byte_size)
            .ok_or(DatasetStoreError::DeltaLimit)?;
        if total > MAX_DELTA_BYTES {
            return Err(DatasetStoreError::DeltaLimit);
        }
        columns.push((patch.column_id.clone(), blob));
    }
    let inserted = if overlay.inserted.is_empty() {
        None
    } else {
        let reused =
            previous
                .filter(|before| {
                    before.metadata.schema.as_ref() == schema
                        && before.overlay.inserted.len() == overlay.inserted.len()
                        && before.overlay.inserted.iter().zip(&overlay.inserted).all(
                            |(old, batch)| {
                                old.schema() == batch.schema()
                                    && old.num_rows() == batch.num_rows()
                                    && old
                                        .columns()
                                        .iter()
                                        .zip(batch.columns())
                                        .all(|(a, b)| Arc::ptr_eq(a, b))
                            },
                        )
                })
                .and_then(|before| before.encoded_overlay.inserted.clone());
        let blob = if let Some(blob) = reused {
            blob
        } else {
            let schema = Arc::new(schema.clone());
            let batches = overlay
                .inserted
                .iter()
                .map(|batch| align_batch(batch, schema.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            let batch = arrow::compute::concat_batches(&schema, &batches)?;
            OverlayBlob::new(encode(&batch)?)
        };
        total = total
            .checked_add(blob.byte_size)
            .ok_or(DatasetStoreError::DeltaLimit)?;
        Some(blob)
    };
    let deleted = if overlay.deleted.is_empty() {
        None
    } else {
        if overlay.deleted.len() > MAX_DELTA_BYTES / 8 {
            return Err(DatasetStoreError::DeltaLimit);
        }
        let reused = previous
            .filter(|before| before.overlay.deleted == overlay.deleted)
            .and_then(|before| before.encoded_overlay.deleted.clone());
        let blob = match reused {
            Some(blob) => blob,
            None => {
                let batch = RecordBatch::try_new(
                    Arc::new(Schema::new(vec![Field::new(
                        "row_id",
                        DataType::Int64,
                        false,
                    )])),
                    vec![Arc::new(Int64Array::from(overlay.deleted.to_vec()))],
                )?;
                OverlayBlob::new(encode(&batch)?)
            }
        };
        total = total
            .checked_add(blob.byte_size)
            .ok_or(DatasetStoreError::DeltaLimit)?;
        Some(blob)
    };
    if total > MAX_DELTA_BYTES {
        return Err(DatasetStoreError::DeltaLimit);
    }
    Ok(EncodedOverlay {
        columns,
        inserted,
        deleted,
    })
}

pub(crate) async fn load_overlay(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> Result<(DatasetOverlay, EncodedOverlay), DatasetStoreError> {
    let (size, missing): (i64, i64) = sqlx::query_as("SELECT coalesce(sum(length(b.data_ipc)),0),count(*)-count(b.id) FROM (SELECT blob_id FROM column_edits WHERE snapshot_id=? UNION ALL SELECT blob_id FROM inserted_rows WHERE snapshot_id=? UNION ALL SELECT blob_id FROM deleted_rows WHERE snapshot_id=?) refs LEFT JOIN overlay_blobs b ON b.id=refs.blob_id").bind(snapshot).bind(snapshot).bind(snapshot).fetch_one(&mut *connection).await?;
    if missing != 0 || size < 0 || size as u64 > MAX_DELTA_BYTES as u64 {
        return Err(DatasetStoreError::CorruptCatalog);
    }
    let mut columns = Vec::new();
    let mut encoded = EncodedOverlay::default();
    for row in sqlx::query("SELECT e.column_id,b.id,b.data_ipc FROM column_edits e JOIN overlay_blobs b ON b.id=e.blob_id WHERE e.snapshot_id=?")
        .bind(snapshot)
        .fetch_all(&mut *connection)
        .await?
    {
        let column_id: String = row.try_get("column_id")?;
        let bytes: Vec<u8> = row.try_get("data_ipc")?;
        let blob = OverlayBlob::stored(row.try_get("id")?, bytes.len());
        let batch = decode(bytes)?;
        if batch.num_columns() != 2
            || yss_database_arrow::column_identity(batch.schema().field(1)).ok()
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
        encoded.columns.push((column_id.clone().into(), blob));
        columns.push(DatasetColumnPatch {
            column_id: column_id.into(),
            row_ids: row_ids.values().to_vec().into_boxed_slice(),
            values: batch.column(1).clone(),
        });
    }
    let inserted =
        sqlx::query_as::<_, (String, Vec<u8>)>("SELECT b.id,b.data_ipc FROM inserted_rows e JOIN overlay_blobs b ON b.id=e.blob_id WHERE e.snapshot_id=?")
            .bind(snapshot)
            .fetch_optional(&mut *connection)
            .await?
            .map(|(id, bytes)| {
                encoded.inserted = Some(OverlayBlob::stored(id, bytes.len()));
                decode(bytes)
            })
            .transpose()?
            .into_iter()
            .collect();
    let deleted = sqlx::query_as::<_, (String, Vec<u8>)>(
        "SELECT b.id,b.data_ipc FROM deleted_rows e JOIN overlay_blobs b ON b.id=e.blob_id WHERE e.snapshot_id=?",
    )
    .bind(snapshot)
    .fetch_optional(&mut *connection)
    .await?
    .map(|(id, bytes)| {
        encoded.deleted = Some(OverlayBlob::stored(id, bytes.len()));
        let batch = decode(bytes)?;
        if batch.num_columns() != 1 {
            return Err(DatasetStoreError::CorruptCatalog);
        }
        let rows = batch.column(0).as_any().downcast_ref::<Int64Array>()
            .ok_or(DatasetStoreError::CorruptCatalog)?;
        if rows.null_count() != 0 || rows.values().windows(2).any(|ids| ids[0] >= ids[1]) {
            return Err(DatasetStoreError::CorruptCatalog);
        }
        Ok(rows.values().to_vec().into_boxed_slice())
    }).transpose()?.unwrap_or_default();
    Ok((
        DatasetOverlay {
            columns: columns.into_boxed_slice(),
            inserted,
            deleted,
        },
        encoded,
    ))
}
