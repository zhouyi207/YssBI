//! Project-session-bound automation capabilities.

use std::sync::Arc;

use yss_automation_contract::{
    AutomationCapabilityRequest, AutomationCapabilityResult, CapabilityContractError,
    CapabilityControl, CapabilityFailure, CapabilityFailureCode, CapabilityId,
    CapabilityInvocationContext, DatasetColumnSchema, DatasetProfileInspection,
    DatasetSchemaInspection, GraphPortInspection, InspectDatasetProfileRequest,
    InspectDatasetSchemaRequest, InspectProjectRequest, InspectResultRequest, NodeCatalogMatch,
    NodeCatalogSearchResult, ProjectInspection, ProjectResourceInspection,
    ProjectResourceKindInspection, ResultCategoryInspection, ResultInspection,
    ResultValueInspection, SearchNodeCatalogRequest,
};
use yss_database_contract::DatabaseId;
use yss_database_runtime::session_api::{catalog_snapshot, revalidate_catalog_snapshot};
use yss_execution::plan::{PlotDataKind, ResultCategory, StatisticalReportKind};
use yss_execution::result::{ResultId, StoredResult};
use yss_execution::value::RuntimeValue;
use yss_graph_catalog::LocalizedCatalogItem;
use yss_graph_document::{PortAddress, PortRef};

use crate::catalog_query::{
    CatalogQueryApplicationError, LocalizedCatalogRequest, localized_node_catalog_in_session,
};
use crate::execution::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
mod graph;
pub use graph::{
    AutomationGraphAction, AutomationGraphDraft, AutomationGraphUpdate,
    prepare_automation_graph_action,
};

impl ApplicationState {
    /// Synchronous business entry point; async adapters must dispatch it to a blocking worker.
    pub fn invoke_automation_capability(
        &self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        control: &CapabilityControl,
    ) -> Result<AutomationCapabilityResult, CapabilityFailure> {
        invoke_capability(self, context, request, control)
    }
}

fn invoke_capability(
    application: &ApplicationState,
    context: CapabilityInvocationContext,
    request: AutomationCapabilityRequest,
    control: &CapabilityControl,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    control.check()?;
    let read_only =
        request.capability_id().descriptor().effect == yss_automation_contract::ToolEffect::Inspect;
    request
        .validate()
        .map_err(|error| invalid_request(request.capability_id(), error))?;
    let captured = application
        .capture_session()
        .map_err(map_session_capture_error)?;
    ensure_project_binding(&captured, &context)?;

    let result = match request {
        AutomationCapabilityRequest::InspectGraph(request) => {
            graph::inspect_saved_graph(application, &captured, request)
                .map(AutomationCapabilityResult::GraphInspection)
        }
        AutomationCapabilityRequest::SearchNodeCatalog(request) => {
            search_node_catalog(application, &captured, request)
                .map(AutomationCapabilityResult::NodeCatalogSearch)
        }
        AutomationCapabilityRequest::InspectDatasetSchema(request) => {
            inspect_dataset_schema(&captured, request)
                .map(AutomationCapabilityResult::DatasetSchemaInspection)
        }
        AutomationCapabilityRequest::InspectDatasetProfile(request) => {
            inspect_dataset_profile(&captured, request, control)
                .map(AutomationCapabilityResult::DatasetProfileInspection)
        }
        AutomationCapabilityRequest::InspectResult(request) => {
            inspect_result(application, &captured, request, control)
                .map(AutomationCapabilityResult::ResultInspection)
        }
        AutomationCapabilityRequest::InspectProject(request) => {
            inspect_project(&captured, request).map(AutomationCapabilityResult::ProjectInspection)
        }
        AutomationCapabilityRequest::ListGraphResults(request) => {
            graph::list_graph_results(&captured, request.graph_path)
                .map(AutomationCapabilityResult::GraphResults)
        }
        AutomationCapabilityRequest::CompileGraph(_)
        | AutomationCapabilityRequest::ExecuteGraph(_)
        | AutomationCapabilityRequest::SaveGraph(_) => Err(CapabilityFailure::new(
            CapabilityFailureCode::GraphClientUnavailable,
        )),
        AutomationCapabilityRequest::ApplyGraphEdit(request) => {
            let _ = request;
            Err(CapabilityFailure::new(
                CapabilityFailureCode::GraphClientUnavailable,
            ))
        }
    }?;

    // A committed mutation receipt must not be replaced by a late cancellation or timeout.
    if read_only {
        control.check()?;
    }

    application
        .revalidate_captured_session(&captured)
        .map_err(map_session_revalidation_error)?;
    Ok(result)
}

