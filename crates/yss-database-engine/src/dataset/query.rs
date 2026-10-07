//! Managed-table queries retain row identity through projection, filtering and ordering.
use super::*;
use yss_relational_contract::{RelationPredicate, SortColumn};

#[derive(Clone, Debug, PartialEq)]
pub struct DatasetRowsQuery {
    /// Empty selects all user columns. Internal columns cannot be selected by name.
    pub columns: Vec<String>,
    /// All predicates must match. Null tests carry no value.
    pub filters: Vec<RelationPredicate>,
    pub order: Vec<SortColumn>,
    pub offset: usize,
    pub limit: usize,
}

impl DatasetQuery {
    fn user_field(&self, name: &str) -> Result<&Field, RelationError> {
        let rows = yss_database_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        if name == rows.row_id || name == rows.display_order {
            return Err(RelationError::InvalidInput);
        }
        self.schema
            .field_with_name(name)
            .map_err(|_| RelationError::InvalidInput)
    }

    fn selected_schema(&self, columns: &[String]) -> Result<SchemaRef, RelationError> {
        if columns.is_empty() {
            return Ok(self.schema.clone());
        }
        let rows = yss_database_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        let mut fields = [&rows.row_id, &rows.display_order]
            .into_iter()
            .map(|name| {
                self.schema
                    .field_with_name(name)
                    .cloned()
                    .map(Arc::new)
                    .map_err(|_| RelationError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = BTreeSet::new();
        for name in columns {
            if !seen.insert(name) {
                return Err(RelationError::InvalidInput);
            }
            fields.push(Arc::new(self.user_field(name)?.clone()));
        }
        Ok(Arc::new(Schema::new_with_metadata(
            fields,
            self.schema.metadata().clone(),
        )))
    }

    /// Keep exact fields and identity metadata so existing profile operations can select columns.
    pub fn project_columns(&self, columns: &[String]) -> Result<Self, RelationError> {
        let schema = self.selected_schema(columns)?;
        let frame = self
            .frame
            .clone()
            .select(
                schema
                    .fields()
                    .iter()
                    .map(|field| column(None, field.name())),
            )
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

    pub fn page_query(
        &self,
        query: &DatasetRowsQuery,
        control: &RelationControl,
    ) -> Result<DatasetQueryPage, RelationError> {
        control.check()?;
        if query.limit == 0 {
            return Err(RelationError::InvalidInput);
        }
        let schema = self.selected_schema(&query.columns)?;
        let mut frame = self.frame.clone();
        for predicate in &query.filters {
            self.user_field(&predicate.column)?;
            let expression = crate::relation::predicate_expression(&self.schema, predicate)?;
            frame = frame
                .filter(expression)
                .map_err(|_| RelationError::InvalidPlan)?;
        }
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        for sort in &query.order {
            if !seen.insert(&sort.column) {
                return Err(RelationError::InvalidInput);
            }
            let field = self.user_field(&sort.column)?;
            order.push(
                crate::series_transform::ordered_value(column(None, &sort.column), field)?
                    .sort(sort.ascending, sort.nulls_first),
            );
        }
        let rows = yss_database_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        // Stable tie-breaking also applies when a requested ordering has duplicate values.
        order.push(column(None, &rows.display_order).sort(true, false));
        order.push(column(None, &rows.row_id).sort(true, false));
        frame = frame
            .sort(order)
            .and_then(|frame| {
                frame.select(
                    schema
                        .fields()
                        .iter()
                        .map(|field| column(None, field.name())),
                )
            })
            .map_err(|_| RelationError::InvalidPlan)?;
        let probe = query
            .limit
            .checked_add(1)
            .ok_or(RelationError::InvalidInput)?;
        let prefix = self.ordered_single_file && query.filters.is_empty() && query.order.is_empty();
        frame = crate::limit_frame(frame, query.offset, probe, prefix)?;
        self.read_bounded(frame, schema, query.limit, control)
    }
}
