use super::input::GroupInput;
use crate::{
    DataFusionRuntime,
    composition::{derived_field, meaning, merge_bindings},
    relation::controlled,
    series_transform::col,
    spill::RelationSpill,
};
use arrow::{
    array::{ArrayRef, UInt64Array},
    datatypes::{DataType, Field, Schema, SchemaRef},
    record_batch::RecordBatch,
};
use futures_util::StreamExt;
use std::sync::Arc;
use yss_relational_contract::{
    GroupMapMode, GroupedRelationHandle, RelationBinding, RelationControl, RelationError,
    RelationHandle,
};

pub(super) struct GroupOutput {
    mode: GroupMapMode,
    key_fields: Vec<Field>,
    bindings: Vec<RelationBinding>,
    layout: Option<OutputLayout>,
    writer: Option<RelationSpill>,
    rows: u64,
}

struct OutputLayout {
    returned: SchemaRef,
    visible: SchemaRef,
    stored: SchemaRef,
    position: String,
}

fn result_fields(schema: &Schema) -> Vec<Field> {
    schema
        .fields()
        .iter()
        .map(|f| derived_field(f, f.name()).with_nullable(true))
        .collect()
}

impl GroupOutput {
    pub fn new(source: &GroupedRelationHandle, mode: GroupMapMode) -> Result<Self, RelationError> {
        let key_fields = if let GroupMapMode::Apply { key_prefix } = &mode {
            source
                .keys()
                .iter()
                .map(|key| {
                    let schema = source.source().schema();
                    let field = schema
                        .field_with_name(key)
                        .map_err(|_| RelationError::InvalidInput)?;
                    Ok(derived_field(field, &format!("{key_prefix}{key}")).with_nullable(true))
                })
                .collect::<Result<_, RelationError>>()?
        } else {
            Vec::new()
        };
        Ok(Self {
            mode,
            key_fields,
            bindings: source.source().bindings().to_vec(),
            layout: None,
            writer: None,
            rows: 0,
        })
    }

    fn prepare(
        &mut self,
        engine: &DataFusionRuntime,
        result: &RelationHandle,
        control: &RelationControl,
    ) -> Result<(), RelationError> {
        control.check()?;
        let schema = result.schema();
        if let Some(layout) = &self.layout {
            if schema.fields().len() != layout.returned.fields().len() {
                return Err(RelationError::GroupSchemaMismatch);
            }
            for (actual, expected) in schema.fields().iter().zip(layout.returned.fields()) {
                if actual.name() != expected.name()
                    || actual.data_type() != expected.data_type()
                    || meaning(actual)? != meaning(expected)?
                {
                    return Err(RelationError::GroupSchemaMismatch);
                }
            }
        } else {
            if self
                .key_fields
                .iter()
                .any(|field| schema.index_of(field.name()).is_ok())
            {
                return Err(RelationError::GroupKeyCollision);
            }
            // Column identity belongs to the result, not to whichever group happened to be first.
            let returned = Arc::new(Schema::new(result_fields(&schema)));
            let visible = Arc::new(Schema::new(
                self.key_fields
                    .iter()
                    .cloned()
                    .chain(result_fields(&schema))
                    .collect::<Vec<_>>(),
            ));
            let mut position = "__yssbi_group_position".to_owned();
            while visible.index_of(&position).is_ok() {
                position.push('_');
            }
            let mut fields = visible.fields().to_vec();
            fields.push(Arc::new(Field::new(&position, DataType::UInt64, false)));
            let stored = Arc::new(Schema::new(fields));
            self.writer = Some(RelationSpill::new(engine, stored.clone(), control)?);
            self.layout = Some(OutputLayout {
                returned,
                visible,
                stored,
                position,
            });
        }
        merge_bindings(&mut self.bindings, result.bindings())
    }

