use yss_harness_contract::*;

use super::*;
use yss_harness_contract::{
    ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort, AutomationCapabilityRequest,
    HarnessEvent, HarnessSessionState, IdempotencyKey, InspectGraphRequest, PrincipalId,
    ProjectSessionBinding, SourceHash, ToolInvocationId, ToolInvocationState, UnixMillis,
};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

#[tokio::test]
async fn connect_preserves_literal_percent_encoded_directory_names() {
    let directory = std::env::temp_dir().join(format!(
        "yss-harness-literal-path-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let literal = directory.join("profile%20literal");
    let decoded = directory.join("profile literal");
    // Both directories exist so decoding would silently select the wrong database.
    std::fs::create_dir_all(decoded.join("db")).unwrap();
    let store = SqliteHarnessStore::connect(literal.clone()).await.unwrap();
    let database = std::path::Path::new("db").join("statistical-harness.sqlite");
    let literal_exists = literal.join(&database).is_file();
    let decoded_exists = decoded.join(&database).exists();
    store.pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();

    assert!(
        literal_exists,
        "the database must use the literal directory name"
    );
    assert!(!decoded_exists, "a filesystem path must not be URL-decoded");
}

#[tokio::test]
async fn capability_workflow_definitions_round_trip_and_reject_version_replacement() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let definition: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "dataset_quality_review",
        "version": "1.0.0",
        "steps": [{
            "id": "inspect_schema",
            "dependsOn": [],
            "request": {
                "type": "inspect_dataset_schema",
                "payload": {"databaseId": "dataset-1"}
            }
        }]
    }))
    .unwrap();
    store.save_definition(&definition).await.unwrap();
    store.save_definition(&definition).await.unwrap();
    let mut conflicting = definition.clone();
    conflicting.steps[0].request =
        AutomationCapabilityRequest::InspectDatasetSchema(InspectDatasetSchemaRequest {
            database_id: "dataset-2".into(),
        });
    assert_eq!(
        store.save_definition(&conflicting).await.unwrap_err().code,
        PersistenceFailureCode::Conflict
    );
    assert_eq!(
        store
            .load_definition(&definition.id, &definition.version)
            .await
            .unwrap(),
        Some(definition)
    );
}

#[tokio::test]
async fn rejects_incompatible_schema_without_rewriting_records() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE assistant_session (id TEXT PRIMARY KEY NOT NULL, state TEXT NOT NULL, payload_json TEXT NOT NULL)")
            .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO assistant_session VALUES ('old', 'active', 'unchanged')")
        .execute(&pool)
        .await
        .unwrap();
    let store = SqliteHarnessStore {
        pool,
        path: None,
        knowledge_generation: Default::default(),
    };
    assert_eq!(
        store.ensure_schema().await.unwrap_err().code,
        PersistenceFailureCode::InvalidRecord
    );
    let payload: String =
        sqlx::query_scalar("SELECT payload_json FROM assistant_session WHERE id = 'old'")
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(payload, "unchanged");
    let tables: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table'")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(tables, 1);
}

#[tokio::test]
async fn current_schema_reopens_and_rejects_malformed_json() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    store.ensure_schema().await.unwrap();
    assert!(
        sqlx::query("INSERT INTO assistant_session VALUES ('bad', 'active', 'not json')")
            .execute(&store.pool)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO assistant_session VALUES ('valid', 'active', '{}')")
        .execute(&store.pool)
        .await
        .unwrap();
    store.ensure_schema().await.unwrap();
    let payload: String =
        sqlx::query_scalar("SELECT payload_json FROM assistant_session WHERE id = 'valid'")
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(payload, "{}");
}

