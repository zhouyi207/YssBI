use std::collections::BTreeSet;
use std::sync::Arc;

use arrow::array::{Array, Int64Array, StringArray, new_null_array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use serde_json::Value;
use uuid::Uuid;
use yss_datafusion::{DataFusionRuntime, DatasetQuery};
use yss_relational_contract::{
    DatasetColumnPatch, DatasetRelationInput, RelationBinding, RelationControl,
};

use crate::{DatasetSnapshot, DatasetStore, DatasetStoreError, PreparedDataset};

pub struct DatasetCellEdit<'a> {
    pub row_id: i64,
    pub column: &'a str,
    pub value: Value,
}

pub struct DatasetColumnCast<'a> {
    pub column: &'a str,
    pub data_type: DataType,
    pub force: bool,
}

fn user_column<'a>(
    snapshot: &'a DatasetSnapshot,
    name: &str,
) -> Result<&'a Field, DatasetStoreError> {
    let rows = yss_tabular_arrow::dataset_row_columns(&snapshot.metadata.schema)
        .map_err(|_| DatasetStoreError::InvalidSchema)?
        .ok_or(DatasetStoreError::InvalidSchema)?;
    if name == rows.row_id || name == rows.display_order {
        return Err(DatasetStoreError::InvalidValue);
    }
    snapshot
        .metadata
        .schema
        .field_with_name(name)
        .map_err(|_| DatasetStoreError::InvalidValue)
}

impl DatasetStore {
    pub fn prepare_compaction(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        self.materialize(
            before,
            operation,
            before.metadata.schema.clone(),
            before.query(engine, "dataset-compaction")?,
            false,
            control,
        )
    }

