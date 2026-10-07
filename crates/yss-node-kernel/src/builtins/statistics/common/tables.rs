//! Columnar conversion of finite or explicitly absent numbers to a declared relation.
use crate::{KernelError, KernelInvocation, RuntimeValue};

pub(in crate::builtins::statistics) fn numeric_table<T, V: Into<Option<f64>>, const N: usize>(
    rows: &[T],
    output_index: usize,
    names: [&str; N],
    project: impl Fn(&T) -> [V; N],
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    scalar_table(
        rows,
        output_index,
        names,
        |row| {
            let mut values = std::array::from_fn(|_| {
                RuntimeValue::Scalar(yss_data_contract::TabularScalar::Null)
            });
            for (slot, value) in values.iter_mut().zip(project(row)) {
                if let Some(value) = value.into() {
                    *slot =
                        RuntimeValue::float64(value).map_err(|_| KernelError::NonFiniteResult)?;
                }
            }
            Ok(values)
        },
        inv,
    )
}

pub(in crate::builtins::statistics) fn scalar_table<T, const N: usize>(
    rows: &[T],
    output_index: usize,
    names: [&str; N],
    project: impl Fn(&T) -> Result<[RuntimeValue; N], KernelError>,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let fields = inv
        .outputs
        .get(output_index)
        .and_then(|o| o.fields.as_deref())
        .ok_or(KernelError::ShapeMismatch)?;
    if fields.len() != N
        || fields
            .iter()
            .zip(names)
            .any(|(f, name)| f.name.as_ref() != name)
    {
        return Err(KernelError::ShapeMismatch);
    }
    let mut columns: [Vec<RuntimeValue>; N] =
        std::array::from_fn(|_| Vec::with_capacity(rows.len()));
    for (i, row) in rows.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        for (column, value) in columns.iter_mut().zip(project(row)?) {
            column.push(value);
        }
    }
    let columns = columns.map(|c| RuntimeValue::List(c.into()));
    crate::builtins::relational::materialize(fields, &columns.iter().collect::<Vec<_>>(), inv)
}

pub(in crate::builtins::statistics) fn matrix_table(
    data: Vec<Vec<f64>>,
    output: usize,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let fields = inv
        .outputs
        .get(output)
        .and_then(|o| o.fields.as_deref())
        .ok_or(KernelError::OutputContractMismatch)?;
    if fields.is_empty() || data.iter().any(|row| row.len() != fields.len()) {
        return Err(KernelError::OutputContractMismatch);
    }
    inv.control.check_bytes(
        data.len()
            .checked_mul(fields.len())
            .and_then(|n| n.checked_mul(size_of::<RuntimeValue>() * 8)),
    )?;
    let mut values = Vec::with_capacity(fields.len());
    for column in 0..fields.len() {
        let mut data_column = inv.control.reserve(data.len())?;
        for (i, row) in data.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            data_column.push(row[column]);
        }
        values.push(super::numeric_list(&data_column, inv)?);
    }
    crate::builtins::relational::materialize(fields, &values.iter().collect::<Vec<_>>(), inv)
}
