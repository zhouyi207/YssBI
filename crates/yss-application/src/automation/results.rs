//! Harness projections of existing immutable results; no result ownership or cache lives here.
use crate::graph::results::{
    self, ResultQueryApplicationError, ResultStructure, ResultValueProjection,
};
use crate::{ApplicationState, session::ApplicationSession};
use std::{collections::BTreeSet, sync::Arc};
use yss_data_contract::TabularScalar;
use yss_graph_document::GraphResourcePath;
use yss_graph_execution::{
    identity::ExecutionSessionId,
    result::{ResultId, ResultReadSnapshot, ResultReference, StoredResultSnapshot},
    run_registry::RunId,
};
use yss_harness_contract::*;
use yss_node_kernel::RuntimeValue;

fn reference(value: &ResultRef) -> Result<ResultReference, CapabilityFailure> {
    let id = uuid::Uuid::parse_str(value.execution_session_id())
        .map_err(|_| unavailable("reference_expired"))?;
    Ok(ResultReference {
        execution_session_id: ExecutionSessionId::new(id),
        result_id: ResultId::from_existing(value.result_id()),
    })
}

pub(super) fn result_ref(reference: ResultReference) -> ResultRef {
    ResultRef::new(
        reference.execution_session_id.as_uuid().to_string(),
        reference.result_id.get(),
    )
}

fn validity(value: yss_graph_execution::result::ResultValidity) -> ResultValidity {
    use yss_graph_execution::result::ResultValidity as V;
    match value {
        V::CurrentValid => ResultValidity::CurrentValid,
        V::CurrentStale => ResultValidity::CurrentStale,
        V::Retained => ResultValidity::Retained,
    }
}

fn snapshot(
    captured: &ApplicationSession,
    reference: ResultReference,
) -> Result<ResultReadSnapshot, CapabilityFailure> {
    results::query_result_with_validity(captured, reference)
        .map_err(query_error)?
        .ok_or_else(|| unavailable("result_reclaimed"))
}

fn table_marker(reference: &ResultRef, part: &str, count: usize) -> RuntimeValue {
    RuntimeValue::Record(Arc::new(
        [
            (
                "kind".into(),
                RuntimeValue::from(TabularScalar::String("tableRef".into())),
            ),
            (
                "tableRef".into(),
                RuntimeValue::from(TabularScalar::String(
                    String::from(TableRef::new(reference.clone(), Some(part.into()))).into(),
                )),
            ),
            (
                "rowCount".into(),
                RuntimeValue::from(TabularScalar::Unsigned(count as u64)),
            ),
        ]
        .into(),
    ))
}

pub(super) fn inspect_result(
    application: &ApplicationState,
    captured: &ApplicationSession,
    request: InspectResultRequest,
) -> Result<ResultInspection, CapabilityFailure> {
    let reference = reference(&request.result_ref)?;
    let read = snapshot(captured, reference)?;
    let structure = ResultStructure::project(read.result.value());
    let value = if matches!(
        read.result.value().value().unannotated(),
        RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::List(_)
    ) {
        let known = structure.columns.is_some();
        let columns = structure.columns.unwrap_or_default();
        let selected = columns
            .iter()
            .skip(request.schema_offset)
            .take(request.schema_limit)
            .map(column)
            .collect::<Vec<_>>();
        ResultValueInspection::Tabular {
            table_ref: TableRef::new(request.result_ref.clone(), None),
            schema_page: InspectionPage::known(
                request.schema_offset,
                selected.len(),
                columns.len(),
            ),
            columns: selected,
            row_count: structure.total_count,
            schema_known: known,
        }
    } else {
        let projection = application
            .query_result_projection_with_tables(reference, &|part, count| {
                table_marker(&request.result_ref, part, count)
            })
            .map_err(query_error)?
            .ok_or_else(|| unavailable("result_reclaimed"))?;
        let value = match projection {
            ResultValueProjection::Value(value) => {
                crate::result_encoding::runtime_value_to_json(&value).map_err(query_error)?
            }
            ResultValueProjection::LinearModel(model) => {
                serde_json::to_value(model).map_err(|_| unavailable("unrepresentable_value"))?
            }
            ResultValueProjection::LinearReport(report) => {
                crate::result_encoding::report_to_json_with_refs(
                    *report,
                    serde_json::json!(request.result_ref),
                    &|part, count| serde_json::json!({"kind": "tableRef", "tableRef": TableRef::new(request.result_ref.clone(), Some(part.into())), "rowCount": count}),
                )
            }
        };
        ResultValueInspection::Json(
            crate::result_encoding::bound_inline_json(value).map_err(query_error)?,
        )
    };
    // Revalidate ownership and current validity after projection, preserving the original value.
    let current = snapshot(captured, reference)?;
    Ok(ResultInspection {
        result_ref: request.result_ref,
        validity: validity(current.validity),
        category: super::inspect_result_category(read.result.value().category()),
        value,
    })
}

