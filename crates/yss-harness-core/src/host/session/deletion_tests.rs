use crate::test_support::{
    FixedClock, FixtureResourceResolver, InMemoryHarnessStore, MockAgentDriver, SequentialIds,
    fixed_model,
};
use crate::{HarnessError, HarnessHost, HarnessPorts};
use std::sync::Arc;
use yss_harness_contract::*;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

#[tokio::test]
async fn deletion_requires_the_owner_and_an_idle_session_and_prevents_reopening() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = HarnessHost::new(HarnessPorts {
        resources: Arc::new(FixtureResourceResolver),
        models: fixed_model(Arc::new(MockAgentDriver::new("unused"))),
        extension: Arc::new(crate::test_support::NoteExtension::default()),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        clock: Arc::new(FixedClock::new(1000)),
        ids: Arc::new(SequentialIds::default()),
    })
    .unwrap();
    let principal = PrincipalId::try_new("user").unwrap();
    let session = host
        .session_access()
        .await
        .create_conversation(
            principal.clone(),
            "project-key".into(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project".into()),
                ProjectSessionId::new("project-session"),
            ),
        )
        .await
        .unwrap();
    assert!(matches!(
        host.session_access()
            .await
            .delete_conversation(
                &session.id,
                &PrincipalId::try_new("other-user").unwrap(),
                "project-key",
            )
            .await,
        Err(HarnessError::SessionNotFound)
    ));
    let (_cancel, admission) = host.admit_turn(&session.id).unwrap();
    assert!(matches!(
        host.session_access()
            .await
            .delete_conversation(&session.id, &principal, "project-key",)
            .await,
        Err(HarnessError::ConcurrentTurn)
    ));
    drop(admission);

    host.session_access()
        .await
        .delete_conversation(&session.id, &principal, "project-key")
        .await
        .unwrap();

    assert!(host.events_after(&session.id, 0).await.unwrap().is_empty());
    assert!(matches!(
        host.session_access()
            .await
            .open_conversation(
                &session.id,
                &principal,
                "project-key",
                session.project.clone(),
            )
            .await,
        Err(HarnessError::SessionNotFound)
    ));
    assert!(matches!(
        host.submit_turn(
            &session.id,
            &session.project,
            "Do not resurrect".into(),
            vec![],
            None,
            Default::default(),
        )
        .await,
        Err(HarnessError::SessionNotFound)
    ));
}
