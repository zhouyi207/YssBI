use super::*;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::task::Poll;
use tokio::sync::Notify;
use yss_harness_contract::{
    HarnessSessionState, HarnessSessionStorePort, HarnessTurnId, HarnessTurnRecord,
    PersistenceFailure, PersistenceFuture,
};
use yss_harness_core::test_support::{
    FixedClock, InMemoryHarnessStore, MockAgentDriver, RejectingCapabilityGateway, SequentialIds,
};

const LOAD: u8 = 1;
const UPDATE: u8 = 2;

#[derive(Default)]
struct PausedSessions {
    store: Arc<InMemoryHarnessStore>,
    pause: AtomicU8,
    entered: Notify,
    release: Notify,
}

impl PausedSessions {
    async fn pause_once(&self, point: u8) {
        if self
            .pause
            .compare_exchange(point, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }
}

impl HarnessSessionStorePort for PausedSessions {
    fn list_conversations<'a>(
        &'a self,
        principal: &'a PrincipalId,
        project_key: &'a str,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>> {
        self.store.list_conversations(principal, project_key)
    }

    fn load_running_turns<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessTurnRecord>, PersistenceFailure>> {
        self.store.load_running_turns()
    }

    fn create_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        self.store.create_session(record)
    }

    fn load_session<'a>(
        &'a self,
        id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessSessionRecord>, PersistenceFailure>> {
        self.store.load_session(id)
    }

    fn load_active_sessions<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>> {
        Box::pin(async move {
            self.pause_once(LOAD).await;
            self.store.load_active_sessions().await
        })
    }

    fn update_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            self.pause_once(UPDATE).await;
            self.store.update_session(record).await
        })
    }

    fn create_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        self.store.create_turn(record)
    }

    fn load_turn<'a>(
        &'a self,
        id: &'a HarnessTurnId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessTurnRecord>, PersistenceFailure>> {
        self.store.load_turn(id)
    }

    fn update_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        self.store.update_turn(record)
    }
}

