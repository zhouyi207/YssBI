//! Read-only MCP adapter over the YssBI Capability Gateway.

#![forbid(unsafe_code)]

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router};
use yss_automation_contract::{
    AutomationCapabilityRequest, AutomationIdKind, CapabilityFailure, CapabilityGatewayPort,
    CapabilityInvocationContext, CapabilityInvocationId, HarnessSessionId, IdGeneratorPort,
    InspectDatasetProfileRequest, InspectDatasetSchemaRequest, InspectGraphRequest,
    InspectProjectRequest, InspectResultRequest, PrincipalId, ProjectSessionBinding,
    SearchNodeCatalogRequest,
};

#[derive(Clone)]
pub struct McpCapabilityServer {
    gateway: Arc<dyn CapabilityGatewayPort>,
    ids: Arc<dyn IdGeneratorPort>,
    principal_id: PrincipalId,
    harness_session_id: HarnessSessionId,
    project: ProjectSessionBinding,
}

impl McpCapabilityServer {
    pub fn new(
        gateway: Arc<dyn CapabilityGatewayPort>,
        ids: Arc<dyn IdGeneratorPort>,
        principal_id: PrincipalId,
        harness_session_id: HarnessSessionId,
        project: ProjectSessionBinding,
    ) -> Self {
        Self {
            gateway,
            ids,
            principal_id,
            harness_session_id,
            project,
        }
    }

    async fn invoke(&self, request: AutomationCapabilityRequest) -> CallToolResult {
        let invocation_id = self
            .ids
            .next_id(AutomationIdKind::CapabilityInvocation)
            .ok()
            .and_then(|id| CapabilityInvocationId::try_new(id).ok());
        let Some(invocation_id) = invocation_id else {
            return structured_failure(CapabilityFailure::new(
                yss_automation_contract::CapabilityFailureCode::InternalFailure,
            ));
        };
        let context = CapabilityInvocationContext::new(
            self.principal_id.clone(),
            self.harness_session_id.clone(),
            invocation_id,
            self.project.clone(),
        );
        match self.gateway.invoke(context, request).await {
            Ok(result) => match serde_json::to_value(result) {
                Ok(value) => CallToolResult::structured(value),
                Err(_) => structured_failure(CapabilityFailure::new(
                    yss_automation_contract::CapabilityFailureCode::InternalFailure,
                )),
            },
            Err(failure) => structured_failure(failure),
        }
    }
}

#[tool_router(server_handler)]
impl McpCapabilityServer {
    #[tool(
        name = "inspect_graph",
        description = "Inspect a bounded YssBI graph snapshot without mutation."
    )]
    async fn inspect_graph(
        &self,
        Parameters(request): Parameters<InspectGraphRequest>,
    ) -> CallToolResult {
        self.invoke(AutomationCapabilityRequest::InspectGraph(request))
            .await
    }

    #[tool(
        name = "search_node_catalog",
        description = "Search the localized YssBI node catalog without mutation."
    )]
    async fn search_node_catalog(
        &self,
        Parameters(request): Parameters<SearchNodeCatalogRequest>,
    ) -> CallToolResult {
        self.invoke(AutomationCapabilityRequest::SearchNodeCatalog(request))
            .await
    }

    #[tool(
        name = "inspect_dataset_schema",
        description = "Inspect a bounded dataset schema and its current revisions."
    )]
    async fn inspect_dataset_schema(
        &self,
        Parameters(request): Parameters<InspectDatasetSchemaRequest>,
    ) -> CallToolResult {
        self.invoke(AutomationCapabilityRequest::InspectDatasetSchema(request))
            .await
    }

    #[tool(
        name = "inspect_dataset_profile",
        description = "Inspect bounded data-quality and shape statistics for a dataset."
    )]
    async fn inspect_dataset_profile(
        &self,
        Parameters(request): Parameters<InspectDatasetProfileRequest>,
    ) -> CallToolResult {
        self.invoke(AutomationCapabilityRequest::InspectDatasetProfile(request))
            .await
    }

    #[tool(
        name = "inspect_result",
        description = "Inspect a bounded structured YssBI execution result."
    )]
    async fn inspect_result(
        &self,
        Parameters(request): Parameters<InspectResultRequest>,
    ) -> CallToolResult {
        self.invoke(AutomationCapabilityRequest::InspectResult(request))
            .await
    }

    #[tool(
        name = "inspect_project",
        description = "Inspect bounded YssBI project metadata and resource identities."
    )]
    async fn inspect_project(
        &self,
        Parameters(request): Parameters<InspectProjectRequest>,
    ) -> CallToolResult {
        self.invoke(AutomationCapabilityRequest::InspectProject(request))
            .await
    }
}

fn structured_failure(failure: CapabilityFailure) -> CallToolResult {
    CallToolResult::structured_error(serde_json::json!({
        "code": failure.code,
        "details": failure.details,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use yss_automation_contract::{
        AutomationCapabilityResult, CapabilityFuture, DatasetSchemaInspection, IdGenerationFailure,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    struct SequentialIds(AtomicU64);

    impl IdGeneratorPort for SequentialIds {
        fn next_id(&self, _kind: AutomationIdKind) -> Result<String, IdGenerationFailure> {
            Ok(format!("mcp-{}", self.0.fetch_add(1, Ordering::AcqRel)))
        }
    }

    struct StaticGateway;

    impl CapabilityGatewayPort for StaticGateway {
        fn invoke<'a>(
            &'a self,
            _context: CapabilityInvocationContext,
            request: AutomationCapabilityRequest,
        ) -> CapabilityFuture<'a> {
            Box::pin(async move {
                assert!(matches!(
                    request,
                    AutomationCapabilityRequest::InspectDatasetSchema(_)
                ));
                Ok(AutomationCapabilityResult::DatasetSchemaInspection(
                    DatasetSchemaInspection {
                        database_id: "database-1".to_owned(),
                        runtime_revision: 1,
                        schema_revision: 2,
                        columns: Vec::new(),
                    },
                ))
            })
        }
    }

    #[tokio::test]
    async fn mcp_tool_routes_through_the_typed_gateway() {
        let server = McpCapabilityServer::new(
            Arc::new(StaticGateway),
            Arc::new(SequentialIds(AtomicU64::new(1))),
            PrincipalId::try_new("external-agent").unwrap(),
            HarnessSessionId::try_new("external-session").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
        );

        let result = server
            .inspect_dataset_schema(Parameters(InspectDatasetSchemaRequest {
                database_id: "database-1".to_owned(),
            }))
            .await;

        assert_eq!(result.is_error, Some(false));
        assert_eq!(
            result.structured_content.unwrap()["payload"]["databaseId"],
            "database-1"
        );
    }
}
