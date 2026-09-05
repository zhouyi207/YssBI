//! Project-session-bound automation capabilities.

use std::sync::Arc;

use yss_automation_contract::{
    ApplyGraphEditRequest, AutomationCapabilityRequest, AutomationCapabilityResult,
    CapabilityContractError, CapabilityFailure, CapabilityFailureCode, CapabilityFuture,
    CapabilityGatewayPort, CapabilityId, CapabilityInvocationContext, DatasetColumnSchema,
    DatasetProfileInspection, DatasetSchemaInspection, GraphConnectionInspection,
    GraphEditOperation, GraphEditPortRef, GraphEditReceipt, GraphInspection, GraphNodeInspection,
    GraphPortInspection, InspectDatasetProfileRequest, InspectDatasetSchemaRequest,
    InspectGraphRequest, InspectProjectRequest, InspectResultRequest, NodeCatalogMatch,
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
use yss_graph_document::{
    ConnectionId, GraphResourcePath, NodeId, NodePosition, OrderKey, PortAddress, PortInstanceId,
    PortRef,
};
use yss_graph_document_edit::{GraphDocumentPatch, apply_graph_document_patch};
use yss_graph_editor::{EditorGraphMutation, NodePositionMutation};
use yss_graph_protocol::PortKey;
use yss_project_identity::{OperationId, ResourceRevision};

use crate::catalog_query::{
    CatalogQueryApplicationError, LocalizedCatalogRequest, localized_node_catalog_in_session,
};
use crate::execution::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::graph_commit::commit_captured_graph_candidate;
use crate::resource_mutation::build_catalog_mutation_validation_snapshot;

impl CapabilityGatewayPort for ApplicationState {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move { invoke_capability(self, context, request) })
    }
}

fn invoke_capability(
    application: &ApplicationState,
    context: CapabilityInvocationContext,
    request: AutomationCapabilityRequest,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    request
        .validate()
        .map_err(|error| invalid_request(request.capability_id(), error))?;
    let captured = application
        .capture_session()
        .map_err(map_session_capture_error)?;
    ensure_project_binding(&captured, &context)?;

    let result = match request {
        AutomationCapabilityRequest::InspectGraph(request) => {
            inspect_graph(&captured, request).map(AutomationCapabilityResult::GraphInspection)
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
            inspect_dataset_profile(&captured, request)
                .map(AutomationCapabilityResult::DatasetProfileInspection)
        }
        AutomationCapabilityRequest::InspectResult(request) => {
            inspect_result(&captured, request).map(AutomationCapabilityResult::ResultInspection)
        }
        AutomationCapabilityRequest::InspectProject(request) => {
            inspect_project(&captured, request).map(AutomationCapabilityResult::ProjectInspection)
        }
        AutomationCapabilityRequest::ApplyGraphEdit(request) => {
            apply_graph_edit(application, &captured, &context, request)
                .map(AutomationCapabilityResult::GraphEditReceipt)
        }
    }?;

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

fn apply_graph_edit(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    context: &CapabilityInvocationContext,
    request: ApplyGraphEditRequest,
) -> Result<GraphEditReceipt, CapabilityFailure> {
    if context.approval_grant_id().is_none() {
        return Err(CapabilityFailure::new(
            CapabilityFailureCode::ApprovalRequired,
        ));
    }
    let graph_path = GraphResourcePath::new(&request.graph_path).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
            .with_detail("field", "graphPath")
    })?;
    let localized = localized_node_catalog_in_session(
        application,
        captured,
        LocalizedCatalogRequest::new(
            captured.project_instance_id().clone(),
            request.locale.clone(),
        ),
    )
    .map_err(map_catalog_error)?
    .into_transport_parts()
    .into_fields()
    .3;
    let catalog = build_catalog_mutation_validation_snapshot(captured).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::MutationRejected)
            .with_detail("graphPath", graph_path.as_str())
    })?;
    let operation_id = OperationId::new();
    let capture = captured
        .project()
        .capture_graph_operation(
            captured.project_instance_id(),
            &graph_path,
            ResourceRevision::new(request.base_revision),
            operation_id,
        )
        .map_err(|_| {
            CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                .with_detail("graphPath", graph_path.as_str())
                .with_detail("baseRevision", request.base_revision.to_string())
        })?;
    let original = capture.document.as_ref().clone();
    let resolution = crate::resource_mutation::DraftResolutionContext::capture(captured, &original)
        .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::MutationRejected))?;
    let mut staged = original.clone();
    let mut combined_operations = Vec::new();
    for operation in request.operations {
        let mutation = editor_mutation(operation, &localized.items)?;
        let analysis = resolution.resolve(captured, &graph_path, &staged, &request.locale);
        let patch = captured
            .graph()
            .plan_editor_mutation(
                &graph_path,
                &staged,
                mutation,
                &catalog,
                analysis.semantic_snapshot(),
            )
            .map_err(|_| {
                CapabilityFailure::new(CapabilityFailureCode::MutationRejected)
                    .with_detail("graphPath", graph_path.as_str())
            })?;
        apply_graph_document_patch(&mut staged, &patch).map_err(|_| {
            CapabilityFailure::new(CapabilityFailureCode::MutationRejected)
                .with_detail("graphPath", graph_path.as_str())
        })?;
        combined_operations.extend(patch.operations);
    }
    resolution
        .revalidate(captured)
        .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::RevisionConflict))?;
    let combined = GraphDocumentPatch::new(combined_operations);
    if combined.is_empty() {
        return Err(CapabilityFailure::new(
            CapabilityFailureCode::MutationRejected,
        ));
    }
    let mut candidate = original;
    apply_graph_document_patch(&mut candidate, &combined).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::MutationRejected)
            .with_detail("graphPath", graph_path.as_str())
    })?;
    let receipt =
        commit_captured_graph_candidate(application, captured, capture, Arc::new(candidate))
            .map_err(|_| {
                CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                    .with_detail("graphPath", graph_path.as_str())
            })?;
    Ok(GraphEditReceipt {
        graph_path: graph_path.into_string(),
        from_revision: receipt.from_revision.get(),
        to_revision: receipt.to_revision.get(),
        operation_id: receipt.operation_id.to_string(),
        client_key: request.client_key,
        can_undo: receipt.history.can_undo,
    })
}

