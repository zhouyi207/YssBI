//! Positional values at the boundary between materialized computation and lazy tables.
use super::{Column, column_retaining, columns};
use crate::builtins::relational::kernel_error;
use crate::{KernelError, KernelInvocation, RuntimeValue};
use arrow_array::RecordBatch;
use arrow_schema::Schema;
use std::{borrow::Cow, sync::Arc};
use yss_data_contract::TabularScalar;
use yss_relational_contract::{RelationError, RelationHandle, SeriesHandle};

/// Keep a shared lazy expression when possible; otherwise read each operand in its own order.
pub(in crate::builtins) fn prepare<'a>(
    inputs: &[&'a RuntimeValue],
    inv: &KernelInvocation<'_>,
) -> Result<Vec<Cow<'a, RuntimeValue>>, KernelError> {
    inv.check_control()?;
    let handles = inputs
        .iter()
        .filter_map(|v| match v.unannotated() {
            RuntimeValue::Series(handle) => Some(handle),
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some(first) = handles.first() else {
        return Ok(inputs.iter().map(|v| Cow::Borrowed(*v)).collect());
    };
    if !inputs
        .iter()
        .any(|v| matches!(v.unannotated(), RuntimeValue::List(_)))
        && handles
            .iter()
            .all(|h| first.relation().shares_row_domain(h.relation()))
    {
        return Ok(inputs.iter().map(|v| Cow::Borrowed(*v)).collect());
    }
    let mut retained = 0usize;
    for value in inputs {
        if let RuntimeValue::List(values) = value.unannotated() {
            retained = inv.control.check_bytes(
                values
                    .len()
                    .checked_mul(size_of::<RuntimeValue>() * 3)
                    .and_then(|n| retained.checked_add(n)),
            )?;
            for (i, value) in values.iter().enumerate() {
                if i.is_multiple_of(1024) {
                    inv.check_control()?;
                }
                if let RuntimeValue::Scalar(TabularScalar::String(value)) = value.unannotated() {
                    retained = inv.control.check_bytes(retained.checked_add(value.len()))?;
                }
            }
        }
    }
    let mut output = Vec::with_capacity(inputs.len());
    let mut rows = None;
    for value in inputs {
        let value = if matches!(value.unannotated(), RuntimeValue::Series(_)) {
            let column = column_retaining(value, inv, retained)?;
            retained = inv
                .control
                .check_bytes(column.bytes().and_then(|n| retained.checked_add(n)))?;
            Cow::Owned(column.into_value(inv)?)
        } else {
            Cow::Borrowed(*value)
        };
        if let RuntimeValue::List(values) = value.unannotated() {
            if rows.is_some_and(|rows| rows != values.len()) {
                return Err(KernelError::ShapeMismatch);
            }
            rows = Some(values.len());
        }
        output.push(value);
    }
    inv.check_control()?;
    Ok(output)
}

/// Put equal-length values in one temporary table so existing transforms share one coordinate.
pub(in crate::builtins) fn relation(
    inputs: &[&RuntimeValue],
    inv: &KernelInvocation<'_>,
) -> Result<RelationHandle, KernelError> {
    let columns = columns(inputs, inv, true)?;
    let mut fields = Vec::with_capacity(columns.len());
    let mut arrays = Vec::with_capacity(columns.len());
    for (i, column) in columns.iter().enumerate() {
        inv.check_control()?;
        let (field, array) = yss_database_arrow::materialized_column(
            &format!("value_{i}"),
            &column.values,
            column.metadata.as_ref(),
        )
        .map_err(|_| KernelError::InvalidParameter)?;
        fields.push(field);
        arrays.push(array);
    }
    let batch = RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays)
        .map_err(|_| KernelError::ShapeMismatch)?;
    inv.relations
        .clone()
        .materialize(batch, &inv.relation_control())
        .map_err(kernel_error)
}

/// Append an independent operand to a frozen table by position, preserving the table's Arrow types.
/// The caller removes the last, temporary column after applying its operation.
pub(in crate::builtins) fn attach(
    source: &RelationHandle,
    operand: &RuntimeValue,
    inv: &KernelInvocation<'_>,
) -> Result<(RelationHandle, SeriesHandle), KernelError> {
    let column: Column = column_retaining(operand, inv, 0)?;
    let mut retained = inv.control.check_bytes(column.bytes())?;
    let mut control = inv.relation_control();
    control.max_input_bytes -= retained;
    let mut batches = Vec::new();
    let mut rows = 0usize;
    source
        .visit_batches(&control, &mut |batch| {
            control.check()?;
            rows = rows
                .checked_add(batch.num_rows())
                .ok_or(RelationError::MemoryLimitExceeded)?;
            retained = batch
                .get_array_memory_size()
                .checked_mul(3)
                .and_then(|n| retained.checked_add(n))
                .filter(|n| *n <= inv.control.max_input_bytes)
                .ok_or(RelationError::MemoryLimitExceeded)?;
            batches
                .try_reserve(1)
                .map_err(|_| RelationError::MemoryLimitExceeded)?;
            batches.push(batch);
            Ok(())
        })
        .map_err(kernel_error)?;
    if rows != column.values.len() {
        return Err(KernelError::ShapeMismatch);
    }
    inv.check_control()?;
    let schema = batches
        .first()
        .map_or_else(|| source.schema(), RecordBatch::schema);
    let batch = arrow_select::concat::concat_batches(&schema, &batches)
        .map_err(|_| KernelError::InvalidParameter)?;
    let mut name = "__operand".to_owned();
    while schema.index_of(&name).is_ok() {
        name.push('_');
    }
    let (field, array) =
        yss_database_arrow::materialized_column(&name, &column.values, column.metadata.as_ref())
            .map_err(|_| KernelError::InvalidParameter)?;
    let mut fields = schema.fields().iter().cloned().collect::<Vec<_>>();
    fields.push(Arc::new(field));
    let mut arrays = batch.columns().to_vec();
    arrays.push(array);
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new_with_metadata(fields, schema.metadata().clone())),
        arrays,
    )
    .map_err(|_| KernelError::ShapeMismatch)?;
    inv.check_control()?;
    let relation = inv
        .relations
        .clone()
        .materialize(batch, &inv.relation_control())
        .map_err(kernel_error)?;
    let series = relation.select_series(&name).map_err(kernel_error)?;
    Ok((relation, series))
}
