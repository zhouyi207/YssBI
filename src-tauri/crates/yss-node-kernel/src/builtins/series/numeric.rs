//! Numeric materialization retains the compact f64 path used by scientific consumers.
use crate::builtins::{numeric_input, relational::kernel_error};
use crate::{KernelError, KernelInvocation, RuntimeValue};

pub(in crate::builtins) fn columns(
    values: &[&RuntimeValue],
    inv: &KernelInvocation<'_>,
    retained: usize,
) -> Result<Vec<Vec<f64>>, KernelError> {
    read(values, inv, retained, true)
}

pub(in crate::builtins) fn independent_columns(
    values: &[&RuntimeValue],
    inv: &KernelInvocation<'_>,
    retained: usize,
) -> Result<Vec<Vec<f64>>, KernelError> {
    read(values, inv, retained, false)
}

fn read(
    values: &[&RuntimeValue],
    inv: &KernelInvocation<'_>,
    mut retained: usize,
    paired: bool,
) -> Result<Vec<Vec<f64>>, KernelError> {
    inv.check_control()?;
    inv.control.check_bytes(Some(retained))?;
    let first = values.first().ok_or(KernelError::InvalidNumericInput)?;
    let mut known_rows = None;
    let mut known_bytes = retained;
    for value in values {
        if let RuntimeValue::List(values) = value.unannotated() {
            if paired && known_rows.is_some_and(|rows| rows != values.len()) {
                return Err(KernelError::ShapeMismatch);
            }
            known_rows = Some(values.len());
            known_bytes = inv.control.check_bytes(
                values
                    .len()
                    .checked_mul(size_of::<f64>())
                    .and_then(|n| known_bytes.checked_add(n)),
            )?;
        }
    }
    if let RuntimeValue::Series(first) = first.unannotated()
        && values.iter().all(|v| matches!(v.unannotated(), RuntimeValue::Series(s) if first.relation().shares_row_domain(s.relation())))
    {
        let handles = values.iter().map(|v| match v.unannotated() {
            RuntimeValue::Series(s) => s.clone(), _ => unreachable!(),
        }).collect::<Vec<_>>();
        let mut control = inv.relation_control();
        control.max_input_bytes -= retained;
        return first.relation().numeric_columns(&handles, &control).map_err(kernel_error);
    }
    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(values.len());
    for value in values {
        inv.check_control()?;
        let column = match value.unannotated() {
            RuntimeValue::Series(handle) => {
                let mut control = inv.relation_control();
                control.max_input_bytes -= retained;
                handle
                    .relation()
                    .numeric_columns(std::slice::from_ref(handle), &control)
                    .map_err(kernel_error)?
                    .remove(0)
            }
            RuntimeValue::List(values) => {
                inv.control.check_bytes(
                    values
                        .len()
                        .checked_mul(size_of::<f64>())
                        .and_then(|n| retained.checked_add(n)),
                )?;
                let mut column = inv.control.reserve(values.len())?;
                for (i, value) in values.iter().enumerate() {
                    if i.is_multiple_of(1024) {
                        inv.check_control()?;
                    }
                    column.push(numeric_input(Some(value.unannotated()))?);
                }
                column
            }
            _ => return Err(KernelError::InvalidNumericInput),
        };
        if paired
            && columns
                .first()
                .is_some_and(|first| first.len() != column.len())
        {
            return Err(KernelError::ShapeMismatch);
        }
        retained = inv.control.check_bytes(
            column
                .len()
                .checked_mul(size_of::<f64>())
                .and_then(|n| retained.checked_add(n)),
        )?;
        columns.push(column);
    }
    inv.check_control()?;
    Ok(columns)
}