    fn write(
        &mut self,
        batch: &RecordBatch,
        keys: &[datafusion::common::ScalarValue],
        positions: ArrayRef,
        control: &RelationControl,
    ) -> Result<(), RelationError> {
        let layout = self.layout.as_ref().ok_or(RelationError::InvalidPlan)?;
        let mut columns = if matches!(self.mode, GroupMapMode::Apply { .. }) {
            keys.iter()
                .map(|key| {
                    key.to_array_of_size(batch.num_rows())
                        .map_err(|_| RelationError::InvalidInput)
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        columns.extend_from_slice(batch.columns());
        columns.push(positions);
        let stored = RecordBatch::try_new(layout.stored.clone(), columns)
            .map_err(|_| RelationError::InvalidPlan)?;
        self.writer
            .as_mut()
            .ok_or(RelationError::InvalidPlan)?
            .write(&stored, control)?;
        self.rows = self
            .rows
            .checked_add(batch.num_rows() as u64)
            .ok_or(RelationError::MemoryLimitExceeded)?;
        Ok(())
    }

    pub async fn append(
        &mut self,
        engine: &DataFusionRuntime,
        group: &GroupInput,
        result: &RelationHandle,
        control: &RelationControl,
    ) -> Result<(), RelationError> {
        let result = result.resolve(control.clone()).await?;
        if self.mode == GroupMapMode::Transform && !group.value.same_rows_and_order(&result) {
            return Err(RelationError::UnalignedSeries);
        }
        self.prepare(engine, &result, control)?;
        let mut stream = result.stream(control.clone()).await?;
        let mut positions = if self.mode == GroupMapMode::Transform {
            Some(group.positions.stream(control.clone()).await?)
        } else {
            None
        };
        let mut pending: Option<RecordBatch> = None;
        let mut position_offset = 0;
        while let Some(batch) = controlled(stream.next(), control).await? {
            let batch = batch?;
            if batch.get_array_memory_size() > control.max_input_bytes {
                return Err(RelationError::MemoryLimitExceeded);
            }
            if let Some(positions) = &mut positions {
                let mut offset = 0;
                while offset < batch.num_rows() {
                    while pending
                        .as_ref()
                        .is_none_or(|b| position_offset == b.num_rows())
                    {
                        pending = Some(
                            controlled(positions.next(), control)
                                .await?
                                .ok_or(RelationError::ShapeMismatch)??,
                        );
                        position_offset = 0;
                    }
                    let positions = pending.as_ref().ok_or(RelationError::InvalidPlan)?;
                    let count =
                        (batch.num_rows() - offset).min(positions.num_rows() - position_offset);
                    self.write(
                        &batch.slice(offset, count),
                        &[],
                        positions.column(0).slice(position_offset, count),
                        control,
                    )?;
                    offset += count;
                    position_offset += count;
                }
            } else {
                let end = self
                    .rows
                    .checked_add(batch.num_rows() as u64)
                    .ok_or(RelationError::MemoryLimitExceeded)?;
                self.write(
                    &batch,
                    &group.keys,
                    Arc::new(UInt64Array::from_iter_values(self.rows..end)),
                    control,
                )?;
            }
        }
        if let Some(mut positions) = positions {
            if pending
                .as_ref()
                .is_some_and(|b| position_offset != b.num_rows())
            {
                return Err(RelationError::ShapeMismatch);
            }
            while let Some(batch) = controlled(positions.next(), control).await? {
                if batch?.num_rows() != 0 {
                    return Err(RelationError::ShapeMismatch);
                }
            }
        }
        control.check()
    }

    pub fn prepare_empty(
        &mut self,
        engine: &DataFusionRuntime,
        source: &RelationHandle,
        result: &RelationHandle,
        control: &RelationControl,
    ) -> Result<(), RelationError> {
        let result = engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(result.resolve(control.clone()))?;
        if self.mode == GroupMapMode::Transform && !source.same_rows_and_order(&result) {
            return Err(RelationError::UnalignedSeries);
        }
        self.prepare(engine, &result, control)
    }

    pub fn finish(
        self,
        engine: &Arc<DataFusionRuntime>,
        source: &RelationHandle,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        let layout = self.layout.ok_or(RelationError::InvalidInput)?;
        let mut result = self.writer.ok_or(RelationError::InvalidInput)?.finish(
            engine,
            layout.visible,
            self.bindings.into(),
            vec![col(&layout.position).sort(true, false)],
            control,
        )?;
        if self.mode == GroupMapMode::Transform {
            result.row_identity = source
                .row_identity()
                .cloned()
                .ok_or(RelationError::UnalignedSeries)?;
        }
        result.into_handle()
    }
}
