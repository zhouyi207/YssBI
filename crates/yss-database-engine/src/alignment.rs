//! Native range/unnest and joins complete time grids without collecting input columns.
use crate::{
    DataFusionRuntime,
    relation::DataFusionRelation,
    series_transform::{checked, col, field, lossless, over, plan, whole},
};
use arrow::datatypes::{DataType, Field};
use datafusion::{
    common::Column,
    dataframe::DataFrame,
    functions_aggregate::expr_fn as aggregate,
    functions_nested::expr_fn::range,
    functions_window::expr_fn as window,
    logical_expr::{Expr, JoinType, expr_fn::cast},
    prelude::lit,
};
use std::sync::Arc;
use yss_data_contract::SemanticType;
use yss_relational_contract::*;

fn qualified(alias: &str, name: &str) -> Expr {
    Expr::Column(Column::new(Some(alias.to_owned()), name.to_owned()))
}
fn decimal(expr: Expr) -> Expr {
    cast(expr, DataType::Decimal128(38, 0))
}
fn append(frame: DataFrame, name: &str, expression: Expr) -> Result<DataFrame, RelationError> {
    let mut columns = frame
        .schema()
        .columns()
        .into_iter()
        .map(Expr::Column)
        .collect::<Vec<_>>();
    columns.push(expression.alias(name));
    crate::windows::select(frame, columns, &[])
}

impl DataFusionRuntime {
    pub(crate) fn range_relation(
        self: Arc<Self>,
        start: i64,
        end: i64,
        step: i64,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        control.check()?;
        if step == 0 {
            return Err(RelationError::InvalidInput);
        }
        let distance = if step > 0 {
            i128::from(end) - i128::from(start)
        } else {
            i128::from(start) - i128::from(end)
        };
        let length = if distance <= 0 {
            0
        } else {
            (distance - 1) / i128::from(step).abs() + 1
        };
        if length > (control.max_input_bytes / 64) as i128 {
            return Err(RelationError::MemoryLimitExceeded);
        }
        self.positional_relation(
            field("value", DataType::Int64, SemanticType::Numeric, false)?,
            cast(
                decimal(col(crate::literal::POSITION)) * decimal(lit(step)) + decimal(lit(start)),
                DataType::Int64,
            ),
            usize::try_from(length).map_err(|_| RelationError::MemoryLimitExceeded)?,
        )
    }
}