pub(super) fn read_table(
    application: &ApplicationState,
    captured: &ApplicationSession,
    request: ReadResultTableRequest,
    control: &CapabilityControl,
) -> Result<ResultTablePage, CapabilityFailure> {
    let reference = reference(request.table_ref.result_ref())?;
    let read = snapshot(captured, reference)?;
    control.check()?;
    let (page, column_page) = if let Some(part) = request.table_ref.part() {
        let part = part.parse().map_err(|_| unavailable("table_not_found"))?;
        let mut page = application
            .query_result_table_with_refs(
                reference,
                part,
                request.offset,
                request.limit,
                &|part, count| table_marker(request.table_ref.result_ref(), part, count),
            )
            .map_err(report_error)?;
        let (names, column_page) = selected_columns(&page.columns, &request)?;
        let indices = results::column_indices(&page.columns, &names).map_err(query_error)?;
        page.columns = indices.iter().map(|&i| page.columns[i].clone()).collect();
        page.values = page
            .values
            .iter()
            .map(|value| {
                let RuntimeValue::List(values) = value else {
                    return Err(unavailable("unrepresentable_value"));
                };
                Ok(RuntimeValue::List(
                    indices.iter().map(|&i| values[i].clone()).collect(),
                ))
            })
            .collect::<Result<_, CapabilityFailure>>()?;
        (page, column_page)
    } else {
        let structure = ResultStructure::project(read.result.value());
        if !matches!(
            read.result.value().value().unannotated(),
            RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::List(_)
        ) {
            return Err(unavailable("table_not_found"));
        }
        let schema = structure
            .columns
            .ok_or_else(|| unavailable("schema_unavailable"))?;
        let (names, column_page) = selected_columns(&schema, &request)?;
        let page = application
            .query_result_columns_with_control(
                reference,
                &names,
                request.offset,
                request.limit,
                &yss_relational_contract::RelationControl {
                    cancellation: control.cancellation_flag(),
                    deadline: control.deadline(),
                    max_input_bytes: MAX_CAPABILITY_RESULT_BYTES,
                },
            )
            .map_err(query_error)?
            .ok_or_else(|| unavailable("result_reclaimed"))?;
        (page, column_page)
    };
    let rows = page
        .values
        .iter()
        .map(crate::result_encoding::runtime_value_to_json)
        .collect::<Result<Vec<_>, _>>()
        .map_err(query_error)?;
    control.check()?;
    let current = snapshot(captured, reference)?;
    let paging = InspectionPage {
        offset: page.offset,
        returned: rows.len(),
        total: page.total_count,
        has_more: page.has_more,
        next_offset: page.has_more.then_some(page.offset + rows.len()),
    };
    Ok(ResultTablePage {
        table_ref: request.table_ref,
        validity: validity(current.validity),
        columns: page.columns.iter().map(column).collect(),
        rows,
        page: paging,
        column_page,
    })
}

fn selected_columns(
    schema: &[yss_relational_contract::RelationColumn],
    request: &ReadResultTableRequest,
) -> Result<(Vec<String>, InspectionPage), CapabilityFailure> {
    let candidates = if request.columns.is_empty() {
        (0..schema.len()).collect()
    } else {
        results::column_indices(schema, &request.columns).map_err(query_error)?
    };
    let names = candidates
        .iter()
        .skip(request.column_offset)
        .take(request.column_limit)
        .map(|&i| schema[i].name.to_string())
        .collect::<Vec<_>>();
    if names.is_empty() && !schema.is_empty() {
        return Err(CapabilityContractError::InvalidField("columnOffset")
            .into_failure(CapabilityId::ReadResultTable));
    }
    let page = InspectionPage::known(request.column_offset, names.len(), candidates.len());
    Ok((names, page))
}
fn column(value: &yss_relational_contract::RelationColumn) -> ResultColumn {
    ResultColumn {
        name: value.name.to_string(),
        data_type: value.data_type.to_string(),
    }
}