fn editor_mutation(
    operation: GraphEditOperation,
    catalog: &[LocalizedCatalogItem],
) -> Result<EditorGraphMutation, CapabilityFailure> {
    match operation {
        GraphEditOperation::CreateNode {
            node_type_id,
            resource_path,
            x,
            y,
            user_label,
        } => {
            let descriptor = catalog
                .iter()
                .find(|item| {
                    item.node_type_id.as_ref() == node_type_id
                        && item.resource_path.as_ref().map(|path| path.as_str())
                            == resource_path.as_deref()
                })
                .map(|item| item.creation.clone())
                .ok_or_else(|| {
                    CapabilityFailure::new(CapabilityFailureCode::MutationRejected)
                        .with_detail("nodeTypeId", node_type_id)
                })?;
            Ok(EditorGraphMutation::CreateNode {
                descriptor,
                position: NodePosition { x, y },
                user_label,
                connect_from: None,
            })
        }
        GraphEditOperation::MoveNodes { positions } => Ok(EditorGraphMutation::MoveNodes {
            positions: positions
                .into_iter()
                .map(|position| {
                    Ok(NodePositionMutation {
                        node_id: parse_node_id(&position.node_id)?,
                        position: NodePosition {
                            x: position.x,
                            y: position.y,
                        },
                    })
                })
                .collect::<Result<Vec<_>, CapabilityFailure>>()?,
        }),
        GraphEditOperation::DeleteNodes { node_ids } => Ok(EditorGraphMutation::DeleteNodes {
            node_ids: node_ids
                .iter()
                .map(|node_id| parse_node_id(node_id))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        GraphEditOperation::Connect {
            output,
            input,
            order,
        } => Ok(EditorGraphMutation::Connect {
            output: parse_edit_port(output)?,
            input: parse_edit_port(input)?,
            order: order.map(OrderKey::new),
        }),
        GraphEditOperation::DisconnectConnections { connection_ids } => {
            Ok(EditorGraphMutation::DisconnectConnections {
                connection_ids: connection_ids
                    .iter()
                    .map(|id| {
                        uuid::Uuid::parse_str(id)
                            .map(ConnectionId::from_uuid)
                            .map_err(|_| invalid_edit_identity("connectionId"))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
    }
}

fn parse_node_id(value: &str) -> Result<NodeId, CapabilityFailure> {
    uuid::Uuid::parse_str(value)
        .map(NodeId::from_uuid)
        .map_err(|_| invalid_edit_identity("nodeId"))
}

fn parse_edit_port(value: GraphEditPortRef) -> Result<PortAddress, CapabilityFailure> {
    match value {
        GraphEditPortRef::Declared { node_id, port_key } => Ok(PortAddress::declared(
            parse_node_id(&node_id)?,
            PortKey::new(port_key).map_err(|_| invalid_edit_identity("portKey"))?,
        )),
        GraphEditPortRef::Instance {
            node_id,
            template_key,
            instance_id,
        } => Ok(PortAddress::instance(
            parse_node_id(&node_id)?,
            PortKey::new(template_key).map_err(|_| invalid_edit_identity("templateKey"))?,
            uuid::Uuid::parse_str(&instance_id)
                .map(PortInstanceId::from_uuid)
                .map_err(|_| invalid_edit_identity("instanceId"))?,
        )),
    }
}

fn invalid_edit_identity(field: &'static str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::InvalidRequest).with_detail("field", field)
}

fn inspect_project(
    captured: &ApplicationSession,
    _request: InspectProjectRequest,
) -> Result<ProjectInspection, CapabilityFailure> {
    let project = captured
        .project()
        .get_data()
        .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::ProjectSessionUnavailable))?;
    let mut resources = Vec::with_capacity(
        project.graphs.len()
            + project.databases.len()
            + project.charts.len()
            + project.variables.len(),
    );
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
    resources.extend(
        project
            .variables
            .values()
            .map(|variable| ProjectResourceInspection {
                kind: ProjectResourceKindInspection::Variable,
                resource_id: variable.id.to_string(),
                display_name: variable.name.clone(),
                revision: None,
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

fn inspect_graph(
    captured: &ApplicationSession,
    request: InspectGraphRequest,
) -> Result<GraphInspection, CapabilityFailure> {
    let graph_path = GraphResourcePath::new(&request.graph_path).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
            .with_detail("field", "graphPath")
    })?;
    let project = captured.project().get_data().map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::GraphUnavailable)
            .with_detail("graphPath", graph_path.as_str())
    })?;
    let graph = project.graphs.get(&graph_path).ok_or_else(|| {
        CapabilityFailure::new(CapabilityFailureCode::GraphUnavailable)
            .with_detail("graphPath", graph_path.as_str())
    })?;
    let result_count = graph
        .document
        .nodes
        .len()
        .saturating_add(graph.document.connections.len());
    enforce_result_bound(CapabilityId::InspectGraph, result_count)?;

    let nodes = graph
        .document
        .nodes
        .values()
        .map(|node| GraphNodeInspection {
            node_id: node.id.to_string(),
            node_type_id: node.node_type.as_str().to_owned(),
            user_label: node.user_label.clone(),
            x: node.position.x,
            y: node.position.y,
        })
        .collect();
    let connections = graph
        .document
        .connections
        .values()
        .map(|connection| GraphConnectionInspection {
            connection_id: connection.id.to_string(),
            output: inspect_port(&connection.output),
            input: inspect_port(&connection.input),
        })
        .collect();

    Ok(GraphInspection {
        graph_path: graph_path.into_string(),
        nodes,
        connections,
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
        .filter(|item| catalog_item_matches(item, &normalized_query))
        .map(|item| NodeCatalogMatch {
            node_type_id: item.node_type_id.to_string(),
            title: item.title.to_string(),
            category_id: item.category_id.to_string(),
            style_id: item.style_id.to_string(),
            resource_path: item
                .resource_path
                .as_ref()
                .map(|path| path.as_str().to_owned()),
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        left.title
            .cmp(&right.title)
            .then_with(|| left.node_type_id.cmp(&right.node_type_id))
    });
    matches.truncate(usize::from(request.limit));

    Ok(NodeCatalogSearchResult {
        locale: catalog.locale.into_string(),
        matches,
    })
}

fn catalog_item_matches(item: &LocalizedCatalogItem, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }
    let fixed_fields = [
        item.node_type_id.as_ref(),
        item.title.as_ref(),
        item.category_id.as_ref(),
        item.style_id.as_ref(),
    ];
    fixed_fields
        .into_iter()
        .chain(item.aliases.iter().map(AsRef::as_ref))
        .chain(item.technical_terms.iter().map(AsRef::as_ref))
        .chain(item.backend_search_text.iter().map(AsRef::as_ref))
        .chain(item.resource_names.iter().map(AsRef::as_ref))
        .any(|value| value.to_lowercase().contains(normalized_query))
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
    let overview = yss_database_runtime::session_api::dataset_overview(
        captured.database(),
        DatabaseId::from_existing(request.database_id.clone().into_boxed_str()),
    )
    .map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
            .with_detail("databaseId", &request.database_id)
    })?;
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
    captured: &ApplicationSession,
    request: InspectResultRequest,
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
    let value = inspect_stored_result(result.value(), &mut budget)?;
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
    use super::*;

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
