use std::sync::Arc;

use arrow::datatypes::{Field, Schema};
use uuid::Uuid;
use yss_database_engine::{DataFusionRuntime, DatasetQuery};
use yss_relational_contract::{DatasetRelationInput, RelationBinding, RelationControl};

use crate::{DatasetSnapshot, DatasetStore, DatasetStoreError, PreparedDataset};

mod columns;
pub use columns::DatasetColumnCast;
mod rows;
pub use rows::{DatasetCellEdit, DatasetInsertPosition, DatasetRowInsertion};

fn user_column<'a>(
    snapshot: &'a DatasetSnapshot,
    name: &str,
) -> Result<&'a Field, DatasetStoreError> {
    let rows = yss_database_arrow::dataset_row_columns(&snapshot.metadata.schema)
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
            for (field, array) in prepared
                .metadata
                .schema
                .fields()
                .iter()
                .zip(batch.columns())
            {
                if yss_database_arrow::validate_semantic_array(field, array.as_ref()).is_err() {
                    write_error = Some(DatasetStoreError::InvalidValue);
                    return Err(yss_relational_contract::RelationError::InvalidInput);
                }
            }
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
        mut prepared: PreparedDataset,
        engine: &Arc<DataFusionRuntime>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        match prepared.encode_overlay() {
            Ok(encoded) => {
                prepared.encoded_overlay = Some(encoded);
                return Ok(prepared);
            }
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
            encoded_overlay: None,
            new_generation: false,
            directory: None,
            snapshot_leases: Box::new([before.clone()]),
            _generation_lease: None,
        })
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