    pub fn prepare_cast_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        cast: DatasetColumnCast<'_>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let DatasetColumnCast {
            column: name,
            data_type,
            force,
        } = cast;
        let field = user_column(before, name)?;
        let query = before.query(engine, "dataset-cast")?;
        let categories = if matches!(data_type, DataType::Dictionary(..)) {
            Some(yss_tabular_arrow::CategoryDomain {
                labels: query.distinct_labels(name, control)?,
                ordered: false,
            })
        } else {
            None
        };
        let id = yss_tabular_arrow::column_identity(field)
            .map_err(|_| DatasetStoreError::InvalidSchema)?;
        let field = yss_tabular_arrow::with_column_metadata(
            field.clone().with_data_type(data_type),
            id,
            categories.as_ref(),
        )
        .map_err(|_| DatasetStoreError::InvalidSchema)?;
        let schema = Arc::new(Schema::new_with_metadata(
            before
                .metadata
                .schema
                .fields()
                .iter()
                .map(|current| {
                    if current.name() == name {
                        Arc::new(field.clone())
                    } else {
                        current.clone()
                    }
                })
                .collect::<Vec<_>>(),
            before.metadata.schema.metadata().clone(),
        ));
        let query = query.cast_column(name, schema.clone(), force)?;
        self.materialize(before, operation, schema, query, true, control)
    }

    fn materialize(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        schema: Arc<Schema>,
        query: DatasetQuery,
        data_changed: bool,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        if !Arc::ptr_eq(self, &before.store) {
            return Err(DatasetStoreError::InvalidIdentity);
        }
        let mut metadata = before.metadata.clone();
        metadata.snapshot_id = Uuid::new_v4().to_string().into();
        metadata.generation_id = Uuid::new_v4().to_string().into();
        metadata.data_revision = metadata
            .data_revision
            .checked_add(u64::from(data_changed))
            .ok_or(DatasetStoreError::InvalidSchema)?;
        metadata.schema_revision = metadata
            .schema_revision
            .checked_add(u64::from(schema != metadata.schema))
            .ok_or(DatasetStoreError::InvalidSchema)?;
        metadata.schema = schema;
        let mut prepared = self.prepare_generation(
            metadata,
            operation,
            Some(before.metadata.snapshot_id.clone()),
        )?;
        prepared.next_row_id = before.next_row_id;
        prepared.snapshot_leases = Box::new([before.clone()]);
        self.write_generation(prepared, query, control)
    }

    fn write_generation(
        &self,
        mut prepared: PreparedDataset,
        query: DatasetQuery,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let expected_rows = prepared.metadata.row_count;
        let mut writer = crate::prepare::GenerationWriter::new(&prepared)?;
        let mut write_error = None;
        let result = query.visit_batches(control, &mut |batch| {
            writer.write(&batch).map_err(|error| {
                write_error = Some(error);
                yss_relational_contract::RelationError::QueryFailed
            })
        });
        if let Some(error) = write_error {
            return Err(error);
        }
        result?;
        writer.finish(&mut prepared)?;
        if prepared.metadata.row_count != expected_rows {
            return Err(DatasetStoreError::InvalidValue);
        }
        Ok(prepared)
    }

    fn finish_delta(
        self: &Arc<Self>,
        prepared: PreparedDataset,
        engine: &Arc<DataFusionRuntime>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        match crate::codec::encode_overlay(&prepared.metadata.schema, &prepared.overlay) {
            Ok(_) => return Ok(prepared),
            Err(DatasetStoreError::DeltaLimit) => {}
            Err(error) => return Err(error),
        }
        let query = engine.dataset_query(
            RelationBinding {
                project_session: "dataset-edit-compaction".into(),
                dataset: prepared.metadata.id.clone(),
                snapshot: prepared.metadata.snapshot_id.clone(),
                revision: prepared.metadata.data_revision,
            },
            DatasetRelationInput {
                base_schema: prepared.base_schema.clone(),
                schema: prepared.metadata.schema.clone(),
                files: prepared
                    .files
                    .iter()
                    .map(|file| self.root.join(&file.relative_path))
                    .collect(),
                overlay: prepared.overlay.clone(),
            },
            Arc::new(prepared.snapshot_leases.clone()),
        )?;
        let mut metadata = prepared.metadata.clone();
        metadata.generation_id = Uuid::new_v4().to_string().into();
        let mut compacted = self.prepare_generation(
            metadata,
            &prepared.operation_id,
            prepared.expected_snapshot.clone(),
        )?;
        compacted.next_row_id = prepared.next_row_id;
        compacted.snapshot_leases = prepared.snapshot_leases.clone();
        self.write_generation(compacted, query, control)
    }

    fn prepare_change(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        data_changed: bool,
        schema_changed: bool,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        if !Arc::ptr_eq(self, &before.store) || operation.trim().is_empty() {
            return Err(DatasetStoreError::InvalidIdentity);
        }
        let mut metadata = before.metadata.clone();
        metadata.snapshot_id = Uuid::new_v4().to_string().into();
        metadata.data_revision = metadata
            .data_revision
            .checked_add(u64::from(data_changed))
            .ok_or(DatasetStoreError::InvalidValue)?;
        metadata.schema_revision = metadata
            .schema_revision
            .checked_add(u64::from(schema_changed))
            .ok_or(DatasetStoreError::InvalidValue)?;
        Ok(PreparedDataset {
            store: self.clone(),
            operation_id: operation.into(),
            expected_snapshot: Some(before.metadata.snapshot_id.clone()),
            metadata,
            files: before.files.clone(),
            next_row_id: before.next_row_id,
            retain_files: false,
            base_schema: before.base_schema.clone(),
            overlay: before.overlay.clone(),
            new_generation: false,
            directory: None,
            snapshot_leases: Box::new([before.clone()]),
            _generation_lease: None,
        })
    }

    pub fn prepare_cell_edit(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        edit: DatasetCellEdit<'_>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let DatasetCellEdit {
            row_id,
            column,
            value,
        } = edit;
        let field = user_column(before, column)?;
        if !before
            .query(engine, "dataset-edit")?
            .contains_rows(&[row_id], control)?
        {
            return Err(DatasetStoreError::RowNotFound);
        }
        let value = match value {
            Value::String(value) if value.is_empty() => Value::Null,
            value => value,
        };
        let value = yss_tabular_arrow::json_to_array(field, &[value])
            .map_err(|_| DatasetStoreError::InvalidValue)?;
        if value.get_array_memory_size() > control.max_input_bytes {
            return Err(yss_relational_contract::RelationError::MemoryLimitExceeded.into());
        }
        let column_id = yss_tabular_arrow::column_identity(field)
            .map_err(|_| DatasetStoreError::InvalidSchema)?;
        let mut prepared = self.prepare_change(before, operation, true, false)?;
        let mut patches = prepared.overlay.columns.to_vec();
        if let Some(patch) = patches
            .iter_mut()
            .find(|patch| patch.column_id.as_ref() == column_id)
        {
            if let Some(index) = patch.row_ids.iter().position(|id| *id == row_id) {
                let selection = (0..patch.row_ids.len())
                    .map(|row| if row == index { (1, 0) } else { (0, row) })
                    .collect::<Vec<_>>();
                patch.values = arrow::compute::interleave(
                    &[patch.values.as_ref(), value.as_ref()],
                    &selection,
                )?;
            } else {
                let mut ids = patch.row_ids.to_vec();
                ids.push(row_id);
                patch.row_ids = ids.into_boxed_slice();
                patch.values = arrow::compute::concat(&[patch.values.as_ref(), value.as_ref()])?;
            }
        } else {
            patches.push(DatasetColumnPatch {
                column_id: column_id.into(),
                row_ids: Box::new([row_id]),
                values: value,
            });
        }
        prepared.overlay.columns = patches.into_boxed_slice();
        self.finish_delta(prepared, engine, control)
    }

    pub fn prepare_add_row(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        index: usize,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        if index > before.metadata.row_count {
            return Err(DatasetStoreError::InvalidValue);
        }
        let query = before.query(engine, "dataset-edit")?;
        let rows = yss_tabular_arrow::dataset_row_columns(&before.metadata.schema)
            .map_err(|_| DatasetStoreError::InvalidSchema)?
            .ok_or(DatasetStoreError::InvalidSchema)?;
        let left = if index == 0 {
            None
        } else {
            Some(order_at(&query, &rows.display_order, index - 1, control)?)
        };
        let right = if index == before.metadata.row_count {
            None
        } else {
            Some(order_at(&query, &rows.display_order, index, control)?)
        };
        let order = order_between(left.as_deref(), right.as_deref())?;
        let mut prepared = self.prepare_change(before, operation, true, false)?;
        let id = prepared.next_row_id;
        prepared.next_row_id = id.checked_add(1).ok_or(DatasetStoreError::InvalidValue)?;
        prepared.metadata.row_count = prepared
            .metadata
            .row_count
            .checked_add(1)
            .ok_or(DatasetStoreError::InvalidValue)?;
        let mut arrays = before
            .metadata
            .schema
            .fields()
            .iter()
            .map(|field| new_null_array(field.data_type(), 1))
            .collect::<Vec<_>>();
        arrays[before
            .metadata
            .schema
            .index_of(&rows.row_id)
            .map_err(|_| DatasetStoreError::InvalidSchema)?] = Arc::new(Int64Array::from(vec![id]));
        arrays[before
            .metadata
            .schema
            .index_of(&rows.display_order)
            .map_err(|_| DatasetStoreError::InvalidSchema)?] =
            Arc::new(StringArray::from(vec![order]));
        let batch = RecordBatch::try_new(before.metadata.schema.clone(), arrays)?;
        let mut inserted = prepared.overlay.inserted.to_vec();
        inserted.push(batch);
        prepared.overlay.inserted = inserted.into_boxed_slice();
        self.finish_delta(prepared, engine, control)
    }

    pub fn prepare_delete_rows(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        row_ids: &[i64],
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let selected = row_ids.iter().copied().collect::<BTreeSet<_>>();
        if selected.is_empty() || selected.len() != row_ids.len() {
            return Err(DatasetStoreError::InvalidValue);
        }
        let query = before.query(engine, "dataset-edit")?;
        if !query.contains_rows(row_ids, control)? {
            return Err(DatasetStoreError::RowNotFound);
        }
        let mut prepared = self.prepare_change(before, operation, true, false)?;
        prepared.metadata.row_count = prepared
            .metadata
            .row_count
            .checked_sub(selected.len())
            .ok_or(DatasetStoreError::InvalidValue)?;
        let mut deleted = prepared
            .overlay
            .deleted
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        deleted.extend(selected);
        prepared.overlay.deleted = deleted.into_iter().collect();
        self.finish_delta(prepared, engine, control)
    }

    pub fn prepare_rename_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        from: &str,
        to: &str,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        user_column(before, from)?;
        if to.trim().is_empty() || (to != from && before.metadata.schema.index_of(to).is_ok()) {
            return Err(DatasetStoreError::InvalidValue);
        }
        let mut prepared = self.prepare_change(before, operation, false, true)?;
        prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
            before
                .metadata
                .schema
                .fields()
                .iter()
                .map(|field| {
                    if field.name() == from {
                        Arc::new(field.as_ref().clone().with_name(to))
                    } else {
                        field.clone()
                    }
                })
                .collect::<Vec<_>>(),
            before.metadata.schema.metadata().clone(),
        ));
        Ok(prepared)
    }

    pub fn prepare_add_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        name: &str,
        data_type: DataType,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        if name.trim().is_empty() || before.metadata.schema.index_of(name).is_ok() {
            return Err(DatasetStoreError::InvalidValue);
        }
        let mut fields = before.metadata.schema.fields().to_vec();
        let field = yss_tabular_arrow::with_column_metadata(
            Field::new(name, data_type, true),
            &Uuid::new_v4().to_string(),
            None,
        )
        .map_err(|_| DatasetStoreError::InvalidSchema)?;
        fields.push(Arc::new(field));
        let mut prepared = self.prepare_change(before, operation, false, true)?;
        prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
            fields,
            before.metadata.schema.metadata().clone(),
        ));
        Ok(prepared)
    }

    pub fn prepare_delete_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        name: &str,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let field = user_column(before, name)?;
        if before.metadata.schema.fields().len() <= 3 {
            return Err(DatasetStoreError::InvalidValue);
        }
        let id = yss_tabular_arrow::column_identity(field)
            .map_err(|_| DatasetStoreError::InvalidSchema)?;
        let mut prepared = self.prepare_change(before, operation, false, true)?;
        prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
            before
                .metadata
                .schema
                .fields()
                .iter()
                .filter(|field| field.name() != name)
                .cloned()
                .collect::<Vec<_>>(),
            before.metadata.schema.metadata().clone(),
        ));
        prepared.overlay.columns = prepared
            .overlay
            .columns
            .iter()
            .filter(|patch| patch.column_id.as_ref() != id)
            .cloned()
            .collect();
        Ok(prepared)
    }

    pub fn prepare_rename(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        name: &str,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        if name.trim().is_empty() {
            return Err(DatasetStoreError::InvalidValue);
        }
        let mut prepared = self.prepare_change(before, operation, false, false)?;
        prepared.metadata.name = name.into();
        Ok(prepared)
    }

    pub fn prepare_delete(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let mut prepared = self.prepare_change(before, operation, true, false)?;
        prepared.metadata.deleted = true;
        Ok(prepared)
    }

    /// Undo/redo publishes the selected immutable content as a new head. Revisions and row ID
    /// allocation never move backwards; old queries retain their original snapshots.
    pub fn prepare_restore(
        self: &Arc<Self>,
        current: &Arc<DatasetSnapshot>,
        target: &Arc<DatasetSnapshot>,
        operation: &str,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        if !Arc::ptr_eq(self, &target.store) || current.metadata.id != target.metadata.id {
            return Err(DatasetStoreError::InvalidIdentity);
        }
        let mut prepared = self.prepare_change(
            current,
            operation,
            true,
            current.metadata.schema != target.metadata.schema,
        )?;
        prepared.metadata.schema = target.metadata.schema.clone();
        prepared.metadata.name = target.metadata.name.clone();
        prepared.metadata.deleted = target.metadata.deleted;
        prepared.metadata.generation_id = target.metadata.generation_id.clone();
        prepared.metadata.row_count = target.metadata.row_count;
        prepared.files = target.files.clone();
        prepared.base_schema = target.base_schema.clone();
        prepared.overlay = target.overlay.clone();
        prepared.snapshot_leases = Box::new([current.clone(), target.clone()]);
        Ok(prepared)
    }
}

