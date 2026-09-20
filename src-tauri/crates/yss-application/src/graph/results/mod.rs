use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use thiserror::Error;

use super::inputs::{GraphInputError, GraphResolutionContext};
use crate::session::{ApplicationSession, ApplicationState, SessionCaptureError};
use yss_data_contract::TabularScalar;
use yss_graph_document::{GraphResourcePath, PortAddress};
use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use yss_graph_execution::result::{
    ResultReference, ResultRetentionError, StoredResult, StoredResultSnapshot,
};
use yss_node_kernel::RuntimeValue;
use yss_relational_contract::{RelationColumn, RelationControl, RelationError};

pub mod report;
mod retention;
mod structure;
pub(crate) use structure::ResultStructure;

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
    #[error("result cannot be represented as JSON")]
    UnrepresentableValue,
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
    pub outputs:
        std::collections::BTreeMap<PlanOutputRef, yss_graph_execution::result::ResultCacheState>,
    pub connections: Vec<yss_graph_execution::result::ConnectionResultState>,
}

fn with_current_graph_results<T>(
    captured: &ApplicationSession,
    graph: &GraphResourcePath,
    read: impl FnOnce(Option<&mut GraphResolutionContext>) -> Result<T, ResultQueryApplicationError>,
) -> Result<T, ResultQueryApplicationError> {
    let keys = captured.execution().result_resource_keys(graph.as_str());
    let mut context = if keys.is_empty() {
        None
    } else {
        let context = GraphResolutionContext::capture_catalog(captured)?;
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
    /// Full result metadata and JSON values, with native report data kept behind table references.
    pub fn query_result_json(
        &self,
        reference: ResultReference,
    ) -> Result<Option<serde_json::Value>, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        let Some(snapshot) = captured.execution().query_result(reference.result_id) else {
            return Ok(None);
        };
        let value = match snapshot.value().value().unannotated() {
            RuntimeValue::LinearRegression(result) => {
                Ok(report::report_projection(reference, result).into_json())
            }
            RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::List(_) => {
                Err(ResultQueryApplicationError::InvalidPageRequest)
            }
            value => runtime_value_to_json(value),
        };
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        if captured
            .execution()
            .query_result(reference.result_id)
            .is_none()
        {
            return Ok(None);
        }
        value.map(Some)
    }

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
        let result = with_current_graph_results(&captured, &graph, |_| {
            let Some(cache) = captured
                .execution()
                .query_result_cache_states(graph.as_str(), &semantic_input_hash)
            else {
                return Ok(None);
            };
            Ok(Some(GraphResultState {
                execution_session_id: captured.execution_session_id(),
                semantic_input_hash,
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
    control.check()?;
    let structure = ResultStructure::project(result);
    let result = result.value().unannotated();
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
            || structure.columns.as_ref().is_some_and(|columns| {
                columns.len() != page.columns.len()
                    || columns.iter().zip(&page.columns).any(|(expected, actual)| {
                        expected.name != actual.name || expected.data_type != actual.data_type
                    })
            })
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
        let count = structure
            .total_count
            .ok_or(ResultQueryApplicationError::InvalidPageRequest)?;
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
            RuntimeValue::List(values) => values
                .iter()
                .skip(offset)
                .take(limit)
                .map(|value| RuntimeValue::List(Arc::from([value.clone()])))
                .collect(),
            value if offset == 0 => Box::new([value.clone()]),
            _ => Box::new([]),
        };
        ResultPageProjection {
            offset: offset.min(count),
            requested_limit: limit,
            total_count: Some(count),
            has_more: offset.saturating_add(values.len()) < count,
            kind: structure.kind,
            columns: structure.columns.unwrap_or_default(),
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
    control.check()?;
    Ok(page)
}

fn scalar_value(value: &TabularScalar) -> RuntimeValue {
    RuntimeValue::Scalar(value.display_value())
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
    let bytes = match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::String(value)) | RuntimeValue::Resource(value) => {
            value.len().checked_mul(6)
        }
        RuntimeValue::List(values) => {
            for value in values.iter() {
                charge_value(value, remaining, depth + 1)?;
            }
            Some(2)
        }
        RuntimeValue::Record(values) => {
            for (key, value) in values.iter() {
                *remaining = remaining
                    .checked_sub(key.len().saturating_mul(6))
                    .ok_or(ResultQueryApplicationError::PageTooLarge)?;
                charge_value(value, remaining, depth + 1)?;
            }
            Some(2)
        }
        RuntimeValue::LinearRegression(_) => {
            return Err(ResultQueryApplicationError::InvalidPageRequest);
        }
        _ => Some(24),
    }
    .ok_or(ResultQueryApplicationError::PageTooLarge)?;
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or(ResultQueryApplicationError::PageTooLarge)?;
    Ok(())
}

pub(crate) fn runtime_value_to_json(
    value: &RuntimeValue,
) -> Result<serde_json::Value, ResultQueryApplicationError> {
    Ok(match value {
        RuntimeValue::Annotated(value) => runtime_value_to_json(value.value())?,
        RuntimeValue::Scalar(value) => serde_json::to_value(value.display_value())
            .map_err(|_| ResultQueryApplicationError::UnrepresentableValue)?,
        RuntimeValue::Resource(value) => value.as_ref().into(),
        RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::LinearRegression(_) => {
            return Err(ResultQueryApplicationError::UnrepresentableValue);
        }
        RuntimeValue::List(values) => values
            .iter()
            .map(runtime_value_to_json)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        RuntimeValue::Record(values) => values
            .iter()
            .map(|(key, value)| Ok((key.to_string(), runtime_value_to_json(value)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, ResultQueryApplicationError>>()?
            .into_iter()
            .collect::<serde_json::Map<_, _>>()
            .into(),
    })
}

#[cfg(test)]
mod tests {
    mod paging;
    use super::*;
    use yss_graph_execution::plan::{PlanGraphId, PlanPortAddress};

    #[test]
    fn annotated_series_paging_preserves_sequence_shape_and_offsets() {
        let result = StoredResult::new(
            RuntimeValue::List(std::sync::Arc::from([
                RuntimeValue::Scalar(TabularScalar::String("001".into())),
                RuntimeValue::Scalar(TabularScalar::Null),
            ]))
            .with_metadata(yss_data_contract::ConversionMetadata {
                semantic: yss_data_contract::ColumnSemantic::new(
                    yss_data_contract::SemanticType::Identifier,
                ),
                temporal: None,
                dummy_base_level: None,
            })
            .unwrap(),
        );
        let control = RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(5),
            max_input_bytes: 1024,
        };
        let page = project_result_page(&result, 1, 1, &control).unwrap();
        assert!(matches!(page.kind, ResultPageKind::Sequence));
        assert_eq!(page.total_count, Some(2));
        assert_eq!(
            page.values.as_ref(),
            &[RuntimeValue::List(Arc::from([RuntimeValue::Scalar(
                TabularScalar::Null
            )]))]
        );
        assert_eq!(page.columns.len(), 1);
        assert_eq!(page.columns[0].data_type.as_ref(), "Identifier");
        assert!(!page.has_more);

        use yss_data_contract::ValueType;
        use yss_graph_execution::plan::{PlanOutputContract, PlanSourceIdentity, ResultCategory};
        let contract = PlanOutputContract {
            data_type: ValueType::DataSeries(Box::new(ValueType::number())),
            schema: None,
            category: ResultCategory::Value,
            source: PlanSourceIdentity::new(PlanGraphId::from_existing("test".into()), None, None),
        };
        for values in [vec![], vec![RuntimeValue::Scalar(TabularScalar::Null); 3]] {
            let count = values.len();
            let stored = StoredResult::new(RuntimeValue::List(values.into()))
                .with_output_contract(contract.clone());
            let descriptor = ResultStructure::project(&stored);
            for offset in [0, 1, 9] {
                let page = project_result_page(&stored, offset, 1, &control).unwrap();
                assert_eq!(page.columns[0].data_type.as_ref(), "Numeric");
                assert_eq!(
                    page.columns[0].name,
                    descriptor.columns.as_ref().unwrap()[0].name
                );
                assert_eq!(page.total_count, Some(count));
                assert_eq!(page.offset, offset.min(count));
                assert!(
                    page.values
                        .iter()
                        .all(|row| matches!(row, RuntimeValue::List(v) if v.len() == 1))
                );
            }
        }
        // A nested list is one structured cell, never mistaken for a multi-column row.
        let nested =
            RuntimeValue::List(Arc::from([RuntimeValue::Scalar(TabularScalar::Integer(7))]));
        let stored = StoredResult::new(RuntimeValue::List(Arc::from([nested.clone()])));
        let page = project_result_page(&stored, 0, 1, &control).unwrap();
        assert_eq!(
            runtime_value_to_json(&page.values[0]).unwrap(),
            serde_json::json!([[7]])
        );
        assert_eq!(page.columns[0].data_type.as_ref(), "Any");
        let scalar = StoredResult::new(RuntimeValue::Scalar(TabularScalar::Integer(7)));
        let page = project_result_page(&scalar, 0, 1, &control).unwrap();
        assert_eq!(page.kind, ResultStructure::project(&scalar).kind);
        assert!(page.columns.is_empty());
        assert_eq!(
            runtime_value_to_json(&page.values[0]).unwrap(),
            serde_json::json!(7)
        );
    }

    #[test]
    fn relation_and_series_pages_share_descriptor_column_names_and_types() {
        use arrow::{
            array::{ArrayRef, Date32Array, StringArray, UInt64Array},
            datatypes::{DataType, Field, Schema},
            record_batch::RecordBatch,
        };
        use yss_relational_contract::RelationFactory;
        let control = RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_input_bytes: 1024 * 1024,
        };
        let factory = yss_database_runtime::dataset_query_engine().unwrap();
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(vec![
                Field::new("label", DataType::Utf8, false),
                Field::new("date", DataType::Date32, false),
                Field::new("id", DataType::UInt64, false),
            ])),
            vec![
                Arc::new(StringArray::from(vec!["a", "b"])) as ArrayRef,
                Arc::new(Date32Array::from(vec![0, 1])),
                Arc::new(UInt64Array::from(vec![u64::MAX, 1])),
            ],
        )
        .unwrap();
        let relation = factory.materialize(batch, &control).unwrap();
        for value in [
            RuntimeValue::Relation(relation.clone()),
            RuntimeValue::Series(relation.select_series("label").unwrap()),
        ] {
            let result = StoredResult::new(value);
            let descriptor = ResultStructure::project(&result);
            let columns = descriptor.columns.unwrap();
            assert_eq!(columns[0].data_type.as_ref(), "String");
            for offset in [0, 1, 2] {
                let page = project_result_page(&result, offset, 1, &control).unwrap();
                assert_eq!(page.columns.len(), columns.len());
                for (expected, actual) in columns.iter().zip(&page.columns) {
                    assert_eq!(expected.name, actual.name);
                    assert_eq!(expected.data_type, actual.data_type);
                }
                if offset == 0 && columns.len() == 3 {
                    let row = runtime_value_to_json(&page.values[0]).unwrap();
                    assert_eq!(row[0], "a");
                    assert_eq!(row[2], u64::MAX.to_string());
                }
            }
        }
    }

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
