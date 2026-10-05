use crate::{HarnessError, HarnessHost};
use yss_harness_contract::{
    AutomationIdKind, CancellationReason, HarnessEvent, HarnessSessionId, HarnessSessionRecord,
    HarnessSessionState, PrincipalId, ProjectSessionBinding,
};

/// Serializes session selection and binding writes without owning the current project.
pub struct HarnessSessionAccess<'a> {
    host: &'a HarnessHost,
    _guard: tokio::sync::MutexGuard<'a, ()>,
}

impl HarnessSessionAccess<'_> {
    pub async fn select_model(
        &mut self,
        session_id: &HarnessSessionId,
        principal: &PrincipalId,
        project_key: &str,
        model: yss_harness_contract::LanguageModelSelection,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let mut session = self
            .host
            .conversation(session_id, principal, project_key)
            .await?;
        session
            .conversation
            .as_mut()
            .ok_or(HarnessError::SessionNotFound)?
            .model = Some(model);
        session.updated_at = self.host.ports.clock.now();
        self.host.ports.sessions.update_session(&session).await?;
        Ok(session)
    }

    pub async fn rename_conversation(
        &mut self,
        session_id: &HarnessSessionId,
        principal: &PrincipalId,
        project_key: &str,
        title: String,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(HarnessError::InvalidMessage);
        }
        let (_cancel, _admission) = self.host.admit_turn(session_id)?;
        let mut session = self
            .host
            .conversation(session_id, principal, project_key)
            .await?;
        session
            .conversation
            .as_mut()
            .ok_or(HarnessError::SessionNotFound)?
            .title = title.to_owned();
        session.updated_at = self.host.ports.clock.now();
        self.host.ports.sessions.update_session(&session).await?;
        Ok(session)
    }

    pub async fn create_conversation(
        &mut self,
        principal_id: PrincipalId,
        project_key: String,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        self.host
            .create_conversation(principal_id, project_key, project)
            .await
    }

    pub async fn list_conversations(
        &mut self,
        principal: &PrincipalId,
        project_key: &str,
    ) -> Result<Vec<HarnessSessionRecord>, HarnessError> {
        self.host.list_conversations(principal, project_key).await
    }

    pub async fn open_conversation(
        &mut self,
        session_id: &HarnessSessionId,
        principal: &PrincipalId,
        project_key: &str,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        self.host
            .open_conversation(session_id, principal, project_key, project)
            .await
    }

    pub async fn reconcile_project_session(
        &mut self,
        current: &ProjectSessionBinding,
    ) -> Result<usize, HarnessError> {
        self.host.reconcile_project_session(current).await
    }
}

impl HarnessHost {
    pub async fn session_access(&self) -> HarnessSessionAccess<'_> {
        HarnessSessionAccess {
            host: self,
            _guard: self.session_access.lock().await,
        }
    }

    pub async fn create_session(
        &self,
        principal_id: PrincipalId,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let _access = self.session_access().await;
        self.new_session(principal_id, project, None).await
    }

    async fn create_conversation(
        &self,
        principal_id: PrincipalId,
        project_key: String,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let metadata = yss_harness_contract::HarnessConversationMetadata {
            project_key,
            title: String::new(),
            last_opened_at: self.ports.clock.now(),
            model: None,
        };
        self.new_session(principal_id, project, Some(metadata))
            .await
    }

    async fn list_conversations(
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

    async fn open_conversation(
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

    async fn new_session(
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

    async fn reconcile_project_session(
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
            stale_count += 1;
        }
        Ok(stale_count)
    }
}
