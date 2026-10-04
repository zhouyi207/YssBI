//! Materialize explicit result boundaries through the runtime's spill storage.
use std::sync::Arc;

use arrow::array::UInt64Array;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use datafusion::common::Column;
use datafusion::logical_expr::Expr;
use futures_util::StreamExt;
use yss_relational_contract::{RelationControl, RelationError, RelationHandle};

use crate::DataFusionRuntime;
use crate::relation::controlled;
use crate::spill::RelationSpill;

impl DataFusionRuntime {
    pub(crate) fn snapshot_relation(
        self: &Arc<Self>,
        source: &RelationHandle,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        control.check()?;
        self.runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(async {
                let source = source.resolve(control.clone()).await?;
                let schema = source.schema();
                let mut row_position = "__yssbi_result_position".to_owned();
                while schema.index_of(&row_position).is_ok() {
                    row_position.push('_');
                }
                let mut fields = schema.fields().to_vec();
                fields.push(Arc::new(Field::new(&row_position, DataType::UInt64, false)));
                let stored_schema =
                    Arc::new(Schema::new_with_metadata(fields, schema.metadata().clone()));
                let mut writer = RelationSpill::new(self, stored_schema.clone(), control)?;
                let mut stream = source.stream(control.clone()).await?;
                let mut rows = 0u64;
                while let Some(batch) = controlled(stream.next(), control).await? {
                    let batch = batch?;
                    let bytes = batch
                        .num_rows()
                        .checked_mul(8)
                        .and_then(|positions| positions.checked_add(batch.get_array_memory_size()))
                        .and_then(|bytes| bytes.checked_mul(2))
                        .ok_or(RelationError::MemoryLimitExceeded)?;
                    if bytes > control.max_input_bytes {
                        return Err(RelationError::MemoryLimitExceeded);
                    }
                    let end = rows
                        .checked_add(batch.num_rows() as u64)
                        .ok_or(RelationError::MemoryLimitExceeded)?;
                    let mut columns = batch.columns().to_vec();
                    columns.push(Arc::new(UInt64Array::from_iter_values(rows..end)));
                    let stored = RecordBatch::try_new(stored_schema.clone(), columns)
                        .map_err(|_| RelationError::InvalidPlan)?;
                    writer.write(&stored, control)?;
                    rows = end;
                    control.check()?;
                }
                let order = vec![Expr::Column(Column::from_name(row_position)).sort(true, false)];
                // The file lease is independent of the original dataset. Bindings remain
                // provenance, while reads use only the fully evaluated immutable file.
                let mut relation =
                    writer.finish(self, schema, Arc::from(source.bindings()), order, control)?;
                relation.row_identity = source.row_identity().cloned().unwrap_or_default();
                relation.into_handle()
            })
    }
}
