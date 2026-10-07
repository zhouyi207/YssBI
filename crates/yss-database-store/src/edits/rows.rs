use super::*;
use arrow::array::{Array, Int64Array, StringArray};
use arrow::record_batch::RecordBatch;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use yss_relational_contract::{DatasetColumnPatch, RelationError};

pub struct DatasetCellEdit<'a> {
    pub row_id: i64,
    pub column: &'a str,
    pub value: Value,
}

pub enum DatasetInsertPosition {
    Index(usize),
    BeforeRow(i64),
    End,
}

pub struct DatasetRowInsertion<'a> {
    pub position: DatasetInsertPosition,
    pub rows: &'a [BTreeMap<String, Value>],
}

impl DatasetStore {
    pub fn prepare_cell_edit(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        edit: DatasetCellEdit<'_>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        self.prepare_cell_edits(before, engine, operation, vec![edit], control)
    }

    pub fn prepare_cell_edits(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        edits: Vec<DatasetCellEdit<'_>>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        control.check()?;
        if edits.is_empty() {
            return Err(DatasetStoreError::InvalidValue);
        }
        let mut columns = BTreeMap::<&str, BTreeMap<i64, Value>>::new();
        let mut row_ids = BTreeSet::new();
        for edit in edits {
            user_column(before, edit.column)?;
            if columns
                .entry(edit.column)
                .or_default()
                .insert(edit.row_id, edit.value)
                .is_some()
            {
                return Err(DatasetStoreError::InvalidValue);
            }
            row_ids.insert(edit.row_id);
        }
        if !before
            .query(engine, "dataset-edit")?
            .contains_rows(&row_ids.into_iter().collect::<Vec<_>>(), control)?
        {
            return Err(DatasetStoreError::RowNotFound);
        }
        let mut prepared = self.prepare_change(before, operation, true, false)?;
        let mut patches = prepared.overlay.columns.to_vec();
        let mut bytes = 0usize;
        for (name, edits) in columns {
            control.check()?;
            let field = user_column(before, name)?;
            let (ids, values): (Vec<_>, Vec<_>) = edits.into_iter().unzip();
            let values = yss_database_arrow::json_to_array(field, &values)
                .map_err(|_| DatasetStoreError::InvalidValue)?;
            bytes = bytes
                .checked_add(values.get_array_memory_size())
                .ok_or(RelationError::MemoryLimitExceeded)?;
            if bytes > control.max_input_bytes {
                return Err(RelationError::MemoryLimitExceeded.into());
            }
            let column_id = yss_database_arrow::column_identity(field)
                .map_err(|_| DatasetStoreError::InvalidSchema)?;
            if let Some(patch) = patches
                .iter_mut()
                .find(|patch| patch.column_id.as_ref() == column_id)
            {
                merge_patch(patch, ids, values)?;
            } else {
                patches.push(DatasetColumnPatch {
                    column_id: column_id.into(),
                    row_ids: ids.into_boxed_slice(),
                    values,
                });
            }
        }
        prepared.overlay.columns = patches.into_boxed_slice();
        control.check()?;
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
        self.prepare_insert_rows(
            before,
            engine,
            operation,
            DatasetRowInsertion {
                position: DatasetInsertPosition::Index(index),
                rows: &[BTreeMap::new()],
            },
            control,
        )
        .map(|(prepared, _)| prepared)
    }