fn ensure_project_binding(
    captured: &ApplicationSession,
    context: &CapabilityInvocationContext,
) -> Result<(), CapabilityFailure> {
    let binding = context.project();
    if binding.project_instance_id() != captured.project_instance_id()
        || binding.project_session_id() != captured.project_session_id()
    {
        return Err(CapabilityFailure::new(
            CapabilityFailureCode::ProjectSessionMismatch,
        ));
    }
    Ok(())
}

fn inspect_project(
    captured: &ApplicationSession,
    _request: InspectProjectRequest,
) -> Result<ProjectInspection, CapabilityFailure> {
    let project = captured
        .project()
        .get_data()
        .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::ProjectSessionUnavailable))?;
    let mut resources =
        Vec::with_capacity(project.graphs.len() + project.databases.len() + project.charts.len());
    resources.extend(
        project
            .graphs
            .iter()
            .map(|(path, graph)| ProjectResourceInspection {
                kind: ProjectResourceKindInspection::Graph,
                resource_id: path.as_str().to_owned(),
                display_name: graph.name.clone(),
                revision: None,
            }),
    );
    resources.extend(
        project
            .databases
            .values()
            .map(|database| ProjectResourceInspection {
                kind: ProjectResourceKindInspection::Database,
                resource_id: database.id.as_str().to_owned(),
                display_name: database.name.to_string(),
                revision: None,
            }),
    );
    resources.extend(
        project
            .charts
            .iter()
            .map(|(path, chart)| ProjectResourceInspection {
                kind: ProjectResourceKindInspection::Chart,
                resource_id: path.as_str().to_owned(),
                display_name: path.display_name().as_str().to_owned(),
                revision: Some(chart.revision.get()),
            }),
    );
    enforce_result_bound(CapabilityId::InspectProject, resources.len())?;
    resources.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.resource_id.cmp(&right.resource_id))
    });
    Ok(ProjectInspection {
        project_name: project.metadata.project_name,
        resources,
    })
}

fn inspect_port(address: &PortAddress) -> GraphPortInspection {
    match &address.port {
        PortRef::Declared { key } => GraphPortInspection::Declared {
            node_id: address.node_id.to_string(),
            port_key: key.as_str().to_owned(),
        },
        PortRef::Instance {
            template,
            instance_id,
        } => GraphPortInspection::Instance {
            node_id: address.node_id.to_string(),
            template_key: template.as_str().to_owned(),
            instance_id: instance_id.to_string(),
        },
    }
}

fn search_node_catalog(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: SearchNodeCatalogRequest,
) -> Result<NodeCatalogSearchResult, CapabilityFailure> {
    let result = localized_node_catalog_in_session(
        application,
        captured,
        LocalizedCatalogRequest::new(
            captured.project_instance_id().clone(),
            request.locale.clone(),
        ),
    )
    .map_err(map_catalog_error)?;
    let (_, _, _, catalog) = result.into_transport_parts().into_fields();
    let normalized_query = request.query.to_lowercase();
    let mut matches = catalog
        .items
        .iter()
        .filter_map(|item| {
            let score = catalog_item_score(item, &normalized_query);
            (score > 0).then_some((score, item))
        })
        .map(|(score, item)| {
            (
                score,
                NodeCatalogMatch {
                    node_type_id: item.node_type_id.to_string(),
                    title: item.title.to_string(),
                    category_id: item.category_id.to_string(),
                    style_id: item.style_id.to_string(),
                    resource_path: item
                        .resource_path
                        .as_ref()
                        .map(|path| path.as_str().to_owned()),
                },
            )
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.node_type_id.cmp(&right.node_type_id))
    });
    matches.truncate(usize::from(request.limit));

    Ok(NodeCatalogSearchResult {
        locale: catalog.locale.into_string(),
        matches: matches.into_iter().map(|(_, item)| item).collect(),
    })
}

