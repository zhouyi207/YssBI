use crate::{HarnessError, HarnessHost, MemoryService};
use std::sync::Arc;
use yss_harness_contract::{
    AutomationIdKind, CancellationReason, HarnessEvent, HarnessSessionId, HarnessSessionRecord,
    HarnessSessionState, MemoryRecord, MemoryRecordId, PrincipalId, ProjectSessionBinding,
};

impl HarnessHost {
    pub async fn create_session(
        &self,
        principal_id: PrincipalId,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        self.new_session(principal_id, project, None).await
    }

    pub async fn create_conversation(
        &self,
        principal_id: PrincipalId,
        project_key: String,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let metadata = yss_harness_contract::HarnessConversationMetadata {
            project_key,
            title: String::new(),
            last_opened_at: self.ports.clock.now(),
        };
        self.new_session(principal_id, project, Some(metadata))
            .await
    }

    pub async fn list_conversations(
        &self,
        principal: &PrincipalId,
        project_key: &str,
    ) -> Result<Vec<HarnessSessionRecord>, HarnessError> {
        let mut sessions = self
            .ports
            .sessions
            .list_conversations(principal, project_key)
            .await?;
        sessions.sort_by(|left, right| {
            right
                .conversation
                .as_ref()
                .map(|value| value.last_opened_at)
                .cmp(&left.conversation.as_ref().map(|value| value.last_opened_at))
                .then_with(|| right.created_at.cmp(&left.created_at))
                .then_with(|| right.id.as_str().cmp(left.id.as_str()))
        });
        Ok(sessions)
    }

    pub async fn conversation(
        &self,
        session_id: &HarnessSessionId,
        principal: &PrincipalId,
        project_key: &str,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if &session.principal_id != principal
            || session
                .conversation
                .as_ref()
                .is_none_or(|value| value.project_key != project_key)
        {
            return Err(HarnessError::SessionNotFound);
        }
        Ok(session)
    }

    pub async fn open_conversation(
        &self,
        session_id: &HarnessSessionId,
        principal: &PrincipalId,
        project_key: &str,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let mut session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if &session.principal_id != principal
            || session
                .conversation
                .as_ref()
                .is_none_or(|value| value.project_key != project_key)
        {
            return Err(HarnessError::SessionNotFound);
        }
        if self
            .active_turns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains_key(session_id)
        {
            if session.project == project && session.state == HarnessSessionState::Active {
                return Ok(session);
            }
            return Err(HarnessError::ConcurrentTurn);
        }
        let (_cancellation, _admission) = self.admit_turn(session_id)?;
        session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        session.project = project;
        session.state = HarnessSessionState::Active;
        session.updated_at = self.ports.clock.now();
        session
            .conversation
            .as_mut()
            .ok_or(HarnessError::SessionNotFound)?
            .last_opened_at = session.updated_at;
        self.ports.sessions.update_session(&session).await?;
        Ok(session)
    }

    pub(super) async fn new_session(
        &self,
        principal_id: PrincipalId,
        project: ProjectSessionBinding,
        conversation: Option<yss_harness_contract::HarnessConversationMetadata>,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let id =
            HarnessSessionId::try_new(self.ports.ids.next_id(AutomationIdKind::HarnessSession)?)?;
        let now = self.ports.clock.now();
        let record = HarnessSessionRecord {
            id: id.clone(),
            principal_id,
            project,
            conversation,
            state: HarnessSessionState::Active,
            created_at: now,
            updated_at: now,
        };
        self.ports.sessions.create_session(&record).await?;
        self.event_writer()
            .append(&id, None, HarnessEvent::SessionCreated)
            .await?;
        Ok(record)
    }

    pub async fn reconcile_project_session(
        &self,
        current: &ProjectSessionBinding,
    ) -> Result<usize, HarnessError> {
        let mut stale_count = 0usize;
        for mut session in self.ports.sessions.load_active_sessions().await? {
            if &session.project == current {
                continue;
            }
            session.state = HarnessSessionState::Stale;
            session.updated_at = self.ports.clock.now();
            self.ports.sessions.update_session(&session).await?;
            self.cancel_active_turn(&session.id, CancellationReason::ProjectReplaced);
            MemoryService::new(
                Arc::clone(&self.ports.memory),
                Arc::clone(&self.ports.clock),
                Arc::clone(&self.ports.ids),
            )
            .expire_session(&session.id)
            .await?;
            stale_count += 1;
        }
        Ok(stale_count)
    }

    pub async fn session_memory(
        &self,
        session_id: &HarnessSessionId,
    ) -> Result<Vec<MemoryRecord>, HarnessError> {
        self.ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        Ok(MemoryService::new(
            Arc::clone(&self.ports.memory),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .records_for_session(session_id)
        .await?)
    }

    pub async fn delete_session_memory(
        &self,
        session_id: &HarnessSessionId,
        record_id: &MemoryRecordId,
    ) -> Result<(), HarnessError> {
        if !self
            .session_memory(session_id)
            .await?
            .iter()
            .any(|record| &record.id == record_id)
        {
            return Err(HarnessError::MemoryNotFound);
        }
        MemoryService::new(
            Arc::clone(&self.ports.memory),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .delete(record_id)
        .await?;
        self.event_writer()
            .append(
                session_id,
                None,
                HarnessEvent::MemoryDeleted {
                    record_id: record_id.clone(),
                },
            )
            .await?;
        Ok(())
    }
}