#[tokio::test]
async fn sqlite_enforces_event_sequence_and_tool_idempotency() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let project = ProjectSessionBinding::new(
        ProjectInstanceId::from_existing("project-1".into()),
        ProjectSessionId::new("project-session-1"),
    );
    let session = HarnessSessionRecord {
        conversation: None,
        id: HarnessSessionId::try_new("session-1").unwrap(),
        principal_id: PrincipalId::try_new("user-1").unwrap(),
        project: project.clone(),
        state: HarnessSessionState::Active,
        created_at: UnixMillis::from_existing(10),
        updated_at: UnixMillis::from_existing(10),
    };
    store.create_session(&session).await.unwrap();
    let event = HarnessEventEnvelope {
        sequence: 1,
        session_id: session.id.clone(),
        turn_id: None,
        occurred_at: UnixMillis::from_existing(11),
        event: HarnessEvent::SessionCreated,
    };
    assert_eq!(
        store
            .append_event(
                &event.session_id,
                None,
                event.occurred_at,
                event.event.clone()
            )
            .await
            .unwrap(),
        event
    );
    let next = store
        .append_event(
            &event.session_id,
            None,
            event.occurred_at,
            event.event.clone(),
        )
        .await
        .unwrap();
    assert_eq!(next.sequence, 2);

    let invocation = ToolInvocationRecord {
        agent_run_id: None,
        id: ToolInvocationId::try_new("tool-1").unwrap(),
        idempotency_key: IdempotencyKey::try_new("idem-1").unwrap(),
        session_id: session.id.clone(),
        turn_id: yss_harness_contract::HarnessTurnId::try_new("turn-1").unwrap(),
        workflow_run_id: None,
        workflow_step_id: None,
        project,
        capability_id: yss_harness_contract::CapabilityId::InspectGraph,
        request: AutomationCapabilityRequest::InspectGraph(InspectGraphRequest::overview(
            "events/Main.yssbi-event".to_owned(),
        )),
        state: ToolInvocationState::Running,
        result: None,
        failure: None,
        started_at: UnixMillis::from_existing(12),
        deadline: UnixMillis::from_existing(42),
        finished_at: None,
    };
    assert!(matches!(
        store.begin(&invocation).await.unwrap(),
        ToolInvocationBegin::Started
    ));
    assert!(matches!(
        store.begin(&invocation).await.unwrap(),
        ToolInvocationBegin::Existing(existing) if *existing == invocation
    ));
    assert_eq!(
        store.load_running_invocations().await.unwrap(),
        std::slice::from_ref(&invocation)
    );
    let mut finished = invocation.clone();
    finished.state = ToolInvocationState::Failed;
    finished.finished_at = Some(UnixMillis::from_existing(43));
    finished.failure = Some(yss_harness_contract::CapabilityFailure::new(
        yss_harness_contract::CapabilityFailureCode::InternalFailure,
    ));
    store.finish(&finished).await.unwrap();
    assert!(store.load_running_invocations().await.unwrap().is_empty());
    let mut turn = HarnessTurnRecord {
        id: invocation.turn_id.clone(),
        session_id: session.id.clone(),
        state: HarnessTurnState::Running,
        user_message: "Profile".into(),
        final_text: None,
        started_at: UnixMillis::from_existing(12),
        finished_at: None,
    };
    store.create_turn(&turn).await.unwrap();
    assert_eq!(store.load_running_turns().await.unwrap(), [turn.clone()]);
    turn.state = HarnessTurnState::Failed;
    turn.finished_at = Some(UnixMillis::from_existing(43));
    store.update_turn(&turn).await.unwrap();
    assert!(store.load_running_turns().await.unwrap().is_empty());
    assert_eq!(store.load_turn(&turn.id).await.unwrap(), Some(turn.clone()));
    assert_eq!(
        store
            .load_invocation(&session.id, &invocation.id)
            .await
            .unwrap(),
        Some(finished)
    );
    assert!(
        store
            .load_invocation(
                &HarnessSessionId::try_new("another-session").unwrap(),
                &invocation.id
            )
            .await
            .unwrap()
            .is_none()
    );
    turn.state = HarnessTurnState::Completed;
    turn.final_text = Some("Completed".into());
    store.update_turn(&turn).await.unwrap();
    assert_eq!(store.load_turn(&turn.id).await.unwrap(), Some(turn));
    assert_eq!(store.latest_sequence(&session.id).await.unwrap(), 2);
    assert_eq!(
        store.load_session(&session.id).await.unwrap(),
        Some(session.clone())
    );

    let grant = ApprovalGrantRecord {
        id: ApprovalGrantId::try_new("approval-1").unwrap(),
        principal_id: session.principal_id.clone(),
        session_id: session.id,
        project: invocation.project.clone(),
        capability_id: yss_harness_contract::CapabilityId::ApplyGraphEdit,
        request_fingerprint: SourceHash::try_new("fingerprint-1").unwrap(),
        issued_at: UnixMillis::from_existing(13),
        expires_at: UnixMillis::from_existing(100),
        consumed_at: None,
    };
    ApprovalStorePort::insert(&store, &grant).await.unwrap();
    assert!(
        ApprovalStorePort::consume(&store, &grant.id, UnixMillis::from_existing(14))
            .await
            .unwrap()
    );
    assert!(
        !ApprovalStorePort::consume(&store, &grant.id, UnixMillis::from_existing(15))
            .await
            .unwrap()
    );
    assert_eq!(
        ApprovalStorePort::load(&store, &grant.id)
            .await
            .unwrap()
            .unwrap()
            .consumed_at,
        Some(UnixMillis::from_existing(14))
    );
}
