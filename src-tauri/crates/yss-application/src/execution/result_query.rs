use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use thiserror::Error;

use super::session_slot::{ApplicationState, SessionCaptureError};
use yss_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use yss_execution::result::{ResultId, StoredResult, StoredResultSnapshot};
use yss_execution::value::RuntimeValue;
use yss_graph_document::{GraphResourcePath, PortAddress};
use yss_relational_contract::{RelationColumn, RelationControl, RelationError};
use yss_tabular_contract::TabularScalar;

pub struct ResultPinQuery {
    graph_path: GraphResourcePath,
    output: PortAddress,
}

impl ResultPinQuery {
    pub fn new(graph_path: GraphResourcePath, output: PortAddress) -> Self {
        Self { graph_path, output }
    }
}

#[derive(Debug, Error)]
pub enum ResultQueryApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("result query session changed")]
    SessionChanged,
    #[error("result page request is invalid")]
    InvalidPageRequest,
    #[error("result page exceeds its payload budget")]
    PageTooLarge,
    #[error("result page query failed")]
    Relation(#[from] RelationError),
}

pub const MAX_RESULT_PAGE_ROWS: usize = 1_000;
const MAX_RESULT_PAGE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultPageKind {
    Scalar,
    Sequence,
}

#[derive(Debug)]
pub struct ResultPageProjection {
    pub offset: usize,
    pub requested_limit: usize,
    pub total_count: Option<usize>,
    pub has_more: bool,
    pub kind: ResultPageKind,
    pub columns: Box<[RelationColumn]>,
    pub values: Box<[RuntimeValue]>,
}

impl ApplicationState {
    pub fn query_result_page(
        &self,
        result_id: ResultId,
        offset: usize,
        limit: usize,
    ) -> Result<Option<ResultPageProjection>, ResultQueryApplicationError> {
        self.query_result_page_with_control(
            result_id,
            offset,
            limit,
            &RelationControl {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: Instant::now() + Duration::from_secs(30),
                max_input_bytes: MAX_RESULT_PAGE_BYTES,
            },
        )
    }

    pub fn query_result_page_with_control(
        &self,
        result_id: ResultId,
        offset: usize,
        limit: usize,
        control: &RelationControl,
    ) -> Result<Option<ResultPageProjection>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        let Some(result) = captured.execution().query_result(result_id) else {
            return Ok(None);
        };
        let page = project_result_page(result.value(), offset, limit, control);
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        // A failed/finished old scan cannot republish a result invalidated while it was reading.
        if captured.execution().query_result(result_id).is_none() {
            return Ok(None);
        }
        page.map(Some)
    }
    pub fn query_result(
        &self,
        result_id: ResultId,
    ) -> Result<Option<StoredResultSnapshot>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        Ok(captured.execution().query_result(result_id))
    }

    pub fn query_pin_result(
        &self,
        query: ResultPinQuery,
    ) -> Result<Option<StoredResultSnapshot>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        let output = PlanOutputRef::new(
            PlanGraphId::from_existing(query.graph_path.as_str().to_owned().into_boxed_str()),
            PlanPortAddress::from_existing(query.output.to_string().into_boxed_str()),
        );
        Ok(captured.execution().query_pin_result(&output))
    }
}

