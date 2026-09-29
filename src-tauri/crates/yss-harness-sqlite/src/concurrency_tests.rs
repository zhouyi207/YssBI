use super::*;
use yss_harness_contract::*;
use yss_harness_contract::{
    PrincipalId, ProjectSessionBinding, WorkflowStepId, WorkflowStepRecord, WorkflowStepState,
};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

fn session() -> HarnessSessionRecord {
    HarnessSessionRecord {
        id: HarnessSessionId::try_new("concurrent-session").unwrap(),
        principal_id: PrincipalId::try_new("user-1").unwrap(),
        project: ProjectSessionBinding::new(
            ProjectInstanceId::from_existing("project-1".into()),
            ProjectSessionId::new("project-session-1"),
        ),
        conversation: None,
        state: HarnessSessionState::Active,
        created_at: UnixMillis::from_existing(1),
        updated_at: UnixMillis::from_existing(1),
    }
}

#[tokio::test]
async fn stale_conversations_remain_loadable_and_can_be_reactivated() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let mut session = session();
    session.conversation = Some(HarnessConversationMetadata {
        project_key: "project-root".into(),
        title: "Dataset review".into(),
        last_opened_at: session.created_at,
    });
    store.create_session(&session).await.unwrap();
    assert_eq!(
        store.load_active_sessions().await.unwrap(),
        vec![session.clone()]
    );

    session.state = HarnessSessionState::Stale;
    session.updated_at = UnixMillis::from_existing(2);
    store.update_session(&session).await.unwrap();
    assert!(store.load_active_sessions().await.unwrap().is_empty());
    assert_eq!(
        store
            .list_conversations(&session.principal_id, "project-root")
            .await
            .unwrap(),
        vec![session.clone()]
    );
    assert_eq!(
        store.load_session(&session.id).await.unwrap(),
        Some(session.clone())
    );

    session.state = HarnessSessionState::Active;
    session.updated_at = UnixMillis::from_existing(3);
    store.update_session(&session).await.unwrap();
    assert_eq!(store.load_active_sessions().await.unwrap(), vec![session]);
}

#[tokio::test]
async fn workflow_save_rejects_a_stale_completion_after_cancellation() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let session = session();
    store.create_session(&session).await.unwrap();
    let run = WorkflowRunRecord {
        id: WorkflowRunId::try_new("run-1").unwrap(),
        revision: 0,
        session_id: session.id,
        turn_id: None,
        project: session.project,
        definition_id: WorkflowId::try_new("workflow").unwrap(),
        definition_version: WorkflowVersion::try_new("1.0.0").unwrap(),
        state: WorkflowRunState::Running,
        steps: [(
            WorkflowStepId::try_new("step").unwrap(),
            WorkflowStepRecord {
                state: WorkflowStepState::Running,
                attempt: 1,
            },
        )]
        .into(),
        created_at: UnixMillis::from_existing(1),
        updated_at: UnixMillis::from_existing(1),
    };
    let mut stale = store.save_run(&run, None).await.unwrap();
    assert_eq!(
        store.save_run(&run, None).await.unwrap_err().code,
        PersistenceFailureCode::Conflict
    );
    let mut cancelled = stale.clone();
    cancelled.state = WorkflowRunState::Cancelled;
    let committed = store
        .save_run(&cancelled, Some(cancelled.revision))
        .await
        .unwrap();
    assert_eq!(committed.revision, 1);
    stale.state = WorkflowRunState::Completed;
    assert_eq!(
        store
            .save_run(&stale, Some(stale.revision))
            .await
            .unwrap_err()
            .code,
        PersistenceFailureCode::Conflict
    );
    assert_eq!(store.load_run(&run.id).await.unwrap(), Some(committed));
}

#[tokio::test]
async fn concurrent_event_append_with_an_insert_failure_keeps_a_contiguous_durable_stream() {
    let directory = std::env::temp_dir().join(format!(
        "yss-harness-events-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let first = SqliteHarnessStore::connect(directory.clone())
        .await
        .unwrap();
    let second = SqliteHarnessStore::connect(directory.clone())
        .await
        .unwrap();
    let session = session();
    first.create_session(&session).await.unwrap();
    for _ in 0..9 {
        first
            .append_event(
                &session.id,
                None,
                session.created_at,
                HarnessEvent::SessionCreated,
            )
            .await
            .unwrap();
    }
    sqlx::query("CREATE TRIGGER fail_selected_event BEFORE INSERT ON assistant_event WHEN json_extract(NEW.payload_json, '$.event.type') = 'turn_failed' BEGIN SELECT RAISE(ABORT, 'injected append failure'); END")
        .execute(&first.pool).await.unwrap();
    let (failure, b, c) = tokio::join!(
        first.append_event(
            &session.id,
            None,
            session.created_at,
            HarnessEvent::TurnFailed
        ),
        second.append_event(
            &session.id,
            None,
            session.created_at,
            HarnessEvent::SessionCreated
        ),
        first.append_event(
            &session.id,
            None,
            session.created_at,
            HarnessEvent::SessionCreated
        ),
    );
    assert!(failure.is_err());
    let mut sequences = [b.unwrap().sequence, c.unwrap().sequence];
    sequences.sort();
    assert_eq!(sequences, [10, 11]);
    let persisted = second.load_events_after(&session.id, 0).await.unwrap();
    assert_eq!(
        persisted
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        (1..=11).collect::<Vec<_>>()
    );
    assert!(
        !persisted
            .iter()
            .any(|event| matches!(event.event, HarnessEvent::TurnFailed))
    );
    assert_eq!(second.latest_sequence(&session.id).await.unwrap(), 11);
    sqlx::query("DROP TRIGGER fail_selected_event")
        .execute(&first.pool)
        .await
        .unwrap();
    first.pool.close().await;
    second.pool.close().await;
    let reopened = SqliteHarnessStore::connect(directory.clone())
        .await
        .unwrap();
    assert_eq!(
        reopened
            .append_event(
                &session.id,
                None,
                session.created_at,
                HarnessEvent::SessionCreated
            )
            .await
            .unwrap()
            .sequence,
        12
    );
    reopened.pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}