pub(super) fn summary(
    snapshot: &StoredResultSnapshot,
    state: ResultValidity,
) -> GraphResultReference {
    GraphResultReference {
        result_ref: result_ref(snapshot.provenance().reference()),
        run_id: snapshot.provenance().run_id().get(),
        output: snapshot.output().port().as_str().into(),
        validity: state,
        category: super::inspect_result_category(snapshot.value().category()),
    }
}

pub(super) fn list_results(
    captured: &ApplicationSession,
    request: ListGraphResultsRequest,
) -> Result<GraphResults, CapabilityFailure> {
    let path = GraphResourcePath::new(&request.graph.id)
        .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::InvalidRequest))?;
    let catalog = super::resources::read_index(captured)?;
    let resource = request.graph.resource();
    if GraphResourceRef::for_path(path.as_str()).kind != request.graph.kind
        || !catalog
            .resources
            .iter()
            .any(|entry| entry.resource == resource)
    {
        return Err(CapabilityFailure::new(
            CapabilityFailureCode::GraphUnavailable,
        ));
    }
    let nodes = request
        .node_ids
        .iter()
        .map(|id| uuid::Uuid::parse_str(id).map(|id| id.to_string()))
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|_| {
            CapabilityContractError::InvalidField("nodeIds")
                .into_failure(CapabilityId::ListGraphResults)
        })?;
    let outputs = request
        .outputs
        .into_iter()
        .map(super::graph::parse_edit_port)
        .map(|value| value.map(|port| port.to_string()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let entries = results::query_graph_result_entries(
        captured,
        &path,
        request.run_id.map(RunId::from_existing),
    )
    .map_err(query_error)?;
    captured
        .project()
        .validate_project_index_version(
            captured.project_instance_id(),
            catalog.publication_revision,
            catalog.authority_generation,
        )
        .map_err(super::map_project_inspection_error)?;
    let selected = entries
        .iter()
        .filter(|entry| {
            let node = entry
                .result
                .value()
                .output_contract()
                .and_then(|contract| contract.source.node());
            (nodes.is_empty() || node.is_some_and(|node| nodes.contains(node.as_str())))
                && (outputs.is_empty() || outputs.contains(entry.result.output().port().as_str()))
        })
        .collect::<Vec<_>>();
    let references = selected
        .iter()
        .skip(request.offset)
        .take(request.limit)
        .map(|entry| summary(&entry.result, validity(entry.validity)))
        .collect::<Vec<_>>();
    let known_run = request.run_id.is_some_and(|run| {
        !entries.is_empty()
            || captured.execution_snapshot().iter().any(|event| {
                event.identity().run_id().get() == run && event.identity().graph_path() == &path
            })
    });
    let observation = request
        .run_id
        .filter(|_| known_run)
        .and_then(|run| super::graph::inspect_run(captured, run));
    let run_status = request.run_id.map(|_| {
        observation
            .as_ref()
            .map_or_else(|| "unavailable".into(), |(status, _)| status.clone())
    });
    Ok(GraphResults {
        graph: request.graph,
        run_status,
        run_timing: observation.map(|(_, timing)| timing),
        page: InspectionPage::known(request.offset, references.len(), selected.len()),
        results: references,
    })
}

fn unavailable(reason: &str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable).with_detail("reason", reason)
}
fn query_error(error: ResultQueryApplicationError) -> CapabilityFailure {
    use yss_relational_contract::RelationError;
    match error {
        ResultQueryApplicationError::SessionChanged => unavailable("reference_expired"),
        ResultQueryApplicationError::PageTooLarge
        | ResultQueryApplicationError::Relation(RelationError::MemoryLimitExceeded) => {
            CapabilityFailure::new(CapabilityFailureCode::ResultTooLarge)
        }
        ResultQueryApplicationError::InvalidPageRequest => {
            CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
                .with_detail("reason", "invalid_table_selection")
        }
        ResultQueryApplicationError::Relation(RelationError::Cancelled) => {
            CapabilityFailure::new(CapabilityFailureCode::Cancelled)
        }
        ResultQueryApplicationError::Relation(RelationError::DeadlineExceeded) => {
            CapabilityFailure::new(CapabilityFailureCode::DeadlineElapsed)
        }
        _ => unavailable("result_unavailable"),
    }
}
fn report_error(error: results::report::ReportQueryError) -> CapabilityFailure {
    use results::report::ReportQueryError as E;
    unavailable(match error {
        E::Stale => "reference_expired",
        E::Unavailable => "result_reclaimed",
        E::WrongKind | E::InvalidRequest => "table_not_found",
        _ => "result_unavailable",
    })
}
