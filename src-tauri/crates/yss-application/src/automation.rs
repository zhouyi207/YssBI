//! Project-session-bound automation capabilities.

use std::sync::Arc;

use yss_database_contract::DatabaseId;
use yss_database_runtime::session_api::{catalog_snapshot, revalidate_catalog_snapshot};
use yss_graph_document::{PortAddress, PortRef};
use yss_graph_execution::plan::{PlotDataKind, ResultCategory, StatisticalReportKind};
use yss_graph_execution::result::ResultId;
use yss_harness_contract::{
    AutomationCapabilityRequest, AutomationCapabilityResult, CapabilityContractError,
    CapabilityControl, CapabilityFailure, CapabilityFailureCode, CapabilityId,
    CapabilityInvocationContext, DatasetColumnSchema, DatasetProfileInspection,
    DatasetSchemaInspection, GraphPortInspection, InspectDatasetProfileRequest,
    InspectDatasetSchemaRequest, InspectProjectRequest, InspectResultRequest, NodeCatalogMatch,
    NodeCatalogSearchResult, ProjectInspection, ProjectResourceInspection,
    ProjectResourceKindInspection, ResultCategoryInspection, ResultInspection,
    ResultValueInspection, SearchNodeCatalogRequest,
};
use yss_node_catalog::LocalizedCatalogItem;
use yss_node_kernel::RuntimeValue;

use crate::graph::catalog::{
    CatalogQueryApplicationError, LocalizedCatalogRequest, localized_node_catalog_in_session,
};
use crate::session::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
mod graph;
pub use graph::invoke_graph_capability;

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
        request.capability_id().descriptor().effect == yss_harness_contract::ToolEffect::Inspect;
    request
        .validate()
        .map_err(|error| invalid_request(request.capability_id(), error))?;
    let captured = application
        .capture_session()
        .map_err(map_session_capture_error)?;
    ensure_project_binding(&captured, &context)?;

    let result = match request {
        request @ (AutomationCapabilityRequest::InspectGraph(_)
        | AutomationCapabilityRequest::ApplyGraphEdit(_)
        | AutomationCapabilityRequest::ValidateGraph(_)
        | AutomationCapabilityRequest::ExecuteGraph(_)
        | AutomationCapabilityRequest::SaveGraph(_)) => {
            return graph::invoke_graph_capability(application, context, request, control);
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
        .read_project_index(captured.project_instance_id())
        .map_err(map_project_inspection_error)?;
    let mut resources =
        Vec::with_capacity(project.graphs.len() + project.databases.len() + project.charts.len());
    resources.extend(
        project
            .graphs
            .iter()
            .map(|graph| ProjectResourceInspection {
                kind: ProjectResourceKindInspection::Graph,
                resource_id: graph.path.clone(),
                display_name: graph.name.clone(),
                revision: None,
            }),
    );
    resources.extend(
        project
            .databases
            .iter()
            .map(|database| ProjectResourceInspection {
                kind: ProjectResourceKindInspection::Database,
                resource_id: database.id.clone(),
                display_name: database.name.clone().unwrap_or_else(|| database.id.clone()),
                revision: None,
            }),
    );
    resources.extend(
        project
            .charts
            .iter()
            .map(|chart| ProjectResourceInspection {
                kind: ProjectResourceKindInspection::Chart,
                resource_id: chart.chart_path.as_str().to_owned(),
                display_name: chart.name.clone(),
                revision: None,
            }),
    );
    enforce_result_bound(CapabilityId::InspectProject, resources.len())?;
    resources.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.resource_id.cmp(&right.resource_id))
    });
    captured
        .project()
        .validate_project_index_version(
            captured.project_instance_id(),
            project.publication_revision,
            project.authority_generation(),
        )
        .map_err(map_project_inspection_error)?;
    Ok(ProjectInspection {
        project_name: project.project_name,
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
        .filter(|item| item.available)
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
    let execution_session_id =
        uuid::Uuid::parse_str(&request.execution_session_id).map_err(|_| {
            invalid_request(
                CapabilityId::InspectResult,
                CapabilityContractError::InvalidField("executionSessionId"),
            )
        })?;
    if execution_session_id != captured.execution_session_id().as_uuid() {
        return Err(CapabilityFailure::new(
            CapabilityFailureCode::ResultUnavailable,
        ));
    }
    let result = captured
        .execution()
        .query_result(ResultId::from_existing(request.result_id))
        .ok_or_else(|| {
            CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable)
                .with_detail("resultId", request.result_id.to_string())
        })?;
    let reference = result.provenance().reference();
    let page = if let Some(part) = request.part.as_deref() {
        let part = match part {
            "coefficients" => crate::graph::results::report::ResultTablePart::Coefficients,
            "observations" => crate::graph::results::report::ResultTablePart::Observations,
            _ => {
                return Err(invalid_request(
                    CapabilityId::InspectResult,
                    CapabilityContractError::InvalidField("part"),
                ));
            }
        };
        Some(
            application
                .query_result_table(reference, part, request.offset, usize::from(request.limit))
                .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable))?,
        )
    } else if matches!(
        result.value().value().unannotated(),
        RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::List(_)
    ) {
        Some(
            application
                .query_result_page_with_control(
                    reference,
                    request.offset,
                    usize::from(request.limit),
                    &yss_relational_contract::RelationControl {
                        cancellation: control.cancellation_flag(),
                        deadline: control.deadline(),
                        max_input_bytes: 1024 * 1024,
                    },
                )
                .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable))?
                .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable))?,
        )
    } else {
        None
    };
    let value = if let Some(page) = page {
        let rows = page
            .values
            .iter()
            .map(crate::graph::results::runtime_value_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))?;
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
            next_offset: page.offset + rows.len(),
            has_more: page.has_more,
            rows,
        }
    } else {
        ResultValueInspection::Json(
            application
                .query_result_json(reference)
                .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable))?
                .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable))?,
        )
    };
    Ok(ResultInspection {
        result_id: request.result_id,
        category: inspect_result_category(result.value().category()),
        value,
    })
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
        StatisticalReportKind::LinearRegressionSummary => "linear_regression_summary",
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

