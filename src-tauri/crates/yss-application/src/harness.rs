//! Harness startup and project-session coordination over injected neutral ports.

use std::sync::Arc;
use thiserror::Error;
use yss_harness_contract::{HarnessSessionRecord, PrincipalId, ProjectSessionBinding};
use yss_harness_core::{
    HarnessError, HarnessHost, HarnessPorts, KnowledgeError, install_builtin_statistical_knowledge,
};

use crate::session::{ApplicationState, SessionCaptureError};

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
        let binding = self.harness_project_binding()?;
        host.reconcile_project_session(&binding).await?;
        Ok(host.create_session(principal, binding).await?)
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
        application.clear_project_for_application().unwrap();
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
