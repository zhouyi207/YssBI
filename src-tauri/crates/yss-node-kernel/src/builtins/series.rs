//! Controlled Arrow preparation for scientific consumers. Data transforms live in transforms.
use super::relational::kernel_error;
use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_data_contract::{ConversionMetadata, TabularScalar};
use yss_relational_contract::{RelationError, SeriesHandle};

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
    pub(super) metadata: Option<ConversionMetadata>,
}

fn metadata(field: &arrow_schema::Field) -> Result<ConversionMetadata, KernelError> {
    Ok(ConversionMetadata {
        semantic: yss_database_arrow::column_semantic(field)
            .map_err(|_| KernelError::InvalidParameter)?,
        temporal: yss_database_arrow::temporal_metadata(field.data_type()),
        dummy_base_level: field.metadata().get("yssbi.dummy_base_level").cloned(),
    })
}

pub(super) fn load(
    handles: &[SeriesHandle],
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<Column>, KernelError> {
    let first = handles.first().ok_or(KernelError::InvalidParameter)?;
    // Joint projection proves alignment; independent same-length relations are not interchangeable.
    let relation = first
        .relation()
        .project_series(handles)
        .map_err(kernel_error)?;
    let mut columns = handles
        .iter()
        .map(|handle| {
            Ok(Column {
                values: Vec::new(),
                metadata: Some(metadata(handle.plan().field())?),
            })
        })
        .collect::<Result<Vec<_>, KernelError>>()?;
    let mut retained = 0usize;
    relation
        .visit_batches(&invocation.relation_control(), &mut |batch| {
            invocation.relation_control().check()?;
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

pub(super) fn column(
    value: &RuntimeValue,
    invocation: &KernelInvocation<'_>,
) -> Result<Column, KernelError> {
    if let RuntimeValue::Series(handle) = value.unannotated() {
        return Ok(load(std::slice::from_ref(handle), invocation)?.remove(0));
    }
    let RuntimeValue::List(values) = value.unannotated() else {
        return Err(KernelError::InvalidParameter);
    };
    let mut bytes = invocation
        .control
        .check_bytes(values.len().checked_mul(size_of::<RuntimeValue>() * 4))?;
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
        metadata: value.metadata().cloned(),
    })
}
