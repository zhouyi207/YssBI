use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use arrow::array::Int64Array;
use arrow::record_batch::RecordBatch;
use yss_database_contract::{DatabaseDecl, DatabaseExportFormat};
use yss_database_edit::EditState;
use yss_database_schema::DatabaseSchemaFact;
use yss_dataset_store::{
    DatasetCellEdit, DatasetColumnCast, DatasetPublication, DatasetSnapshot, DatasetStoreError,
    PreparedDataset,
};
use yss_relational_contract::{RelationBinding, RelationControl, RelationError, RelationHandle};

use crate::error::DatabaseExportError;
use crate::session_api::DatabaseMutationOperation;
use crate::{DatabaseState, DatasetEdit};

pub const MAX_GET_DATAFRAME_ROWS: usize = 10_000;

pub(crate) fn query_control(bytes: usize) -> RelationControl {
    RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(30),
        max_input_bytes: bytes,
    }
}

#[derive(Clone)]
pub struct DatabaseInstance {
    pub decl: DatabaseDecl,
    pub state: DatabaseState,
}

impl DatabaseInstance {
    pub fn snapshot(&self) -> Result<&Arc<DatasetSnapshot>, DatasetStoreError> {
        match &self.state {
            DatabaseState::Dataset { snapshot, .. } if !snapshot.metadata().deleted => Ok(snapshot),
            DatabaseState::Dataset { .. } => Err(DatasetStoreError::NotFound),
            DatabaseState::Failed { .. } => Err(DatasetStoreError::NotFound),
        }
    }
    pub(crate) fn engine(
        &self,
    ) -> Result<&Arc<yss_datafusion::DataFusionRuntime>, DatasetStoreError> {
        match &self.state {
            DatabaseState::Dataset { engine, .. } => Ok(engine),
            DatabaseState::Failed { .. } => Err(DatasetStoreError::NotFound),
        }
    }
    pub fn data_schema(&self) -> Result<DatabaseSchemaFact, DatasetStoreError> {
        yss_tabular_arrow::database_schema_fact(&self.decl.id, &self.snapshot()?.metadata().schema)
            .map_err(|_| DatasetStoreError::InvalidSchema)
    }
    pub fn row_count(&self) -> Result<usize, DatasetStoreError> {
        Ok(self.snapshot()?.metadata().row_count)
    }
    pub(crate) fn query(&self) -> Result<yss_datafusion::DatasetQuery, DatasetStoreError> {
        self.snapshot()?.query(self.engine()?, "database-read")
    }
    pub fn relation(
        &self,
        session: &str,
        revision: u64,
    ) -> Result<RelationHandle, DatasetStoreError> {
        let snapshot = self.snapshot()?;
        let binding = RelationBinding {
            project_session: session.into(),
            dataset: self.decl.id.clone(),
            snapshot: snapshot.metadata().snapshot_id.clone(),
            revision,
        };
        Ok(self
            .engine()?
            .dataset_query(binding, snapshot.relation_input(), snapshot.clone())?
            .relation()?)
    }
    pub fn read_arrow_columns(
        &self,
        names: &[&str],
        offset: usize,
        limit: usize,
        control: &RelationControl,
    ) -> Result<Vec<RecordBatch>, DatasetStoreError> {
        let relation = self
            .query()?
            .relation()?
            .project(&names.iter().map(|name| (*name).into()).collect::<Vec<_>>())?;
        let count = limit.min(self.row_count()?.saturating_sub(offset));
        let relation = relation.limit(offset, count)?;
        let mut batches = Vec::new();
        let mut bytes = 0usize;
        self.engine()?
            .visit_relation(&relation, control, &mut |batch| {
                bytes = bytes
                    .checked_add(batch.get_array_memory_size())
                    .ok_or(RelationError::MemoryLimitExceeded)?;
                if bytes > control.max_input_bytes {
                    return Err(RelationError::MemoryLimitExceeded);
                }
                batches.push(batch);
                Ok(())
            })?;
        Ok(batches)
    }
    pub fn export_to_path(
        &self,
        path: &Path,
        format: DatabaseExportFormat,
    ) -> Result<(), DatabaseExportError> {
        let relation = self.query()?.relation()?;
        let engine = self.engine()?;
        let control = query_control(128 * 1024 * 1024);
        let file = std::fs::File::create(path)?;
        match format {
            DatabaseExportFormat::Csv => {
                let mut writer = arrow::csv::Writer::new(file);
                writer.write(&RecordBatch::new_empty(relation.schema()))?;
                let mut failure = None;
                let result = engine.visit_relation(&relation, &control, &mut |batch| {
                    writer.write(&batch).map_err(|error| {
                        failure = Some(error);
                        RelationError::QueryFailed
                    })
                });
                if let Some(error) = failure {
                    return Err(error.into());
                }
                result?;
                writer.into_inner().sync_all()?;
            }
            DatabaseExportFormat::Parquet => {
                let mut writer = yss_tabular_io::ParquetBatchWriter::new(file, relation.schema())?;
                let mut failure = None;
                let result = engine.visit_relation(&relation, &control, &mut |batch| {
                    writer.write(&batch).map_err(|error| {
                        failure = Some(error);
                        RelationError::QueryFailed
                    })
                });
                if let Some(error) = failure {
                    return Err(error.into());
                }
                result?;
                writer.finish()?;
            }
        }
        Ok(())
    }
    pub fn edit_state(&self) -> EditState {
        match &self.state {
            DatabaseState::Dataset { history, .. } => history.state(),
            DatabaseState::Failed { .. } => EditState {
                can_undo: false,
                can_redo: false,
                is_modified: false,
                undo_count: 0,
                redo_count: 0,
            },
        }
    }
    pub(crate) fn prepare_mutation(
        &self,
        operation: &DatabaseMutationOperation,
        operation_id: &str,
    ) -> Result<PreparedInstanceMutation, DatasetStoreError> {
        let DatabaseState::Dataset {
            snapshot,
            engine,
            history,
        } = &self.state
        else {
            return Err(DatasetStoreError::NotFound);
        };
        let store = snapshot.store();
        let control = query_control(128 * 1024 * 1024);
        let mut next_history = history.clone();
        let mut push_history = false;
        let prepared = match operation {
            DatabaseMutationOperation::DeleteDatabase => {
                store.prepare_delete(snapshot, operation_id)?
            }
            DatabaseMutationOperation::Undo => {
                let edit = next_history
                    .pop_undo()
                    .ok_or(DatasetStoreError::InvalidValue)?;
                let mut prepared = store.prepare_restore(snapshot, &edit.before, operation_id)?;
                prepared.preserve_display_name(snapshot)?;
                next_history.push_redo(edit);
                prepared
            }
            DatabaseMutationOperation::Redo => {
                let edit = next_history
                    .pop_redo()
                    .ok_or(DatasetStoreError::InvalidValue)?;
                let mut prepared = store.prepare_restore(snapshot, &edit.after, operation_id)?;
                prepared.preserve_display_name(snapshot)?;
                next_history.push_undo(edit);
                prepared
            }
            DatabaseMutationOperation::RenameDatabase { name } => {
                store.prepare_rename(snapshot, operation_id, name)?
            }
            DatabaseMutationOperation::Save => {
                next_history.clear();
                store.prepare_rename(snapshot, operation_id, &snapshot.metadata().name)?
            }
            operation => {
                push_history = true;
                match operation {
                    DatabaseMutationOperation::EditCell {
                        row,
                        column,
                        value,
                        row_id,
                    } => {
                        let row_id = match row_id {
                            Some(id) => *id,
                            None => self.row_id_at(*row, &control)?,
                        };
                        let value = serde_json::to_value(value)
                            .map_err(|_| DatasetStoreError::InvalidValue)?;
                        store.prepare_cell_edit(
                            snapshot,
                            engine,
                            operation_id,
                            DatasetCellEdit {
                                row_id,
                                column,
                                value,
                            },
                            &control,
                        )?
                    }
                    DatabaseMutationOperation::AddRow { index } => store.prepare_add_row(
                        snapshot,
                        engine,
                        operation_id,
                        if *index == usize::MAX {
                            snapshot.metadata().row_count
                        } else {
                            *index
                        },
                        &control,
                    )?,
                    DatabaseMutationOperation::DeleteRows { indices, row_ids } => {
                        let ids = match row_ids {
                            Some(ids) if ids.len() == indices.len() => ids.to_vec(),
                            Some(_) => return Err(DatasetStoreError::InvalidValue),
                            None => indices
                                .iter()
                                .map(|index| self.row_id_at(*index, &control))
                                .collect::<Result<Vec<_>, _>>()?,
                        };
                        store.prepare_delete_rows(snapshot, engine, operation_id, &ids, &control)?
                    }
                    DatabaseMutationOperation::AddColumn { name, data_type } => {
                        store.prepare_add_column(snapshot, operation_id, name, data_type.clone())?
                    }
                    DatabaseMutationOperation::DeleteColumn { name } => {
                        store.prepare_delete_column(snapshot, operation_id, name)?
                    }
                    DatabaseMutationOperation::RenameColumn { old_name, new_name } => {
                        store.prepare_rename_column(snapshot, operation_id, old_name, new_name)?
                    }
                    DatabaseMutationOperation::CastColumn {
                        name,
                        data_type,
                        force,
                    } => store.prepare_cast_column(
                        snapshot,
                        engine,
                        operation_id,
                        DatasetColumnCast {
                            column: name,
                            data_type: data_type.clone(),
                            force: *force,
                        },
                        &control,
                    )?,
                    _ => return Err(DatasetStoreError::InvalidValue),
                }
            }
        };
        Ok(PreparedInstanceMutation {
            before: self.clone(),
            prepared,
            history: next_history,
            push_history,
        })
    }
    fn row_id_at(&self, index: usize, control: &RelationControl) -> Result<i64, DatasetStoreError> {
        let page = self.query()?.page(index, 1, control)?;
        let batch = page.batches.first().ok_or(DatasetStoreError::RowNotFound)?;
        let rows = yss_tabular_arrow::dataset_row_columns(&batch.schema())
            .map_err(|_| DatasetStoreError::InvalidSchema)?
            .ok_or(DatasetStoreError::InvalidSchema)?;
        let array = batch
            .column_by_name(&rows.row_id)
            .and_then(|array| array.as_any().downcast_ref::<Int64Array>())
            .ok_or(DatasetStoreError::InvalidSchema)?;
        Ok(array.value(0))
    }
}

