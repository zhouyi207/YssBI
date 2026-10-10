//! Controlled Arrow preparation for materialized consumers. Data transforms live in transforms.
use super::relational::kernel_error;
use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_data_contract::{ConversionMetadata, TabularScalar};
use yss_relational_contract::{RelationError, SeriesHandle};

mod positional;
pub(super) use positional::{attach, prepare, relation};
mod numeric;
pub(super) use numeric::{
    columns as numeric_columns, independent_columns as independent_numeric_columns,
};

pub(crate) const INPUT_PREPARATION_REVISION: u32 = 1;

// Covers decoded vectors/strings, validation indexes and temporary Arrow field copies.
// This is conservative workspace admission, not a process RSS measurement.
const METADATA_STORAGE_MULTIPLIER: usize = 32;

#[derive(Clone, Copy)]
pub(crate) enum SeriesKernel {
    Range,
    Length,
    Count,
    Sum,
    Mean,
    Standardize,
    InverseStandardize,
    Dummy,
    Difference,
    PercentChange,
    RollingMean,
    Lag,
    PanelDifference,
}

pub(super) struct Column {
    pub(super) values: Vec<TabularScalar>,
    pub(super) metadata: Option<std::sync::Arc<ConversionMetadata>>,
    pub(super) metadata_bytes: usize,
}

impl Column {
    pub(in crate::builtins) fn bytes(&self) -> Option<usize> {
        self.values.iter().try_fold(
            self.values
                .len()
                .checked_mul(size_of::<RuntimeValue>() * 4)?
                .checked_add(self.metadata_bytes)?,
            |bytes, value| match value {
                TabularScalar::String(value) => bytes.checked_add(value.len().checked_mul(4)?),
                _ => Some(bytes),
            },
        )
    }

    fn into_value(self, invocation: &KernelInvocation<'_>) -> Result<RuntimeValue, KernelError> {
        let mut values = invocation.control.reserve(self.values.len())?;
        for (i, value) in self.values.into_iter().enumerate() {
            if i.is_multiple_of(1024) {
                invocation.check_control()?;
            }
            values.push(RuntimeValue::Scalar(value));
        }
        let value = RuntimeValue::List(values.into());
        match self.metadata {
            Some(metadata) => value
                .with_metadata(metadata)
                .map_err(|_| KernelError::InvalidParameter),
            None => Ok(value),
        }
    }
}

fn metadata(
    field: &arrow_schema::Field,
    invocation: &KernelInvocation<'_>,
    retained: usize,
) -> Result<(ConversionMetadata, usize), KernelError> {
    let mut bytes = size_of::<ConversionMetadata>() * 4;
    for (position, (key, value)) in field.metadata().iter().enumerate() {
        if position.is_multiple_of(1024) {
            invocation.check_control()?;
        }
        bytes = invocation.control.check_bytes(
            key.len()
                .checked_add(value.len())
                .and_then(|n| n.checked_add(size_of::<(String, String)>()))
                .and_then(|n| n.checked_mul(METADATA_STORAGE_MULTIPLIER))
                .and_then(|n| bytes.checked_add(n)),
        )?;
    }
    invocation
        .control
        .check_bytes(retained.checked_add(bytes))?;
    let metadata = ConversionMetadata {
        semantic: yss_database_arrow::column_semantic(field)
            .map_err(|_| KernelError::InvalidParameter)?,
        temporal: yss_database_arrow::temporal_metadata(field.data_type()),
        dummy_base_level: field.metadata().get("yssbi.dummy_base_level").cloned(),
    };
    invocation.check_control()?;
    Ok((metadata, bytes))
}

