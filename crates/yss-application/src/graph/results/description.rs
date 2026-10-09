//! Typed presentation of the existing bounded description-result projection.
use std::collections::BTreeMap;

use yss_data_contract::TabularScalar;
use yss_node_kernel::RuntimeValue;

use super::ResultQueryApplicationError;

pub const NUMERIC_FIELDS: [&str; 7] = ["mean", "std", "min", "q25", "median", "q75", "max"];

#[derive(Debug)]
pub struct DescriptionColumn {
    pub name: String,
    pub count: u64,
    pub missing: u64,
    pub statistics: DescriptionStatistics,
}

#[derive(Debug)]
pub enum DescriptionStatistics {
    Numeric {
        /// Mean, standard deviation, minimum, Q25, median, Q75 and maximum.
        metrics: [TabularScalar; 7],
    },
    Categorical {
        semantic: &'static str,
        unique: u64,
        categories: Vec<DescriptionCategory>,
    },
}

#[derive(Debug)]
pub struct DescriptionCategory {
    pub value: TabularScalar,
    pub label: Option<String>,
    pub frequency: u64,
    pub proportion: f64,
}

type Fields = BTreeMap<Box<str>, RuntimeValue>;
type Invalid = ResultQueryApplicationError;

/// Validate the complete value returned by `query_result_projection`, without JSON conversion.
/// Field positions preserve source order independently of record-key ordering.
pub fn columns(value: &RuntimeValue) -> Result<Vec<DescriptionColumn>, Invalid> {
    let root = record(value)?;
    if root.len() != 1 {
        return invalid();
    }
    let fields = record(field(root, "columns")?)?;
    let mut columns = fields
        .iter()
        .map(|(name, value)| {
            let fields = record(value)?;
            let position = count(field(fields, "position")?)?;
            let observed = count(field(fields, "count")?)?;
            let missing = count(field(fields, "missing")?)?;
            let statistics = match text(field(fields, "semantic")?)? {
                "Numeric" if fields.len() == 11 => DescriptionStatistics::Numeric {
                    metrics: numeric(fields)?,
                },
                kind @ ("Categorical" | "Ordinal" | "Binary") if fields.len() == 6 => {
                    DescriptionStatistics::Categorical {
                        semantic: match kind {
                            "Ordinal" => "Ordinal",
                            "Binary" => "Binary",
                            _ => "Categorical",
                        },
                        unique: count(field(fields, "unique")?)?,
                        categories: categories(field(fields, "categories")?)?,
                    }
                }
                _ => return invalid(),
            };
            Ok((
                position,
                DescriptionColumn {
                    name: name.to_string(),
                    count: observed,
                    missing,
                    statistics,
                },
            ))
        })
        .collect::<Result<Vec<_>, Invalid>>()?;
    columns.sort_by_key(|(position, _)| *position);
    if columns
        .iter()
        .enumerate()
        .any(|(index, (position, _))| *position != index as u64 + 1)
    {
        return invalid();
    }
    Ok(columns.into_iter().map(|(_, column)| column).collect())
}

fn numeric(fields: &Fields) -> Result<[TabularScalar; 7], Invalid> {
    let mut values = std::array::from_fn(|_| TabularScalar::Null);
    for (value, key) in values.iter_mut().zip(NUMERIC_FIELDS) {
        *value = metric(field(fields, key)?)?;
    }
    Ok(values)
}

fn categories(value: &RuntimeValue) -> Result<Vec<DescriptionCategory>, Invalid> {
    let entries = record(value)?;
    (1..=entries.len())
        .map(|position| {
            let fields = record(field(entries, &position.to_string())?)?;
            let value = match field(fields, "value")?.unannotated() {
                RuntimeValue::Scalar(
                    value @ (TabularScalar::String(_)
                    | TabularScalar::Bool(_)
                    | TabularScalar::Integer(_)
                    | TabularScalar::Unsigned(_)),
                ) => value.clone(),
                RuntimeValue::Scalar(TabularScalar::Float64(value)) => {
                    TabularScalar::Float64(*value)
                }
                _ => return invalid(),
            };
            let label = fields
                .get("label")
                .map(|value| text(value).map(str::to_owned))
                .transpose()?;
            if fields.len() != 3 + usize::from(label.is_some()) {
                return invalid();
            }
            let frequency = count(field(fields, "frequency")?)?;
            let proportion = match metric(field(fields, "proportion")?)? {
                TabularScalar::Float64(value) => value.as_f64(),
                TabularScalar::Integer(value) if (0..=1).contains(&value) => value as f64,
                TabularScalar::Unsigned(value) if value <= 1 => value as f64,
                _ => return invalid(),
            };
            if !(0.0..=1.0).contains(&proportion) {
                return invalid();
            }
            Ok(DescriptionCategory {
                value,
                label,
                frequency,
                proportion,
            })
        })
        .collect()
}

fn record(value: &RuntimeValue) -> Result<&Fields, Invalid> {
    match value.unannotated() {
        RuntimeValue::Record(fields) => Ok(fields),
        _ => invalid(),
    }
}
fn field<'a>(fields: &'a Fields, key: &str) -> Result<&'a RuntimeValue, Invalid> {
    fields.get(key).ok_or(Invalid::UnrepresentableValue)
}
fn text(value: &RuntimeValue) -> Result<&str, Invalid> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::String(text)) => Ok(text),
        _ => invalid(),
    }
}
fn count(value: &RuntimeValue) -> Result<u64, Invalid> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::Unsigned(value)) => Ok(*value),
        RuntimeValue::Scalar(TabularScalar::Integer(value)) if *value >= 0 => Ok(*value as u64),
        _ => invalid(),
    }
}
fn metric(value: &RuntimeValue) -> Result<TabularScalar, Invalid> {
    match value.unannotated() {
        RuntimeValue::Scalar(
            value @ (TabularScalar::Null | TabularScalar::Integer(_) | TabularScalar::Unsigned(_)),
        ) => Ok(value.clone()),
        RuntimeValue::Scalar(TabularScalar::Float64(value)) => Ok(TabularScalar::Float64(*value)),
        _ => invalid(),
    }
}
fn invalid<T>() -> Result<T, Invalid> {
    Err(Invalid::UnrepresentableValue)
}

#[cfg(test)]
mod tests;
