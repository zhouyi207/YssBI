//! Shared authorization for Core admission and the Application gateway.

use super::agent_definition;
use yss_harness_contract::{
    AgentInvocationScope, AgentResourceOperation as Op, AgentRole,
    AutomationCapabilityRequest as Request, CapabilityFailure, CapabilityFailureCode,
    ManageResourceRequest as Manage, ProjectResourceKind as Kind, ProjectResourceRef,
    ResourceCreation,
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
            | (AgentRole::Report, Kind::Doc)
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

/// Called both before ledger admission and at the Application gateway boundary.
pub fn authorize_agent_capability(
    scope: &AgentInvocationScope,
    request: &Request,
) -> Result<(), CapabilityFailure> {
    if !agent_definition(scope.role)
        .capabilities
        .contains(&request.capability_id())
    {
        return Err(denied());
    }
    match request {
        Request::InspectProject(_) | Request::InspectUiIntent(_) | Request::RequestUiIntent(_) => {
            if scope.role == AgentRole::Manager && scope.task.is_none() {
                Ok(())
            } else {
                Err(denied())
            }
        }
        Request::SearchNodeCatalog(_) => Ok(()),
        Request::InspectResource(request) => {
            authorize_agent_resource(scope, &request.resource, Op::Inspect)
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
        Request::InspectGraph(request) => graph_resource(scope, &request.graph_path, Op::Inspect),
        Request::ValidateGraph(request) => graph_resource(scope, &request.graph_path, Op::Inspect),
        Request::ListGraphResults(request) => {
            graph_resource(scope, &request.graph_path, Op::Inspect)
        }
        Request::ApplyGraphEdit(request) => graph_resource(scope, &request.graph_path, Op::Edit),
        Request::ExecuteGraph(request) => graph_resource(scope, &request.graph_path, Op::Execute),
        Request::SaveGraph(request) => graph_resource(scope, &request.graph_path, Op::Save),
        Request::InspectResult(request) => {
            if scope.role == AgentRole::Manager && scope.task.is_none() {
                return Ok(());
            }
            if scope.task.as_ref().is_some_and(|task| {
                task.results.iter().any(|result| {
                    result.execution_session_id == request.execution_session_id
                        && result.result_id == request.result_id
                })
            }) {
                Ok(())
            } else {
                Err(denied())
            }
        }
        Request::ManageResource(Manage::Create { specification }) => {
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
        Request::ManageResource(request) => {
            let (resource, operation) = match request {
                Manage::Rename { resource, .. } => (resource, Op::Rename),
                Manage::Duplicate { resource, .. } => (resource, Op::Duplicate),
                Manage::Delete { resource, .. } => (resource, Op::Delete),
                Manage::Save { resource, .. } => (resource, Op::Save),
                Manage::Create { .. } => unreachable!(),
            };
            authorize_agent_resource(scope, resource, operation)
        }
        Request::EditResource(request) => {
            authorize_agent_resource(scope, &request.resource, Op::Edit)?;
            if let yss_harness_contract::ResourceEdit::Chart { settings } = &request.edit {
                authorize_agent_resource(
                    scope,
                    &ProjectResourceRef {
                        kind: Kind::Database,
                        id: settings.database_id.clone(),
                    },
                    Op::Inspect,
                )?;
            }
            Ok(())
        }
        Request::ExportDataset(request) => {
            authorize_agent_resource(scope, &request.resource, Op::Export)?;
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