pub(crate) struct PreparedInstanceMutation {
    before: DatabaseInstance,
    prepared: PreparedDataset,
    history: yss_database_edit::EditHistory<DatasetEdit>,
    push_history: bool,
}
impl PreparedInstanceMutation {
    pub fn schema_changed(&self) -> Result<bool, DatasetStoreError> {
        Ok(self.prepared.metadata().schema != self.before.snapshot()?.metadata().schema)
    }
    pub fn recovery(
        &self,
    ) -> Result<(Arc<yss_dataset_store::DatasetStore>, DatasetPublication), DatasetStoreError> {
        Ok((
            self.before.snapshot()?.store().clone(),
            self.prepared.publication(),
        ))
    }
    pub fn edit_state(&self) -> EditState {
        let mut state = self.history.state();
        if self.push_history {
            state.undo_count += 1;
            state.redo_count = 0;
            state.can_undo = true;
            state.can_redo = false;
            state.is_modified = true;
        }
        state
    }
    pub fn commit(mut self) -> Result<(DatabaseInstance, DatasetPublication), DatasetStoreError> {
        let snapshot = self.before.snapshot()?.clone();
        let engine = self.before.engine()?.clone();
        let committed = snapshot.store().commit(self.prepared)?;
        if self.push_history {
            self.history.push(DatasetEdit {
                before: snapshot,
                after: committed.snapshot.clone(),
            });
        }
        let mut decl = self.before.decl;
        decl.name = committed.snapshot.metadata().name.clone();
        let instance = DatabaseInstance {
            decl,
            state: DatabaseState::Dataset {
                snapshot: committed.snapshot,
                engine,
                history: self.history,
            },
        };
        Ok((instance, committed.publication))
    }
}