fn catalog_item_score(item: &LocalizedCatalogItem, normalized_query: &str) -> usize {
    if normalized_query.trim().is_empty() {
        return 1;
    }
    let fixed_fields = [
        item.node_type_id.as_ref(),
        item.title.as_ref(),
        item.category_id.as_ref(),
        item.style_id.as_ref(),
    ];
    let fields = fixed_fields
        .into_iter()
        .chain(item.aliases.iter().map(AsRef::as_ref))
        .chain(item.technical_terms.iter().map(AsRef::as_ref))
        .chain(item.backend_search_text.iter().map(AsRef::as_ref))
        .chain(item.resource_names.iter().map(AsRef::as_ref))
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    let exact = usize::from(
        fields
            .iter()
            .any(|value| value.contains(normalized_query.trim())),
    ) * 100;
    let tokens = normalized_query
        .split(|c: char| c.is_whitespace() || matches!(c, '.' | '_' | '-' | '/' | ',' | '|'))
        .filter(|token| !token.is_empty());
    exact
        + tokens
            .filter(|token| fields.iter().any(|value| value.contains(token)))
            .count()
}

fn inspect_dataset_schema(
    captured: &ApplicationSession,
    request: InspectDatasetSchemaRequest,
) -> Result<DatasetSchemaInspection, CapabilityFailure> {
    let project = captured.project().get_data().map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
    if !project.databases.contains_key(request.database_id.as_str()) {
        return Err(
            CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
                .with_detail("databaseId", &request.database_id),
        );
    }

    let catalog = catalog_snapshot(captured.database()).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
    let schema = catalog
        .schemas()
        .iter()
        .find(|schema| schema.database().as_str() == request.database_id)
        .ok_or_else(|| {
            CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
                .with_detail("databaseId", &request.database_id)
        })?;
    enforce_result_bound(CapabilityId::InspectDatasetSchema, schema.columns().len())?;
    let inspection = DatasetSchemaInspection {
        database_id: schema.database().as_str().to_owned(),
        runtime_revision: schema.runtime_revision().get(),
        schema_revision: schema.schema_revision().get(),
        columns: schema
            .columns()
            .iter()
            .map(|column| DatasetColumnSchema {
                name: column.name().as_str().to_owned(),
                data_type: column.data_type().to_string(),
                nullable: column.nullable(),
            })
            .collect(),
    };
    revalidate_catalog_snapshot(captured.database(), &catalog).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
    Ok(inspection)
}

fn inspect_dataset_profile(
    captured: &ApplicationSession,
    request: InspectDatasetProfileRequest,
    control: &CapabilityControl,
) -> Result<DatasetProfileInspection, CapabilityFailure> {
    let project = captured.project().get_data().map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
    if !project.databases.contains_key(request.database_id.as_str()) {
        return Err(
            CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
                .with_detail("databaseId", &request.database_id),
        );
    }
    let catalog = catalog_snapshot(captured.database()).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
    let schema = catalog
        .schemas()
        .iter()
        .find(|schema| schema.database().as_str() == request.database_id)
        .ok_or_else(|| {
            CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
                .with_detail("databaseId", &request.database_id)
        })?;
    let overview = yss_database_runtime::session_api::dataset_overview_with_control(
        captured.database(),
        DatabaseId::from_existing(request.database_id.clone().into_boxed_str()),
        &yss_relational_contract::RelationControl {
            cancellation: control.cancellation_flag(),
            deadline: control.deadline(),
            max_input_bytes: 16 * 1024 * 1024,
        },
    )
    .map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
    control.check()?;
    let inspection = DatasetProfileInspection {
        database_id: request.database_id.clone(),
        runtime_revision: schema.runtime_revision().get(),
        schema_revision: schema.schema_revision().get(),
        row_count: overview.size_shape.n_rows,
        column_count: overview.size_shape.n_columns,
        estimated_memory_bytes: overview.size_shape.estimated_dataframe_memory_bytes,
        duplicated_rows: overview.size_shape.duplicated_rows,
        numeric_columns: overview.schema_overview.numeric_cols,
        categorical_columns: overview.schema_overview.categorical_cols,
        string_columns: overview.schema_overview.string_cols,
        temporal_columns: overview.schema_overview.datetime_cols,
        boolean_columns: overview.schema_overview.bool_cols,
        total_nulls: overview.data_completeness.total_nulls,
        null_ratio: overview.data_completeness.null_ratio,
        columns_with_nulls: overview.data_completeness.cols_with_nulls,
        rows_with_nulls: overview.data_completeness.rows_with_nulls,
    };
    revalidate_catalog_snapshot(captured.database(), &catalog).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
    Ok(inspection)
}

