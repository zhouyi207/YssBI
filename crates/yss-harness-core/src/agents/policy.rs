//! Shared authorization for Core admission and the Application gateway.

use super::agent_definition;
use yss_harness_contract::{
    AgentInvocationScope, AgentResourceOperation as Op, AgentRole, CapabilityFailure,
    CapabilityFailureCode, CapabilityId, ProjectResourceKind as Kind, ProjectResourceRef,
    ResourceCreation, model::CapabilityInput as Request,
};

fn denied() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
        .with_detail("reason", "agent_scope_denied")
}

pub fn creation_kind(specification: &ResourceCreation) -> Kind {
    match specification {
        ResourceCreation::EventGraph { .. } => Kind::EventGraph,
        ResourceCreation::FunctionGraph { .. } => Kind::FunctionGraph,
        ResourceCreation::Database { .. } => Kind::Database,
        ResourceCreation::Chart { .. } => Kind::Chart,
        ResourceCreation::Mind { .. } => Kind::Mind,
        ResourceCreation::Doc { .. } => Kind::Doc,
    }
}

fn may_write(role: AgentRole, kind: Kind) -> bool {
    matches!(
        (role, kind),
        (AgentRole::Data, Kind::Database)
            | (AgentRole::Stats, Kind::EventGraph | Kind::FunctionGraph)
            | (AgentRole::Plot, Kind::Chart)
            | (AgentRole::Report, Kind::Doc | Kind::Mind)
    )
}

pub fn authorize_agent_resource(
    scope: &AgentInvocationScope,
    resource: &ProjectResourceRef,
    operation: Op,
) -> Result<(), CapabilityFailure> {
    if scope.role == AgentRole::Manager && scope.task.is_none() && operation == Op::Inspect {
        return Ok(());
    }
    let task = scope.task.as_ref().ok_or_else(denied)?;
    if operation != Op::Inspect && !may_write(scope.role, resource.kind) {
        return Err(denied());
    }
    if task
        .resources
        .iter()
        .any(|access| access.resource == *resource && access.operations.contains(&operation))
    {
        Ok(())
    } else {
        Err(denied())
    }
}

fn graph_resource(
    scope: &AgentInvocationScope,
    path: &str,
    operation: Op,
) -> Result<(), CapabilityFailure> {
    if scope.role == AgentRole::Manager && scope.task.is_none() && operation == Op::Inspect {
        return Ok(());
    }
    let resource = scope
        .task
        .as_ref()
        .and_then(|task| {
            task.resources.iter().find(|access| {
                access.resource.id == path
                    && matches!(access.resource.kind, Kind::EventGraph | Kind::FunctionGraph)
            })
        })
        .ok_or_else(denied)?;
    authorize_agent_resource(scope, &resource.resource, operation)
}

fn authorize_role_capability(
    scope: &AgentInvocationScope,
    capability: CapabilityId,
) -> Result<(), CapabilityFailure> {
    if agent_definition(scope.role)
        .capabilities
        .contains(&capability)
    {
        Ok(())
    } else {
        Err(denied())
    }
}