fn map_project_inspection_error(error: yss_project::ProjectOperationError) -> CapabilityFailure {
    use yss_project::ProjectOperationError;
    CapabilityFailure::new(match error {
        ProjectOperationError::StaleProjectLifecycle { .. } => {
            CapabilityFailureCode::ProjectSessionChanged
        }
        ProjectOperationError::CatalogResourceStale { .. } => {
            CapabilityFailureCode::RevisionConflict
        }
        ProjectOperationError::ProjectLifecycleAdmissionClosed { .. }
        | ProjectOperationError::ProjectRecoveryRequired { .. } => {
            CapabilityFailureCode::ProjectSessionUnavailable
        }
        _ => CapabilityFailureCode::CatalogUnavailable,
    })
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
    use yss_data_contract::TabularScalar;
    use yss_graph_document::{NodeId, NodePosition};
    use yss_graph_editor::EditorGraphMutation;
    use yss_harness_contract::GraphEditOperation;

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
    fn result_json_preserves_long_text_deep_objects_and_complete_arrays() {
        let text = "x".repeat(2_000_000);
        let mut value = RuntimeValue::Record(std::sync::Arc::new(
            [
                (
                    "text".into(),
                    RuntimeValue::Scalar(TabularScalar::String(text.clone().into())),
                ),
                (
                    "coefficients".into(),
                    RuntimeValue::List(
                        (0..150)
                            .map(|value| RuntimeValue::Scalar(TabularScalar::Integer(value)))
                            .collect(),
                    ),
                ),
            ]
            .into(),
        ));
        for _ in 0..8 {
            value = RuntimeValue::Record(std::sync::Arc::new([("nested".into(), value)].into()));
        }
        let json = crate::graph::results::runtime_value_to_json(&value).unwrap();
        let mut nested = &json;
        for _ in 0..8 {
            nested = &nested["nested"];
        }
        assert_eq!(nested["text"], text);
        assert_eq!(nested["coefficients"].as_array().unwrap().len(), 150);
        let result = AutomationCapabilityResult::ResultInspection(ResultInspection {
            result_id: 1,
            category: ResultCategoryInspection::Value,
            value: ResultValueInspection::Json(json),
        });
        assert!(result.validate_budget(1_048_576).is_ok());
    }

    #[test]
    fn ai_reads_the_shared_result_json_and_follows_table_references() {
        let (application, reference, model) = crate::graph::results::report::tests::fixture(1_000);
        let captured = application.capture_session().unwrap();
        let control = CapabilityControl::new(
            yss_harness_contract::CancellationToken::default(),
            std::time::Duration::from_secs(10),
        );
        let inspect = |part: Option<&str>, offset, limit| {
            inspect_result(
                &application,
                &captured,
                InspectResultRequest {
                    execution_session_id: reference.execution_session_id.as_uuid().to_string(),
                    result_id: reference.result_id.get(),
                    part: part.map(str::to_owned),
                    offset,
                    limit,
                },
                &control,
            )
            .unwrap()
            .value
        };
        let stale = inspect_result(
            &application,
            &captured,
            InspectResultRequest {
                execution_session_id: uuid::Uuid::new_v4().to_string(),
                result_id: reference.result_id.get(),
                part: None,
                offset: 0,
                limit: 20,
            },
            &control,
        );
        assert!(
            matches!(stale, Err(failure) if failure.code == CapabilityFailureCode::ResultUnavailable)
        );
        let ResultValueInspection::Json(json) = inspect(None, 0, 20) else {
            panic!("full result JSON");
        };
        assert_eq!(
            json,
            application.query_result_json(reference).unwrap().unwrap()
        );
        assert_eq!(json["model_basic_info"]["num_observation"], 1_000);
        assert!(json["model_basic_info"]["f_statistic"].is_number());
        assert_eq!(json["coefficients"]["kind"], "tableRef");
        assert_eq!(json["observations"]["rowCount"], 1_000);
        assert!(json.get("design").is_none());
        assert!(json.get("residuals").is_none());
        let ResultValueInspection::Table {
            rows,
            columns,
            next_offset,
            has_more,
            ..
        } = inspect(Some("coefficients"), 0, 1)
        else {
            panic!("coefficient page");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(columns[1], "coef");
        assert_eq!(rows[0][1], model.coefficients[0]);
        assert!(has_more);
        assert_eq!(next_offset, 1);
        let ResultValueInspection::Table {
            rows,
            next_offset,
            has_more,
            ..
        } = inspect(Some("observations"), 7, 3)
        else {
            panic!("observation page");
        };
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][0], 8);
        assert_eq!(rows[0][2], model.residuals[7]);
        assert_eq!(next_offset, 10);
        assert!(has_more);
        let series = captured
            .execution()
            .query_graph_results("events/report.yssbi-event", 100)
            .into_iter()
            .find(|result| matches!(result.value().value(), RuntimeValue::List(_)))
            .unwrap();
        let page = inspect_result(
            &application,
            &captured,
            InspectResultRequest {
                execution_session_id: reference.execution_session_id.as_uuid().to_string(),
                result_id: series.provenance().result_id().get(),
                part: None,
                offset: 5,
                limit: 7,
            },
            &control,
        )
        .unwrap();
        let ResultValueInspection::Table {
            rows,
            has_more,
            next_offset,
            ..
        } = &page.value
        else {
            panic!("in-memory series must remain paged");
        };
        assert_eq!(rows.len(), 7);
        assert_eq!(*next_offset, 12);
        assert!(*has_more);
        assert!(
            AutomationCapabilityResult::ResultInspection(page)
                .validate_budget(1)
                .is_err()
        );
    }

    #[test]
    fn graph_edit_mapping_preserves_typed_node_positions_and_rejects_bad_ids() {
        let node_id = uuid::Uuid::from_u128(1);
        let mutation = editor_mutation(
            GraphEditOperation::MoveNodes {
                positions: vec![yss_harness_contract::GraphEditPosition {
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
