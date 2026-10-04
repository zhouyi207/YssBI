//! Shared bounded Arrow storage for retained results and group mapping.
use std::sync::Arc;

use arrow::{datatypes::SchemaRef, ipc::writer::FileWriter, record_batch::RecordBatch};
use datafusion::{
    datasource::{
        file_format::arrow::ArrowFormat,
        listing::{ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl},
    },
    execution::spill_file::{SpillFile, SpillWriter},
    logical_expr::expr::Sort,
};
use yss_relational_contract::{RelationBinding, RelationControl, RelationError};

use crate::{
    DataFusionRuntime,
    relation::{DataFusionRelation, query_error},
};

pub(crate) struct RelationSpill {
    writer: FileWriter<Box<dyn SpillWriter>>,
    file: Arc<dyn SpillFile>,
    schema: SchemaRef,
}

impl RelationSpill {
    pub(crate) fn new(
        engine: &DataFusionRuntime,
        schema: SchemaRef,
        control: &RelationControl,
    ) -> Result<Self, RelationError> {
        control.check()?;
        let file = engine
            .environment
            .disk_manager
            .create_tmp_file("retaining a graph relation")
            .map_err(query_error)?;
        let writer = FileWriter::try_new(file.open_writer().map_err(query_error)?, &schema)
            .map_err(|_| RelationError::QueryFailed)?;
        Ok(Self {
            writer,
            file,
            schema,
        })
    }

    pub(crate) fn write(
        &mut self,
        batch: &RecordBatch,
        control: &RelationControl,
    ) -> Result<(), RelationError> {
        control.check()?;
        if batch
            .get_array_memory_size()
            .checked_mul(2)
            .is_none_or(|bytes| bytes > control.max_input_bytes)
        {
            return Err(RelationError::MemoryLimitExceeded);
        }
        self.writer
            .write(batch)
            .map_err(|_| RelationError::QueryFailed)?;
        control.check()
    }

    pub(crate) fn finish(
        mut self,
        engine: &Arc<DataFusionRuntime>,
        visible_schema: SchemaRef,
        bindings: Arc<[RelationBinding]>,
        order: Vec<Sort>,
        control: &RelationControl,
    ) -> Result<DataFusionRelation, RelationError> {
        control.check()?;
        self.writer
            .finish()
            .map_err(|_| RelationError::QueryFailed)?;
        self.writer.get_mut().finish().map_err(query_error)?;
        drop(self.writer);
        control.check()?;
        let url =
            url::Url::from_file_path(self.file.path().ok_or(RelationError::SourceUnavailable)?)
                .map_err(|_| RelationError::SourceUnavailable)?;
        let path = ListingTableUrl::try_new(url, None).map_err(query_error)?;
        let table = ListingTable::try_new(
            ListingTableConfig::new(path)
                .with_listing_options(
                    ListingOptions::new(Arc::new(ArrowFormat)).with_file_extension(""),
                )
                .with_schema(self.schema),
        )
        .map_err(query_error)?;
        let frame = engine
            .context()
            .read_table(Arc::new(table))
            .map_err(query_error)?
            .sort(order.clone())
            .map_err(query_error)?;
        DataFusionRelation::new(
            frame,
            visible_schema,
            bindings,
            Arc::new(self.file),
            engine.clone(),
            true,
            order,
        )
    }
}
