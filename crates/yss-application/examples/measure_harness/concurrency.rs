//! Deterministic external commit between a worker's read and its next write.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use yss_application::harness::ApplicationCapabilityGateway;
use yss_harness_contract::*;

pub struct ConcurrentEdit {
    pub gateway: ApplicationCapabilityGateway,
    pub pending: AtomicBool,
    pub evidence_path: PathBuf,
}

impl CapabilityGatewayPort for ConcurrentEdit {
    fn recover_graph_edit<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: ApplyGraphEditRequest,
    ) -> AgentFuture<'a, Result<Option<GraphEditReceipt>, CapabilityFailure>> {
        self.gateway.recover_graph_edit(context, request)
    }

    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        control: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            let eligible = context
                .agent()
                .is_some_and(|agent| agent.role == AgentRole::Data)
                && request.capability_id() == CapabilityId::ReadDatabaseRows;
            let result = self
                .gateway
                .invoke(context.clone(), request, control.clone())
                .await?;
            if eligible
                && let AutomationCapabilityResult::DatabaseRead(read) = &result
                && let DatabaseReadContent::Rows { row_ids, .. } = &read.content
                && let Some(row_id) = row_ids.first()
                && self.pending.swap(false, Ordering::AcqRel)
            {
                let external = CapabilityInvocationContext::new(
                    PrincipalId::try_new("measurement-concurrent-writer").map_err(|_| invalid())?,
                    HarnessSessionId::try_new("measurement-concurrent-writer")
                        .map_err(|_| invalid())?,
                    CapabilityInvocationId::try_new(uuid::Uuid::new_v4().to_string())
                        .map_err(|_| invalid())?,
                    context.project().clone(),
                );
                let value =
                    serde_json::from_value(serde_json::json!(321.25)).map_err(|_| invalid())?;
                let edit = AutomationCapabilityRequest::EditResource(EditResourceRequest {
                    resource: read.database.resource(),
                    version: read.version.clone(),
                    edit: ResourceEdit::UpdateCells {
                        cells: vec![model::DatabaseCellEdit {
                            row_id: *row_id,
                            column: "x4".into(),
                            value,
                        }],
                    },
                });
                let receipt = self.gateway.invoke(external, edit, control).await?;
                let evidence = serde_json::json!({
                    "readInvocationId": context.invocation_id(), "resource": read.database.resource(),
                    "rowId": row_id, "column": "x4", "value": 321.25,
                    "receipt": model::capability_result(&receipt).map_err(|_| invalid())?,
                });
                std::fs::write(
                    &self.evidence_path,
                    serde_json::to_vec_pretty(&evidence).map_err(|_| invalid())?,
                )
                .map_err(|_| invalid())?;
                println!("Injected external database commit after worker read");
            }
            Ok(result)
        })
    }
}

fn invalid() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::InternalFailure)
}
