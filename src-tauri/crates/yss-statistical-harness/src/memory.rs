use std::sync::Arc;

use yss_automation_contract::{
    AutomationIdKind, ClockPort, IdGeneratorPort, MemoryAuthor, MemoryProposal, MemoryRecord,
    MemoryRecordId, MemoryScope, MemoryStatus, MemoryStorePort, PersistenceFailure,
    ProjectSessionBinding, RetentionPolicy, SensitivityClass,
};

const MAX_MEMORY_FIELD_BYTES: usize = 4_096;

pub struct MemoryService {
    store: Arc<dyn MemoryStorePort>,
    clock: Arc<dyn ClockPort>,
    ids: Arc<dyn IdGeneratorPort>,
}

impl MemoryService {
    pub fn new(
        store: Arc<dyn MemoryStorePort>,
        clock: Arc<dyn ClockPort>,
        ids: Arc<dyn IdGeneratorPort>,
    ) -> Self {
        Self { store, clock, ids }
    }

    pub async fn propose(&self, proposal: MemoryProposal) -> Result<MemoryRecord, MemoryError> {
        validate_proposal(&proposal)?;
        let now = self.clock.now();
        let record = MemoryRecord {
            id: MemoryRecordId::try_new(self.ids.next_id(AutomationIdKind::MemoryRecord)?)?,
            session_id: proposal.session_id,
            scope: proposal.scope,
            kind: proposal.value.kind(),
            value: proposal.value,
            source_refs: proposal.source_refs,
            confidence: proposal.confidence,
            status: if proposal.created_by == MemoryAuthor::User {
                MemoryStatus::Active
            } else {
                MemoryStatus::Proposed
            },
            project: proposal.project,
            sensitivity: proposal.sensitivity,
            created_by: proposal.created_by,
            supersedes: proposal.supersedes,
            retention: proposal.retention,
            created_at: now,
            updated_at: now,
        };
        self.store.insert(&record).await?;
        Ok(record)
    }

    pub async fn approve(
        &self,
        id: &MemoryRecordId,
        current_project: Option<&ProjectSessionBinding>,
    ) -> Result<MemoryRecord, MemoryError> {
        let mut record = self.store.load(id).await?.ok_or(MemoryError::NotFound)?;
        if record.status != MemoryStatus::Proposed {
            return Err(MemoryError::InvalidTransition);
        }
        if record
            .project
            .as_ref()
            .is_some_and(|project| Some(project) != current_project)
        {
            return Err(MemoryError::ProjectChanged);
        }
        let mut superseded = match &record.supersedes {
            Some(superseded_id) => Some(
                self.store
                    .load(superseded_id)
                    .await?
                    .ok_or(MemoryError::NotFound)?,
            ),
            None => None,
        };
        if let Some(previous) = &mut superseded {
            if previous.status != MemoryStatus::Active
                || previous.scope != record.scope
                || previous.project != record.project
            {
                return Err(MemoryError::InvalidSupersession);
            }
            previous.status = MemoryStatus::Superseded;
            previous.updated_at = self.clock.now();
        }
        record.status = MemoryStatus::Active;
        record.updated_at = self.clock.now();
        self.store.activate(&record, superseded.as_ref()).await?;
        Ok(record)
    }

    pub async fn invalidate_source(
        &self,
        source_id: &str,
        current_revision: Option<&str>,
    ) -> Result<usize, MemoryError> {
        let mut invalidated = 0usize;
        for mut record in self.store.list_active().await? {
            let stale = record.source_refs.iter().any(|source| {
                source.source_id == source_id
                    && current_revision.is_none_or(|revision| source.source_revision != revision)
            });
            if stale {
                record.status = MemoryStatus::Invalidated;
                record.updated_at = self.clock.now();
                self.store.update(&record).await?;
                invalidated += 1;
            }
        }
        Ok(invalidated)
    }

    pub async fn delete(&self, id: &MemoryRecordId) -> Result<(), MemoryError> {
        let mut record = self.store.load(id).await?.ok_or(MemoryError::NotFound)?;
        record.status = MemoryStatus::Deleted;
        record.updated_at = self.clock.now();
        self.store.update(&record).await?;
        Ok(())
    }

    pub async fn records_for_session(
        &self,
        session_id: &yss_automation_contract::HarnessSessionId,
    ) -> Result<Vec<MemoryRecord>, MemoryError> {
        Ok(self.store.query_session(session_id).await?)
    }

    pub async fn expire_session(
        &self,
        session_id: &yss_automation_contract::HarnessSessionId,
    ) -> Result<usize, MemoryError> {
        let mut expired = 0usize;
        for mut record in self.store.query_session(session_id).await? {
            if record.retention == RetentionPolicy::Session
                && !matches!(
                    record.status,
                    MemoryStatus::Deleted | MemoryStatus::Superseded
                )
            {
                record.status = MemoryStatus::Deleted;
                record.updated_at = self.clock.now();
                self.store.update(&record).await?;
                expired += 1;
            }
        }
        Ok(expired)
    }
}