fn inspect_result(
    application: &ApplicationState,
    captured: &ApplicationSession,
    request: InspectResultRequest,
    control: &CapabilityControl,
) -> Result<ResultInspection, CapabilityFailure> {
    let result = captured
        .execution()
        .query_result(ResultId::from_existing(request.result_id))
        .ok_or_else(|| {
            CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable)
                .with_detail("resultId", request.result_id.to_string())
        })?;
    let mut budget = ResultProjectionBudget {
        remaining: usize::from(CapabilityId::InspectResult.descriptor().maximum_results),
    };
    let value = if matches!(
        result.value().value(),
        StoredResult::Runtime(RuntimeValue::Relation(_) | RuntimeValue::Series(_))
    ) {
        let page = application
            .query_result_page_with_control(
                ResultId::from_existing(request.result_id),
                request.offset,
                usize::from(request.limit),
                &yss_relational_contract::RelationControl {
                    cancellation: control.cancellation_flag(),
                    deadline: control.deadline(),
                    max_input_bytes: 1024 * 1024,
                },
            )
            .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable))?
            .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable))?;
        let count = page
            .values
            .len()
            .min(budget.remaining / (page.columns.len().max(1) + 1));
        if count == 0 && !page.values.is_empty() {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::ResultTooLarge,
            ));
        }
        let rows = page
            .values
            .iter()
            .take(count)
            .map(|row| inspect_runtime_value(row, 0, &mut budget))
            .collect::<Result<_, _>>()?;
        ResultValueInspection::Table {
            columns: page
                .columns
                .iter()
                .map(|column| column.name.to_string())
                .collect(),
            column_types: page
                .columns
                .iter()
                .map(|column| column.data_type.to_string())
                .collect(),
            rows,
            next_offset: request.offset + count,
            has_more: page.has_more || count < page.values.len(),
        }
    } else {
        inspect_stored_result(result.value(), &mut budget)?
    };
    Ok(ResultInspection {
        result_id: request.result_id,
        category: inspect_result_category(result.value().category()),
        value,
    })
}

fn inspect_stored_result(
    result: &StoredResult,
    budget: &mut ResultProjectionBudget,
) -> Result<ResultValueInspection, CapabilityFailure> {
    let projection = match result {
        StoredResult::Runtime(value) => inspect_runtime_value(value, 0, budget)?,
        StoredResult::Scalar(value) if value.is_finite() => ResultValueInspection::Decimal(*value),
        StoredResult::Scalar(_) => {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ));
        }
        StoredResult::Text(value) => {
            let (value, truncated) = bounded_text(value, 4_096);
            ResultValueInspection::String { value, truncated }
        }
        StoredResult::Empty => ResultValueInspection::Empty,
        StoredResult::Categorized { value, .. } => return inspect_stored_result(value, budget),
    };
    Ok(projection)
}

struct ResultProjectionBudget {
    remaining: usize,
}

