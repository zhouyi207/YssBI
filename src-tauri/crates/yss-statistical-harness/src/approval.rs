use std::sync::Arc;

use yss_automation_contract::{
    ApprovalGrantId, ApprovalGrantRecord, ApprovalPolicy, ApprovalStorePort,
    AutomationCapabilityRequest, AutomationIdKind, ClockPort, HarnessSessionId, IdGeneratorPort,
    PersistenceFailure, PrincipalId, ProjectSessionBinding, SourceHash,
};

pub struct ApprovalService {
    store: Arc<dyn ApprovalStorePort>,
    clock: Arc<dyn ClockPort>,
    ids: Arc<dyn IdGeneratorPort>,
}

impl ApprovalService {
    pub fn new(
        store: Arc<dyn ApprovalStorePort>,
        clock: Arc<dyn ClockPort>,
        ids: Arc<dyn IdGeneratorPort>,
    ) -> Self {
        Self { store, clock, ids }
    }

    pub async fn issue(
        &self,
        principal_id: PrincipalId,
        session_id: HarnessSessionId,
        project: ProjectSessionBinding,
        request: &AutomationCapabilityRequest,
        ttl_ms: u64,
    ) -> Result<ApprovalGrantRecord, ApprovalError> {
        request
            .validate()
            .map_err(|_| ApprovalError::InvalidRequest)?;
        if request.capability_id().descriptor().approval != ApprovalPolicy::Required {
            return Err(ApprovalError::NotRequired);
        }
        if ttl_ms == 0 || ttl_ms > 10 * 60 * 1_000 {
            return Err(ApprovalError::InvalidExpiry);
        }
        let issued_at = self.clock.now();
        let expires_at = issued_at
            .checked_add(ttl_ms)
            .ok_or(ApprovalError::InvalidExpiry)?;
        let record = ApprovalGrantRecord {
            id: ApprovalGrantId::try_new(self.ids.next_id(AutomationIdKind::ApprovalGrant)?)?,
            principal_id,
            session_id,
            project,
            capability_id: request.capability_id(),
            request_fingerprint: request_fingerprint(request)?,
            issued_at,
            expires_at,
            consumed_at: None,
        };
        self.store.insert(&record).await?;
        Ok(record)
    }

    pub async fn consume(
        &self,
        id: &ApprovalGrantId,
        principal_id: &PrincipalId,
        session_id: &HarnessSessionId,
        project: &ProjectSessionBinding,
        request: &AutomationCapabilityRequest,
    ) -> Result<ApprovalGrantRecord, ApprovalError> {
        let mut record = self.store.load(id).await?.ok_or(ApprovalError::NotFound)?;
        let now = self.clock.now();
        if record.principal_id != *principal_id
            || record.session_id != *session_id
            || record.project != *project
            || record.capability_id != request.capability_id()
            || record.request_fingerprint != request_fingerprint(request)?
        {
            return Err(ApprovalError::BindingMismatch);
        }
        if record.expires_at <= now {
            return Err(ApprovalError::Expired);
        }
        if record.consumed_at.is_some() || !self.store.consume(id, now).await? {
            return Err(ApprovalError::AlreadyConsumed);
        }
        record.consumed_at = Some(now);
        Ok(record)
    }
}

fn request_fingerprint(request: &AutomationCapabilityRequest) -> Result<SourceHash, ApprovalError> {
    let digest = yss_canonical_hash::hash_canonical("yssbi.approval.request.v1", request)
        .map_err(|_| ApprovalError::Fingerprint)?;
    SourceHash::try_new(
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .map_err(|_| ApprovalError::Fingerprint)
}

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval identity is invalid")]
    Identity(#[from] yss_automation_contract::AutomationIdentityError),
    #[error("approval id generation failed")]
    IdGeneration(#[from] yss_automation_contract::IdGenerationFailure),
    #[error("approval persistence failed")]
    Persistence(#[from] PersistenceFailure),
    #[error("approval request is invalid")]
    InvalidRequest,
    #[error("capability does not require approval")]
    NotRequired,
    #[error("approval expiry is invalid")]
    InvalidExpiry,
    #[error("approval request fingerprint failed")]
    Fingerprint,
    #[error("approval grant was not found")]
    NotFound,
    #[error("approval grant binding does not match")]
    BindingMismatch,
    #[error("approval grant expired")]
    Expired,
    #[error("approval grant was already consumed")]
    AlreadyConsumed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FixedClock, InMemoryHarnessStore, SequentialIds};
    use yss_automation_contract::{ApplyGraphEditRequest, GraphEditOperation, GraphEditPosition};
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    #[tokio::test]
    async fn approval_is_exactly_bound_and_consumed_once() {
        let store = Arc::new(InMemoryHarnessStore::default());
        let service = ApprovalService::new(
            store,
            Arc::new(FixedClock::new(100)),
            Arc::new(SequentialIds::default()),
        );
        let principal = PrincipalId::try_new("user-1").unwrap();
        let session = HarnessSessionId::try_new("session-1").unwrap();
        let project = ProjectSessionBinding::new(
            ProjectInstanceId::from_existing("project-1".into()),
            ProjectSessionId::new("project-session-1"),
        );
        let request = AutomationCapabilityRequest::ApplyGraphEdit(ApplyGraphEditRequest {
            graph_path: "events/Main.yssbi-event".to_owned(),
            base_revision: 1,
            client_key: "assistant-edit-1".to_owned(),
            locale: "en-US".to_owned(),
            operations: vec![GraphEditOperation::MoveNodes {
                positions: vec![GraphEditPosition {
                    node_id: "00000000-0000-0000-0000-000000000001".to_owned(),
                    x: 10.0,
                    y: 20.0,
                }],
            }],
        });
        let grant = service
            .issue(
                principal.clone(),
                session.clone(),
                project.clone(),
                &request,
                1_000,
            )
            .await
            .unwrap();

        service
            .consume(&grant.id, &principal, &session, &project, &request)
            .await
            .unwrap();
        assert!(matches!(
            service
                .consume(&grant.id, &principal, &session, &project, &request)
                .await,
            Err(ApprovalError::AlreadyConsumed)
        ));
    }
}