fn host(sessions: &Arc<PausedSessions>) -> Arc<HarnessHost> {
    let store = &sessions.store;
    Arc::new(
        HarnessHost::new(HarnessPorts {
            resources: Arc::new(yss_harness_core::test_support::FixtureResourceResolver),
            models: yss_harness_core::test_support::fixed_model(Arc::new(MockAgentDriver::new(
                "unused",
            ))),
            capability_gateway: Arc::new(RejectingCapabilityGateway),
            sessions: sessions.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            approvals: store.clone(),
            clock: Arc::new(FixedClock::new(1_000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap(),
    )
}

async fn poll_once<F: Future>(mut future: Pin<&mut F>) -> Option<F::Output> {
    poll_fn(|cx| {
        Poll::Ready(match future.as_mut().poll(cx) {
            Poll::Ready(value) => Some(value),
            Poll::Pending => None,
        })
    })
    .await
}

#[derive(Clone, Copy, Debug)]
enum Selection {
    List,
    Create,
    Open,
}

async fn select(
    application: &ApplicationState,
    host: &HarnessHost,
    principal: &PrincipalId,
    session: &HarnessSessionId,
    selection: Selection,
) -> Result<(), HarnessSessionError> {
    match selection {
        Selection::List => application
            .list_harness_sessions(host, principal)
            .await
            .map(|_| ()),
        Selection::Create => application
            .create_harness_session(host, principal.clone())
            .await
            .map(|_| ()),
        Selection::Open => application
            .open_harness_session(host, principal, session)
            .await
            .map(|_| ()),
    }
}

#[tokio::test]
async fn delayed_project_selection_cannot_invalidate_a_successor_conversation() {
    let mut invalidated = Vec::new();
    for selection in [Selection::List, Selection::Create, Selection::Open] {
        let application = ApplicationState::initialize().unwrap();
        let sessions = Arc::new(PausedSessions::default());
        let host = host(&sessions);
        let principal = PrincipalId::try_new("local-user").unwrap();
        let first = application
            .create_harness_session(&host, principal.clone())
            .await
            .unwrap();
        sessions.pause.store(LOAD, Ordering::Release);
        let old = {
            let application = application.clone();
            let host = host.clone();
            let principal = principal.clone();
            tokio::spawn(async move {
                select(&application, &host, &principal, &first.id, selection).await
            })
        };
        sessions.entered.notified().await;
        application
            .clear_project_for_application(
                application.capture_session().unwrap().project_instance_id(),
            )
            .unwrap();
        let mut successor = Box::pin(application.create_harness_session(&host, principal));
        // Drive the successor to completion or to the session-selection gate, without sleeps.
        let completed = poll_once(successor.as_mut()).await;
        sessions.release.notify_one();
        assert!(matches!(
            old.await.unwrap(),
            Err(HarnessSessionError::Changed)
        ));
        let successor = match completed {
            Some(result) => result,
            None => successor.await,
        }
        .unwrap();
        let current = sessions.load_session(&successor.id).await.unwrap().unwrap();
        if current.state != HarnessSessionState::Active {
            invalidated.push(selection);
        }
    }
    assert!(
        invalidated.is_empty(),
        "old selections invalidated successors: {invalidated:?}"
    );
}

#[tokio::test]
async fn session_selection_write_tails_and_queued_requests_preserve_current_binding() {
    {
        let application = ApplicationState::initialize().unwrap();
        let sessions = Arc::new(PausedSessions::default());
        let host = host(&sessions);
        let principal = PrincipalId::try_new("local-user").unwrap();
        let first = application
            .create_harness_session(&host, principal.clone())
            .await
            .unwrap();
        sessions.pause.store(UPDATE, Ordering::Release);
        let turn = {
            let host = host.clone();
            let id = first.id.clone();
            let binding = first.project.clone();
            tokio::spawn(async move {
                host.submit_turn(
                    &id,
                    &binding,
                    "First title".into(),
                    vec![],
                    None,
                    Default::default(),
                )
                .await
            })
        };
        sessions.entered.notified().await;
        application
            .clear_project_for_application(
                application.capture_session().unwrap().project_instance_id(),
            )
            .unwrap();
        let mut successor = Box::pin(application.create_harness_session(&host, principal));
        let completed = poll_once(successor.as_mut()).await;
        sessions.release.notify_one();
        let _ = turn.await.unwrap();
        match completed {
            Some(result) => result,
            None => successor.await,
        }
        .unwrap();
        assert_eq!(
            sessions
                .load_session(&first.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            HarnessSessionState::Stale,
            "late title UPDATE must not reactivate a replaced session"
        );
    }
    let root = std::env::temp_dir().join(format!("harness-selection-{}", uuid::Uuid::new_v4()));
    let project = yss_project::ProjectState::new();
    let created = project
        .create_project_transaction("First", &root, yss_project_identity::OperationId::new())
        .unwrap();
    let application = ApplicationState::initialize().unwrap();
    application
        .load_project_for_application(created.metadata_path.to_str().unwrap())
        .unwrap();
    let sessions = Arc::new(PausedSessions::default());
    let host = host(&sessions);
    let principal = PrincipalId::try_new("local-user").unwrap();
    let first = application
        .create_harness_session(&host, principal.clone())
        .await
        .unwrap();
    application
        .clear_project_for_application(application.capture_session().unwrap().project_instance_id())
        .unwrap();

    sessions.pause.store(UPDATE, Ordering::Release);
    let old = {
        let application = application.clone();
        let host = host.clone();
        let principal = principal.clone();
        tokio::spawn(async move { application.list_harness_sessions(&host, &principal).await })
    };
    sessions.entered.notified().await;
    application
        .load_project_for_application(created.metadata_path.to_str().unwrap())
        .unwrap();
    let mut reopen = Box::pin(application.open_harness_session(&host, &principal, &first.id));
    let completed = poll_once(reopen.as_mut()).await;
    sessions.release.notify_one();
    assert!(matches!(
        old.await.unwrap(),
        Err(HarnessSessionError::Changed)
    ));
    let reopened = match completed {
        Some(result) => result,
        None => reopen.as_mut().await,
    }
    .unwrap();
    drop(reopen);
    let actual = sessions.load_session(&first.id).await.unwrap().unwrap();
    assert_eq!(
        actual.project, reopened.project,
        "old UPDATE must not restore the preceding project binding"
    );
    assert_eq!(actual.state, HarnessSessionState::Active);

    let (_, _, queued_key) = application.harness_conversation_scope().unwrap();
    let before_queued = sessions
        .list_conversations(&principal, &queued_key)
        .await
        .unwrap()
        .len();
    sessions.pause.store(LOAD, Ordering::Release);
    let blocking = {
        let application = application.clone();
        let host = host.clone();
        let principal = principal.clone();
        tokio::spawn(async move { application.list_harness_sessions(&host, &principal).await })
    };
    sessions.entered.notified().await;
    let mut queued = Box::pin(application.create_harness_session(&host, principal.clone()));
    assert!(poll_once(queued.as_mut()).await.is_none());
    application
        .clear_project_for_application(application.capture_session().unwrap().project_instance_id())
        .unwrap();
    sessions.release.notify_one();
    assert!(matches!(
        blocking.await.unwrap(),
        Err(HarnessSessionError::Changed)
    ));
    assert!(matches!(queued.await, Err(HarnessSessionError::Changed)));
    assert_eq!(
        sessions
            .list_conversations(&principal, &queued_key)
            .await
            .unwrap()
            .len(),
        before_queued,
        "a stale queued request must be rejected before creating another conversation"
    );
    let current = application
        .create_harness_session(&host, principal)
        .await
        .unwrap();
    assert_eq!(
        current.project,
        application.harness_project_binding().unwrap()
    );
    drop(application);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn queued_turn_submission_preserves_the_opened_project_binding() {
    let root = std::env::temp_dir().join(format!("harness-submit-{}", uuid::Uuid::new_v4()));
    let project = yss_project::ProjectState::new();
    let created = project
        .create_project_transaction("First", &root, yss_project_identity::OperationId::new())
        .unwrap();
    let application = ApplicationState::initialize().unwrap();
    application
        .load_project_for_application(created.metadata_path.to_str().unwrap())
        .unwrap();
    let sessions = Arc::new(PausedSessions::default());
    let host = host(&sessions);
    let principal = PrincipalId::try_new("local-user").unwrap();
    let first = application
        .create_harness_session(&host, principal.clone())
        .await
        .unwrap();
    // The IPC submit path first opens a conversation and retains that binding.
    let opened = application
        .open_harness_session(&host, &principal, &first.id)
        .await
        .unwrap();
    application
        .load_project_for_application(created.metadata_path.to_str().unwrap())
        .unwrap();
    let reopened = application
        .open_harness_session(&host, &principal, &first.id)
        .await
        .unwrap();
    assert_ne!(opened.project, reopened.project);
    let before = host.events_after(&first.id, 0).await.unwrap();
    let access = host.session_access().await;
    let mut stale = Box::pin(host.submit_turn(
        &opened.id,
        &opened.project,
        "Message for the previous binding".into(),
        vec![],
        None,
        Default::default(),
    ));
    assert!(poll_once(stale.as_mut()).await.is_none());
    drop(access);
    let stale = stale.await;
    assert!(
        matches!(stale, Err(HarnessError::SessionNotActive)),
        "queued submission must not borrow the conversation's replacement binding: {stale:?}"
    );
    assert_eq!(host.events_after(&first.id, 0).await.unwrap(), before);
    host.submit_turn(
        &reopened.id,
        &reopened.project,
        "Message for the current binding".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    assert!(host.events_after(&first.id, 0).await.unwrap().iter().any(|entry|
        matches!(&entry.event, yss_harness_contract::HarnessEvent::TurnStarted { user_message, .. }
            if user_message == "Message for the current binding")));
    drop(application);
    let _ = std::fs::remove_dir_all(root);
}
