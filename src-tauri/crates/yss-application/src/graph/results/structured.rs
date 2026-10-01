use std::sync::Arc;

use super::{
    MAX_INLINE_PROJECTION_BYTES, MAX_RESULT_PAGE_BYTES, MAX_RESULT_PAGE_ROWS, ResultPageKind,
    ResultPageProjection, ResultQueryApplicationError, charge_value,
};
use yss_data_contract::TabularScalar;
use yss_graph_execution::plan::{ResultCategory, StatisticalReportKind};
use yss_graph_execution::result::StoredResultSnapshot;
use yss_node_kernel::RuntimeValue;
use yss_relational_contract::RelationColumn;

pub(super) fn supports(snapshot: &StoredResultSnapshot) -> bool {
    snapshot.value().category()
        == ResultCategory::StatisticalReport(StatisticalReportKind::Structured)
}

#[cfg(test)]
#[path = "tests/structured.rs"]
mod tests;

pub(crate) fn path_tokens(path: &str) -> Option<Vec<String>> {
    if !path.starts_with('/') || path.len() > 4096 {
        return None;
    }
    let mut tokens = Vec::new();
    for token in path[1..].split('/') {
        if tokens.len() == 64 {
            return None;
        }
        let mut decoded = String::new();
        let mut chars = token.chars();
        while let Some(c) = chars.next() {
            decoded.push(if c == '~' {
                match chars.next()? {
                    '0' => '~',
                    '1' => '/',
                    _ => return None,
                }
            } else {
                c
            });
        }
        tokens.push(decoded);
    }
    Some(tokens)
}

fn child_path(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}

pub(crate) fn project(value: &RuntimeValue) -> Result<RuntimeValue, ResultQueryApplicationError> {
    let mut remaining = MAX_INLINE_PROJECTION_BYTES;
    project_at(value, "", 0, &mut remaining)
}

fn project_at(
    value: &RuntimeValue,
    path: &str,
    depth: usize,
    remaining: &mut usize,
) -> Result<RuntimeValue, ResultQueryApplicationError> {
    if depth > 64 || path.len() > 4096 {
        return Err(ResultQueryApplicationError::PageTooLarge);
    }
    match value.unannotated() {
        RuntimeValue::List(values) => {
            // Array contents stay with the immutable result, regardless of their size.
            // Only a page request for this part reads its elements; nested arrays keep refs.
            let reference = RuntimeValue::Record(Arc::new(
                [
                    (
                        "kind".into(),
                        TabularScalar::String("tableRef".into()).into(),
                    ),
                    (
                        "part".into(),
                        TabularScalar::String(format!("structured:{path}").into()).into(),
                    ),
                    (
                        "rowCount".into(),
                        TabularScalar::Unsigned(values.len() as u64).into(),
                    ),
                ]
                .into(),
            ));
            charge_value(&reference, remaining, depth)?;
            Ok(reference)
        }
        RuntimeValue::Record(fields) => {
            *remaining = remaining
                .checked_sub(5)
                .ok_or(ResultQueryApplicationError::PageTooLarge)?;
            let mut projected = std::collections::BTreeMap::new();
            for (key, value) in fields.iter() {
                *remaining = remaining
                    .checked_sub(key.len().saturating_mul(6))
                    .ok_or(ResultQueryApplicationError::PageTooLarge)?;
                projected.insert(
                    key.clone(),
                    project_at(value, &child_path(path, key), depth + 1, remaining)?,
                );
            }
            Ok(RuntimeValue::Record(Arc::new(projected)))
        }
        value => {
            charge_value(value, remaining, depth)?;
            Ok(value.clone())
        }
    }
}

pub(crate) fn at_path<'a>(root: &'a RuntimeValue, path: &str) -> Option<&'a RuntimeValue> {
    let mut value = root;
    for token in path_tokens(path)? {
        value = match value.unannotated() {
            RuntimeValue::Record(fields) => fields.get(token.as_str()),
            RuntimeValue::List(values) => token
                .parse::<usize>()
                .ok()
                .filter(|index| index.to_string() == token)
                .and_then(|index| values.get(index)),
            _ => None,
        }?;
    }
    Some(value.unannotated())
}

pub(super) fn page(
    root: &RuntimeValue,
    path: &str,
    offset: usize,
    limit: usize,
) -> Result<ResultPageProjection, ResultQueryApplicationError> {
    if limit == 0 || limit > MAX_RESULT_PAGE_ROWS || offset.checked_add(limit).is_none() {
        return Err(ResultQueryApplicationError::InvalidPageRequest);
    }
    let tokens = path_tokens(path).ok_or(ResultQueryApplicationError::InvalidPageRequest)?;
    let value = at_path(root, path).ok_or(ResultQueryApplicationError::InvalidPageRequest)?;
    let RuntimeValue::List(values) = value.unannotated() else {
        return Err(ResultQueryApplicationError::InvalidPageRequest);
    };
    let count = values.len();
    let offset = offset.min(count);
    let end = offset.saturating_add(limit).min(count);
    let mut remaining = MAX_RESULT_PAGE_BYTES;
    let mut page = Vec::with_capacity(end - offset);
    for (index, value) in values.iter().enumerate().take(end).skip(offset) {
        page.push(RuntimeValue::List(Arc::from([project_at(
            value,
            &child_path(path, &index.to_string()),
            tokens.len() + 1,
            &mut remaining,
        )?])));
    }
    Ok(ResultPageProjection {
        offset,
        requested_limit: limit,
        total_count: Some(count),
        has_more: end < count,
        kind: ResultPageKind::Sequence,
        columns: Box::new([RelationColumn {
            name: "value".into(),
            data_type: "Any".into(),
        }]),
        values: page.into(),
    })
}
