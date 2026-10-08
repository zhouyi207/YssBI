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
        model: None,
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

#[tokio::test]
async fn indexed_invocation_identity_cannot_be_replaced_when_finishing() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let session = session();
    store.create_session(&session).await.unwrap();
    let original = ToolInvocationRecord {
        id: ToolInvocationId::try_new("tool-1").unwrap(),
        idempotency_key: IdempotencyKey::try_new("key-1").unwrap(),
        session_id: session.id.clone(),
        turn_id: HarnessTurnId::try_new("turn-1").unwrap(),
        agent_run_id: None,
        workflow_run_id: None,
        workflow_step_id: None,
        project: session.project,
        capability_id: CapabilityId::InspectGraph,
        request: AutomationCapabilityRequest::InspectGraph(InspectGraphRequest::overview(
            "events/Main.yssbi-event",
        ))
        .into(),
        state: ToolInvocationState::Running,
        result: None,
        failure: None,
        started_at: UnixMillis::from_existing(1),
        deadline: UnixMillis::from_existing(10),
        finished_at: None,
    };
    store.begin(&original).await.unwrap();
    for change_session in [false, true] {
        let mut changed = original.clone();
        if change_session {
            changed.session_id = HarnessSessionId::try_new("another-session").unwrap();
        } else {
            changed.id = ToolInvocationId::try_new("another-invocation").unwrap();
        }
        changed.state = ToolInvocationState::Failed;
        assert_eq!(
            store.finish(&changed).await.unwrap_err().code,
            PersistenceFailureCode::NotFound
        );
        assert_eq!(
            store
                .load_invocation(&original.session_id, &original.id)
                .await
                .unwrap(),
            Some(original.clone())
        );
    }
    let mut finished = original.clone();
    finished.state = ToolInvocationState::Failed;
    store.finish(&finished).await.unwrap();
    assert_eq!(
        store
            .load_invocation(&original.session_id, &original.id)
            .await
            .unwrap(),
        Some(finished)
    );
}

#[tokio::test]
async fn indexed_turn_parent_cannot_change_during_an_update() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let session = session();
    store.create_session(&session).await.unwrap();
    let original = HarnessTurnRecord {
        id: HarnessTurnId::try_new("turn-1").unwrap(),
        session_id: session.id,
        state: HarnessTurnState::Running,
        user_message: "Inspect".into(),
        final_text: None,
        started_at: UnixMillis::from_existing(1),
        finished_at: None,
    };
    store.create_turn(&original).await.unwrap();
    let mut changed = original.clone();
    changed.session_id = HarnessSessionId::try_new("another-session").unwrap();
    changed.state = HarnessTurnState::Completed;
    assert_eq!(
        store.update_turn(&changed).await.unwrap_err().code,
        PersistenceFailureCode::NotFound
    );
    assert_eq!(
        store.load_turn(&original.id).await.unwrap(),
        Some(original.clone())
    );
    changed.session_id = original.session_id;
    store.update_turn(&changed).await.unwrap();
    assert_eq!(store.load_turn(&changed.id).await.unwrap(), Some(changed));
}

#[tokio::test]
async fn indexed_approval_consumption_matches_the_inserted_record() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let session = session();
    let grant = ApprovalGrantRecord {
        id: ApprovalGrantId::try_new("already-consumed").unwrap(),
        principal_id: session.principal_id,
        session_id: session.id,
        project: session.project,
        capability_id: CapabilityId::ApplyGraphEdit,
        request_fingerprint: SourceHash::try_new("fingerprint-1").unwrap(),
        issued_at: UnixMillis::from_existing(1),
        expires_at: UnixMillis::from_existing(10),
        consumed_at: Some(UnixMillis::from_existing(2)),
    };
    ApprovalStorePort::insert(&store, &grant).await.unwrap();
    let consumed: Option<i64> =
        sqlx::query_scalar("SELECT consumed_at FROM approval_grant WHERE id = ?")
            .bind(grant.id.as_str())
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(consumed, Some(2));
    assert!(
        !ApprovalStorePort::consume(&store, &grant.id, UnixMillis::from_existing(3))
            .await
            .unwrap()
    );
    assert_eq!(
        ApprovalStorePort::load(&store, &grant.id).await.unwrap(),
        Some(grant.clone())
    );
    let mut invalid = grant;
    invalid.id = ApprovalGrantId::try_new("unrepresentable-time").unwrap();
    invalid.consumed_at = Some(UnixMillis::from_existing(u64::MAX));
    assert_eq!(
        ApprovalStorePort::insert(&store, &invalid)
            .await
            .unwrap_err()
            .code,
        PersistenceFailureCode::InvalidRecord
    );
    assert!(
        ApprovalStorePort::load(&store, &invalid.id)
            .await
            .unwrap()
            .is_none()
    );
}
