use crate::{KernelError, KernelInvocation, RuntimeValue};
use arrow_array::{Array, RecordBatch, StringArray};
use arrow_schema::DataType;
use std::{collections::BTreeMap, sync::Arc};
use yss_data_contract::TabularScalar;
use yss_relational_contract::{RelationControl, RelationError, RelationHandle};

use super::kernel_error;

type Row = BTreeMap<Box<str>, RuntimeValue>;

const NUMERIC_FIELDS: &[&str] = &["mean", "std", "min", "q25", "median", "q75", "max"];
const CATEGORY_FIELDS: &[&str] = &["unique"];

pub(super) fn describe(
    relation: &RelationHandle,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let control = inv.relation_control();
    let mut columns = BTreeMap::new();
    let mut retained = size_of::<RuntimeValue>() * 4;
    // Evaluate existing aggregate plans during execution; the report retains no relation handles.
    relation
        .describe()
        .and_then(|summary| {
            summary.visit_batches(&control, &mut |batch| {
                for mut row in batch_rows(&batch, &control, &mut retained)? {
                    control.check()?;
                    let Some(RuntimeValue::Scalar(TabularScalar::String(name))) =
                        row.remove("column")
                    else {
                        return Err(RelationError::InvalidInput);
                    };
                    let fields = match row.get("semantic") {
                        Some(RuntimeValue::Scalar(TabularScalar::String(kind))) => {
                            match kind.as_ref() {
                                "Numeric" => NUMERIC_FIELDS,
                                "Categorical" | "Ordinal" | "Binary" => CATEGORY_FIELDS,
                                _ => return Err(RelationError::InvalidInput),
                            }
                        }
                        _ => return Err(RelationError::InvalidInput),
                    };
                    row.retain(|key, _| {
                        matches!(key.as_ref(), "semantic" | "count" | "missing")
                            || fields.contains(&key.as_ref())
                    });
                    // Object key order does not carry the source column order.
                    row.insert(
                        "position".into(),
                        TabularScalar::Unsigned(columns.len() as u64 + 1).into(),
                    );
                    if columns.insert(name, row).is_some() {
                        return Err(RelationError::InvalidInput);
                    }
                }
                Ok(())
            })
        })
        .map_err(kernel_error)?;
    for (name, row) in &mut columns {
        if row.contains_key("unique") {
            let categories = category_frequencies(relation, name, &control, &mut retained)
                .map_err(kernel_error)?;
            row.insert("categories".into(), categories);
        }
    }
    Ok(RuntimeValue::Record(Arc::new(BTreeMap::from([(
        "columns".into(),
        RuntimeValue::Record(Arc::new(
            columns
                .into_iter()
                .map(|(name, row)| (name, RuntimeValue::Record(Arc::new(row))))
                .collect(),
        )),
    )]))))
}

fn category_frequencies(
    relation: &RelationHandle,
    column: &str,
    control: &RelationControl,
    retained: &mut usize,
) -> Result<RuntimeValue, RelationError> {
    control.check()?;
    let frequency = relation.frequency(column, false)?;
    let semantic = yss_database_arrow::column_semantic(frequency.schema().field(0))
        .map_err(|_| RelationError::InvalidInput)?;
    charge(
        retained,
        semantic
            .values
            .len()
            .checked_mul(size_of::<(&str, &str)>() * 4),
        control,
    )?;
    let labels: BTreeMap<_, _> = semantic
        .values
        .iter()
        .map(|entry| (entry.value.as_str(), entry.label.as_str()))
        .collect();
    let mut categories = BTreeMap::new();
    frequency.visit_batches(control, &mut |batch| {
        let rows = batch_rows(&batch, control, retained)?;
        // Arrow's canonical text matches semantic codebooks without rounding wide integer codes.
        let codes =
            yss_database_arrow::lossless_cast(batch.column(0).as_ref(), &DataType::Utf8, false)
                .map_err(|_| RelationError::InvalidInput)?;
        charge(retained, Some(codes.get_array_memory_size()), control)?;
        let codes = codes
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(RelationError::InvalidInput)?;
        for (index, mut row) in rows.into_iter().enumerate() {
            control.check()?;
            let label = labels.get(codes.value(index));
            charge(
                retained,
                Some(size_of::<RuntimeValue>() * 4 + 20 + label.map_or(0, |text| text.len())),
                control,
            )?;
            if let Some(label) = label {
                row.insert(
                    "label".into(),
                    TabularScalar::String((*label).into()).into(),
                );
            }
            // Indexed records keep every category inline in Result JSON and preserve Ordinal order.
            categories.insert(
                (categories.len() + 1).to_string().into(),
                RuntimeValue::Record(Arc::new(row)),
            );
        }
        Ok(())
    })?;
    Ok(RuntimeValue::Record(Arc::new(categories)))
}

fn charge(
    retained: &mut usize,
    bytes: Option<usize>,
    control: &RelationControl,
) -> Result<(), RelationError> {
    *retained = bytes
        .and_then(|bytes| bytes.checked_add(*retained))
        .filter(|bytes| *bytes <= control.max_input_bytes)
        .ok_or(RelationError::MemoryLimitExceeded)?;
    Ok(())
}

fn batch_rows(
    batch: &RecordBatch,
    control: &RelationControl,
    retained: &mut usize,
) -> Result<Vec<Row>, RelationError> {
    control.check()?;
    let schema = batch.schema();
    let row_bytes = schema.fields().iter().try_fold(0usize, |bytes, field| {
        bytes
            .checked_add(size_of::<RuntimeValue>() * 4)?
            .checked_add(field.name().len())
    });
    charge(
        retained,
        row_bytes
            .and_then(|bytes| bytes.checked_mul(batch.num_rows()))
            .and_then(|bytes| {
                batch
                    .get_array_memory_size()
                    .checked_mul(4)?
                    .checked_add(bytes)
            }),
        control,
    )?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(batch.num_rows())
        .map_err(|_| RelationError::MemoryLimitExceeded)?;
    rows.resize_with(batch.num_rows(), BTreeMap::new);
    for (field, array) in schema.fields().iter().zip(batch.columns()) {
        control.check()?;
        // Category codes can use exact decimals that have no lossless floating-point carrier.
        let array = if matches!(
            array.data_type(),
            DataType::Decimal32(..)
                | DataType::Decimal64(..)
                | DataType::Decimal128(..)
                | DataType::Decimal256(..)
        ) {
            let text = yss_database_arrow::lossless_cast(array.as_ref(), &DataType::Utf8, false)
                .map_err(|_| RelationError::InvalidInput)?;
            charge(retained, Some(text.get_array_memory_size()), control)?;
            text
        } else {
            array.clone()
        };
        let values = yss_database_arrow::materialized_values(array.as_ref())
            .map_err(|_| RelationError::InvalidInput)?;
        for (index, (row, value)) in rows.iter_mut().zip(values).enumerate() {
            if index.is_multiple_of(1024) {
                control.check()?;
            }
            row.insert(field.name().as_str().into(), RuntimeValue::Scalar(value));
        }
    }
    Ok(rows)
}