    pub fn prepare_insert_rows(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        insertion: DatasetRowInsertion<'_>,
        control: &RelationControl,
    ) -> Result<(PreparedDataset, Vec<i64>), DatasetStoreError> {
        control.check()?;
        let count = insertion.rows.len();
        if count == 0 {
            return Err(DatasetStoreError::InvalidValue);
        }
        if count > control.max_input_bytes / 16 {
            return Err(RelationError::MemoryLimitExceeded.into());
        }
        for row in insertion.rows {
            for name in row.keys() {
                user_column(before, name)?;
            }
        }
        let roles = yss_database_arrow::dataset_row_columns(&before.metadata.schema)
            .map_err(|_| DatasetStoreError::InvalidSchema)?
            .ok_or(DatasetStoreError::InvalidSchema)?;
        let query = before.query(engine, "dataset-insert")?;
        let (left, right) = match insertion.position {
            DatasetInsertPosition::Index(index) => {
                if index > before.metadata.row_count {
                    return Err(DatasetStoreError::InvalidValue);
                }
                let left = if index == 0 {
                    None
                } else {
                    Some(order_at(&query, &roles.display_order, index - 1, control)?)
                };
                let right = if index == before.metadata.row_count {
                    None
                } else {
                    Some(order_at(&query, &roles.display_order, index, control)?)
                };
                (left, right)
            }
            position => {
                let anchor = match position {
                    DatasetInsertPosition::BeforeRow(id) => Some(id),
                    _ => None,
                };
                let neighbors = query
                    .insertion_neighbors(anchor, control)?
                    .ok_or(DatasetStoreError::RowNotFound)?;
                (neighbors.left, neighbors.right)
            }
        };
        let mut prepared = self.prepare_change(before, operation, true, false)?;
        let end_id = prepared
            .next_row_id
            .checked_add(i64::try_from(count).map_err(|_| DatasetStoreError::InvalidValue)?)
            .ok_or(DatasetStoreError::InvalidValue)?;
        let ids = (prepared.next_row_id..end_id).collect::<Vec<_>>();
        prepared.next_row_id = end_id;
        prepared.metadata.row_count = prepared
            .metadata
            .row_count
            .checked_add(count)
            .ok_or(DatasetStoreError::InvalidValue)?;
        let mut orders = vec![String::new(); count];
        fill_orders(&mut orders, left.as_deref(), right.as_deref())?;
        let mut bytes = 0usize;
        let mut arrays = Vec::new();
        for field in before.metadata.schema.fields() {
            control.check()?;
            let values: arrow::array::ArrayRef = if field.name() == &roles.row_id {
                Arc::new(Int64Array::from(ids.clone()))
            } else if field.name() == &roles.display_order {
                Arc::new(StringArray::from_iter_values(orders.iter()))
            } else {
                let values = insertion
                    .rows
                    .iter()
                    .map(|row| row.get(field.name()).cloned().unwrap_or(Value::Null))
                    .collect::<Vec<_>>();
                yss_database_arrow::json_to_array(field, &values)
                    .map_err(|_| DatasetStoreError::InvalidValue)?
            };
            bytes = bytes
                .checked_add(values.get_array_memory_size())
                .ok_or(RelationError::MemoryLimitExceeded)?;
            if bytes > control.max_input_bytes {
                return Err(RelationError::MemoryLimitExceeded.into());
            }
            arrays.push(values);
        }
        let batch = RecordBatch::try_new(before.metadata.schema.clone(), arrays)?;
        let mut inserted = prepared.overlay.inserted.to_vec();
        inserted.push(batch);
        prepared.overlay.inserted = inserted.into_boxed_slice();
        control.check()?;
        Ok((self.finish_delta(prepared, engine, control)?, ids))
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
        if !before
            .query(engine, "dataset-edit")?
            .contains_rows(row_ids, control)?
        {
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
}

fn merge_patch(
    patch: &mut DatasetColumnPatch,
    ids: Vec<i64>,
    values: arrow::array::ArrayRef,
) -> Result<(), DatasetStoreError> {
    let mut remaining = ids
        .into_iter()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect::<BTreeMap<_, _>>();
    let mut selection = patch
        .row_ids
        .iter()
        .enumerate()
        .map(|(index, id)| remaining.remove(id).map_or((0, index), |index| (1, index)))
        .collect::<Vec<_>>();
    let mut row_ids = patch.row_ids.to_vec();
    for (id, index) in remaining {
        row_ids.push(id);
        selection.push((1, index));
    }
    patch.values =
        arrow::compute::interleave(&[patch.values.as_ref(), values.as_ref()], &selection)?;
    patch.row_ids = row_ids.into_boxed_slice();
    Ok(())
}

fn fill_orders(
    output: &mut [String],
    left: Option<&str>,
    right: Option<&str>,
) -> Result<(), DatasetStoreError> {
    if output.is_empty() {
        return Ok(());
    }
    // Divide the gap evenly so batch size grows order-key depth logarithmically.
    let middle = output.len() / 2;
    let order = order_between(left, right)?;
    fill_orders(&mut output[..middle], left, Some(&order))?;
    fill_orders(&mut output[middle + 1..], Some(&order), right)?;
    output[middle] = order;
    Ok(())
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
