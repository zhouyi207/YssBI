use crate::{
    DataFusionRuntime,
    relation::{DataFusionRelation, controlled},
    series_transform::col,
    spill::RelationSpill,
};
use arrow::{
    datatypes::{Schema, SchemaRef},
    record_batch::RecordBatch,
};
use datafusion::common::ScalarValue;
use futures_util::StreamExt;
use std::sync::Arc;
use yss_relational_contract::{
    GroupedRelationHandle, RelationBatchStream, RelationBinding, RelationControl, RelationError,
    RelationHandle,
};

pub(super) struct GroupInput {
    pub value: RelationHandle,
    pub positions: RelationHandle,
    pub keys: Vec<ScalarValue>,
}

pub(super) struct GroupReader {
    stream: RelationBatchStream,
    batch: Option<RecordBatch>,
    offset: usize,
    exhausted: bool,
    schema: SchemaRef,
    bindings: Arc<[RelationBinding]>,
    columns: Vec<Box<str>>,
    keys: Vec<usize>,
    position: String,
}

impl GroupReader {
    pub async fn new(
        groups: &GroupedRelationHandle,
        control: &RelationControl,
    ) -> Result<Self, RelationError> {
        let source = groups.source().resolve(control.clone()).await?;
        let adapter = source
            .plan()
            .as_any()
            .downcast_ref::<DataFusionRelation>()
            .ok_or(RelationError::InvalidInput)?;
        let (frame, position) = adapter.positioned()?;
        let order = groups
            .keys()
            .iter()
            .map(|key| col(key).sort(true, true))
            .chain(std::iter::once(col(&position).sort(true, false)))
            .collect::<Vec<_>>();
        let mut fields = source.schema().fields().to_vec();
        fields.push(
            frame
                .schema()
                .field_with_unqualified_name(&position)
                .map_err(|_| RelationError::InvalidPlan)?
                .clone(),
        );
        let schema = Arc::new(Schema::new_with_metadata(
            fields,
            source.schema().metadata().clone(),
        ));
        let sorted = DataFusionRelation::new(
            frame
                .sort(order.clone())
                .map_err(|_| RelationError::InvalidPlan)?,
            schema.clone(),
            adapter.bindings.clone(),
            adapter.lease.clone(),
            adapter.executor.clone(),
            false,
            order,
        )?
        .into_handle()?;
        let stream = sorted.stream(control.clone()).await?;
        Ok(Self {
            stream,
            batch: None,
            offset: 0,
            exhausted: false,
            schema,
            bindings: Arc::from(source.bindings()),
            columns: source
                .schema()
                .fields()
                .iter()
                .map(|f| f.name().as_str().into())
                .collect(),
            keys: groups
                .keys()
                .iter()
                .map(|key| {
                    source
                        .schema()
                        .index_of(key)
                        .map_err(|_| RelationError::InvalidInput)
                })
                .collect::<Result<_, _>>()?,
            position,
        })
    }

    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    async fn fill(&mut self, control: &RelationControl) -> Result<(), RelationError> {
        while !self.exhausted
            && self
                .batch
                .as_ref()
                .is_none_or(|b| self.offset == b.num_rows())
        {
            self.batch = controlled(self.stream.next(), control).await?.transpose()?;
            self.offset = 0;
            self.exhausted = self.batch.is_none();
            if self
                .batch
                .as_ref()
                .is_some_and(|b| b.get_array_memory_size() > control.max_input_bytes)
            {
                return Err(RelationError::MemoryLimitExceeded);
            }
        }
        Ok(())
    }

    fn keys_at(&self, row: usize) -> Result<Vec<ScalarValue>, RelationError> {
        let batch = self.batch.as_ref().ok_or(RelationError::InvalidPlan)?;
        self.keys
            .iter()
            .map(|index| {
                ScalarValue::try_from_array(batch.column(*index), row)
                    .map_err(|_| RelationError::InvalidInput)
            })
            .collect()
    }

    pub async fn next(
        &mut self,
        engine: &Arc<DataFusionRuntime>,
        control: &RelationControl,
    ) -> Result<Option<GroupInput>, RelationError> {
        self.fill(control).await?;
        if self.exhausted {
            return Ok(None);
        }
        let keys = self.keys_at(self.offset)?;
        let mut writer = RelationSpill::new(engine, self.schema.clone(), control)?;
        loop {
            let batch = self.batch.as_ref().ok_or(RelationError::InvalidPlan)?;
            let start = self.offset;
            while self.offset < batch.num_rows() && self.keys_at(self.offset)? == keys {
                self.offset += 1;
                if self.offset.is_multiple_of(1024) {
                    control.check()?;
                }
            }
            writer.write(&batch.slice(start, self.offset - start), control)?;
            self.fill(control).await?;
            if self.exhausted || self.keys_at(self.offset)? != keys {
                break;
            }
        }
        let stored = writer
            .finish(
                engine,
                self.schema.clone(),
                self.bindings.clone(),
                vec![col(&self.position).sort(true, false)],
                control,
            )?
            .into_handle()?;
        Ok(Some(GroupInput {
            value: stored.project(&self.columns)?,
            positions: stored.project(&[self.position.as_str().into()])?,
            keys,
        }))
    }
}