fn project_result_page(
    result: &StoredResult,
    offset: usize,
    limit: usize,
    control: &RelationControl,
) -> Result<ResultPageProjection, ResultQueryApplicationError> {
    if limit == 0 || limit > MAX_RESULT_PAGE_ROWS || offset.checked_add(limit).is_none() {
        return Err(ResultQueryApplicationError::InvalidPageRequest);
    }
    let result = result.value();
    let relation = match result {
        StoredResult::Runtime(RuntimeValue::Relation(relation)) => Some(relation.clone()),
        StoredResult::Runtime(RuntimeValue::Series(series)) => Some(series.as_relation()?),
        _ => None,
    };
    let page = if let Some(relation) = relation {
        let page = relation
            .page(offset, limit, control)
            .map_err(|error| match error {
                RelationError::MemoryLimitExceeded => ResultQueryApplicationError::PageTooLarge,
                error => ResultQueryApplicationError::Relation(error),
            })?;
        if page.row_count > limit
            || (page.has_more && page.row_count == 0)
            || page.columns.len() != page.data.columns().len()
            || page
                .data
                .columns()
                .iter()
                .zip(&page.columns)
                .any(|(data, column)| {
                    data.values().len() != page.row_count
                        || data.name().as_str() != column.name.as_ref()
                })
        {
            return Err(RelationError::InvalidInput.into());
        }
        let total_count = (!page.has_more && (offset == 0 || page.row_count > 0))
            .then_some(offset + page.row_count);
        let values = (0..page.row_count)
            .map(|row| {
                RuntimeValue::List(
                    page.data
                        .columns()
                        .iter()
                        .map(|column| scalar_value(&column.values()[row]))
                        .collect(),
                )
            })
            .collect();
        ResultPageProjection {
            offset,
            requested_limit: limit,
            total_count,
            has_more: page.has_more,
            kind: ResultPageKind::Sequence,
            columns: page.columns,
            values,
        }
    } else {
        let (count, kind) = match result {
            StoredResult::Runtime(RuntimeValue::List(values)) => {
                (values.len(), ResultPageKind::Sequence)
            }
            StoredResult::Empty => (0, ResultPageKind::Scalar),
            _ => (1, ResultPageKind::Scalar),
        };
        let mut budget = MAX_RESULT_PAGE_BYTES;
        match result {
            StoredResult::Runtime(RuntimeValue::List(values)) => {
                for value in values.iter().skip(offset).take(limit) {
                    charge_value(value, &mut budget, 0)?;
                }
            }
            StoredResult::Runtime(value) if offset == 0 => charge_value(value, &mut budget, 0)?,
            StoredResult::Text(value) if offset == 0 && value.len() > MAX_RESULT_PAGE_BYTES / 6 => {
                return Err(ResultQueryApplicationError::PageTooLarge);
            }
            _ => {}
        }
        let values: Box<[_]> = match result {
            StoredResult::Runtime(RuntimeValue::List(values)) => {
                values.iter().skip(offset).take(limit).cloned().collect()
            }
            StoredResult::Runtime(value) if offset == 0 => Box::new([value.clone()]),
            StoredResult::Scalar(value) if offset == 0 => Box::new([RuntimeValue::Decimal(*value)]),
            StoredResult::Text(value) if offset == 0 => {
                Box::new([RuntimeValue::String(value.clone())])
            }
            _ => Box::new([]),
        };
        ResultPageProjection {
            offset: offset.min(count),
            requested_limit: limit,
            total_count: Some(count),
            has_more: offset.saturating_add(values.len()) < count,
            kind,
            columns: Box::new([]),
            values,
        }
    };
    let mut remaining = MAX_RESULT_PAGE_BYTES;
    for column in &page.columns {
        let bytes = column
            .name
            .len()
            .checked_add(column.data_type.len())
            .and_then(|bytes| bytes.checked_mul(6))
            .and_then(|bytes| bytes.checked_add(40))
            .ok_or(ResultQueryApplicationError::PageTooLarge)?;
        remaining = remaining
            .checked_sub(bytes)
            .ok_or(ResultQueryApplicationError::PageTooLarge)?;
    }
    for value in &page.values {
        charge_value(value, &mut remaining, 0)?;
    }
    Ok(page)
}

fn scalar_value(value: &TabularScalar) -> RuntimeValue {
    match value.display_value() {
        TabularScalar::Null => RuntimeValue::Null,
        TabularScalar::Bool(value) => RuntimeValue::Bool(value),
        TabularScalar::Integer(value) => RuntimeValue::Integer(value),
        TabularScalar::Unsigned(value) => RuntimeValue::Unsigned(value),
        TabularScalar::Decimal(value) => RuntimeValue::Decimal(value.as_f64()),
        TabularScalar::String(value) => RuntimeValue::String(value),
    }
}

fn charge_value(
    value: &RuntimeValue,
    remaining: &mut usize,
    depth: usize,
) -> Result<(), ResultQueryApplicationError> {
    if depth > 64 {
        return Err(ResultQueryApplicationError::PageTooLarge);
    }
    *remaining = remaining
        .checked_sub(3)
        .ok_or(ResultQueryApplicationError::PageTooLarge)?;
    let bytes = match value {
        RuntimeValue::String(value) | RuntimeValue::Resource(value) => value.len().checked_mul(6),
        RuntimeValue::List(values) => {
            for value in values {
                charge_value(value, remaining, depth + 1)?;
            }
            Some(2)
        }
        RuntimeValue::Record(values) => {
            for (key, value) in values {
                *remaining = remaining
                    .checked_sub(key.len().saturating_mul(6))
                    .ok_or(ResultQueryApplicationError::PageTooLarge)?;
                charge_value(value, remaining, depth + 1)?;
            }
            Some(2)
        }
        _ => Some(24),
    }
    .ok_or(ResultQueryApplicationError::PageTooLarge)?;
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or(ResultQueryApplicationError::PageTooLarge)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    mod paging;
    use super::*;
    use yss_execution::plan::{PlanGraphId, PlanPortAddress};

    #[test]
    fn result_query_maps_opaque_graph_and_port_identities() {
        let graph =
            GraphResourcePath::new("events/main.yssbi-event").expect("test graph path is valid");
        let address = PortAddress::declared(
            yss_graph_document::NodeId::from_uuid(uuid::Uuid::nil()),
            yss_graph_protocol::PortKey::new("result").expect("valid port key"),
        );
        let query = ResultPinQuery::new(graph, address.clone());
        let plan = PlanOutputRef::new(
            PlanGraphId::from_existing(query.graph_path.as_str().to_owned().into_boxed_str()),
            PlanPortAddress::from_existing(query.output.to_string().into_boxed_str()),
        );
        assert_eq!(plan.graph().as_str(), "events/main.yssbi-event");
        assert_eq!(plan.port().as_str(), address.to_string());
    }
}
