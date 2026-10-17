use std::sync::Arc;

use serde_json::Value;
use yss_harness_contract::{
    ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort, AutomationIdKind, ClockPort,
    HarnessSessionId, IdGeneratorPort, PersistenceFailure, PrincipalId, ProjectSessionBinding,
    SourceHash, ToolEffect, ToolSpec,
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
        tool: &ToolSpec,
        request: &Value,
        ttl_ms: u64,
    ) -> Result<ApprovalGrantRecord, ApprovalError> {
        if tool.effect != ToolEffect::Mutate {
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
            tool: tool.name.clone(),
            request_fingerprint: request_fingerprint(&tool.name, request)?,
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
        tool: &str,
        request: &Value,
    ) -> Result<ApprovalGrantRecord, ApprovalError> {
        let mut record = self.store.load(id).await?.ok_or(ApprovalError::NotFound)?;
        let now = self.clock.now();
        if record.principal_id != *principal_id
            || record.session_id != *session_id
            || record.project != *project
            || record.tool != tool
            || record.request_fingerprint != request_fingerprint(tool, request)?
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

fn request_fingerprint(tool: &str, request: &Value) -> Result<SourceHash, ApprovalError> {
    let digest = yss_canonical_hash::hash_canonical("yssbi.approval.request.v1", &(tool, request))
        .map_err(|_| ApprovalError::Fingerprint)?;
    SourceHash::try_new(hex::encode(digest)).map_err(|_| ApprovalError::Fingerprint)
}

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval identity is invalid")]
    Identity(#[from] yss_harness_contract::AutomationIdentityError),
    #[error("approval id generation failed")]
    IdGeneration(#[from] yss_harness_contract::IdGenerationFailure),
    #[error("approval persistence failed")]
    Persistence(#[from] PersistenceFailure),
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
    use crate::test_support::{FixedClock, InMemoryHarnessStore, SequentialIds, note_specs};

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
        let project = crate::test_support::project();
        let specs = note_specs(true);
        let write = specs.iter().find(|spec| spec.name == "write_note").unwrap();
        let read = specs.iter().find(|spec| spec.name == "read_note").unwrap();
        let request = serde_json::json!({"id": "a", "text": "approved"});
        assert!(matches!(
            service
                .issue(
                    principal.clone(),
                    session.clone(),
                    project.clone(),
                    read,
                    &request,
                    1_000
                )
                .await,
            Err(ApprovalError::NotRequired)
        ));
        let grant = service
            .issue(
                principal.clone(),
                session.clone(),
                project.clone(),
                write,
                &request,
                1_000,
            )
            .await
            .unwrap();
        let changed = serde_json::json!({"id": "a", "text": "changed"});
        assert!(matches!(
            service
                .consume(
                    &grant.id,
                    &principal,
                    &session,
                    &project,
                    &write.name,
                    &changed
                )
                .await,
            Err(ApprovalError::BindingMismatch)
        ));
        service
            .consume(
                &grant.id,
                &principal,
                &session,
                &project,
                &write.name,
                &request,
            )
            .await
            .unwrap();
        assert!(matches!(
            service
                .consume(
                    &grant.id,
                    &principal,
                    &session,
                    &project,
                    &write.name,
                    &request
                )
                .await,
            Err(ApprovalError::AlreadyConsumed)
        ));
    }
}
