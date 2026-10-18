//! Project-session-bound automation capabilities.

use std::sync::Arc;

use yss_assistant_contract::{
    AutomationCapabilityRequest, AutomationCapabilityResult, CapabilityInvocationContext,
    GraphPortInspection, ResultCategoryInspection,
};
#[cfg(test)]
use yss_assistant_contract::{
    InspectResultRequest, ReadResultTableRequest, ResultInspection, ResultRef, ResultValidity,
    ResultValueInspection, TableRef,
};
use yss_graph_document::{PortAddress, PortRef};
use yss_graph_execution::plan::ResultCategory;
use yss_harness_contract::{CapabilityControl, CapabilityFailure, CapabilityFailureCode};
#[cfg(test)]
use yss_node_kernel::RuntimeValue;

use crate::graph::catalog::CatalogQueryApplicationError;
use crate::session::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
mod graph;
mod resources;
mod results;
pub use graph::invoke_graph_capability;

impl ApplicationState {
    /// Synchronous business entry point; async adapters must dispatch it to a blocking worker.
    pub fn invoke_automation_capability(
        &self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        control: &CapabilityControl,
        publish: &mut dyn FnMut(&crate::events::CommittedResourceMutation),
    ) -> Result<AutomationCapabilityResult, CapabilityFailure> {
        invoke_capability(self, context, request, control, publish)
    }
}

fn invoke_capability(
    application: &ApplicationState,
    context: CapabilityInvocationContext,
    request: AutomationCapabilityRequest,
    control: &CapabilityControl,
    publish: &mut dyn FnMut(&crate::events::CommittedResourceMutation),
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    // Agent authority is enforced once by the Assistant extension before this owner call.
    control.check()?;
    let read_only =
        request.capability_id().descriptor().effect == yss_harness_contract::ToolEffect::Inspect;
    request.validate().map_err(|error| {
        yss_assistant_contract::contract_failure(error, request.capability_id())
    })?;
    let captured = application
        .capture_session()
        .map_err(map_session_capture_error)?;
    ensure_project_binding(&captured, &context)?;

    let result = match request {
        AutomationCapabilityRequest::ReadMind(request) => {
            resources::read_mind(application, &captured, request, control)
                .map(AutomationCapabilityResult::MindRead)
        }
        AutomationCapabilityRequest::InspectChart(request) => {
            resources::inspect_chart(application, &captured, request, control)
                .map(AutomationCapabilityResult::ChartInspection)
        }
        AutomationCapabilityRequest::ReadDocument(request) => {
            resources::read_document(application, &captured, request, control)
                .map(AutomationCapabilityResult::DocumentRead)
        }
        AutomationCapabilityRequest::ReadDatabase(request) => {
            resources::read_database(application, &captured, request, control)
                .map(AutomationCapabilityResult::DatabaseRead)
        }
        AutomationCapabilityRequest::InspectResource(request) => {
            resources::inspect_resource(application, &captured, request, control)
                .map(AutomationCapabilityResult::ResourceInspection)
        }
        AutomationCapabilityRequest::ManageResource(request) => {
            return resources::manage_resource(application, &captured, request, control, publish)
                .map(AutomationCapabilityResult::ResourceManaged);
        }
        AutomationCapabilityRequest::EditResource(request) => {
            return resources::edit_resource(application, &captured, request, control, publish)
                .map(AutomationCapabilityResult::ResourceEdited);
        }
        AutomationCapabilityRequest::ExportDatabase(request) => {
            return resources::export_database(application, &captured, request, control)
                .map(AutomationCapabilityResult::DatabaseExported);
        }
        AutomationCapabilityRequest::InspectUiIntent(request) => application
            .inspect_ui_intent(captured.project_instance_id(), request)
            .map(AutomationCapabilityResult::UiIntentInspection)
            .map_err(map_ui_error),
        AutomationCapabilityRequest::RequestUiIntent(request) => application
            .request_ui_intent(
                captured.project_instance_id(),
                &format!("harness:{}", context.harness_session_id().as_str()),
                request.into(),
            )
            .map(AutomationCapabilityResult::UiIntentReceipt)
            .map_err(map_ui_error),
        request @ (AutomationCapabilityRequest::InspectGraph(_)
        | AutomationCapabilityRequest::FindNodes(_)
        | AutomationCapabilityRequest::FindConstants(_)
        | AutomationCapabilityRequest::InspectConstants(_)
        | AutomationCapabilityRequest::InspectNodes(_)
        | AutomationCapabilityRequest::FindConnections(_)
        | AutomationCapabilityRequest::GraphMutation(_)
        | AutomationCapabilityRequest::ApplyGraphEdit(_)
        | AutomationCapabilityRequest::ValidateGraph(_)
        | AutomationCapabilityRequest::ExecuteGraph(_)
        | AutomationCapabilityRequest::SaveGraph(_)) => {
            return graph::invoke_graph_capability(application, context, request, control);
        }
        AutomationCapabilityRequest::BrowseNodes(request) => {
            catalog::browse_nodes(application, &captured, request)
                .map(AutomationCapabilityResult::NodeCatalogPage)
        }
        AutomationCapabilityRequest::InspectNodeType(request) => {
            catalog::inspect_node_type(application, &captured, request)
                .map(AutomationCapabilityResult::NodeTypeInspection)
        }
        AutomationCapabilityRequest::InspectResult(request) => {
            results::inspect_result(application, &captured, request)
                .map(AutomationCapabilityResult::ResultInspection)
        }
        AutomationCapabilityRequest::ReadResultTable(request) => {
            results::read_table(application, &captured, request, control)
                .map(AutomationCapabilityResult::ResultTablePage)
        }
        AutomationCapabilityRequest::ListResources(request) => {
            resources::project_inspection(&captured, request)
                .map(AutomationCapabilityResult::ProjectInspection)
        }
        AutomationCapabilityRequest::ListGraphResults(request) => {
            results::list_results(&captured, request).map(AutomationCapabilityResult::GraphResults)
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

fn map_ui_error(error: crate::presentation::UiError) -> CapabilityFailure {
    use crate::presentation::UiError;
    let code = match error {
        UiError::Invalid => CapabilityFailureCode::INVALID_REQUEST,
        UiError::Conflict => CapabilityFailureCode::INVOCATION_CONFLICT,
        UiError::Session => CapabilityFailureCode::PROJECT_SESSION_CHANGED,
        UiError::Capacity => CapabilityFailureCode::RESULT_TOO_LARGE,
        UiError::Unavailable | UiError::Workbench => CapabilityFailureCode::RESULT_UNAVAILABLE,
    };
    CapabilityFailure::new(code).with_detail("uiCode", error.to_string())
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
            CapabilityFailureCode::PROJECT_SESSION_MISMATCH,
        ));
    }
    Ok(())
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

mod catalog;

fn inspect_result_category(category: ResultCategory) -> ResultCategoryInspection {
    match category {
        ResultCategory::Value => ResultCategoryInspection::Value,
        ResultCategory::PlotData(kind) => ResultCategoryInspection::PlotData {
            plot_kind: kind.name().to_owned(),
        },
        ResultCategory::StatisticalReport(kind) => ResultCategoryInspection::StatisticalReport {
            report_kind: kind.name().to_owned(),
        },
    }
}

fn map_session_capture_error(_: SessionCaptureError) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::PROJECT_SESSION_UNAVAILABLE)
}