fn validate_proposal(proposal: &MemoryProposal) -> Result<(), MemoryError> {
    if proposal.scope == MemoryScope::User {
        return Err(MemoryError::PolicyRejected);
    }
    let project_bound = matches!(
        proposal.scope,
        MemoryScope::Session | MemoryScope::Project | MemoryScope::Episodic
    );
    if project_bound != proposal.project.is_some()
        || (proposal.sensitivity == SensitivityClass::Restricted && proposal.project.is_none())
        || !retention_matches(proposal.scope, proposal.retention)
        || (proposal.created_by != MemoryAuthor::User && proposal.source_refs.is_empty())
    {
        return Err(MemoryError::PolicyRejected);
    }
    for field in proposal.value.text_fields() {
        let normalized = field.to_lowercase();
        if field.trim().is_empty()
            || field.len() > MAX_MEMORY_FIELD_BYTES
            || ["api_key=", "password=", "bearer ", "connection_string="]
                .iter()
                .any(|marker| normalized.contains(marker))
        {
            return Err(MemoryError::PolicyRejected);
        }
    }
    if proposal.source_refs.iter().any(|source| {
        source.source_id.trim().is_empty()
            || source.source_revision.trim().is_empty()
            || source.source_id.len() > 256
            || source.source_revision.len() > 256
    }) {
        return Err(MemoryError::PolicyRejected);
    }
    Ok(())
}

fn retention_matches(scope: MemoryScope, retention: RetentionPolicy) -> bool {
    matches!(
        (scope, retention),
        (MemoryScope::Session, RetentionPolicy::Session)
            | (MemoryScope::Project, RetentionPolicy::Project)
            | (MemoryScope::Episodic, RetentionPolicy::Project)
    )
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("memory identity is invalid")]
    Identity(#[from] yss_automation_contract::AutomationIdentityError),
    #[error("memory id generation failed")]
    IdGeneration(#[from] yss_automation_contract::IdGenerationFailure),
    #[error("memory persistence failed")]
    Persistence(#[from] PersistenceFailure),
    #[error("memory proposal was rejected by policy")]
    PolicyRejected,
    #[error("memory record was not found")]
    NotFound,
    #[error("memory status transition is invalid")]
    InvalidTransition,
    #[error("memory project binding changed")]
    ProjectChanged,
    #[error("memory supersession is invalid")]
    InvalidSupersession,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::test_support::{FixedClock, InMemoryHarnessStore, SequentialIds};
    use yss_automation_contract::{
        HarnessSessionId, MemoryConfidence, MemorySourceRef, StructuredMemoryValue,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    #[test]
    fn policy_rejects_agent_memory_without_source_and_secret_like_values() {
        let base = MemoryProposal {
            session_id: HarnessSessionId::try_new("session-1").unwrap(),
            scope: MemoryScope::Session,
            value: StructuredMemoryValue::ResearchQuestion {
                question: "How should this be reported?".to_owned(),
            },
            source_refs: Vec::new(),
            confidence: MemoryConfidence::High,
            project: Some(yss_automation_contract::ProjectSessionBinding::new(
                yss_project_identity::ProjectInstanceId::from_existing("project-1".into()),
                yss_project_identity::ProjectSessionId::new("project-session-1"),
            )),
            sensitivity: SensitivityClass::Internal,
            created_by: MemoryAuthor::AgentProposal,
            supersedes: None,
            retention: RetentionPolicy::Session,
        };
        assert!(validate_proposal(&base).is_err());

        let secret = MemoryProposal {
            source_refs: vec![MemorySourceRef {
                source_id: "user-confirmation".to_owned(),
                source_revision: "1".to_owned(),
            }],
            value: StructuredMemoryValue::ResearchQuestion {
                question: "api_key=secret".to_owned(),
            },
            ..base
        };
        assert!(validate_proposal(&secret).is_err());
    }

    #[tokio::test]
    async fn approved_memory_is_invalidated_when_its_source_revision_changes() {
        let store = Arc::new(InMemoryHarnessStore::default());
        let service = MemoryService::new(
            store.clone(),
            Arc::new(FixedClock::new(10)),
            Arc::new(SequentialIds::default()),
        );
        let project = ProjectSessionBinding::new(
            ProjectInstanceId::from_existing("project-1".into()),
            ProjectSessionId::new("project-session-1"),
        );
        let proposed = service
            .propose(MemoryProposal {
                session_id: HarnessSessionId::try_new("session-1").unwrap(),
                scope: MemoryScope::Project,
                value: StructuredMemoryValue::DatasetSemantic {
                    resource_id: "database-1".to_owned(),
                    meaning: "Monthly revenue".to_owned(),
                },
                source_refs: vec![MemorySourceRef {
                    source_id: "database-1".to_owned(),
                    source_revision: "1".to_owned(),
                }],
                confidence: MemoryConfidence::High,
                project: Some(project.clone()),
                sensitivity: SensitivityClass::Internal,
                created_by: MemoryAuthor::AgentProposal,
                supersedes: None,
                retention: RetentionPolicy::Project,
            })
            .await
            .unwrap();
        let active = service.approve(&proposed.id, Some(&project)).await.unwrap();
        assert_eq!(active.status, MemoryStatus::Active);

        assert_eq!(
            service
                .invalidate_source("database-1", Some("2"))
                .await
                .unwrap(),
            1
        );
        let records = service
            .records_for_session(&active.session_id)
            .await
            .unwrap();
        assert_eq!(records[0].status, MemoryStatus::Invalidated);
    }
}
