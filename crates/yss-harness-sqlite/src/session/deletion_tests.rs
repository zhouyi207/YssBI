use crate::SqliteHarnessStore;
use yss_harness_contract::*;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

async fn populate(store: &SqliteHarnessStore, id: &str) -> HarnessSessionRecord {
    let now = UnixMillis::from_existing(100);
    let session = HarnessSessionRecord {
        id: HarnessSessionId::try_new(id).unwrap(),
        principal_id: PrincipalId::try_new("user").unwrap(),
        project: ProjectSessionBinding::new(
            ProjectInstanceId::from_existing("project".into()),
            ProjectSessionId::new("project-session"),
        ),
        conversation: Some(HarnessConversationMetadata {
            project_key: "project-key".into(),
            title: id.into(),
            last_opened_at: now,
            model: None,
        }),
        state: HarnessSessionState::Active,
        created_at: now,
        updated_at: now,
    };
    store.create_session(&session).await.unwrap();
    let turn = HarnessTurnRecord {
        id: HarnessTurnId::try_new(format!("turn-{id}")).unwrap(),
        session_id: session.id.clone(),
        state: HarnessTurnState::Completed,
        user_message: "Inspect graph".into(),
        final_text: Some("Finished".into()),
        started_at: now,
        finished_at: Some(now),
    };
    store.create_turn(&turn).await.unwrap();
    store
        .append_event(&session.id, None, now, HarnessEvent::SessionCreated)
        .await
        .unwrap();
    store
        .begin(&ToolInvocationRecord {
            id: ToolInvocationId::try_new(format!("tool-{id}")).unwrap(),
            idempotency_key: IdempotencyKey::try_new(format!("key-{id}")).unwrap(),
            agent_run_id: None,
            session_id: session.id.clone(),
            turn_id: turn.id,
            project: session.project.clone(),
            tool: "inspect_graph".into(),
            effect: ToolEffect::Inspect,
            arguments: serde_json::json!({"graph": "events/Main.yssbi-event"}),
            request: Some(serde_json::json!({"graph": "events/Main.yssbi-event"})),
            state: ToolInvocationState::Failed,
            result: None,
            failure: Some(CapabilityFailure::new(
                CapabilityFailureCode::INTERNAL_FAILURE,
            )),
            started_at: now,
            deadline: now,
            finished_at: Some(now),
        })
        .await
        .unwrap();
    session
}

async fn owned_records(store: &SqliteHarnessStore, id: &HarnessSessionId) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    for query in [
        "SELECT payload_json FROM assistant_session WHERE id = ? ORDER BY rowid",
        "SELECT payload_json FROM assistant_turn WHERE session_id = ? ORDER BY rowid",
        "SELECT payload_json FROM assistant_event WHERE session_id = ? ORDER BY rowid",
        "SELECT payload_json FROM tool_invocation WHERE session_id = ? ORDER BY rowid",
    ] {
        records.push(
            sqlx::query_scalar::<_, String>(query)
                .bind(id.as_str())
                .fetch_all(&store.pool)
                .await
                .unwrap(),
        );
    }
    records
}

#[tokio::test]
async fn deletion_is_atomic_and_keeps_other_sessions() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let deleted = populate(&store, "deleted").await;
    let kept = populate(&store, "kept").await;
    let before = owned_records(&store, &deleted.id).await;
    let other = owned_records(&store, &kept.id).await;
    assert!(before.iter().all(|records| records.len() == 1));
    sqlx::query("CREATE TRIGGER reject_deletion BEFORE DELETE ON assistant_session BEGIN SELECT RAISE(ABORT, 'injected deletion failure'); END")
        .execute(&store.pool).await.unwrap();
    assert!(store.delete_session(&deleted.id).await.is_err());
    assert_eq!(owned_records(&store, &deleted.id).await, before);
    assert_eq!(owned_records(&store, &kept.id).await, other);
    sqlx::query("DROP TRIGGER reject_deletion")
        .execute(&store.pool)
        .await
        .unwrap();

    store.delete_session(&deleted.id).await.unwrap();
    assert!(
        owned_records(&store, &deleted.id)
            .await
            .iter()
            .all(Vec::is_empty)
    );
    assert_eq!(owned_records(&store, &kept.id).await, other);
    assert_eq!(
        store
            .list_conversations(&kept.principal_id, "project-key")
            .await
            .unwrap(),
        [kept]
    );
    assert_eq!(
        store.delete_session(&deleted.id).await.unwrap_err().code,
        PersistenceFailureCode::NotFound
    );
}