fn map_project_inspection_error(error: yss_project::ProjectOperationError) -> CapabilityFailure {
    use yss_project::ProjectOperationError;
    CapabilityFailure::new(match error {
        ProjectOperationError::StaleProjectLifecycle { .. } => {
            CapabilityFailureCode::PROJECT_SESSION_CHANGED
        }
        ProjectOperationError::CatalogResourceStale { .. } => {
            CapabilityFailureCode::REVISION_CONFLICT
        }
        ProjectOperationError::ProjectRecoveryRequired { .. } => {
            CapabilityFailureCode::PROJECT_SESSION_UNAVAILABLE
        }
        _ => yss_assistant_contract::failure_code::CATALOG_UNAVAILABLE,
    })
}

fn map_session_revalidation_error(error: SessionRevalidationError) -> CapabilityFailure {
    match error {
        SessionRevalidationError::Unavailable(_) => {
            CapabilityFailure::new(CapabilityFailureCode::PROJECT_SESSION_UNAVAILABLE)
        }
        SessionRevalidationError::Changed => {
            CapabilityFailure::new(CapabilityFailureCode::PROJECT_SESSION_CHANGED)
        }
    }
}

fn map_catalog_error(error: CatalogQueryApplicationError) -> CapabilityFailure {
    match error {
        CatalogQueryApplicationError::SessionCapture(error) => map_session_capture_error(error),
        CatalogQueryApplicationError::SessionChanged => {
            CapabilityFailure::new(CapabilityFailureCode::PROJECT_SESSION_CHANGED)
        }
        CatalogQueryApplicationError::CatalogProjectStale => {
            CapabilityFailure::new(CapabilityFailureCode::PROJECT_SESSION_MISMATCH)
        }
        CatalogQueryApplicationError::Editing(_)
        | CatalogQueryApplicationError::Parameters(_)
        | CatalogQueryApplicationError::Project(_)
        | CatalogQueryApplicationError::Database(_)
        | CatalogQueryApplicationError::Contract(_)
        | CatalogQueryApplicationError::Graph(_) => {
            CapabilityFailure::new(yss_assistant_contract::failure_code::CATALOG_UNAVAILABLE)
        }
    }
}

