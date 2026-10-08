//! Validated presentation of the bounded structured-result projection.
use std::collections::BTreeMap;

use yss_data_contract::TabularScalar;
use yss_node_kernel::RuntimeValue;
use yss_relational_contract::RelationColumn;

use super::{ReportQueryError, ResultPageProjection, ResultTablePart};

pub struct ReportSection {
    pub title: String,
    pub content: ReportSectionContent,
}

pub enum ReportSectionContent {
    Equation(String),
    Table(ReportTable),
}

#[derive(Clone)]
pub struct ReportTable {
    part: ResultTablePart,
    row_count: usize,
    columns: Vec<(String, String)>,
    stability: bool,
}

pub struct StabilityPoint {
    pub re: f64,
    pub im: f64,
    pub modulus: Option<f64>,
}

pub struct ReportTablePage {
    pub table: ResultPageProjection,
    pub roots: Option<Vec<StabilityPoint>>,
}

/// Read declarations only; arrays remain behind the original Results table references.
pub fn sections(value: &RuntimeValue) -> Result<Vec<ReportSection>, ReportQueryError> {
    let root = record(value)?;
    let Some(display) = root.get("report_display") else {
        return Ok(Vec::new());
    };
    let display = record(display)?;
    let sections = record(field(display, "sections")?)?;
    if display.len() != 1 || sections.len() > 24 {
        return Err(ReportQueryError::UnrepresentableValue);
    }
    sections
        .iter()
        .map(|(id, section)| {
            let declaration = record(section)?;
            let title = text(field(declaration, "title")?)?;
            let kind = text(field(declaration, "kind")?)?;
            let path = text(field(declaration, "path")?)?;
            if id.is_empty()
                || id.len() > 48
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
                || declaration
                    .keys()
                    .any(|key| !matches!(key.as_ref(), "title" | "kind" | "path" | "columns"))
                || title.trim().is_empty()
                || title.len() > 200
            {
                return Err(ReportQueryError::UnrepresentableValue);
            }
            let columns = declaration.get("columns").map(record).transpose()?;
            if columns.is_some_and(|columns| columns.len() > 32) {
                return Err(ReportQueryError::UnrepresentableValue);
            }
            let columns = columns
                .into_iter()
                .flatten()
                .map(|(key, value)| {
                    let label = text(value)?;
                    if key.len() > 100 || label.len() > 100 {
                        return Err(ReportQueryError::UnrepresentableValue);
                    }
                    Ok((key.to_string(), label.to_owned()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let target = crate::graph::results::structured::at_path(value, path)
                .ok_or(ReportQueryError::UnrepresentableValue)?;
            let content = match kind {
                "equation" => {
                    let equation = text(target)?;
                    if equation.len() > 16_384 {
                        return Err(ReportQueryError::UnrepresentableValue);
                    }
                    ReportSectionContent::Equation(equation.to_owned())
                }
                "table" | "stability" => {
                    let (part, row_count) =
                        table_reference(target).ok_or(ReportQueryError::UnrepresentableValue)?;
                    if part != ResultTablePart::Structured(path.into())
                        || (kind == "table" && columns.is_empty())
                    {
                        return Err(ReportQueryError::UnrepresentableValue);
                    }
                    ReportSectionContent::Table(ReportTable {
                        part,
                        row_count,
                        columns,
                        stability: kind == "stability",
                    })
                }
                _ => return Err(ReportQueryError::UnrepresentableValue),
            };
            Ok(ReportSection {
                title: title.to_owned(),
                content,
            })
        })
        .collect()
}

/// Recognizes references from `query_result_projection` and nested table pages.
pub fn table_reference(value: &RuntimeValue) -> Option<(ResultTablePart, usize)> {
    let fields = record(value).ok()?;
    if text(fields.get("kind")?).ok()? != "tableRef" {
        return None;
    }
    let part = text(fields.get("part")?).ok()?.parse().ok()?;
    let count = match fields.get("rowCount")?.unannotated() {
        RuntimeValue::Scalar(TabularScalar::Unsigned(count)) => usize::try_from(*count).ok()?,
        RuntimeValue::Scalar(TabularScalar::Integer(count)) => usize::try_from(*count).ok()?,
        _ => return None,
    };
    matches!(part, ResultTablePart::Structured(_)).then_some((part, count))
}

impl ReportTable {
    pub fn part(&self) -> &ResultTablePart {
        &self.part
    }

    /// Validate every returned row before installing a page; never omit malformed rows.
    pub fn present(
        &self,
        mut page: ResultPageProjection,
    ) -> Result<ReportTablePage, ReportQueryError> {
        if page.total_count != Some(self.row_count) {
            return Err(ReportQueryError::UnrepresentableValue);
        }
        let mut roots = self.stability.then(Vec::new);
        let mut rows = Vec::with_capacity(page.values.len());
        for row in &page.values {
            let RuntimeValue::List(cells) = row.unannotated() else {
                return Err(ReportQueryError::UnrepresentableValue);
            };
            let [value] = cells.as_ref() else {
                return Err(ReportQueryError::UnrepresentableValue);
            };
            let fields = record(value)?;
            if let Some(roots) = &mut roots {
                roots.push(StabilityPoint {
                    re: number(field(fields, "re")?)?,
                    im: number(field(fields, "im")?)?,
                    modulus: fields.get("modulus").map(number).transpose().ok().flatten(),
                });
            } else {
                let cells = self
                    .columns
                    .iter()
                    .map(|(key, _)| {
                        let value = field(fields, key)?;
                        match value.unannotated() {
                            RuntimeValue::Scalar(_) => Ok(value.clone()),
                            _ => Err(ReportQueryError::UnrepresentableValue),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                rows.push(RuntimeValue::List(cells.into()));
            }
        }
        // Stability rows stay available as source data; no modulus is recomputed here.
        if !self.stability {
            page.values = rows.into();
            page.columns = self
                .columns
                .iter()
                .map(|(_, label)| RelationColumn {
                    name: label.clone().into(),
                    data_type: "Any".into(),
                })
                .collect();
        }
        Ok(ReportTablePage { table: page, roots })
    }
}

fn record(value: &RuntimeValue) -> Result<&BTreeMap<Box<str>, RuntimeValue>, ReportQueryError> {
    match value.unannotated() {
        RuntimeValue::Record(fields) => Ok(fields),
        _ => Err(ReportQueryError::UnrepresentableValue),
    }
}

fn field<'a>(
    fields: &'a BTreeMap<Box<str>, RuntimeValue>,
    key: &str,
) -> Result<&'a RuntimeValue, ReportQueryError> {
    fields
        .get(key)
        .ok_or(ReportQueryError::UnrepresentableValue)
}

fn text(value: &RuntimeValue) -> Result<&str, ReportQueryError> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::String(value)) => Ok(value),
        _ => Err(ReportQueryError::UnrepresentableValue),
    }
}

fn number(value: &RuntimeValue) -> Result<f64, ReportQueryError> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::Float64(value)) => Ok(value.as_f64()),
        RuntimeValue::Scalar(TabularScalar::Integer(value)) => Ok(*value as f64),
        RuntimeValue::Scalar(TabularScalar::Unsigned(value)) => Ok(*value as f64),
        _ => Err(ReportQueryError::UnrepresentableValue),
    }
}

#[cfg(test)]
mod tests;
