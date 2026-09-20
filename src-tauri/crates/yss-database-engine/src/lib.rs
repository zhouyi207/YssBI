//! DataFusion plan construction and controlled Arrow execution. No Graph document or UI state.

mod dataset;
mod page;
mod profile;
pub use dataset::{DatasetQuery, DatasetQueryPage};
mod comparison;
mod composition;
mod relation;
mod series;

use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::{Array, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::common::Column;
use datafusion::dataframe::DataFrame;
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::{
    ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl,
};
use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
use datafusion::logical_expr::Expr;
use datafusion::prelude::{SessionConfig, SessionContext};
use futures_util::StreamExt;
use yss_relational_contract::{
    RelationBinding, RelationControl, RelationError, RelationExecutor, RelationHandle, SeriesHandle,
};

/// All query contexts created by one host runtime share this memory/spill authority.
pub struct DataFusionRuntime {
    environment: Arc<RuntimeEnv>,
    runtime: Option<tokio::runtime::Runtime>,
    batch_size: usize,
    target_partitions: usize,
}

impl yss_relational_contract::RelationFactory for DataFusionRuntime {
    fn materialize(
        self: Arc<Self>,
        data: &yss_tabular_contract::TabularSnapshot,
        metadata: &[Option<yss_data_contract::ConversionMetadata>],
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        control.check()?;
        if data.columns().len() != metadata.len() {
            return Err(RelationError::InvalidInput);
        }
        // Account for column conversion scratch space and the materialized row order.
        // DataFusion's query workspace is separately bounded by the shared runtime pool.
        let mut bytes = data
            .row_count()
            .checked_mul(64)
            .ok_or(RelationError::MemoryLimitExceeded)?;
        for column in data.columns() {
            for (index, value) in column.values().iter().enumerate() {
                if index % 1024 == 0 {
                    control.check()?;
                }
                let size = 2 * size_of::<yss_tabular_contract::TabularScalar>()
                    + match value {
                        yss_tabular_contract::TabularScalar::String(value) => value.len(),
                        _ => 0,
                    };
                bytes = bytes
                    .checked_add(size)
                    .filter(|bytes| *bytes <= control.max_input_bytes)
                    .ok_or(RelationError::MemoryLimitExceeded)?;
            }
        }
        let mut fields = Vec::with_capacity(metadata.len());
        let mut arrays = Vec::with_capacity(metadata.len());
        for (column, metadata) in data.columns().iter().zip(metadata) {
            control.check()?;
            let (field, array) = yss_database_arrow::materialized_column(
                column.name().as_str(),
                column.values(),
                metadata.as_ref(),
            )
            .map_err(|_| RelationError::InvalidInput)?;
            fields.push(field);
            arrays.push(array);
        }
        control.check()?;
        let schema = Arc::new(Schema::new(fields));
        let batch = if arrays.is_empty() {
            arrow::record_batch::RecordBatch::new_empty(schema)
        } else {
            arrow::record_batch::RecordBatch::try_new(schema, arrays)
                .map_err(|_| RelationError::InvalidInput)?
        };
        let relation = self.materialized_batch(Arc::from([]), batch)?;
        control.check()?;
        Ok(relation)
    }
}

impl DataFusionRuntime {
    pub fn visit_relation(
        &self,
        relation: &RelationHandle,
        control: &RelationControl,
        visitor: &mut dyn FnMut(arrow::record_batch::RecordBatch) -> Result<(), RelationError>,
    ) -> Result<(), RelationError> {
        self.runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(async {
                let mut stream = relation.stream(control.clone()).await?;
                while let Some(batch) = stream.next().await {
                    control.check()?;
                    visitor(batch?)?;
                }
                control.check()
            })
    }

    pub fn new(memory_bytes: usize, batch_size: usize) -> Result<Arc<Self>, RelationError> {
        if memory_bytes == 0 || batch_size == 0 {
            return Err(RelationError::InvalidInput);
        }
        let environment = RuntimeEnvBuilder::new()
            .with_memory_limit(memory_bytes, 1.0)
            .build_arc()
            .map_err(|_| RelationError::QueryFailed)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| RelationError::QueryFailed)?;
        Ok(Arc::new(Self {
            environment,
            runtime: Some(runtime),
            batch_size,
            // Sort/merge operators reserve memory per partition. CPU-count parallelism can
            // consume the entire pool on high-core hosts before spilling can make progress.
            target_partitions: std::thread::available_parallelism()
                .map_or(1, usize::from)
                .min((memory_bytes / (64 * 1024 * 1024)).max(1)),
        }))
    }

    fn context(&self) -> SessionContext {
        SessionContext::new_with_config_rt(
            SessionConfig::new()
                .with_batch_size(self.batch_size)
                .with_target_partitions(self.target_partitions)
                .with_collect_statistics(false),
            self.environment.clone(),
        )
    }

    /// The caller supplies catalog-committed immutable files/schema and a lease that prevents
    /// their removal. Planning never discovers datasets from a directory or scans their rows.
    pub fn parquet_relation(
        self: &Arc<Self>,
        binding: RelationBinding,
        schema: SchemaRef,
        files: &[PathBuf],
        lease: Arc<dyn Send + Sync>,
    ) -> Result<RelationHandle, RelationError> {
        if files.is_empty() || binding.project_session.is_empty() || binding.snapshot.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        let paths = files
            .iter()
            .map(|path| {
                if !path.is_absolute() || path.is_dir() {
                    return Err(RelationError::InvalidInput);
                }
                let url =
                    url::Url::from_file_path(path).map_err(|_| RelationError::SourceUnavailable)?;
                ListingTableUrl::try_new(url, None).map_err(|_| RelationError::SourceUnavailable)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let options =
            ListingOptions::new(Arc::new(ParquetFormat::default())).with_file_extension(".parquet");
        let table = ListingTable::try_new(
            ListingTableConfig::new_with_multi_paths(paths)
                .with_listing_options(options)
                .with_schema(schema.clone()),
        )
        .map_err(|_| RelationError::InvalidPlan)?;
        let frame = self
            .context()
            .read_table(Arc::new(table))
            .map_err(|_| RelationError::InvalidPlan)?;
        let (frame, schema, order) = ordered_user_frame(frame, &schema)?;
        relation::DataFusionRelation::handle(
            frame,
            schema,
            Arc::from([binding]),
            lease,
            self.clone(),
            false,
            order,
        )
    }

    /// Useful at the already-materialized literal boundary; external imports use batch readers.
    pub fn batch_relation(
        self: &Arc<Self>,
        binding: RelationBinding,
        batch: arrow::record_batch::RecordBatch,
    ) -> Result<RelationHandle, RelationError> {
        self.materialized_batch(Arc::from([binding]), batch)
    }

    fn materialized_batch(
        self: &Arc<Self>,
        bindings: Arc<[RelationBinding]>,
        batch: arrow::record_batch::RecordBatch,
    ) -> Result<RelationHandle, RelationError> {
        let original_schema = batch.schema();
        let unique = |base: &str| {
            let mut name = base.to_owned();
            while original_schema.index_of(&name).is_ok() {
                name.push('_');
            }
            name
        };
        let row_id = unique("__yssbi_row_id");
        let display_order = unique("__yssbi_display_order");
        let row_count = i64::try_from(batch.num_rows()).map_err(|_| RelationError::InvalidInput)?;
        let mut fields = original_schema.fields().to_vec();
        fields.push(Arc::new(Field::new(&row_id, DataType::Int64, false)));
        fields.push(Arc::new(Field::new(&display_order, DataType::Utf8, false)));
        let schema = Arc::new(
            yss_database_arrow::with_row_columns(
                Schema::new_with_metadata(fields, original_schema.metadata().clone()),
                &row_id,
                &display_order,
            )
            .map_err(|_| RelationError::InvalidInput)?,
        );
        let mut columns = batch.columns().to_vec();
        // A document literal is already immutable; these identities live only in that literal's
        // row domain. Persisted datasets must supply catalog-owned RowId and DisplayOrder.
        columns.push(Arc::new(Int64Array::from_iter_values(0..row_count)));
        columns.push(Arc::new(StringArray::from_iter_values(
            (0..row_count).map(|index| format!("{index:020}")),
        )));
        let batch = arrow::record_batch::RecordBatch::try_new(schema.clone(), columns)
            .map_err(|_| RelationError::InvalidInput)?;
        let frame = self
            .context()
            .read_batch(batch)
            .map_err(|_| RelationError::InvalidPlan)?;
        let (frame, schema, order) = ordered_user_frame(frame, &schema)?;
        relation::DataFusionRelation::handle(
            frame,
            schema,
            bindings,
            Arc::new(()),
            self.clone(),
            false,
            order,
        )
    }

    pub async fn prepare_numeric_columns(
        series: &[SeriesHandle],
        control: &RelationControl,
    ) -> Result<Vec<Vec<f64>>, RelationError> {
        control.check()?;
        let first = series.first().ok_or(RelationError::InvalidInput)?;
        if series
            .iter()
            .any(|series| !yss_database_arrow::is_numeric_field(series.plan().field()))
        {
            return Err(RelationError::InvalidInput);
        }
        if series
            .iter()
            .any(|series| series.relation() != first.relation())
        {
            return Err(RelationError::UnalignedSeries);
        }
        let relation = first.relation().project_series(series)?;
        let mut stream = relation.stream(control.clone()).await?;
        let mut values: Vec<Vec<f64>> = vec![Vec::new(); series.len()];
        let mut rows = 0usize;
        while let Some(batch) = stream.next().await {
            control.check()?;
            let batch = batch?;
            rows = rows
                .checked_add(batch.num_rows())
                .ok_or(RelationError::MemoryLimitExceeded)?;
            // Vec growth and the numeric cast scratch space have separate headroom from the
            // DataFusion process memory pool. OLS is still a materialized matrix algorithm.
            let bytes = rows
                .checked_mul(series.len())
                .and_then(|n| n.checked_mul(2 * size_of::<f64>()))
                .and_then(|n| n.checked_add(batch.num_rows().checked_mul(size_of::<f64>())?))
                .ok_or(RelationError::MemoryLimitExceeded)?;
            if bytes > control.max_input_bytes {
                return Err(RelationError::MemoryLimitExceeded);
            }
            for (source, values) in batch.columns().iter().zip(&mut values) {
                if !source.data_type().is_numeric() || source.null_count() != 0 {
                    return Err(RelationError::InvalidInput);
                }
                let casted =
                    yss_database_arrow::lossless_cast(source.as_ref(), &DataType::Float64, false)
                        .map_err(|_| RelationError::InvalidInput)?;
                let numbers = casted
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or(RelationError::InvalidInput)?;
                values
                    .try_reserve(batch.num_rows())
                    .map_err(|_| RelationError::MemoryLimitExceeded)?;
                for (index, value) in numbers.values().iter().enumerate() {
                    if index % 1024 == 0 {
                        control.check()?;
                    }
                    if !value.is_finite() {
                        return Err(RelationError::InvalidInput);
                    }
                    values.push(*value);
                }
            }
        }
        Ok(values)
    }
}

fn limit_frame(
    frame: DataFrame,
    offset: usize,
    limit: usize,
    ordered_single_file: bool,
) -> Result<DataFrame, RelationError> {
    let frame = frame
        .limit(offset, Some(limit))
        .map_err(|_| RelationError::InvalidPlan)?;
    // A small prefix of one ordered file needs neither parallel range reads nor a merge.
    // Keep normal parallelism for filters, overlays, multiple files and large offsets.
    const MAX_SEQUENTIAL_PREFIX_ROWS: usize = 128 * 1024;
    if ordered_single_file
        && offset
            .checked_add(limit)
            .is_some_and(|rows| rows <= MAX_SEQUENTIAL_PREFIX_ROWS)
    {
        let (mut state, plan) = frame.into_parts();
        *state.config_mut() = state.config().clone().with_target_partitions(1);
        Ok(DataFrame::new(state, plan))
    } else {
        Ok(frame)
    }
}

fn ordered_user_frame(
    frame: DataFrame,
    schema: &Schema,
) -> Result<
    (
        DataFrame,
        arrow::datatypes::SchemaRef,
        Vec<datafusion::logical_expr::expr::Sort>,
    ),
    RelationError,
> {
    let rows = yss_database_arrow::dataset_row_columns(schema)
        .map_err(|_| RelationError::InvalidInput)?
        .ok_or(RelationError::InvalidInput)?;
    let column = |name: &str| Expr::Column(Column::from_name(name.to_owned()));
    let order = vec![
        column(&rows.display_order).sort(true, false),
        column(&rows.row_id).sort(true, false),
    ];
    let frame = frame
        .sort(order.clone())
        .map_err(|_| RelationError::InvalidPlan)?;
    let projection = schema
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            (field.name() != &rows.row_id && field.name() != &rows.display_order).then_some(index)
        })
        .collect::<Vec<_>>();
    let schema = schema
        .project(&projection)
        .map_err(|_| RelationError::InvalidInput)?;
    Ok((
        frame,
        Arc::new(yss_database_arrow::without_row_metadata(&schema)),
        order,
    ))
}

impl RelationExecutor for DataFusionRuntime {
    fn page(
        &self,
        relation: &RelationHandle,
        offset: usize,
        limit: usize,
        control: &RelationControl,
    ) -> Result<yss_relational_contract::RelationPage, RelationError> {
        self.runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(page::read_page(relation, offset, limit, control))
    }
    fn numeric_columns(
        &self,
        series: &[SeriesHandle],
        control: &RelationControl,
    ) -> Result<Vec<Vec<f64>>, RelationError> {
        self.runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(Self::prepare_numeric_columns(series, control))
    }
}

impl Drop for DataFusionRuntime {
    fn drop(&mut self) {
        // A stream or Result may release the last lease from an asynchronous worker.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

#[cfg(test)]
mod tests;