pub(super) fn load(
    handles: &[SeriesHandle],
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<Column>, KernelError> {
    load_with_retained_bytes(handles, invocation, 0)
}

pub(super) fn load_with_retained_bytes(
    handles: &[SeriesHandle],
    invocation: &KernelInvocation<'_>,
    retained_bytes: usize,
) -> Result<Vec<Column>, KernelError> {
    invocation.control.check_bytes(Some(retained_bytes))?;
    let first = handles.first().ok_or(KernelError::InvalidParameter)?;
    if handles
        .iter()
        .any(|h| !first.relation().shares_row_domain(h.relation()))
    {
        let mut retained = retained_bytes;
        let mut columns: Vec<Column> = Vec::with_capacity(handles.len());
        for handle in handles {
            let column =
                load_with_retained_bytes(std::slice::from_ref(handle), invocation, retained)?
                    .remove(0);
            if columns
                .first()
                .is_some_and(|first| first.values.len() != column.values.len())
            {
                return Err(KernelError::ShapeMismatch);
            }
            retained = invocation
                .control
                .check_bytes(column.bytes().and_then(|n| retained.checked_add(n)))?;
            columns.push(column);
        }
        return Ok(columns);
    }
    let mut retained = retained_bytes;
    let mut columns = invocation.control.reserve(handles.len())?;
    for handle in handles {
        let (metadata, metadata_bytes) = metadata(handle.plan().field(), invocation, retained)?;
        retained = invocation
            .control
            .check_bytes(retained.checked_add(metadata_bytes))?;
        columns.push(Column {
            values: Vec::new(),
            metadata: Some(std::sync::Arc::new(metadata)),
            metadata_bytes,
        });
    }
    // Shared domains can be read in one projection; other inputs pair by current position.
    let relation = first
        .relation()
        .project_series(handles)
        .map_err(kernel_error)?;
    let mut control = invocation.relation_control();
    control.max_input_bytes -= retained;
    relation
        .visit_batches(&control, &mut |batch| {
            control.check()?;
            let estimate = batch
                .num_rows()
                .checked_mul(columns.len())
                .and_then(|n| n.checked_mul(size_of::<RuntimeValue>() * 4))
                .and_then(|n| n.checked_add(batch.get_array_memory_size().saturating_mul(4)))
                .and_then(|n| retained.checked_add(n))
                .filter(|n| *n <= invocation.control.max_input_bytes)
                .ok_or(RelationError::MemoryLimitExceeded)?;
            retained = estimate;
            for (column, array) in columns.iter_mut().zip(batch.columns()) {
                let values = yss_database_arrow::materialized_values(array.as_ref())
                    .map_err(|_| RelationError::InvalidInput)?;
                column
                    .values
                    .try_reserve(values.len())
                    .map_err(|_| RelationError::MemoryLimitExceeded)?;
                column.values.extend(values);
            }
            Ok(())
        })
        .map_err(kernel_error)?;
    Ok(columns)
}

pub(in crate::builtins) fn column_retaining(
    value: &RuntimeValue,
    invocation: &KernelInvocation<'_>,
    retained: usize,
) -> Result<Column, KernelError> {
    if let RuntimeValue::Series(handle) = value.unannotated() {
        return Ok(
            load_with_retained_bytes(std::slice::from_ref(handle), invocation, retained)?.remove(0),
        );
    }
    let RuntimeValue::List(values) = value.unannotated() else {
        return Err(KernelError::InvalidParameter);
    };
    let mut bytes = invocation.control.check_bytes(
        values
            .len()
            .checked_mul(size_of::<RuntimeValue>() * 4)
            .and_then(|n| retained.checked_add(n)),
    )?;
    let mut output = invocation.control.reserve(values.len())?;
    for (index, value) in values.iter().enumerate() {
        if index % 1024 == 0 {
            invocation.check_control()?;
        }
        let scalar = value
            .tabular_scalar()
            .map_err(|_| KernelError::InvalidParameter)?;
        if let TabularScalar::String(value) = &scalar {
            bytes = invocation.control.check_bytes(
                value
                    .len()
                    .checked_mul(4)
                    .and_then(|n| bytes.checked_add(n)),
            )?;
        }
        output.push(scalar);
    }
    Ok(Column {
        values: output,
        metadata: value.shared_metadata().cloned(),
        // Caller-owned annotations are shared; this path performs no metadata decoding.
        metadata_bytes: 0,
    })
}

/// Read paired observations or independent samples without imposing database provenance.
pub(super) fn columns(
    inputs: &[&RuntimeValue],
    invocation: &KernelInvocation<'_>,
    paired: bool,
) -> Result<Vec<Column>, KernelError> {
    if paired
        && !inputs.is_empty()
        && inputs
            .iter()
            .all(|v| matches!(v.unannotated(), RuntimeValue::Series(_)))
    {
        let handles = inputs
            .iter()
            .map(|v| match v.unannotated() {
                RuntimeValue::Series(handle) => handle.clone(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return load(&handles, invocation);
    }
    let mut retained = 0usize;
    let mut columns: Vec<Column> = Vec::with_capacity(inputs.len());
    for input in inputs {
        let column = column_retaining(input, invocation, retained)?;
        if paired
            && columns
                .first()
                .is_some_and(|first| first.values.len() != column.values.len())
        {
            return Err(KernelError::ShapeMismatch);
        }
        retained = invocation
            .control
            .check_bytes(column.bytes().and_then(|n| retained.checked_add(n)))?;
        columns.push(column);
    }
    invocation.check_control()?;
    Ok(columns)
}
