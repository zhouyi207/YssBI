//! Series operations consume controlled batches only during invocation. Standardization
//! retains relation row domains; authoring and graph analysis never evaluate these operations.
use super::{numeric_input, relational::kernel_error};
use crate::{KernelError, KernelInvocation, RuntimeValue};
use std::collections::BTreeMap;
use yss_data_contract::{ColumnSemantic, ConversionMetadata, SemanticType, TabularScalar};
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

struct Column {
    values: Vec<TabularScalar>,
    metadata: Option<ConversionMetadata>,
}

fn metadata(field: &arrow_schema::Field) -> Result<ConversionMetadata, KernelError> {
    Ok(ConversionMetadata {
        semantic: yss_database_arrow::column_semantic(field)
            .map_err(|_| KernelError::InvalidParameter)?,
        temporal: yss_database_arrow::temporal_metadata(field.data_type()),
        dummy_base_level: field.metadata().get("yssbi.dummy_base_level").cloned(),
    })
}

fn load(
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

fn column(value: &RuntimeValue, invocation: &KernelInvocation<'_>) -> Result<Column, KernelError> {
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

fn output(column: Column) -> Result<RuntimeValue, KernelError> {
    let value = RuntimeValue::List(
        column
            .values
            .into_iter()
            .map(RuntimeValue::Scalar)
            .collect(),
    );
    match column.metadata {
        Some(metadata) => value
            .with_metadata(metadata)
            .map_err(|_| KernelError::InvalidParameter),
        None => Ok(value),
    }
}

fn float(value: f64) -> Result<TabularScalar, KernelError> {
    Ok(TabularScalar::Float64(
        value.try_into().map_err(|_| KernelError::NonFiniteResult)?,
    ))
}

fn number(value: &TabularScalar) -> Result<Option<f64>, KernelError> {
    if matches!(value, TabularScalar::Null) {
        return Ok(None);
    }
    numeric_input(Some(&RuntimeValue::Scalar(value.clone()))).map(Some)
}

fn integer(value: Option<&RuntimeValue>) -> Result<i64, KernelError> {
    match value.map(RuntimeValue::unannotated) {
        Some(RuntimeValue::Scalar(TabularScalar::Integer(n))) => Ok(*n),
        Some(RuntimeValue::Scalar(TabularScalar::Unsigned(n))) => {
            i64::try_from(*n).map_err(|_| KernelError::InvalidParameter)
        }
        _ => Err(KernelError::InvalidParameter),
    }
}

fn period(invocation: &KernelInvocation<'_>, key: &str) -> Result<usize, KernelError> {
    usize::try_from(integer(invocation.parameter(key))?)
        .ok()
        .filter(|n| *n > 0)
        .ok_or(KernelError::InvalidParameter)
}

fn text<'a>(invocation: &'a KernelInvocation<'_>, key: &str) -> Result<&'a str, KernelError> {
    match invocation.parameter(key) {
        Some(RuntimeValue::Scalar(TabularScalar::String(value))) => Ok(value),
        _ => Err(KernelError::InvalidParameter),
    }
}

// Ordered keys retain numeric ordering and distinguish physical value classes.
#[derive(Eq, PartialEq, Ord, PartialOrd)]
enum Key {
    Integer(i128),
    Float(u64),
    Text(Box<str>),
    Bool(bool),
}
fn key(value: &TabularScalar) -> Result<Key, KernelError> {
    Ok(match value {
        TabularScalar::Integer(value) => Key::Integer(i128::from(*value)),
        TabularScalar::Unsigned(value) => Key::Integer(i128::from(*value)),
        TabularScalar::Float64(value) => {
            let value = value.as_f64();
            let bits = if value == 0.0 {
                0.0f64.to_bits()
            } else {
                value.to_bits()
            };
            Key::Float(if bits >> 63 == 0 {
                bits ^ (1 << 63)
            } else {
                !bits
            })
        }
        TabularScalar::String(value) => Key::Text(value.clone()),
        TabularScalar::Bool(value) => Key::Bool(*value),
        _ => return Err(KernelError::InvalidParameter),
    })
}

fn difference(
    values: &mut [TabularScalar],
    order: usize,
    invocation: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    // Higher-order differences are repeated first differences, not a single lag of n.
    if order >= values.len() {
        for (i, value) in values.iter_mut().enumerate() {
            if i % 1024 == 0 {
                invocation.check_control()?;
            }
            *value = TabularScalar::Null;
        }
        return Ok(());
    }
    for _ in 0..order.min(values.len()) {
        invocation.check_control()?;
        for row in (1..values.len()).rev() {
            if row % 1024 == 0 {
                invocation.check_control()?;
            }
            values[row] = match (number(&values[row])?, number(&values[row - 1])?) {
                (Some(a), Some(b)) => float(a - b)?,
                _ => TabularScalar::Null,
            };
        }
        if let Some(first) = values.first_mut() {
            *first = TabularScalar::Null;
        }
    }
    Ok(())
}

pub(crate) fn execute(
    kind: SeriesKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    use SeriesKernel::*;
    invocation.check_control()?;
    if matches!(kind, Range) {
        let start = integer(invocation.parameter("start"))?;
        let end = integer(invocation.parameter("end"))?;
        let step = integer(invocation.parameter("step"))?;
        if step == 0 {
            return Err(KernelError::InvalidParameter);
        }
        let distance = if step > 0 {
            i128::from(end) - i128::from(start)
        } else {
            i128::from(start) - i128::from(end)
        };
        let stride = i128::from(step).abs();
        let count = usize::try_from(if distance <= 0 {
            0
        } else {
            (distance - 1) / stride + 1
        })
        .map_err(|_| KernelError::BudgetExceeded)?;
        let mut result = invocation.control.reserve(count)?;
        for i in 0..count {
            if i % 1024 == 0 {
                invocation.check_control()?;
            }
            let value = i128::from(start) + i as i128 * i128::from(step);
            result.push(RuntimeValue::Scalar(TabularScalar::Integer(
                i64::try_from(value).map_err(|_| KernelError::NonFiniteResult)?,
            )));
        }
        return Ok(vec![RuntimeValue::List(result.into())]);
    }
    if matches!(kind, PanelDifference) {
        return panel_difference(invocation);
    }
    let input = invocation
        .inputs
        .first()
        .ok_or(KernelError::InputLayoutMismatch)?;
    if matches!(kind, Length | Count) {
        let mut count = 0usize;
        if let RuntimeValue::Series(series) = input.unannotated() {
            series
                .as_relation()
                .map_err(kernel_error)?
                .visit_batches(&invocation.relation_control(), &mut |batch| {
                    let additional = if matches!(kind, Length) {
                        batch.num_rows()
                    } else {
                        batch.num_rows() - batch.column(0).logical_null_count()
                    };
                    count = count
                        .checked_add(additional)
                        .ok_or(RelationError::InvalidInput)?;
                    Ok(())
                })
                .map_err(kernel_error)?;
        } else if let RuntimeValue::List(values) = input.unannotated() {
            for (i, value) in values.iter().enumerate() {
                if i % 1024 == 0 {
                    invocation.check_control()?;
                }
                if matches!(kind, Length)
                    || !matches!(
                        value.unannotated(),
                        RuntimeValue::Scalar(TabularScalar::Null)
                    )
                {
                    count += 1;
                }
            }
        } else {
            return Err(KernelError::InvalidParameter);
        }
        return Ok(vec![RuntimeValue::Scalar(TabularScalar::Integer(
            i64::try_from(count).map_err(|_| KernelError::NonFiniteResult)?,
        ))]);
    }
    // Keep the relation's row domain when undoing standardization. Turning this into
    // an unbound list would make comparison with the original column unsafe.
    if matches!(kind, InverseStandardize)
        && let RuntimeValue::Series(series) = input.unannotated()
    {
        let mean = numeric_input(invocation.inputs.get(1))?;
        let sd = numeric_input(invocation.inputs.get(2))?;
        if sd <= 0.0 {
            return Err(KernelError::InvalidNumericInput);
        }
        return Ok(vec![RuntimeValue::Series(
            series
                .relation()
                .standardize_series(series, mean, sd, true)
                .map_err(kernel_error)?,
        )]);
    }
    let mut source = column(input, invocation)?;
    let n = source.values.len();
    if matches!(kind, Dummy) {
        let base = text(invocation, "base_level")?;
        let semantic = match invocation.outputs.first().map(|output| &output.data_type) {
            Some(yss_data_contract::ValueType::DataSeries(element)) => match element.as_ref() {
                yss_data_contract::ValueType::Scalar(kind) => *kind,
                _ => return Err(KernelError::InvalidParameter),
            },
            _ => return Err(KernelError::InvalidParameter),
        };
        let mut meaning = source.metadata.take().unwrap_or(ConversionMetadata {
            semantic: ColumnSemantic::new(semantic),
            temporal: None,
            dummy_base_level: None,
        });
        if !matches!(
            meaning.semantic.kind,
            SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
        ) {
            return Err(KernelError::InvalidParameter);
        }
        let present = source.values.iter().any(|v| match v {
            TabularScalar::String(value) => value.as_ref() == base,
            TabularScalar::Integer(value) => value.to_string() == base,
            TabularScalar::Unsigned(value) => value.to_string() == base,
            TabularScalar::Bool(value) => value.to_string() == base,
            _ => false,
        });
        if !base.is_empty() && !present {
            return Err(KernelError::InvalidParameter);
        }
        meaning.dummy_base_level = Some(base.into());
        source.metadata = Some(meaning);
        return Ok(vec![output(source)?]);
    }
    if matches!(kind, Lag) {
        let lag = period(invocation, "window")?;
        for row in (0..n).rev() {
            if row % 1024 == 0 {
                invocation.check_control()?;
            }
            source.values[row] = row
                .checked_sub(lag)
                .map_or(TabularScalar::Null, |i| source.values[i].clone());
        }
        return Ok(vec![output(source)?]);
    }
    source.metadata = None;
    if matches!(kind, Sum) {
        let mut integer_sum = 0i128;
        let all_integer = source.values.iter().all(|v| {
            matches!(
                v,
                TabularScalar::Null | TabularScalar::Integer(_) | TabularScalar::Unsigned(_)
            )
        });
        if all_integer {
            for (i, value) in source.values.iter().enumerate() {
                if i % 1024 == 0 {
                    invocation.check_control()?;
                }
                integer_sum = integer_sum
                    .checked_add(match value {
                        TabularScalar::Integer(value) => i128::from(*value),
                        TabularScalar::Unsigned(value) => i128::from(*value),
                        _ => 0,
                    })
                    .ok_or(KernelError::NonFiniteResult)?;
            }
            let value = if integer_sum <= i128::from(i64::MAX) {
                TabularScalar::Integer(
                    i64::try_from(integer_sum).map_err(|_| KernelError::NonFiniteResult)?,
                )
            } else {
                TabularScalar::Unsigned(
                    u64::try_from(integer_sum).map_err(|_| KernelError::NonFiniteResult)?,
                )
            };
            return Ok(vec![RuntimeValue::Scalar(value)]);
        }
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (i, value) in source.values.iter().enumerate() {
            if i % 1024 == 0 {
                invocation.check_control()?;
            }
            if let Some(value) = number(value)? {
                let adjusted = value - correction;
                let next = sum + adjusted;
                correction = (next - sum) - adjusted;
                sum = next;
            }
        }
        return Ok(vec![RuntimeValue::Scalar(float(sum)?)]);
    }
    if matches!(kind, Mean | Standardize) {
        let (mut count, mut mean, mut m2) = (0usize, 0.0, 0.0);
        for (i, value) in source.values.iter().enumerate() {
            if i % 1024 == 0 {
                invocation.check_control()?;
            }
            if let Some(value) = number(value)? {
                count += 1;
                let delta = value - mean;
                mean += delta / count as f64;
                m2 += delta * (value - mean);
            }
        }
        if matches!(kind, Mean) {
            return Ok(vec![RuntimeValue::Scalar(if count == 0 {
                TabularScalar::Null
            } else {
                float(mean)?
            })]);
        }
        if count < 2 {
            return Err(KernelError::InvalidNumericInput);
        }
        let sd = (m2 / (count - 1) as f64).sqrt();
        if !sd.is_finite() || sd <= 0.0 {
            return Err(KernelError::InvalidNumericInput);
        }
        for (i, value) in source.values.iter_mut().enumerate() {
            if i % 1024 == 0 {
                invocation.check_control()?;
            }
            if let Some(x) = number(value)? {
                *value = float((x - mean) / sd)?;
            }
        }
        let standardized = if let RuntimeValue::Series(series) = input.unannotated() {
            RuntimeValue::Series(
                series
                    .relation()
                    .standardize_series(series, mean, sd, false)
                    .map_err(kernel_error)?,
            )
        } else {
            output(source)?
        };
        return Ok(vec![
            standardized,
            RuntimeValue::Scalar(float(mean)?),
            RuntimeValue::Scalar(float(sd)?),
        ]);
    }
    match kind {
        InverseStandardize => {
            let mean = numeric_input(invocation.inputs.get(1))?;
            let sd = numeric_input(invocation.inputs.get(2))?;
            if sd <= 0.0 {
                return Err(KernelError::InvalidNumericInput);
            }
            for (i, value) in source.values.iter_mut().enumerate() {
                if i % 1024 == 0 {
                    invocation.check_control()?;
                }
                if let Some(x) = number(value)? {
                    *value = float(x * sd + mean)?;
                }
            }
        }
        Difference => difference(&mut source.values, period(invocation, "order")?, invocation)?,
        PercentChange => {
            let lag = period(invocation, "order")?;
            for row in (0..n).rev() {
                if row % 1024 == 0 {
                    invocation.check_control()?;
                }
                source.values[row] = match (
                    number(&source.values[row])?,
                    row.checked_sub(lag)
                        .map(|i| number(&source.values[i]))
                        .transpose()?
                        .flatten(),
                ) {
                    (Some(_), Some(0.0)) => TabularScalar::Null,
                    (Some(a), Some(b)) => float(a / b - 1.0)?,
                    _ => TabularScalar::Null,
                };
            }
        }
        RollingMean => {
            let window = period(invocation, "window")?;
            let mut result = invocation.control.reserve(n)?;
            let (mut sum, mut count) = (0.0, 0usize);
            for row in 0..n {
                if row % 1024 == 0 {
                    invocation.check_control()?;
                }
                if let Some(value) = number(&source.values[row])? {
                    sum += value;
                    count += 1;
                }
                if row >= window
                    && let Some(value) = number(&source.values[row - window])?
                {
                    sum -= value;
                    count -= 1;
                }
                result.push(if row + 1 >= window && count == window {
                    float(sum / window as f64)?
                } else {
                    TabularScalar::Null
                });
            }
            source.values = result;
        }
        _ => return Err(KernelError::InvalidParameter),
    }
    invocation.check_control()?;
    Ok(vec![output(source)?])
}

fn panel_difference(invocation: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let [RuntimeValue::Relation(frame), RuntimeValue::Series(series)] = invocation.inputs else {
        return Err(KernelError::UnalignedSeries);
    };
    let entity = frame
        .select_series(text(invocation, "entity_column")?)
        .map_err(kernel_error)?;
    let time = frame
        .select_series(text(invocation, "time_column")?)
        .map_err(kernel_error)?;
    let mut columns = load(&[series.clone(), entity, time], invocation)?;
    let time = columns.pop().unwrap();
    let entity = columns.pop().unwrap();
    let mut source = columns.pop().unwrap();
    let n = source.values.len();
    invocation
        .control
        .check_bytes(n.checked_mul(size_of::<RuntimeValue>() * 12))?;
    let mut ordered = BTreeMap::new();
    for row in 0..n {
        if row % 1024 == 0 {
            invocation.check_control()?;
        }
        if ordered
            .insert((key(&entity.values[row])?, key(&time.values[row])?), row)
            .is_some()
        {
            return Err(KernelError::InvalidParameter);
        }
    }
    let order = period(invocation, "order")?;
    let mut group = Vec::new();
    let mut previous = None;
    let flush = |group: &mut Vec<usize>, source: &mut Column| -> Result<(), KernelError> {
        let mut values: Vec<TabularScalar> = group
            .iter()
            .map(|row| source.values[*row].clone())
            .collect();
        difference(&mut values, order, invocation)?;
        for (row, value) in group.drain(..).zip(values) {
            source.values[row] = value;
        }
        Ok(())
    };
    for ((entity, _), row) in ordered {
        if previous.as_ref().is_some_and(|old| old != &entity) {
            flush(&mut group, &mut source)?;
        }
        previous = Some(entity);
        group.push(row);
    }
    flush(&mut group, &mut source)?;
    source.metadata = None;
    Ok(vec![output(source)?])
}