fn order_at(
    query: &DatasetQuery,
    column: &str,
    index: usize,
    control: &RelationControl,
) -> Result<String, DatasetStoreError> {
    let page = query.page(index, 1, control)?;
    let batch = page.batches.first().ok_or(DatasetStoreError::RowNotFound)?;
    let array = batch
        .column_by_name(column)
        .and_then(|array| array.as_any().downcast_ref::<StringArray>())
        .ok_or(DatasetStoreError::InvalidSchema)?;
    if array.is_empty() || array.is_null(0) {
        return Err(DatasetStoreError::InvalidSchema);
    }
    Ok(array.value(0).to_owned())
}

fn order_between(left: Option<&str>, right: Option<&str>) -> Result<String, DatasetStoreError> {
    if left.zip(right).is_some_and(|(left, right)| left >= right) {
        return Err(DatasetStoreError::InvalidValue);
    }
    let mut right = right.map(str::as_bytes);
    let left = left.map(str::as_bytes);
    let mut output = Vec::new();
    for index in 0..4096 {
        let low = left
            .and_then(|value| value.get(index))
            .copied()
            .unwrap_or(b' ');
        let high = right
            .and_then(|value| value.get(index))
            .copied()
            .unwrap_or(127);
        if high > low + 1 {
            output.push(low + (high - low) / 2);
            return String::from_utf8(output).map_err(|_| DatasetStoreError::InvalidValue);
        }
        output.push(low);
        if low < high {
            right = None;
        }
    }
    Err(DatasetStoreError::DeltaLimit)
}