fn inspect_runtime_value(
    value: &RuntimeValue,
    depth: usize,
    budget: &mut ResultProjectionBudget,
) -> Result<ResultValueInspection, CapabilityFailure> {
    match value {
        RuntimeValue::Null => Ok(ResultValueInspection::Null),
        RuntimeValue::Bool(value) => Ok(ResultValueInspection::Boolean(*value)),
        RuntimeValue::Integer(value) => Ok(ResultValueInspection::Integer(*value)),
        RuntimeValue::Unsigned(value) => Ok(ResultValueInspection::Unsigned(*value)),
        RuntimeValue::Decimal(value) if value.is_finite() => {
            Ok(ResultValueInspection::Decimal(*value))
        }
        RuntimeValue::Decimal(_) => Err(CapabilityFailure::new(
            CapabilityFailureCode::InternalFailure,
        )),
        RuntimeValue::String(value) => {
            let (value, truncated) = bounded_text(value, 4_096);
            Ok(ResultValueInspection::String { value, truncated })
        }
        RuntimeValue::Resource(resource_id) => Ok(ResultValueInspection::Resource {
            resource_id: resource_id.to_string(),
        }),
        RuntimeValue::Relation(relation) => Ok(ResultValueInspection::Resource {
            resource_id: relation.binding().snapshot.to_string(),
        }),
        RuntimeValue::Series(series) => Ok(ResultValueInspection::Resource {
            resource_id: series.relation().binding().snapshot.to_string(),
        }),
        RuntimeValue::List(values) => {
            let total_count = values.len();
            if depth >= 4 {
                return Ok(ResultValueInspection::List {
                    items: Vec::new(),
                    total_count,
                    truncated: !values.is_empty(),
                });
            }
            let take = values.len().min(budget.remaining);
            budget.remaining -= take;
            let items = values[..take]
                .iter()
                .map(|value| inspect_runtime_value(value, depth + 1, budget))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ResultValueInspection::List {
                items,
                total_count,
                truncated: take < total_count,
            })
        }
        RuntimeValue::Record(values) => {
            let total_count = values.len();
            if depth >= 4 {
                return Ok(ResultValueInspection::Record {
                    entries: Default::default(),
                    total_count,
                    truncated: !values.is_empty(),
                });
            }
            let take = values.len().min(budget.remaining);
            budget.remaining -= take;
            let entries = values
                .iter()
                .take(take)
                .map(|(key, value)| {
                    inspect_runtime_value(value, depth + 1, budget)
                        .map(|value| (key.to_string(), value))
                })
                .collect::<Result<_, _>>()?;
            Ok(ResultValueInspection::Record {
                entries,
                total_count,
                truncated: take < total_count,
            })
        }
    }
}

fn bounded_text(value: &str, maximum_chars: usize) -> (String, bool) {
    let bounded = value.chars().take(maximum_chars).collect::<String>();
    let truncated = value.chars().count() > maximum_chars;
    (bounded, truncated)
}

fn inspect_result_category(category: ResultCategory) -> ResultCategoryInspection {
    match category {
        ResultCategory::Value => ResultCategoryInspection::Value,
        ResultCategory::PlotData(kind) => ResultCategoryInspection::PlotData {
            plot_kind: plot_kind(kind).to_owned(),
        },
        ResultCategory::StatisticalReport(kind) => ResultCategoryInspection::StatisticalReport {
            report_kind: report_kind(kind).to_owned(),
        },
    }
}

fn plot_kind(kind: PlotDataKind) -> &'static str {
    match kind {
        PlotDataKind::Scatter => "scatter",
        PlotDataKind::Line => "line",
        PlotDataKind::Plot => "plot",
        PlotDataKind::Ecdf => "ecdf",
        PlotDataKind::Kde => "kde",
        PlotDataKind::Histogram => "histogram",
        PlotDataKind::Correlation => "correlation",
        PlotDataKind::Correlogram => "correlogram",
    }
}

fn report_kind(kind: StatisticalReportKind) -> &'static str {
    match kind {
        StatisticalReportKind::OlsSummary => "ols_summary",
        StatisticalReportKind::BinarySummary => "binary_summary",
        StatisticalReportKind::Iv2slsSummary => "iv_2sls_summary",
        StatisticalReportKind::IvLimlSummary => "iv_liml_summary",
        StatisticalReportKind::PraisSummary => "prais_summary",
        StatisticalReportKind::VarSummary => "var_summary",
        StatisticalReportKind::VarSoc => "var_soc",
        StatisticalReportKind::PanelSummary => "panel_summary",
        StatisticalReportKind::PanelDid => "panel_did",
        StatisticalReportKind::DfAdfSummary => "df_adf_summary",
        StatisticalReportKind::DfAdfSummaryList => "df_adf_summary_list",
        StatisticalReportKind::VecSummary => "vec_summary",
        StatisticalReportKind::VecRankSummary => "vec_rank_summary",
    }
}

fn enforce_result_bound(
    capability_id: CapabilityId,
    result_count: usize,
) -> Result<(), CapabilityFailure> {
    let maximum = usize::from(capability_id.descriptor().maximum_results);
    if result_count > maximum {
        return Err(
            CapabilityFailure::new(CapabilityFailureCode::ResultTooLarge)
                .with_detail("capabilityId", capability_id.as_str())
                .with_detail("maximumResults", maximum.to_string()),
        );
    }
    Ok(())
}

