//! Harness startup and project-session coordination over injected neutral ports.

use std::sync::Arc;
use thiserror::Error;
use yss_harness_contract::{
    HarnessSessionId, HarnessSessionRecord, PrincipalId, ProjectSessionBinding,
};
use yss_harness_core::{
    HarnessError, HarnessHost, HarnessPorts, KnowledgeError, install_builtin_statistical_knowledge,
};

use crate::session::{ApplicationSession, ApplicationState, SessionCaptureError};

#[derive(Debug, Error)]
pub enum HarnessInitializationError {
    #[error("initial Harness project session could not be captured")]
    SessionCapture(#[from] SessionCaptureError),
    #[error("Harness host could not be initialized")]
    Host(#[from] HarnessError),
    #[error("Harness builtin knowledge could not be initialized")]
    Knowledge(#[from] KnowledgeError),
}

#[derive(Debug, Error)]
pub enum HarnessSessionError {
    #[error("Harness project changed during session selection")]
    Changed,
    #[error("Harness project root is unavailable")]
    ProjectUnavailable,
    #[error("Harness project session could not be captured")]
    SessionCapture(#[from] SessionCaptureError),
    #[error("Harness session could not be created")]
    Host(#[from] HarnessError),
}

impl ApplicationState {
    pub async fn initialize_harness(
        &self,
        ports: HarnessPorts,
    ) -> Result<Arc<HarnessHost>, HarnessInitializationError> {
        let current_project = self.harness_project_binding()?;
        install_builtin_statistical_knowledge(ports.knowledge.clone(), ports.clock.now()).await?;
        let host = Arc::new(HarnessHost::new(ports)?);
        // Recover interrupted work before reconciling project bindings and workflow state.
        host.recover_interrupted_turns().await?;
        host.reconcile_project_session(&current_project).await?;
        host.recover_workflows().await?;
        Ok(host)
    }

    pub async fn create_harness_session(
        &self,
        host: &HarnessHost,
        principal: PrincipalId,
    ) -> Result<HarnessSessionRecord, HarnessSessionError> {
        let (captured, binding, project_key) = self.harness_conversation_scope()?;
        host.reconcile_project_session(&binding).await?;
        let record = host
            .create_conversation(principal, project_key, binding)
            .await?;
        self.revalidate_captured_session(&captured)
            .map_err(|_| HarnessSessionError::Changed)?;
        Ok(record)
    }

    pub async fn list_harness_sessions(
        &self,
        host: &HarnessHost,
        principal: &PrincipalId,
    ) -> Result<Vec<HarnessSessionRecord>, HarnessSessionError> {
        let (captured, binding, key) = self.harness_conversation_scope()?;
        host.reconcile_project_session(&binding).await?;
        let sessions = host.list_conversations(principal, &key).await?;
        self.revalidate_captured_session(&captured)
            .map_err(|_| HarnessSessionError::Changed)?;
        Ok(sessions)
    }

    pub async fn open_harness_session(
        &self,
        host: &HarnessHost,
        principal: &PrincipalId,
        session_id: &HarnessSessionId,
    ) -> Result<HarnessSessionRecord, HarnessSessionError> {
        let (captured, binding, key) = self.harness_conversation_scope()?;
        host.reconcile_project_session(&binding).await?;
        let session = host
            .open_conversation(session_id, principal, &key, binding)
            .await?;
        self.revalidate_captured_session(&captured)
            .map_err(|_| HarnessSessionError::Changed)?;
        Ok(session)
    }

    pub async fn validate_harness_session(
        &self,
        host: &HarnessHost,
        principal: &PrincipalId,
        session_id: &HarnessSessionId,
    ) -> Result<(), HarnessSessionError> {
        let (captured, binding, key) = self.harness_conversation_scope()?;
        let session = host.conversation(session_id, principal, &key).await?;
        if session.project != binding {
            return Err(HarnessSessionError::Changed);
        }
        self.revalidate_captured_session(&captured)
            .map_err(|_| HarnessSessionError::Changed)
    }

    fn harness_conversation_scope(
        &self,
    ) -> Result<(Arc<ApplicationSession>, ProjectSessionBinding, String), HarnessSessionError> {
        let captured = self.capture_session()?;
        let key = if captured.project().get_path().is_some() {
            let project = captured
                .project()
                .capture_project_session()
                .map_err(|_| HarnessSessionError::ProjectUnavailable)?;
            if &project.instance_id != captured.project_instance_id() {
                return Err(HarnessSessionError::Changed);
            }
            let root = yss_filesystem::RootBinding::for_existing(project.root.as_path())
                .map_err(|_| HarnessSessionError::ProjectUnavailable)?;
            format!(
                "root:{}",
                root.identity()
                    .ok_or(HarnessSessionError::ProjectUnavailable)?
                    .as_str()
            )
        } else {
            format!("unsaved:{}", captured.project_instance_id())
        };
        let binding = ProjectSessionBinding::new(
            captured.project_instance_id().clone(),
            captured.project_session_id().clone(),
        );
        self.revalidate_captured_session(&captured)
            .map_err(|_| HarnessSessionError::Changed)?;
        Ok((captured, binding, key))
    }

    fn harness_project_binding(&self) -> Result<ProjectSessionBinding, SessionCaptureError> {
        let captured = self.capture_session()?;
        Ok(ProjectSessionBinding::new(
            captured.project_instance_id().clone(),
            captured.project_session_id().clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_harness_contract::{
        HarnessSessionState, HarnessSessionStorePort, HarnessTurnId, HarnessTurnRecord,
        HarnessTurnState, UnixMillis,
    };
    use yss_harness_core::test_support::{
        FixedClock, InMemoryHarnessStore, MockAgentDriver, RejectingCapabilityGateway,
        SequentialIds,
    };
    use yss_harness_core::{KnowledgeQuery, KnowledgeService};
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    fn project_application(project: Arc<yss_project::ProjectState>) -> ApplicationState {
        let components = crate::session::NodeComponents::builtins().unwrap();
        let candidate = crate::session::build_current_project_candidate(
            crate::session::ApplicationSessionEpoch::INITIAL,
            project,
            [],
            &components,
        )
        .unwrap();
        let application = ApplicationState::new(Arc::new(
            crate::session::ApplicationSessionSlot::new(components),
        ));
        application.install_candidate(candidate).unwrap();
        application
    }

    fn persistent_ports(
        store: Arc<yss_harness_sqlite::SqliteHarnessStore>,
        clock: Arc<FixedClock>,
        ids: Arc<SequentialIds>,
    ) -> HarnessPorts {
        HarnessPorts {
            agent_driver: Arc::new(MockAgentDriver::new("Saved answer")),
            capability_gateway: Arc::new(RejectingCapabilityGateway),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: Arc::new(InMemoryHarnessStore::default()),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store,
            clock,
            ids,
        }
    }

    #[tokio::test]
    async fn conversations_restore_after_project_and_database_reopen_and_remain_isolated() {
        let directory =
            std::env::temp_dir().join(format!("yss-conversations-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let project = Arc::new(yss_project::ProjectState::new());
        let created = project
            .create_project_transaction(
                "Conversations",
                &directory.join("project"),
                yss_project_identity::OperationId::new(),
            )
            .unwrap();
        project
            .activate_project_from_path(&created.metadata_path)
            .unwrap();
        let application = project_application(project);
        let data_directory = directory.join("app-data");
        let store = Arc::new(
            yss_harness_sqlite::SqliteHarnessStore::connect(data_directory.clone())
                .await
                .unwrap(),
        );
        let clock = Arc::new(FixedClock::new(1_000));
        let ids = Arc::new(SequentialIds::default());
        let host = application
            .initialize_harness(persistent_ports(store.clone(), clock.clone(), ids.clone()))
            .await
            .unwrap();
        let principal = PrincipalId::try_new("local-user").unwrap();
        let first = application
            .create_harness_session(&host, principal.clone())
            .await
            .unwrap();
        host.submit_turn(&first.id, "Remember the first conversation".into(), None)
            .await
            .unwrap();
        clock.advance(10);
        let second = application
            .create_harness_session(&host, principal.clone())
            .await
            .unwrap();
        host.submit_turn(
            &second.id,
            "Keep the second conversation separate".into(),
            None,
        )
        .await
        .unwrap();
        clock.advance(10);
        application
            .open_harness_session(&host, &principal, &first.id)
            .await
            .unwrap();
        drop(host);
        drop(store);
        drop(application);

        let project = Arc::new(yss_project::ProjectState::new());
        project
            .activate_project_from_path(&created.metadata_path)
            .unwrap();
        let application = project_application(project);
        let store = Arc::new(
            yss_harness_sqlite::SqliteHarnessStore::connect(data_directory)
                .await
                .unwrap(),
        );
        let host = application
            .initialize_harness(persistent_ports(store.clone(), clock.clone(), ids.clone()))
            .await
            .unwrap();
        let sessions = application
            .list_harness_sessions(&host, &principal)
            .await
            .unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, first.id);
        assert_eq!(
            sessions[0].conversation.as_ref().unwrap().title,
            "Remember the first conversation"
        );
        assert!(
            application
                .validate_harness_session(&host, &principal, &first.id)
                .await
                .is_err()
        );
        let restored = application
            .open_harness_session(&host, &principal, &first.id)
            .await
            .unwrap();
        assert_ne!(restored.project, first.project);
        application
            .validate_harness_session(&host, &principal, &restored.id)
            .await
            .unwrap();
        host.submit_turn(&restored.id, "Continue the first conversation".into(), None)
            .await
            .unwrap();
        let messages = |events: Vec<yss_harness_contract::HarnessEventEnvelope>| {
            events
                .into_iter()
                .filter_map(|event| match event.event {
                    yss_harness_contract::HarnessEvent::TurnStarted { user_message } => {
                        Some(user_message)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            messages(host.events_after(&first.id, 0).await.unwrap()),
            [
                "Remember the first conversation",
                "Continue the first conversation"
            ]
        );
        assert_eq!(
            messages(host.events_after(&second.id, 0).await.unwrap()),
            ["Keep the second conversation separate"]
        );
        assert!(
            application
                .list_harness_sessions(&host, &PrincipalId::try_new("another-user").unwrap())
                .await
                .unwrap()
                .is_empty()
        );

        let another_project = Arc::new(yss_project::ProjectState::new());
        let another = another_project
            .create_project_transaction(
                "Other",
                &directory.join("other"),
                yss_project_identity::OperationId::new(),
            )
            .unwrap();
        another_project
            .activate_project_from_path(&another.metadata_path)
            .unwrap();
        let another_application = project_application(another_project);
        assert!(
            another_application
                .list_harness_sessions(&host, &principal)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            another_application
                .open_harness_session(&host, &principal, &first.id)
                .await
                .is_err()
        );
        drop(another_application);
        drop(application);
        drop(host);
        drop(store);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[tokio::test]
    async fn startup_recovers_interrupted_work_and_sessions_follow_project_replacement() {
        let application = ApplicationState::initialize().unwrap();
        let store = Arc::new(InMemoryHarnessStore::default());
        let ports = HarnessPorts {
            agent_driver: Arc::new(MockAgentDriver::new("unused")),
            capability_gateway: Arc::new(RejectingCapabilityGateway),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store.clone(),
            clock: Arc::new(FixedClock::new(2_000)),
            ids: Arc::new(SequentialIds::default()),
        };
        let previous_host = HarnessHost::new(ports.clone()).unwrap();
        let principal = PrincipalId::try_new("local-user").unwrap();
        let old = previous_host
            .create_session(
                principal.clone(),
                ProjectSessionBinding::new(
                    ProjectInstanceId::from_existing("old-project".into()),
                    ProjectSessionId::new("old-session"),
                ),
            )
            .await
            .unwrap();
        let turn = HarnessTurnRecord {
            id: HarnessTurnId::try_new("interrupted-turn").unwrap(),
            session_id: old.id.clone(),
            state: HarnessTurnState::Running,
            user_message: "interrupted".into(),
            final_text: None,
            started_at: UnixMillis::from_existing(1_000),
            finished_at: None,
        };
        store.create_turn(&turn).await.unwrap();
        drop(previous_host);

        let host = application.initialize_harness(ports).await.unwrap();
        assert_eq!(
            store.load_turn(&turn.id).await.unwrap().unwrap().state,
            HarnessTurnState::Failed
        );
        assert_eq!(
            store.load_session(&old.id).await.unwrap().unwrap().state,
            HarnessSessionState::Stale
        );
        assert!(
            !KnowledgeService::new(store.clone())
                .search(KnowledgeQuery {
                    text: "quality".into(),
                    scopes: Vec::new(),
                    project: None,
                    limit: 5,
                })
                .await
                .unwrap()
                .is_empty()
        );

        let session = application
            .create_harness_session(&host, principal.clone())
            .await
            .unwrap();
        assert_eq!(
            session.project,
            application.harness_project_binding().unwrap()
        );
        application
            .clear_project_for_application(
                application.capture_session().unwrap().project_instance_id(),
            )
            .unwrap();
        let replacement = application
            .create_harness_session(&host, principal)
            .await
            .unwrap();
        assert_ne!(replacement.project, session.project);
        assert_eq!(
            store
                .load_session(&session.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            HarnessSessionState::Stale
        );
    }
}
