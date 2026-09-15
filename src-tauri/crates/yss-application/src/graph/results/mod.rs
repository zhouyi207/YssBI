use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use thiserror::Error;

use super::inputs::{DraftResolutionContext, GraphInputError};
use crate::session::{ApplicationSession, ApplicationState, SessionCaptureError};
use yss_graph_document::{GraphResourcePath, PortAddress};
use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use yss_graph_execution::result::{
    ResultReference, ResultRetentionError, StoredResult, StoredResultSnapshot,
};
use yss_graph_execution::value::RuntimeValue;
use yss_relational_contract::{RelationColumn, RelationControl, RelationError};
use yss_tabular_contract::TabularScalar;

pub mod report;
mod retention;

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
    #[error(transparent)]
    Retention(#[from] ResultRetentionError),
    #[error("result page request is invalid")]
    InvalidPageRequest,
    #[error("result page exceeds its payload budget")]
    PageTooLarge,
    #[error("result page query failed")]
    Relation(#[from] RelationError),
    #[error("result dependency facts could not be verified")]
    Resources(#[from] GraphInputError),
}

pub struct GraphResultState {
    pub execution_session_id: yss_graph_execution::identity::ExecutionSessionId,
    pub semantic_input_hash: [u8; 32],
    pub compiled_artifact_id: Option<[u8; 32]>,
    pub outputs:
        std::collections::BTreeMap<PlanOutputRef, yss_graph_execution::result::ResultCacheState>,
    pub connections: Vec<yss_graph_execution::result::ConnectionResultState>,
}

fn with_current_graph_results<T>(
    captured: &ApplicationSession,
    graph: &GraphResourcePath,
    read: impl FnOnce(Option<&mut DraftResolutionContext>) -> Result<T, ResultQueryApplicationError>,
) -> Result<T, ResultQueryApplicationError> {
    let keys = captured.execution().result_resource_keys(graph.as_str());
    let mut context = if keys.is_empty() {
        None
    } else {
        let context = DraftResolutionContext::capture_catalog(captured)?;
        let versions = context.result_resource_versions(captured, &keys)?;
        context.revalidate(captured)?;
        captured
            .execution()
            .observe_result_resource_versions(&versions);
        Some(context)
    };
    let outcome = read(context.as_mut());
    if let Some(context) = context {
        context.revalidate(captured)?;
    }
    outcome
}

pub(crate) fn query_graph_results(
    captured: &ApplicationSession,
    graph: &GraphResourcePath,
    limit: usize,
) -> Result<Vec<StoredResultSnapshot>, ResultQueryApplicationError> {
    with_current_graph_results(captured, graph, |_| {
        Ok(captured
            .execution()
            .query_graph_results(graph.as_str(), limit))
    })
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
        reference: ResultReference,
        offset: usize,
        limit: usize,
    ) -> Result<Option<ResultPageProjection>, ResultQueryApplicationError> {
        self.query_result_page_with_control(
            reference,
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
        reference: ResultReference,
        offset: usize,
        limit: usize,
        control: &RelationControl,
    ) -> Result<Option<ResultPageProjection>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        let result_id = reference.result_id;
        let Some(result) = captured.execution().query_result(result_id) else {
            return Ok(None);
        };
        let page = project_result_page(result.value(), offset, limit, control);
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        // A query may outlive its last owner, but it cannot publish after reclamation.
        if captured.execution().query_result(result_id).is_none() {
            return Ok(None);
        }
        page.map(Some)
    }
    pub fn query_result(
        &self,
        reference: ResultReference,
    ) -> Result<Option<StoredResultSnapshot>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        let result = captured.execution().query_result(reference.result_id);
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        Ok(result)
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
        let result = with_current_graph_results(&captured, &query.graph_path, |_| {
            Ok(captured.execution().query_pin_result(&output))
        });
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        result
    }

    pub fn query_graph_result_state(
        &self,
        graph: GraphResourcePath,
        semantic_input_hash: [u8; 32],
    ) -> Result<Option<GraphResultState>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        let result = with_current_graph_results(&captured, &graph, |context| {
            let Some(cache) = captured
                .execution()
                .query_result_cache_states(graph.as_str(), &semantic_input_hash)
            else {
                return Ok(None);
            };
            let compiled_artifact_id = if let Some(context) = context {
                context.matching_compiled_artifact(&captured, &graph, &semantic_input_hash)?
            } else {
                captured
                    .graph()
                    .compiled_draft(&graph, &semantic_input_hash)
                    .filter(|compiled| {
                        compiled.analysis().kernel_fingerprint()
                            == &captured.execution().kernels().fingerprint().as_bytes()
                            && compiled
                                .analysis()
                                .semantic_snapshot()
                                .dependencies()
                                .entries()
                                .is_empty()
                    })
                    .map(|_| semantic_input_hash)
            };
            Ok(Some(GraphResultState {
                execution_session_id: captured.execution_session_id(),
                semantic_input_hash,
                compiled_artifact_id,
                outputs: cache.outputs,
                connections: cache.connections,
            }))
        });
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        result
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
        RuntimeValue::Relation(relation) => Some(relation.clone()),
        RuntimeValue::Series(series) => Some(series.as_relation()?),
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
            RuntimeValue::List(values) => (values.len(), ResultPageKind::Sequence),
            _ => (1, ResultPageKind::Scalar),
        };
        let mut budget = MAX_RESULT_PAGE_BYTES;
        match result {
            RuntimeValue::List(values) => {
                for value in values.iter().skip(offset).take(limit) {
                    charge_value(value, &mut budget, 0)?;
                }
            }
            value if offset == 0 => charge_value(value, &mut budget, 0)?,
            _ => {}
        }
        let values: Box<[_]> = match result {
            RuntimeValue::List(values) => values.iter().skip(offset).take(limit).cloned().collect(),
            value if offset == 0 => Box::new([value.clone()]),
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
        RuntimeValue::Ols(_) => return Err(ResultQueryApplicationError::InvalidPageRequest),
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
    use yss_graph_execution::plan::{PlanGraphId, PlanPortAddress};

    #[test]
    fn result_query_maps_opaque_graph_and_port_identities() {
        let graph =
            GraphResourcePath::new("events/main.yssbi-event").expect("test graph path is valid");
        let address = PortAddress::declared(
            yss_graph_document::NodeId::from_uuid(uuid::Uuid::nil()),
            yss_node_protocol::PortKey::new("result").expect("valid port key"),
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