fn map_dataset_profile_error(
    error: &yss_database_runtime::error::DatabaseError,
    database_id: &str,
) -> CapabilityFailure {
    use yss_database_runtime::error::DatabaseErrorCode;

    let code = match error.code() {
        DatabaseErrorCode::Cancelled => CapabilityFailureCode::CANCELLED,
        DatabaseErrorCode::Deadline => CapabilityFailureCode::DEADLINE_ELAPSED,
        _ => yss_assistant_contract::failure_code::DATABASE_UNAVAILABLE,
    };
    CapabilityFailure::new(code).with_detail("databaseId", database_id)
}

#[cfg(test)]
mod tests {
    use super::graph::editor_mutation;
    use super::*;
    use yss_assistant_contract::GraphEditOperation;
    use yss_data_contract::TabularScalar;
    use yss_graph_document::{NodeId, NodePosition};
    use yss_graph_editor::EditorGraphMutation;

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
        let json = crate::result_encoding::runtime_value_to_json(&value).unwrap();
        let mut nested = &json;
        for _ in 0..8 {
            nested = &nested["nested"];
        }
        assert_eq!(nested["text"], text);
        assert_eq!(nested["coefficients"].as_array().unwrap().len(), 150);
        let result = AutomationCapabilityResult::ResultInspection(ResultInspection {
            result_ref: ResultRef::new("test".into(), 1),
            validity: ResultValidity::Retained,
            category: ResultCategoryInspection::Value,
            value: ResultValueInspection::Json(json),
        });
        assert!(result.validate_budget(1_048_576).is_ok());
    }

    #[test]
    fn ai_reads_the_shared_result_json_and_follows_table_references() {
        let (application, reference, model) =
            crate::graph::results::report::tests::fixture_with_options(
                1_000,
                "OLS",
                yss_sci_contract::regression::summary::LinearSummaryOptions {
                    observations: true,
                    ..Default::default()
                },
            );
        let captured = application.capture_session().unwrap();
        let result_ref = results::result_ref(reference);
        let control = CapabilityControl::new(
            yss_harness_contract::CancellationToken::default(),
            std::time::Duration::from_secs(10),
        );
        let inspect = |reference| {
            results::inspect_result(
                &application,
                &captured,
                InspectResultRequest {
                    result_ref: reference,
                    schema_offset: 0,
                    schema_limit: 50,
                },
            )
        };
        let overview = inspect(result_ref.clone()).unwrap();
        let ResultValueInspection::Json(json) = overview.value else {
            panic!("result overview");
        };
        let desktop = crate::result_encoding::query_result_json(&application, reference)
            .unwrap()
            .unwrap();
        assert_eq!(json["presentation"], desktop["presentation"]);
        assert_eq!(json["paramNames"], desktop["paramNames"]);
        assert_eq!(json["resultRef"], serde_json::json!(result_ref));
        assert!(json["resultRef"].is_string());
        assert_eq!(json["observations"]["rowCount"], 1000);
        let metrics = json["presentation"]["summary"]["items"].as_array().unwrap();
        assert_eq!(
            metrics
                .iter()
                .find(|v| v["id"] == "numObservations")
                .unwrap()["value"],
            1000
        );
        let request = |table_ref, offset, limit, columns| ReadResultTableRequest {
            table_ref,
            columns,
            offset,
            limit,
            column_offset: 0,
            column_limit: 50,
        };
        let coefficients: TableRef =
            serde_json::from_value(json["coefficients"]["tableRef"].clone()).unwrap();
        let page = results::read_table(
            &application,
            &captured,
            request(coefficients.clone(), 0, 1, vec!["coef".into()]),
            &control,
        )
        .unwrap();
        assert_eq!(page.columns.len(), 1);
        assert_eq!(page.rows[0][0], model.coefficients[0]);
        assert_eq!(page.page.next_offset, Some(1));
        let observations: TableRef =
            serde_json::from_value(json["observations"]["tableRef"].clone()).unwrap();
        let page = results::read_table(
            &application,
            &captured,
            request(observations, 7, 3, vec![]),
            &control,
        )
        .unwrap();
        assert_eq!(page.rows[0][0], 8);
        assert_eq!(page.rows[0][2], model.residuals[7]);
        assert_eq!(page.page.next_offset, Some(10));
        assert_eq!(page.page.total, Some(1000));
        let invalid = results::read_table(
            &application,
            &captured,
            request(coefficients, 0, 1, vec!["absent".into()]),
            &control,
        )
        .unwrap_err();
        assert_eq!(invalid.code, CapabilityFailureCode::INVALID_REQUEST);
        for part in ["null", "", "structured:/invented_table"] {
            let failure = results::read_table(
                &application,
                &captured,
                request(
                    TableRef::new(result_ref.clone(), Some(part.into())),
                    0,
                    20,
                    vec![],
                ),
                &control,
            )
            .unwrap_err();
            assert_eq!(failure.details["reason"], "table_not_found");
        }
        let stale = inspect(ResultRef::new(
            uuid::Uuid::new_v4().to_string(),
            reference.result_id.get(),
        ))
        .unwrap_err();
        assert_eq!(stale.details["reason"], "reference_expired");
        let series = captured
            .execution()
            .query_graph_results("events/report.yssbi-event", 100)
            .into_iter()
            .find(|result| matches!(result.value().value(), RuntimeValue::List(_)))
            .unwrap();
        let series_ref = results::result_ref(series.provenance().reference());
        let ResultValueInspection::Tabular {
            table_ref,
            schema_page,
            ..
        } = inspect(series_ref).unwrap().value
        else {
            panic!("schema overview");
        };
        assert_eq!(schema_page.total, Some(1));
        let page = results::read_table(
            &application,
            &captured,
            request(table_ref.clone(), 5, 7, vec![]),
            &control,
        )
        .unwrap();
        assert_eq!(page.rows.len(), 7);
        assert_eq!(page.page.next_offset, Some(12));
        assert!(
            AutomationCapabilityResult::ResultTablePage(page)
                .validate_budget(1)
                .is_err()
        );
        let cancelled = CapabilityControl::new(
            yss_harness_contract::CancellationToken::default(),
            std::time::Duration::from_secs(10),
        );
        cancelled.cancel_query();
        let expired = CapabilityControl::new(
            yss_harness_contract::CancellationToken::default(),
            std::time::Duration::ZERO,
        );
        let failures = [cancelled, expired].map(|control| {
            results::read_table(
                &application,
                &captured,
                request(table_ref.clone(), 5, 7, vec![]),
                &control,
            )
            .unwrap_err()
            .code
        });
        assert_eq!(
            failures,
            [
                CapabilityFailureCode::CANCELLED,
                CapabilityFailureCode::DEADLINE_ELAPSED
            ]
        );
    }

    #[test]
    fn graph_edit_mapping_preserves_typed_node_positions_and_rejects_bad_ids() {
        let node_id = uuid::Uuid::from_u128(1);
        let mutation = editor_mutation(
            GraphEditOperation::MoveNodes {
                positions: vec![yss_assistant_contract::GraphEditPosition {
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
            CapabilityFailureCode::INVALID_REQUEST
        );
    }

    #[test]
    fn dataset_profile_preserves_typed_query_interruptions() {
        use yss_database_contract::DatabaseId;
        use yss_database_runtime::error::{DatabaseError, DatabaseOperation};
        use yss_database_store::DatasetStoreError;
        use yss_relational_contract::RelationError;

        for (source, expected) in [
            (RelationError::Cancelled, CapabilityFailureCode::CANCELLED),
            (
                RelationError::DeadlineExceeded,
                CapabilityFailureCode::DEADLINE_ELAPSED,
            ),
            (
                RelationError::QueryFailed,
                yss_assistant_contract::failure_code::DATABASE_UNAVAILABLE,
            ),
        ] {
            let error = DatabaseError::dataset(
                DatabaseOperation::Query,
                Some(DatabaseId::from_existing("database-1".into())),
                DatasetStoreError::Query(source),
            );
            let failure = map_dataset_profile_error(&error, "database-1");
            assert_eq!(failure.code, expected);
            assert_eq!(failure.details["databaseId"], "database-1");
            let wire = serde_json::to_value(&failure).unwrap();
            assert_eq!(wire["code"], serde_json::to_value(expected).unwrap());
            assert_eq!(
                wire["details"],
                serde_json::json!({"databaseId": "database-1"})
            );
        }
    }
}
