use std::collections::BTreeSet;
use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Array};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::record_batch::{RecordBatch, RecordBatchOptions};
use datafusion::common::{Column, ScalarValue};
use datafusion::dataframe::DataFrame;
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::{
    ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl,
};
use datafusion::logical_expr::{Expr, JoinType, expr_fn::when};
use futures_util::StreamExt;
use yss_relational_contract::{
    DatasetRelationInput, RelationBinding, RelationControl, RelationError, RelationHandle,
};

use crate::{DataFusionRuntime, ordered_user_frame};

pub struct DatasetQuery {
    pub(crate) frame: DataFrame,
    pub(crate) schema: SchemaRef,
    binding: RelationBinding,
    lease: Arc<dyn Send + Sync>,
    pub(crate) engine: Arc<DataFusionRuntime>,
    ordered_single_file: bool,
}

pub struct DatasetQueryPage {
    pub batches: Vec<RecordBatch>,
    pub row_count: usize,
    pub has_more: bool,
}

pub(crate) fn column(relation: Option<&str>, name: &str) -> Expr {
    Expr::Column(Column::new(
        relation.map(datafusion::common::TableReference::bare),
        name,
    ))
}

fn align(frame: DataFrame, source: &Schema, target: &Schema) -> Result<DataFrame, RelationError> {
    let expressions = target
        .fields()
        .iter()
        .map(|field| {
            let id = yss_tabular_arrow::column_identity(field)
                .map_err(|_| RelationError::InvalidInput)?;
            let source = source
                .fields()
                .iter()
                .find(|field| yss_tabular_arrow::column_identity(field).ok() == Some(id));
            let value = match source {
                Some(source) if source.data_type() == field.data_type() => {
                    column(None, source.name())
                }
                Some(_) => return Err(RelationError::InvalidInput),
                None => Expr::Literal(
                    ScalarValue::try_from(field.data_type())
                        .map_err(|_| RelationError::InvalidInput)?,
                    None,
                ),
            };
            Ok(value.alias(field.name()))
        })
        .collect::<Result<Vec<_>, RelationError>>()?;
    frame
        .select(expressions)
        .map_err(|_| RelationError::InvalidPlan)
}

