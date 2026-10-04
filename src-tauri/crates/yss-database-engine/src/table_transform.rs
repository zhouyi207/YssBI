//! Row selection and reshaping are native plans with explicit schemas and order.
use crate::{
    relation::DataFusionRelation,
    series::DataFusionSeries,
    series_transform::{col, field, finite, literal, null, ordered_value, over, plan, whole},
};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::{
    dataframe::DataFrame,
    functions::expr_fn as scalar,
    functions_aggregate::expr_fn as aggregate,
    functions_window::expr_fn as window,
    logical_expr::{
        Expr,
        expr::Sort,
        expr_fn::{cast, when},
    },
    prelude::lit,
};
use std::{collections::BTreeSet, sync::Arc};
use yss_data_contract::{SemanticType, TabularScalar};
use yss_relational_contract::*;

impl DataFusionRelation {
    pub(crate) fn unique_name(&self, base: &str) -> String {
        let mut name = base.to_owned();
        while self.schema.index_of(&name).is_ok()
            || self
                .domain
                .schema()
                .index_of_column_by_name(None, &name)
                .is_some()
        {
            name.push('_');
        }
        name
    }
    pub(crate) fn positioned(&self) -> Result<(DataFrame, String), RelationError> {
        let position = self.unique_name("__yssbi_transform_position");
        let mut expressions = self
            .columns
            .iter()
            .zip(self.schema.fields())
            .map(|(expr, field)| expr.clone().alias(field.name()))
            .collect::<Vec<_>>();
        expressions.push(
            over(
                window::row_number(),
                vec![],
                self.domain_order.clone(),
                whole(),
            )?
            .alias(&position),
        );
        Ok((self.select_native(expressions)?, position))
    }
    pub(crate) fn rebuilt(
        &self,
        frame: DataFrame,
        fields: Vec<Field>,
        order: Vec<Sort>,
    ) -> Result<RelationHandle, RelationError> {
        Self::new(
            frame,
            Arc::new(Schema::new_with_metadata(
                fields,
                self.schema.metadata().clone(),
            )),
            self.bindings.clone(),
            self.lease.clone(),
            self.executor.clone(),
            false,
            order,
        )
        .and_then(Self::into_handle)
    }
    pub(crate) fn sorted(&self, columns: &[SortColumn]) -> Result<RelationHandle, RelationError> {
        if columns.is_empty()
            || columns
                .iter()
                .map(|s| &s.column)
                .collect::<BTreeSet<_>>()
                .len()
                != columns.len()
        {
            return Err(RelationError::InvalidInput);
        }
        let (frame, position) = self.positioned()?;
        let mut order = Vec::new();
        for column in columns {
            let field = self
                .schema
                .field_with_name(&column.column)
                .map_err(|_| RelationError::InvalidInput)?;
            let value = ordered_value(col(&column.column), field)?;
            order.push(value.sort(column.ascending, column.nulls_first));
        }
        order.push(col(&position).sort(true, false));
        let frame = frame.sort(order.clone()).map_err(plan)?;
        self.rebuilt(
            frame,
            self.schema
                .fields()
                .iter()
                .map(|f| f.as_ref().clone())
                .collect(),
            order,
        )
    }
    pub(crate) fn unique_rows(
        &self,
        keys: &[Box<str>],
        keep: DuplicateKeep,
    ) -> Result<RelationHandle, RelationError> {
        let keys = if keys.is_empty() {
            self.schema
                .fields()
                .iter()
                .map(|f| f.name().as_str())
                .collect::<Vec<_>>()
        } else {
            keys.iter().map(AsRef::as_ref).collect()
        };
        if keys.iter().collect::<BTreeSet<_>>().len() != keys.len()
            || keys.iter().any(|name| self.schema.index_of(name).is_err())
        {
            return Err(RelationError::InvalidInput);
        }
        let (frame, position) = self.positioned()?;
        let marker = self.unique_name("__yssbi_duplicate_rank");
        let partition = keys.into_iter().map(col).collect();
        let marker_expr = match keep {
            DuplicateKeep::None => over(aggregate::count(lit(1_i64)), partition, vec![], whole())?,
            DuplicateKeep::First | DuplicateKeep::Last => over(
                window::row_number(),
                partition,
                vec![col(&position).sort(keep == DuplicateKeep::First, false)],
                whole(),
            )?,
        };
        let frame = frame
            .window(vec![marker_expr.alias(&marker)])
            .map_err(plan)?
            .filter(col(&marker).eq(lit(1_i64)))
            .map_err(plan)?
            .sort(vec![col(&position).sort(true, false)])
            .map_err(plan)?;
        self.rebuilt(
            frame,
            self.schema
                .fields()
                .iter()
                .map(|f| f.as_ref().clone())
                .collect(),
            vec![col(&position).sort(true, false)],
        )
    }
    pub(crate) fn column_set(
        &self,
        name: &str,
        series: &SeriesHandle,
    ) -> Result<RelationHandle, RelationError> {
        if name.trim().is_empty() || name.trim() != name {
            return Err(RelationError::InvalidInput);
        }
        let expression = self.series_expression(series)?;
        let mut fields = self
            .schema
            .fields()
            .iter()
            .map(|f| f.as_ref().clone())
            .collect::<Vec<_>>();
        let mut expressions = self.columns.clone();
        let replacement = crate::composition::derived_field(series.plan().field(), name);
        if let Ok(index) = self.schema.index_of(name) {
            fields[index] = replacement;
            expressions[index] = expression;
        } else {
            fields.push(replacement);
            expressions.push(expression);
        }
        self.project_expressions(fields, expressions)
    }
    pub(crate) fn masked(
        &self,
        mask: &SeriesHandle,
        drop_matches: bool,
    ) -> Result<RelationHandle, RelationError> {
        let mask_expression = self.binary_expression(mask)?;
        let position = self.unique_name("__yssbi_transform_position");
        let name = self.unique_name("__yssbi_filter_mask");
        let mut expressions = self
            .columns
            .iter()
            .zip(self.schema.fields())
            .map(|(expr, field)| expr.clone().alias(field.name()))
            .collect::<Vec<_>>();
        expressions.push(mask_expression.alias(&name));
        expressions.push(
            over(
                window::row_number(),
                vec![],
                self.domain_order.clone(),
                whole(),
            )?
            .alias(&position),
        );
        let frame = self.select_native(expressions)?;
        let predicate = if drop_matches {
            col(&name).is_not_true()
        } else {
            col(&name)
        };
        self.rebuilt(
            frame.filter(predicate).map_err(plan)?,
            self.schema
                .fields()
                .iter()
                .map(|f| f.as_ref().clone())
                .collect(),
            vec![col(&position).sort(true, false)],
        )
    }
    pub(crate) fn binary_expression(&self, series: &SeriesHandle) -> Result<Expr, RelationError> {
        let semantic = yss_database_arrow::column_semantic(series.plan().field())
            .map_err(|_| RelationError::InvalidInput)?;
        if semantic.kind != SemanticType::Binary {
            return Err(RelationError::InvalidInput);
        }
        let value = self.series_expression(series)?;
        if series.plan().field().data_type() == &DataType::Boolean {
            return Ok(if semantic.positive_value.as_deref() == Some("false") {
                !value
            } else {
                value
            });
        }
        let positive = semantic.positive_value.ok_or(RelationError::InvalidInput)?;
        Ok(value.eq(literal(
            &TabularScalar::String(positive.into()),
            series.plan().field().data_type(),
        )?))
    }
    pub(crate) fn chosen(
        &self,
        condition: &SeriesHandle,
        when_true: &SeriesOperand,
        when_false: &SeriesOperand,
        output_field: Option<&Field>,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        let candidate = [when_true, when_false].into_iter().find_map(|v| match v {
            SeriesOperand::Series(s) => Some(s.plan().field().clone()),
            _ => None,
        });
        let output = if let Some(field) = candidate.or_else(|| output_field.cloned()) {
            crate::composition::derived_field(&field, "result").with_nullable(true)
        } else {
            let values = [when_true, when_false]
                .map(|operand| match operand {
                    SeriesOperand::Scalar(v) => Ok(v),
                    _ => Err(RelationError::InvalidInput),
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            let (dtype, semantic) = if values.iter().any(|v| matches!(v, TabularScalar::String(_)))
            {
                (DataType::Utf8, SemanticType::Text)
            } else if values.iter().any(|v| matches!(v, TabularScalar::Bool(_))) {
                (DataType::Boolean, SemanticType::Binary)
            } else if values
                .iter()
                .any(|v| matches!(v, TabularScalar::Float64(_)))
            {
                (DataType::Float64, SemanticType::Numeric)
            } else if values
                .iter()
                .any(|v| matches!(v, TabularScalar::Integer(n) if *n < 0))
            {
                (DataType::Int64, SemanticType::Numeric)
            } else {
                (DataType::UInt64, SemanticType::Numeric)
            };
            field("result", dtype, semantic, true)?
        };
        Ok(Arc::new(DataFusionSeries {
            expression: when(
                self.binary_expression(condition)?,
                self.compatible_operand(when_true, &output)?,
            )
            .otherwise(self.compatible_operand(when_false, &output)?)
            .map_err(plan)?,
            field: Arc::new(output),
        }))
    }
    pub(crate) fn text_concatenated(
        &self,
        operands: &[SeriesOperand],
        separator: &str,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        if operands.len() < 2 {
            return Err(RelationError::InvalidInput);
        }
        let values = operands
            .iter()
            .map(|operand| match operand {
                SeriesOperand::Series(s) => {
                    if yss_database_arrow::column_semantic(s.plan().field())
                        .map_err(|_| RelationError::InvalidInput)?
                        .kind
                        != SemanticType::Text
                    {
                        return Err(RelationError::InvalidInput);
                    }
                    Ok(cast(self.series_expression(s)?, DataType::Utf8))
                }
                SeriesOperand::Scalar(TabularScalar::String(s)) => Ok(lit(s.as_ref())),
                SeriesOperand::Scalar(TabularScalar::Null) => null(&DataType::Utf8),
                _ => Err(RelationError::InvalidInput),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let missing = values
            .iter()
            .cloned()
            .map(Expr::is_null)
            .reduce(Expr::or)
            .ok_or(RelationError::InvalidInput)?;
        let expression = when(missing, null(&DataType::Utf8)?)
            .otherwise(scalar::concat_ws(lit(separator), values))
            .map_err(plan)?;
        Ok(Arc::new(DataFusionSeries {
            expression,
            field: Arc::new(field("result", DataType::Utf8, SemanticType::Text, true)?),
        }))
    }
    pub(crate) fn melted(&self, spec: &UnpivotSpec) -> Result<RelationHandle, RelationError> {
        let names = spec.keys.iter().chain(&spec.columns).collect::<Vec<_>>();
        if spec.columns.is_empty()
            || names.iter().collect::<BTreeSet<_>>().len() != names.len()
            || names.iter().any(|name| self.schema.index_of(name).is_err())
            || spec.variable_name.trim().is_empty()
            || spec.value_name.trim().is_empty()
            || spec.variable_name == spec.value_name
            || spec.keys.contains(&spec.variable_name)
            || spec.keys.contains(&spec.value_name)
        {
            return Err(RelationError::InvalidInput);
        }
        let value_field = self
            .schema
            .field_with_name(&spec.columns[0])
            .map_err(|_| RelationError::InvalidInput)?;
        let meaning = yss_database_arrow::column_semantic(value_field)
            .map_err(|_| RelationError::InvalidInput)?;
        if spec.columns.iter().any(|name| {
            self.schema.field_with_name(name).is_err_and(|_| true)
                || self.schema.field_with_name(name).is_ok_and(|f| {
                    f.data_type() != value_field.data_type()
                        || yss_database_arrow::column_semantic(f).ok().as_ref() != Some(&meaning)
                })
        }) {
            return Err(RelationError::InvalidInput);
        }
        let (source, position) = self.positioned()?;
        let branch = self.unique_name("__yssbi_unpivot_column");
        let mut result: Option<DataFrame> = None;
        for (index, column) in spec.columns.iter().enumerate() {
            let mut expressions = spec.keys.iter().map(|name| col(name)).collect::<Vec<_>>();
            expressions.extend([
                lit(column.as_ref()).alias(spec.variable_name.as_ref()),
                col(column).alias(spec.value_name.as_ref()),
                col(&position),
                lit(index as i64).alias(&branch),
            ]);
            let mut frame = source.clone().select(expressions).map_err(plan)?;
            if !spec.include_null {
                frame = frame
                    .filter(col(&spec.value_name).is_not_null())
                    .map_err(plan)?;
            }
            result = Some(match result {
                None => frame,
                Some(previous) => previous.union(frame).map_err(plan)?,
            });
        }
        let order = vec![
            col(&position).sort(true, false),
            col(&branch).sort(true, false),
        ];
        let frame = result
            .ok_or(RelationError::InvalidInput)?
            .sort(order.clone())
            .map_err(plan)?;
        let mut fields = spec
            .keys
            .iter()
            .map(|name| {
                self.schema
                    .field_with_name(name)
                    .cloned()
                    .map_err(|_| RelationError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        fields.push(field(
            &spec.variable_name,
            DataType::Utf8,
            SemanticType::Text,
            false,
        )?);
        fields.push(crate::composition::derived_field(
            value_field,
            &spec.value_name,
        ));
        self.rebuilt(frame, fields, order)
    }
    pub(crate) fn widened(&self, spec: &PivotSpec) -> Result<RelationHandle, RelationError> {
        let category = self
            .schema
            .field_with_name(&spec.category)
            .map_err(|_| RelationError::InvalidInput)?;
        let value = self
            .schema
            .field_with_name(&spec.value)
            .map_err(|_| RelationError::InvalidInput)?;
        validate_levels(&spec.levels)?;
        if spec.keys.iter().collect::<BTreeSet<_>>().len() != spec.keys.len()
            || spec.keys.iter().any(|key| {
                self.schema.index_of(key).is_err()
                    || spec.levels.iter().any(|level| &level.name == key)
            })
        {
            return Err(RelationError::InvalidInput);
        }
        if spec.aggregate != PivotAggregate::Count && !yss_database_arrow::is_numeric_field(value) {
            return Err(RelationError::InvalidInput);
        }
        let mut aggregates = Vec::new();
        for level in &spec.levels {
            let condition = if level.value == TabularScalar::Null {
                col(&spec.category).is_null()
            } else {
                col(&spec.category).eq(literal(&level.value, category.data_type())?)
            };
            let x = when(condition, col(&spec.value))
                .otherwise(null(value.data_type())?)
                .map_err(plan)?;
            let x = if spec.aggregate == PivotAggregate::Count {
                x
            } else {
                finite(x, value.data_type(), false)
            };
            aggregates.push(pivot_aggregate(spec.aggregate, x).alias(level.name.as_ref()));
        }
        let keys = spec.keys.iter().map(|key| col(key)).collect::<Vec<_>>();
        let mut frame = self
            .frame
            .clone()
            .aggregate(keys.clone(), aggregates)
            .map_err(plan)?;
        let order = keys
            .into_iter()
            .map(|expr| expr.sort(true, false))
            .collect::<Vec<_>>();
        if !order.is_empty() {
            frame = frame.sort(order.clone()).map_err(plan)?;
        }
        let mut fields = spec
            .keys
            .iter()
            .map(|key| {
                self.schema
                    .field_with_name(key)
                    .cloned()
                    .map_err(|_| RelationError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for level in &spec.levels {
            let native = frame
                .schema()
                .as_arrow()
                .field_with_name(&level.name)
                .map_err(|_| RelationError::InvalidPlan)?;
            fields.push(field(
                &level.name,
                native.data_type().clone(),
                SemanticType::Numeric,
                true,
            )?);
        }
        self.rebuilt(frame, fields, order)
    }
    pub(crate) fn resampled(&self, spec: &ResampleSpec) -> Result<RelationHandle, RelationError> {
        let time = self
            .schema
            .field_with_name(&spec.time)
            .map_err(|_| RelationError::InvalidInput)?;
        if yss_database_arrow::column_semantic(time)
            .map_err(|_| RelationError::InvalidInput)?
            .kind
            != SemanticType::Datetime
            || spec.keys.contains(&spec.time)
            || spec.keys.iter().collect::<BTreeSet<_>>().len() != spec.keys.len()
            || spec.columns.is_empty()
            || spec.columns.iter().collect::<BTreeSet<_>>().len() != spec.columns.len()
        {
            return Err(RelationError::InvalidInput);
        }
        let bucket =
            crate::series_transform::truncate_date(col(&spec.time), time.data_type(), &spec.unit)?;
        let input = self
            .frame
            .clone()
            .with_column(spec.time.as_ref(), bucket)
            .map_err(plan)?;
        let mut keys = vec![col(&spec.time)];
        keys.extend(spec.keys.iter().map(|key| col(key)));
        let suffix = aggregate_name(spec.aggregate);
        let mut names = spec.keys.iter().map(AsRef::as_ref).collect::<BTreeSet<_>>();
        names.insert(spec.time.as_ref());
        let mut aggregates = Vec::new();
        let mut output_names = Vec::new();
        for column in &spec.columns {
            let source = self
                .schema
                .field_with_name(column)
                .map_err(|_| RelationError::InvalidInput)?;
            if spec.aggregate != PivotAggregate::Count
                && !yss_database_arrow::is_numeric_field(source)
            {
                return Err(RelationError::InvalidInput);
            }
            let name = format!("{column}_{suffix}");
            if names.contains(name.as_str()) || output_names.contains(&name) {
                return Err(RelationError::InvalidInput);
            }
            let x = if spec.aggregate == PivotAggregate::Count {
                col(column)
            } else {
                finite(col(column), source.data_type(), false)
            };
            aggregates.push(pivot_aggregate(spec.aggregate, x).alias(&name));
            output_names.push(name);
        }
        let order = keys
            .iter()
            .cloned()
            .map(|key| key.sort(true, false))
            .collect::<Vec<_>>();
        let frame = input
            .aggregate(keys, aggregates)
            .map_err(plan)?
            .sort(order.clone())
            .map_err(plan)?;
        let mut fields = vec![time.clone()];
        fields.extend(
            spec.keys
                .iter()
                .map(|key| {
                    self.schema
                        .field_with_name(key)
                        .cloned()
                        .map_err(|_| RelationError::InvalidInput)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        for name in output_names {
            let native = frame
                .schema()
                .as_arrow()
                .field_with_name(&name)
                .map_err(|_| RelationError::InvalidPlan)?;
            fields.push(field(
                &name,
                native.data_type().clone(),
                SemanticType::Numeric,
                true,
            )?);
        }
        self.rebuilt(frame, fields, order)
    }
    pub(crate) fn encoded(
        &self,
        series: &SeriesHandle,
        levels: &[PivotLevel],
        reference: Option<&str>,
    ) -> Result<RelationHandle, RelationError> {
        validate_levels(levels)?;
        let source = series.plan().field();
        if !matches!(
            yss_database_arrow::column_semantic(source)
                .map_err(|_| RelationError::InvalidInput)?
                .kind,
            SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
        ) {
            return Err(RelationError::InvalidInput);
        }
        if reference
            .is_some_and(|reference| !levels.iter().any(|level| level.name.as_ref() == reference))
        {
            return Err(RelationError::InvalidInput);
        }
        let value = self.series_expression(series)?;
        let mut fields = Vec::new();
        let mut expressions = Vec::new();
        for level in levels {
            if reference == Some(level.name.as_ref()) {
                continue;
            }
            let result = if level.value == TabularScalar::Null {
                value.clone().is_null()
            } else {
                value.clone().eq(literal(&level.value, source.data_type())?)
            };
            expressions.push(result);
            fields.push(field(
                &level.name,
                DataType::Boolean,
                SemanticType::Binary,
                level.value != TabularScalar::Null && source.is_nullable(),
            )?);
        }
        if fields.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        self.project_expressions(fields, expressions)
    }
}

fn validate_levels(levels: &[PivotLevel]) -> Result<(), RelationError> {
    if levels.is_empty()
        || levels
            .iter()
            .any(|l| l.name.trim().is_empty() || l.name.trim() != l.name.as_ref())
        || levels
            .iter()
            .map(|l| &l.name)
            .collect::<BTreeSet<_>>()
            .len()
            != levels.len()
        || levels
            .iter()
            .enumerate()
            .any(|(i, l)| levels[..i].iter().any(|p| p.value == l.value))
    {
        Err(RelationError::InvalidInput)
    } else {
        Ok(())
    }
}
pub(crate) fn aggregate_name(operation: PivotAggregate) -> &'static str {
    match operation {
        PivotAggregate::Sum => "sum",
        PivotAggregate::Mean => "mean",
        PivotAggregate::Min => "min",
        PivotAggregate::Max => "max",
        PivotAggregate::Count => "count",
    }
}
fn pivot_aggregate(operation: PivotAggregate, value: Expr) -> Expr {
    match operation {
        PivotAggregate::Sum => aggregate::sum(value),
        PivotAggregate::Mean => aggregate::avg(value),
        PivotAggregate::Min => aggregate::min(value),
        PivotAggregate::Max => aggregate::max(value),
        PivotAggregate::Count => aggregate::count(value),
    }
}