/// Called both before ledger admission and at the Application gateway boundary.
pub(crate) fn authorize_model_capability(
    scope: &AgentInvocationScope,
    request: &Request,
) -> Result<(), CapabilityFailure> {
    authorize_role_capability(scope, request.capability_id())?;
    match request {
        Request::ListResources(_) | Request::InspectUiIntent(_) | Request::RequestUiIntent(_) => {
            if scope.role == AgentRole::Manager && scope.task.is_none() {
                Ok(())
            } else {
                Err(denied())
            }
        }
        Request::BrowseNodes(_)
        | Request::InspectNodeType(_)
        | Request::SearchKnowledge(_)
        | Request::ReadKnowledge(_) => Ok(()),
        Request::InspectChart(request) => {
            authorize_agent_resource(scope, &request.chart.resource(), Op::Inspect)
        }
        Request::UpdateChart(request) => {
            authorize_agent_resource(scope, &request.chart.resource(), Op::Edit)?;
            if let Some(id) = &request.settings.database_id
                && !id.is_empty()
            {
                authorize_agent_resource(
                    scope,
                    &ProjectResourceRef {
                        kind: Kind::Database,
                        id: id.clone(),
                    },
                    Op::Inspect,
                )?;
            }
            Ok(())
        }
        Request::InspectResource(request) => {
            authorize_agent_resource(scope, &request.resource, Op::Inspect)
        }
        Request::InspectMind(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Inspect)
        }
        Request::FindTopics(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Inspect)
        }
        Request::InspectTopics(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Inspect)
        }
        Request::CreateTopics(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Edit)
        }
        Request::UpdateTopics(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Edit)
        }
        Request::MoveTopics(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Edit)
        }
        Request::DeleteTopics(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Edit)
        }
        Request::DuplicateTopics(request) => {
            authorize_agent_resource(scope, &request.mind.resource(), Op::Edit)
        }
        Request::InspectDocument(request) => {
            authorize_agent_resource(scope, &request.document.resource(), Op::Inspect)
        }
        Request::ReadDocument(request) => {
            authorize_agent_resource(scope, &request.document.resource(), Op::Inspect)
        }
        Request::SearchDocument(request) => {
            authorize_agent_resource(scope, &request.document.resource(), Op::Inspect)
        }
        Request::ReplaceDocumentText(request) => {
            authorize_agent_resource(scope, &request.document.resource(), Op::Edit)
        }
        Request::AppendDocument(request) => {
            authorize_agent_resource(scope, &request.document.resource(), Op::Edit)
        }
        Request::WriteDocument(request) => {
            authorize_agent_resource(scope, &request.document.resource(), Op::Edit)
        }
        Request::InspectDatabase(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Inspect)
        }
        Request::InspectDatabaseSchema(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Inspect)
        }
        Request::ProfileDatabase(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Inspect)
        }
        Request::ReadDatabaseRows(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Inspect)
        }
        Request::InsertRows(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::UpdateCells(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::DeleteRows(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::CreateColumns(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::RenameColumns(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::DeleteColumns(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::CastColumns(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::SetColumnSemantics(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Edit)
        }
        Request::InspectDatasetSchema(request) => authorize_agent_resource(
            scope,
            &ProjectResourceRef {
                kind: Kind::Database,
                id: request.database_id.clone(),
            },
            Op::Inspect,
        ),
        Request::InspectDatasetProfile(request) => authorize_agent_resource(
            scope,
            &ProjectResourceRef {
                kind: Kind::Database,
                id: request.database_id.clone(),
            },
            Op::Inspect,
        ),
        Request::InspectGraph(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::FindNodes(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::FindConstants(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::InspectConstants(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::CreateConstants(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::UpdateConstants(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::DeleteConstants(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }

        Request::InspectNodes(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::FindConnections(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::ValidateGraph(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::ListGraphResults(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Inspect)
        }
        Request::ApplyGraphEdit(request) => graph_resource(scope, &request.graph_path, Op::Edit),
        Request::CreateNodes(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::UpdateNodes(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::DeleteNodes(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::DuplicateNodes(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::MoveNodes(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::CreateConnections(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::UpdateConnections(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::DeleteConnections(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Edit)
        }
        Request::ExecuteGraph(request) => {
            authorize_agent_resource(scope, &request.graph.resource(), Op::Execute)
        }
        Request::SaveGraph(request) => graph_resource(scope, &request.graph_path, Op::Save),
        Request::InspectResult(request) => authorize_result(scope, &request.result_ref),
        Request::ReadResultTable(request) => {
            authorize_result(scope, request.table_ref.result_ref())
        }
        Request::CreateResource(value) => authorize_creation(scope, &value.clone().into()),
        Request::ImportDatabase(value) => authorize_creation(
            scope,
            &ResourceCreation::Database {
                source: value.source.clone(),
                name: value.name.clone(),
            },
        ),
        Request::RenameResource(value) => {
            authorize_agent_resource(scope, &value.resource, Op::Rename)
        }
        Request::DuplicateResource(value) => {
            authorize_agent_resource(scope, &value.resource, Op::Duplicate)
        }
        Request::DeleteResource(value) => {
            authorize_agent_resource(scope, &value.resource, Op::Delete)
        }
        Request::SaveResource(value) => authorize_agent_resource(scope, &value.resource, Op::Save),
        Request::UndoResource(value) | Request::RedoResource(value) => {
            if !matches!(
                value.resource.kind,
                Kind::EventGraph | Kind::FunctionGraph | Kind::Database
            ) {
                return Err(denied());
            }
            authorize_agent_resource(scope, &value.resource, Op::Edit)
        }
        Request::EditResource(request) => {
            authorize_agent_resource(scope, &request.resource, Op::Edit)
        }
        Request::ExportDatabase(request) => {
            authorize_agent_resource(scope, &request.database.resource(), Op::Export)?;
            if scope
                .task
                .as_ref()
                .is_some_and(|task| task.export_paths.contains(&request.path))
            {
                Ok(())
            } else {
                Err(denied())
            }
        }
    }
}

fn authorize_creation(
    scope: &AgentInvocationScope,
    specification: &ResourceCreation,
) -> Result<(), CapabilityFailure> {
    if may_write(scope.role, creation_kind(specification))
        && scope.task.as_ref().is_some_and(|task| {
            task.creations
                .iter()
                .any(|grant| &grant.specification == specification)
        })
    {
        Ok(())
    } else {
        Err(denied())
    }
}

/// Internal gateways share the same business authority checks as model admission.
pub fn authorize_agent_capability(
    scope: &AgentInvocationScope,
    request: &yss_harness_contract::AutomationCapabilityRequest,
) -> Result<(), CapabilityFailure> {
    // Original graph owner operations also serve typed mutations and receipt recovery.
    // Model admission separately rejects tools absent from the role registry.
    match request {
        yss_harness_contract::AutomationCapabilityRequest::ApplyGraphEdit(value) => {
            return graph_resource(scope, &value.graph_path, Op::Edit);
        }
        yss_harness_contract::AutomationCapabilityRequest::SaveGraph(value) => {
            return graph_resource(scope, &value.graph_path, Op::Save);
        }
        yss_harness_contract::AutomationCapabilityRequest::GraphMutation(value) => {
            authorize_role_capability(scope, value.input.capability_id())?;
            return authorize_agent_resource(scope, &value.input.graph().resource(), Op::Edit);
        }
        _ => {}
    }
    authorize_model_capability(scope, &request.into())
}

/// A closed graph has a resource revision before an editing session is opened.
/// Capturing that session at the same revision refines, rather than replaces, the baseline.
pub(crate) fn resource_version_matches(
    kind: Kind,
    expected: &yss_harness_contract::ResourceVersion,
    current: &yss_harness_contract::ResourceVersion,
) -> bool {
    expected == current
        || (matches!(kind, Kind::EventGraph | Kind::FunctionGraph)
            && expected.session_id.is_none()
            && expected.revision == current.revision)
}

/// A worker must not mix newly changed resource contents into an older task grant.
pub(crate) fn validate_agent_read(
    scope: &AgentInvocationScope,
    result: &yss_harness_contract::AutomationCapabilityResult,
) -> Result<(), CapabilityFailure> {
    use yss_harness_contract::AutomationCapabilityResult as Result;
    let Some(task) = &scope.task else {
        return Ok(());
    };
    let (access, version) = match result {
        Result::MindRead(value) => (
            task.resources
                .iter()
                .find(|access| access.resource == value.mind.resource()),
            &value.version,
        ),
        Result::ChartInspection(value) => (
            task.resources
                .iter()
                .find(|access| access.resource == value.chart.resource()),
            &value.version,
        ),
        Result::DocumentRead(value) => (
            task.resources
                .iter()
                .find(|access| access.resource == value.document.resource()),
            &value.version,
        ),
        Result::DatabaseRead(value) => (
            task.resources
                .iter()
                .find(|access| access.resource == value.database.resource()),
            &value.version,
        ),
        Result::ResourceInspection(value) => (
            task.resources
                .iter()
                .find(|access| access.resource == value.resource),
            &value.version,
        ),
        Result::GraphInspection(value) => (
            task.resources.iter().find(|access| {
                access.resource.id == value.graph_path
                    && matches!(access.resource.kind, Kind::EventGraph | Kind::FunctionGraph)
            }),
            &value.version,
        ),
        Result::GraphInspectionPage(value) => (
            task.resources.iter().find(|access| {
                access.resource.id == value.graph_path
                    && matches!(access.resource.kind, Kind::EventGraph | Kind::FunctionGraph)
            }),
            &value.version,
        ),
        _ => return Ok(()),
    };
    if let Some(access) = access
        && access.version.as_ref().is_some_and(|expected| {
            !resource_version_matches(access.resource.kind, expected, version)
        })
    {
        return Err(
            CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                .with_detail("reason", "task_input_changed")
                .with_detail("resourceId", &access.resource.id),
        );
    }
    Ok(())
}

fn authorize_result(
    scope: &AgentInvocationScope,
    reference: &yss_harness_contract::ResultRef,
) -> Result<(), CapabilityFailure> {
    if (scope.role == AgentRole::Manager && scope.task.is_none())
        || scope.task.as_ref().is_some_and(|task| {
            task.results
                .iter()
                .any(|result| &result.result_ref == reference)
        })
    {
        Ok(())
    } else {
        Err(denied())
    }
}

#[cfg(test)]
mod authorization_tests {
    use super::*;
    use yss_harness_contract::*;
    #[test]
    fn bound_graph_mutations_require_the_registered_role_and_exact_edit_grant() {
        let graph = GraphResourceRef::for_path("events/measure.yssbi-event");
        let input: model::MoveNodesInput = serde_json::from_value(serde_json::json!({
            "graph": graph,
            "positions": [{"nodeId":"node-1","x":1.0,"y":2.0}],
        }))
        .unwrap();
        let public = Request::MoveNodes(input.clone());
        let bound = AutomationCapabilityRequest::GraphMutation(GraphMutationRequest {
            input: model::GraphMutationInput::MoveNodes(input),
            base_revision: 1,
            graph_hash: "0".repeat(64),
            client_key: "move-nodes".into(),
        });
        for (role, resource, operation, allowed) in [
            (
                AgentRole::Stats,
                graph.resource(),
                AgentResourceOperation::Edit,
                true,
            ),
            (
                AgentRole::Stats,
                graph.resource(),
                AgentResourceOperation::Inspect,
                false,
            ),
            (
                AgentRole::Stats,
                GraphResourceRef::for_path("events/other.yssbi-event").resource(),
                AgentResourceOperation::Edit,
                false,
            ),
            (
                AgentRole::Review,
                graph.resource(),
                AgentResourceOperation::Edit,
                false,
            ),
        ] {
            let scope = AgentInvocationScope {
                run_id: AgentRunId::try_new("worker").unwrap(),
                role,
                task: Some(AgentTaskScope {
                    resources: vec![AgentResourceAccess {
                        resource,
                        version: Some(ResourceVersion {
                            revision: 1,
                            session_id: None,
                        }),
                        operations: vec![operation],
                    }],
                    ..Default::default()
                }),
            };
            assert_eq!(authorize_model_capability(&scope, &public).is_ok(), allowed);
            assert_eq!(authorize_agent_capability(&scope, &bound).is_ok(), allowed);
        }
    }

    #[test]
    fn manager_and_review_validate_graphs_without_execution_authority() {
        let graph = GraphResourceRef::for_path("events/measure.yssbi-event");
        for role in [AgentRole::Manager, AgentRole::Review] {
            let scope = AgentInvocationScope {
                run_id: AgentRunId::try_new("reader").unwrap(),
                role,
                task: (role == AgentRole::Review).then(|| AgentTaskScope {
                    resources: vec![AgentResourceAccess {
                        resource: graph.resource(),
                        version: Some(ResourceVersion {
                            revision: 1,
                            session_id: None,
                        }),
                        operations: vec![AgentResourceOperation::Inspect],
                    }],
                    ..Default::default()
                }),
            };
            let input: model::ValidateGraphInput =
                serde_json::from_value(serde_json::json!({"graph":graph})).unwrap();
            assert!(
                authorize_model_capability(&scope, &Request::ValidateGraph(input.clone())).is_ok()
            );
            let execution = serde_json::from_value(serde_json::json!({"graph":graph})).unwrap();
            assert!(authorize_model_capability(&scope, &Request::ExecuteGraph(execution)).is_err());
            if role == AgentRole::Review {
                let outside = model::ValidateGraphInput {
                    graph: GraphResourceRef::for_path("events/other.yssbi-event"),
                    ..input
                };
                assert!(
                    authorize_model_capability(&scope, &Request::ValidateGraph(outside)).is_err()
                );
            }
        }
    }

    #[test]
    fn report_database_facts_require_the_exact_read_grant_and_never_allow_edits() {
        let database = ProjectResourceRef {
            kind: ProjectResourceKind::Database,
            id: "dataset".into(),
        };
        let scope = AgentInvocationScope {
            run_id: AgentRunId::try_new("report").unwrap(),
            role: AgentRole::Report,
            task: Some(AgentTaskScope {
                resources: vec![AgentResourceAccess {
                    resource: database.clone(),
                    version: Some(ResourceVersion {
                        revision: 1,
                        session_id: None,
                    }),
                    operations: vec![
                        AgentResourceOperation::Inspect,
                        AgentResourceOperation::Edit,
                    ],
                }],
                ..Default::default()
            }),
        };
        for name in ["inspect_database", "inspect_database_schema"] {
            for id in ["dataset", "outside"] {
                let input: Request = serde_json::from_value(serde_json::json!({"type":name,"payload":{"database":{"kind":"database","id":id}}})).unwrap();
                assert_eq!(
                    authorize_model_capability(&scope, &input).is_ok(),
                    id == "dataset"
                );
            }
        }
        assert!(authorize_agent_resource(&scope, &database, AgentResourceOperation::Edit).is_err());
    }

    #[test]
    fn table_reads_inherit_only_the_exact_granted_result() {
        let reference = ResultRef::new("session-a".into(), u64::MAX);
        let scope = AgentInvocationScope {
            run_id: AgentRunId::try_new("report").unwrap(),
            role: AgentRole::Report,
            task: Some(AgentTaskScope {
                results: vec![AgentResultAccess {
                    result_ref: reference.clone(),
                }],
                ..Default::default()
            }),
        };
        for reference in [
            reference,
            ResultRef::new("session-b".into(), u64::MAX),
            ResultRef::new("session-a".into(), 1),
        ] {
            let granted = reference == scope.task.as_ref().unwrap().results[0].result_ref;
            let request = AutomationCapabilityRequest::ReadResultTable(ReadResultTableRequest {
                table_ref: TableRef::new(reference, Some("structured:/values".into())),
                columns: vec![],
                column_offset: 0,
                column_limit: 50,
                offset: 0,
                limit: 20,
            });
            assert_eq!(
                authorize_agent_capability(&scope, &request).is_ok(),
                granted
            );
        }
    }
}