impl DataFusionRuntime {
    pub fn dataset_query(
        self: &Arc<Self>,
        binding: RelationBinding,
        input: DatasetRelationInput,
        lease: Arc<dyn Send + Sync>,
    ) -> Result<DatasetQuery, RelationError> {
        yss_tabular_arrow::validate_storage_schema(&input.schema)
            .map_err(|_| RelationError::InvalidInput)?;
        let rows = yss_tabular_arrow::dataset_row_columns(&input.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        let base_rows = yss_tabular_arrow::dataset_row_columns(&input.base_schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        let ordered_single_file = input.files.len() == 1
            && input.overlay.columns.is_empty()
            && input.overlay.inserted.is_empty()
            && input.overlay.deleted.is_empty();
        let paths = input
            .files
            .iter()
            .map(|path| {
                let url =
                    url::Url::from_file_path(path).map_err(|_| RelationError::InvalidInput)?;
                ListingTableUrl::try_new(url, None).map_err(|_| RelationError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if paths.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        let context = self.context();
        let table = ListingTable::try_new(
            ListingTableConfig::new_with_multi_paths(paths)
                .with_listing_options(
                    ListingOptions::new(Arc::new(ParquetFormat::default()))
                        .with_file_extension(".parquet")
                        .with_file_sort_order(vec![vec![
                            column(None, &base_rows.display_order).sort(true, false),
                            column(None, &base_rows.row_id).sort(true, false),
                        ]]),
                )
                .with_schema(input.base_schema.clone()),
        )
        .map_err(|_| RelationError::InvalidPlan)?;
        let base = context
            .read_table(Arc::new(table))
            .map_err(|_| RelationError::InvalidPlan)?;
        let mut frame = align(base, &input.base_schema, &input.schema)?;
        for batch in input.overlay.inserted.iter() {
            let inserted = align(
                context
                    .read_batch(batch.clone())
                    .map_err(|_| RelationError::InvalidPlan)?,
                &batch.schema(),
                &input.schema,
            )?;
            frame = frame
                .union(inserted)
                .map_err(|_| RelationError::InvalidPlan)?;
        }
        if !input.overlay.deleted.is_empty() {
            let deleted = RecordBatch::try_new(
                Arc::new(Schema::new(vec![Field::new(
                    "row_id",
                    DataType::Int64,
                    false,
                )])),
                vec![Arc::new(Int64Array::from(input.overlay.deleted.to_vec()))],
            )
            .map_err(|_| RelationError::InvalidInput)?;
            let deleted = context
                .read_batch(deleted)
                .and_then(|frame| frame.alias("deleted"))
                .map_err(|_| RelationError::InvalidPlan)?;
            frame = frame
                .alias("current")
                .and_then(|frame| {
                    frame.join_on(
                        deleted,
                        JoinType::LeftAnti,
                        [column(Some("current"), &rows.row_id)
                            .eq(column(Some("deleted"), "row_id"))],
                    )
                })
                .map_err(|_| RelationError::InvalidPlan)?;
        }
        let mut seen_columns = BTreeSet::new();
        let mut patches = Vec::new();
        let mut changed_rows = BTreeSet::new();
        for patch in input.overlay.columns.iter() {
            if !seen_columns.insert(patch.column_id.as_ref())
                || patch.row_ids.len() != patch.values.len()
                || patch.row_ids.iter().collect::<BTreeSet<_>>().len() != patch.row_ids.len()
            {
                return Err(RelationError::InvalidInput);
            }
            let Some(field) = input.schema.fields().iter().find(|field| {
                yss_tabular_arrow::column_identity(field).ok() == Some(patch.column_id.as_ref())
            }) else {
                continue;
            };
            if field.name() == &rows.row_id
                || field.name() == &rows.display_order
                || field.data_type() != patch.values.data_type()
            {
                return Err(RelationError::InvalidInput);
            }
            changed_rows.extend(patch.row_ids.iter().copied());
            patches.push((patch, field));
        }
        let unchanged =
            if let (Some(&first), Some(&last)) = (changed_rows.first(), changed_rows.last()) {
                let changed = RecordBatch::try_new(
                    Arc::new(Schema::new(vec![Field::new(
                        "row_id",
                        DataType::Int64,
                        false,
                    )])),
                    vec![Arc::new(Int64Array::from_iter_values(changed_rows))],
                )
                .map_err(|_| RelationError::InvalidInput)?;
                let changed = context
                    .read_batch(changed)
                    .and_then(|frame| frame.alias("changed"))
                    .map_err(|_| RelationError::InvalidPlan)?;
                let current = frame
                    .alias("current")
                    .map_err(|_| RelationError::InvalidPlan)?;
                let key = column(Some("current"), &rows.row_id);
                let on = key.clone().eq(column(Some("changed"), "row_id"));
                let unchanged = current
                    .clone()
                    .join_on(changed.clone(), JoinType::LeftAnti, [on.clone()])
                    .map_err(|_| RelationError::InvalidPlan)?;
                // Expose a bounded source predicate as well as exact membership. The range
                // enables Parquet pruning without expanding a large delta into SQL literals.
                let range = if first == last {
                    key.eq(Expr::Literal(ScalarValue::Int64(Some(first)), None))
                } else {
                    key.clone()
                        .gt_eq(Expr::Literal(ScalarValue::Int64(Some(first)), None))
                        .and(key.lt_eq(Expr::Literal(ScalarValue::Int64(Some(last)), None)))
                };
                frame = current
                    .filter(range)
                    .and_then(|frame| frame.join_on(changed, JoinType::LeftSemi, [on]))
                    .map_err(|_| RelationError::InvalidPlan)?;
                Some(unchanged)
            } else {
                None
            };
        for (patch, field) in patches {
            let edits = RecordBatch::try_new(
                Arc::new(Schema::new(vec![
                    Field::new("row_id", DataType::Int64, false),
                    field.as_ref().clone().with_name("value"),
                ])),
                vec![
                    Arc::new(Int64Array::from(patch.row_ids.to_vec())) as ArrayRef,
                    patch.values.clone(),
                ],
            )
            .map_err(|_| RelationError::InvalidInput)?;
            let edits = context
                .read_batch(edits)
                .and_then(|frame| frame.alias("patch"))
                .map_err(|_| RelationError::InvalidPlan)?;
            let joined =
                frame
                    .alias("current")
                    .and_then(|frame| {
                        frame.join_on(
                            edits,
                            JoinType::Left,
                            [column(Some("current"), &rows.row_id)
                                .eq(column(Some("patch"), "row_id"))],
                        )
                    })
                    .map_err(|_| RelationError::InvalidPlan)?;
            let expressions = input
                .schema
                .fields()
                .iter()
                .map(|current| {
                    let expression = if current.name() == field.name() {
                        // Presence, not COALESCE(value, base), preserves an explicit NULL edit.
                        when(
                            column(Some("patch"), "row_id").is_not_null(),
                            column(Some("patch"), "value"),
                        )
                        .otherwise(column(Some("current"), current.name()))
                        .map_err(|_| RelationError::InvalidPlan)?
                    } else {
                        column(Some("current"), current.name())
                    };
                    Ok(expression.alias(current.name()))
                })
                .collect::<Result<Vec<_>, RelationError>>()?;
            frame = joined
                .select(expressions)
                .map_err(|_| RelationError::InvalidPlan)?;
        }
        if let Some(unchanged) = unchanged {
            // Native UNION lets filters reach unchanged base values while edited values
            // retain CASE/NULL semantics. Only changed rows pass through the patch joins.
            frame = unchanged
                .union(frame)
                .map_err(|_| RelationError::InvalidPlan)?;
        }
        Ok(DatasetQuery {
            frame,
            schema: input.schema,
            binding,
            lease,
            engine: self.clone(),
            ordered_single_file,
        })
    }
}

impl DatasetQuery {
    pub fn cast_column(
        &self,
        name: &str,
        schema: SchemaRef,
        force: bool,
    ) -> Result<Self, RelationError> {
        let target = schema
            .field_with_name(name)
            .map_err(|_| RelationError::InvalidInput)?
            .data_type()
            .clone();
        let expression = if force {
            datafusion::logical_expr::expr_fn::try_cast(column(None, name), target)
        } else {
            datafusion::logical_expr::expr_fn::cast(column(None, name), target)
        };
        let frame = self
            .frame
            .clone()
            .with_column(name, expression)
            .map_err(|_| RelationError::InvalidPlan)?;
        Ok(Self {
            frame,
            schema,
            binding: self.binding.clone(),
            lease: self.lease.clone(),
            engine: self.engine.clone(),
            ordered_single_file: self.ordered_single_file,
        })
    }

    pub fn distinct_labels(
        &self,
        name: &str,
        control: &RelationControl,
    ) -> Result<Vec<String>, RelationError> {
        let frame = self
            .frame
            .clone()
            .select([column(None, name)])
            .map_err(|_| RelationError::InvalidPlan)?;
        self.engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(async {
                let mut labels = BTreeSet::new();
                let mut bytes = 0usize;
                let mut stream = crate::relation::controlled(frame.execute_stream(), control)
                    .await?
                    .map_err(crate::relation::query_error)?;
                while let Some(batch) = crate::relation::controlled(stream.next(), control).await? {
                    let batch = batch.map_err(crate::relation::query_error)?;
                    let strings = arrow::compute::cast(batch.column(0).as_ref(), &DataType::Utf8)
                        .map_err(|_| RelationError::InvalidInput)?;
                    let strings = strings
                        .as_any()
                        .downcast_ref::<arrow::array::StringArray>()
                        .ok_or(RelationError::InvalidInput)?;
                    for value in strings.iter().flatten() {
                        if !labels.contains(value) {
                            bytes = bytes
                                .checked_add(value.len() + 32)
                                .ok_or(RelationError::MemoryLimitExceeded)?;
                            if bytes > control.max_input_bytes {
                                return Err(RelationError::MemoryLimitExceeded);
                            }
                            labels.insert(value.to_owned());
                        }
                    }
                }
                Ok(labels.into_iter().collect())
            })
    }

    pub fn relation(&self) -> Result<RelationHandle, RelationError> {
        let (frame, schema) = ordered_user_frame(self.frame.clone(), &self.schema)?;
        crate::relation::DataFusionRelation::handle(
            frame,
            schema,
            self.binding.clone(),
            self.lease.clone(),
            self.engine.clone(),
            self.ordered_single_file,
        )
    }

    pub fn page(
        &self,
        offset: usize,
        limit: usize,
        control: &RelationControl,
    ) -> Result<DatasetQueryPage, RelationError> {
        if limit == 0 {
            return Err(RelationError::InvalidInput);
        }
        let rows = yss_tabular_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        let probe = limit.checked_add(1).ok_or(RelationError::InvalidInput)?;
        let frame = self
            .frame
            .clone()
            .sort(vec![
                column(None, &rows.display_order).sort(true, false),
                column(None, &rows.row_id).sort(true, false),
            ])
            .map_err(|_| RelationError::InvalidPlan)?;
        let frame = crate::limit_frame(frame, offset, probe, self.ordered_single_file)?;
        self.read_bounded(frame, limit, control)
    }

    pub fn contains_rows(
        &self,
        row_ids: &[i64],
        control: &RelationControl,
    ) -> Result<bool, RelationError> {
        control.check()?;
        if row_ids.len() > control.max_input_bytes / 8 {
            return Err(RelationError::MemoryLimitExceeded);
        }
        let requested = row_ids.iter().copied().collect::<BTreeSet<_>>();
        let rows = yss_tabular_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new(
                "row_id",
                DataType::Int64,
                false,
            )])),
            vec![Arc::new(Int64Array::from_iter_values(
                requested.iter().copied(),
            ))],
        )
        .map_err(|_| RelationError::InvalidInput)?;
        let selected = self
            .engine
            .context()
            .read_batch(batch)
            .map_err(|_| RelationError::InvalidPlan)?;
        let frame = self
            .frame
            .clone()
            .select([column(None, &rows.row_id)])
            .and_then(|frame| {
                frame.join(
                    selected,
                    JoinType::LeftSemi,
                    &[&rows.row_id],
                    &["row_id"],
                    None,
                )
            })
            .map_err(|_| RelationError::InvalidPlan)?;
        let count = self
            .engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(crate::relation::controlled(frame.count(), control))?
            .map_err(crate::relation::query_error)?;
        Ok(count == requested.len())
    }

    pub fn row(
        &self,
        row_id: i64,
        control: &RelationControl,
    ) -> Result<Option<RecordBatch>, RelationError> {
        let rows = yss_tabular_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        let frame = self
            .frame
            .clone()
            .filter(
                column(None, &rows.row_id)
                    .eq(Expr::Literal(ScalarValue::Int64(Some(row_id)), None)),
            )
            .and_then(|frame| frame.limit(0, Some(1)))
            .map_err(|_| RelationError::InvalidPlan)?;
        Ok(self
            .read_bounded(frame, 1, control)?
            .batches
            .into_iter()
            .find(|batch| batch.num_rows() > 0))
    }

    fn read_bounded(
        &self,
        frame: DataFrame,
        limit: usize,
        control: &RelationControl,
    ) -> Result<DatasetQueryPage, RelationError> {
        let mut batches = Vec::new();
        let mut count = 0usize;
        let mut bytes = 0usize;
        self.visit(frame, control, &mut |batch| {
            bytes = bytes
                .checked_add(batch.get_array_memory_size())
                .ok_or(RelationError::MemoryLimitExceeded)?;
            if bytes > control.max_input_bytes {
                return Err(RelationError::MemoryLimitExceeded);
            }
            let take = batch.num_rows().min(limit.saturating_sub(count));
            count = count
                .checked_add(batch.num_rows())
                .ok_or(RelationError::MemoryLimitExceeded)?;
            if take > 0 {
                batches.push(batch.slice(0, take));
            }
            Ok(())
        })?;
        Ok(DatasetQueryPage {
            batches,
            row_count: count.min(limit),
            has_more: count > limit,
        })
    }

    pub fn visit_batches(
        &self,
        control: &RelationControl,
        visitor: &mut dyn FnMut(RecordBatch) -> Result<(), RelationError>,
    ) -> Result<(), RelationError> {
        let rows = yss_tabular_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        let frame = self
            .frame
            .clone()
            .sort(vec![
                column(None, &rows.display_order).sort(true, false),
                column(None, &rows.row_id).sort(true, false),
            ])
            .map_err(|_| RelationError::InvalidPlan)?;
        self.visit(frame, control, visitor)
    }

    fn visit(
        &self,
        frame: DataFrame,
        control: &RelationControl,
        visitor: &mut dyn FnMut(RecordBatch) -> Result<(), RelationError>,
    ) -> Result<(), RelationError> {
        self.engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(async {
                let mut stream = crate::relation::controlled(frame.execute_stream(), control)
                    .await?
                    .map_err(crate::relation::query_error)?;
                while let Some(batch) = crate::relation::controlled(stream.next(), control).await? {
                    let batch = batch.map_err(crate::relation::query_error)?;
                    let batch = RecordBatch::try_new_with_options(
                        self.schema.clone(),
                        batch.columns().to_vec(),
                        &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
                    )
                    .map_err(|_| RelationError::InvalidInput)?;
                    visitor(batch)?;
                }
                Ok(())
            })
    }
}