fn invalid_request(
    capability_id: CapabilityId,
    error: CapabilityContractError,
) -> CapabilityFailure {
    let failure = CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
        .with_detail("capabilityId", capability_id.as_str());
    match error {
        CapabilityContractError::InvalidField(field) => failure.with_detail("field", field),
        CapabilityContractError::FieldTooLong { field, maximum } => failure
            .with_detail("field", field)
            .with_detail("maximumBytes", maximum.to_string()),
        CapabilityContractError::InvalidLimit { maximum } => {
            failure.with_detail("maximumResults", maximum.to_string())
        }
    }
}

fn map_session_capture_error(_: SessionCaptureError) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::ProjectSessionUnavailable)
}

fn map_session_revalidation_error(error: SessionRevalidationError) -> CapabilityFailure {
    match error {
        SessionRevalidationError::Unavailable(_) => {
            CapabilityFailure::new(CapabilityFailureCode::ProjectSessionUnavailable)
        }
        SessionRevalidationError::Changed => {
            CapabilityFailure::new(CapabilityFailureCode::ProjectSessionChanged)
        }
    }
}

fn map_catalog_error(error: CatalogQueryApplicationError) -> CapabilityFailure {
    match error {
        CatalogQueryApplicationError::SessionCapture(error) => map_session_capture_error(error),
        CatalogQueryApplicationError::SessionChanged => {
            CapabilityFailure::new(CapabilityFailureCode::ProjectSessionChanged)
        }
        CatalogQueryApplicationError::CatalogProjectStale => {
            CapabilityFailure::new(CapabilityFailureCode::ProjectSessionMismatch)
        }
        CatalogQueryApplicationError::Project(_)
        | CatalogQueryApplicationError::Database(_)
        | CatalogQueryApplicationError::Contract(_)
        | CatalogQueryApplicationError::Graph(_) => {
            CapabilityFailure::new(CapabilityFailureCode::CatalogUnavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::graph::editor_mutation;
    use super::*;
    use yss_automation_contract::GraphEditOperation;
    use yss_graph_document::{NodeId, NodePosition};
    use yss_graph_editor::EditorGraphMutation;

    #[test]
    fn result_bounds_fail_closed_at_the_contract_descriptor_limit() {
        let maximum = usize::from(
            CapabilityId::InspectDatasetSchema
                .descriptor()
                .maximum_results,
        );

        assert!(enforce_result_bound(CapabilityId::InspectDatasetSchema, maximum).is_ok());
        assert_eq!(
            enforce_result_bound(CapabilityId::InspectDatasetSchema, maximum + 1)
                .unwrap_err()
                .code,
            CapabilityFailureCode::ResultTooLarge
        );
    }

    #[test]
    fn result_projection_truncates_nested_values_at_the_shared_budget() {
        let value = RuntimeValue::List(
            (0..5)
                .map(RuntimeValue::Integer)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let mut budget = ResultProjectionBudget { remaining: 3 };

        let ResultValueInspection::List {
            items,
            total_count,
            truncated,
        } = inspect_runtime_value(&value, 0, &mut budget).unwrap()
        else {
            panic!("list projection changed shape");
        };
        assert_eq!(items.len(), 3);
        assert_eq!(total_count, 5);
        assert!(truncated);
        assert_eq!(budget.remaining, 0);
    }

    #[test]
    fn graph_edit_mapping_preserves_typed_node_positions_and_rejects_bad_ids() {
        let node_id = uuid::Uuid::from_u128(1);
        let mutation = editor_mutation(
            GraphEditOperation::MoveNodes {
                positions: vec![yss_automation_contract::GraphEditPosition {
                    node_id: node_id.to_string(),
                    x: 10.0,
                    y: 20.0,
                }],
            },
            &[],
        )
        .unwrap();
        let EditorGraphMutation::MoveNodes { positions } = mutation else {
            panic!("move-node mapping changed shape");
        };
        assert_eq!(positions[0].node_id, NodeId::from_uuid(node_id));
        assert_eq!(positions[0].position, NodePosition { x: 10.0, y: 20.0 });

        assert_eq!(
            editor_mutation(
                GraphEditOperation::DeleteNodes {
                    node_ids: vec!["not-a-uuid".to_owned()],
                },
                &[],
            )
            .unwrap_err()
            .code,
            CapabilityFailureCode::InvalidRequest
        );
    }
}