impl DataFusionRelation {
    pub(crate) fn aligned(
        &self,
        time: &str,
        entity: Option<&str>,
        interval: i64,
        max_bytes: usize,
    ) -> Result<RelationHandle, RelationError> {
        if interval <= 0 || entity == Some(time) {
            return Err(RelationError::InvalidInput);
        }
        let time_field = self
            .schema
            .field_with_name(time)
            .map_err(|_| RelationError::InvalidInput)?;
        if !matches!(
            time_field.data_type(),
            DataType::Int64 | DataType::UInt64 | DataType::Float64 | DataType::Date32
        ) {
            return Err(RelationError::InvalidInput);
        }
        if let Some(entity) = entity {
            self.schema
                .field_with_name(entity)
                .map_err(|_| RelationError::InvalidInput)?;
        }
        let (mut source, position) = self.positioned()?;
        let number = self.unique_name("__yssbi_grid_time_number");
        let ordinal = self.unique_name("__yssbi_grid_ordinal");
        let low = self.unique_name("__yssbi_grid_low");
        let high = self.unique_name("__yssbi_grid_high");
        let count_name = self.unique_name("__yssbi_grid_count");
        let index = self.unique_name("__yssbi_grid_index");
        let total = self.unique_name("__yssbi_grid_total");
        let first = self.unique_name("__yssbi_grid_first");
        let time_name = self.unique_name("__yssbi_grid_time");
        let entity_name = self.unique_name("__yssbi_grid_entity");
        let rows_limit = i64::try_from(max_bytes / (self.schema.fields().len().max(1) * 64))
            .map_err(|_| RelationError::MemoryLimitExceeded)?;
        let mut keys = vec![col(time)];
        if let Some(entity) = entity {
            keys.push(col(entity));
        }
        let valid_keys = keys
            .iter()
            .cloned()
            .map(Expr::is_not_null)
            .reduce(Expr::and)
            .ok_or(RelationError::InvalidInput)?
            .and(over(aggregate::count(lit(1_i64)), keys, vec![], whole())?.eq(lit(1_i64)));
        let time_value = lossless(col(time), time_field.data_type(), &DataType::Int64);
        source = append(
            source,
            &number,
            checked(
                time_value,
                valid_keys,
                &DataType::Int64,
                RelationError::InvalidInput,
            ),
        )?;
        if entity.is_some() {
            source = append(
                source,
                &ordinal,
                cast(
                    over(
                        window::dense_rank(),
                        vec![],
                        vec![col(&number).sort(true, false)],
                        whole(),
                    )?,
                    DataType::Int64,
                ) - lit(1_i64),
            )?;
        }
        let grid_key = if entity.is_some() { &ordinal } else { &number };
        let partition = entity.into_iter().map(col).collect::<Vec<_>>();
        let minimum = over(
            aggregate::min(col(grid_key)),
            partition.clone(),
            vec![],
            whole(),
        )?;
        let on_grid = ((decimal(col(grid_key)) - decimal(minimum)) % decimal(lit(interval)))
            .eq(decimal(lit(0_i64)));
        let validated = self.unique_name("__yssbi_grid_validated");
        source = append(
            source,
            &validated,
            checked(
                col(grid_key),
                on_grid,
                &DataType::Int64,
                RelationError::InvalidInput,
            ),
        )?;
        let mut bounds = source
            .clone()
            .aggregate(
                partition,
                vec![
                    aggregate::min(col(&validated)).alias(&low),
                    aggregate::max(col(&validated)).alias(&high),
                    aggregate::min(col(&position)).alias(&first),
                ],
            )
            .map_err(plan)?;
        let count = cast(
            (decimal(col(&high)) - decimal(col(&low))) / decimal(lit(interval))
                + decimal(lit(1_i64)),
            DataType::Int64,
        );
        // A null bound represents empty input; propagate an empty grid.
        bounds = bounds
            .with_column(
                &count_name,
                datafusion::functions::expr_fn::coalesce(vec![count, lit(0_i64)]),
            )
            .map_err(plan)?;
        let totals = bounds
            .clone()
            .aggregate(
                vec![],
                vec![aggregate::sum(decimal(col(&count_name))).alias(&total)],
            )
            .map_err(plan)?;
        bounds = bounds
            .join_on(totals, JoinType::Inner, [lit(true)])
            .map_err(plan)?;
        let allowed = col(&total)
            .lt_eq(decimal(lit(rows_limit)))
            .and(col(&count_name).gt_eq(lit(0_i64)));
        bounds = bounds
            .with_column(
                &index,
                range(
                    lit(0_i64),
                    checked(
                        col(&count_name),
                        allowed,
                        &DataType::Int64,
                        RelationError::MemoryLimitExceeded,
                    ),
                    lit(1_i64),
                ),
            )
            .map_err(plan)?
            .unnest_columns(&[&index])
            .map_err(plan)?;
        let coordinate = cast(
            decimal(col(&low)) + decimal(col(&index)) * decimal(lit(interval)),
            DataType::Int64,
        );
        let mut grid = if entity.is_some() {
            let levels = source
                .clone()
                .select(vec![col(&ordinal), col(time).alias(&time_name)])
                .map_err(plan)?
                .distinct()
                .map_err(plan)?;
            bounds = bounds.with_column(&ordinal, coordinate).map_err(plan)?;
            bounds
                .alias("g")
                .map_err(plan)?
                .join_on(
                    levels.alias("t").map_err(plan)?,
                    JoinType::Inner,
                    [qualified("g", &ordinal).eq(qualified("t", &ordinal))],
                )
                .map_err(plan)?
        } else {
            bounds
                .with_column(
                    &time_name,
                    lossless(coordinate, &DataType::Int64, time_field.data_type()),
                )
                .map_err(plan)?
        };
        if let Some(entity) = entity {
            grid = grid.with_column(&entity_name, col(entity)).map_err(plan)?;
        }
        let mut expressions = vec![col(&time_name), col(&index), col(&first)];
        if entity.is_some() {
            expressions.push(col(&entity_name));
        }
        grid = grid
            .select(expressions)
            .map_err(plan)?
            .alias("grid")
            .map_err(plan)?;
        let source = source.alias("source").map_err(plan)?;
        let mut on = vec![qualified("grid", &time_name).eq(qualified("source", time))];
        if let Some(entity) = entity {
            on.push(qualified("grid", &entity_name).eq(qualified("source", entity)));
        }
        let joined = grid.join_on(source, JoinType::Left, on).map_err(plan)?;
        let mut expressions = self
            .schema
            .fields()
            .iter()
            .map(|f| {
                if f.name() == time {
                    qualified("grid", &time_name).alias(time)
                } else if Some(f.name().as_str()) == entity {
                    qualified("grid", &entity_name).alias(f.name())
                } else {
                    qualified("source", f.name()).alias(f.name())
                }
            })
            .collect::<Vec<_>>();
        expressions.push(qualified("grid", &index).alias(&index));
        expressions.push(qualified("grid", &first).alias(&first));
        let order = if entity.is_some() {
            vec![col(&first).sort(true, false), col(&index).sort(true, false)]
        } else {
            vec![col(&index).sort(true, false)]
        };
        let frame = joined
            .select(expressions)
            .map_err(plan)?
            .sort(order.clone())
            .map_err(plan)?;
        let fields = self
            .schema
            .fields()
            .iter()
            .map(|f| {
                f.as_ref()
                    .clone()
                    .with_nullable(f.name() != time && Some(f.name().as_str()) != entity)
            })
            .collect::<Vec<Field>>();
        self.rebuilt(frame, fields, order)
    }
}
