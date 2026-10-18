use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use yss_harness_contract::*;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

use crate::test_support::{FixedClock, InMemoryHarnessStore, NoteExtension, SequentialIds};
use crate::{HarnessError, HarnessHost, HarnessPorts};
use serde_json::{Value, json};

/// One public methods source whose passage mentions regression diagnostics.
async fn install_guidance(store: &InMemoryHarnessStore) {
    let source = KnowledgeSourceRecord {
        id: KnowledgeSourceId::try_new("methods").unwrap(),
        title: "Methods".into(),
        version: "1.0.0".into(),
        license: "Test notes".into(),
        source_hash: SourceHash::try_new("methods-v1").unwrap(),
        status: KnowledgeSourceStatus::Active,
        sensitivity: SensitivityClass::Public,
        project: None,
        origin: None,
        updated_at: UnixMillis::from_existing(1),
    };
    let document = KnowledgeDocumentRecord {
        id: KnowledgeDocumentId::try_new("ols-diagnostics").unwrap(),
        source_id: source.id.clone(),
        title: "Regression diagnostics".into(),
        body: "Regression reporting pairs estimates with residual checks and diagnostics.".into(),
        scopes: vec!["methods.regression".into()],
        tags: vec!["regression".into(), "diagnostics".into()],
        source_hash: source.source_hash.clone(),
        project: None,
        sensitivity: SensitivityClass::Public,
    };
    store.replace_source(&source, &[document]).await.unwrap();
}

#[derive(Default)]
struct ObservedIndex {
    builds: AtomicUsize,
    hold: bool,
    entered: tokio::sync::Notify,
}

impl KnowledgeIndexPort for ObservedIndex {
    fn build(
        &self,
        chunks: Vec<KnowledgeIndexChunk>,
    ) -> KnowledgeIndexFuture<'_, Arc<dyn KnowledgeIndexReaderPort>> {
        Box::pin(async move {
            self.builds.fetch_add(1, Ordering::Relaxed);
            self.entered.notify_one();
            if self.hold {
                std::future::pending::<()>().await;
            }
            yss_harness_tantivy::TantivyKnowledgeIndex
                .build(chunks)
                .await
        })
    }
}

#[derive(Default)]
struct KnowledgeDriver {
    step: AtomicUsize,
    requests: Mutex<Vec<AgentTurnRequest>>,
}

impl AgentDriverPort for KnowledgeDriver {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        tools: Arc<dyn AgentToolExecutor>,
        _output: Arc<dyn AgentEventOutput>,
        _cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            for name in [SEARCH_KNOWLEDGE_TOOL, READ_KNOWLEDGE_TOOL] {
                assert!(request.tools.iter().any(|tool| tool.name == name));
            }
            match self.step.fetch_add(1, Ordering::Relaxed) {
                0 => {}
                1 => {
                    let search = call(
                        tools.as_ref(),
                        SEARCH_KNOWLEDGE_TOOL,
                        json!({"query": "regression diagnostics", "limit": 1}),
                    )
                    .await?;
                    assert_public(&search);
                    let result: KnowledgeSearchResult = serde_json::from_value(search).unwrap();
                    assert_eq!(result.matches.len(), 1);
                    let read = call(
                        tools.as_ref(),
                        READ_KNOWLEDGE_TOOL,
                        json!({"reference": result.matches[0].reference}),
                    )
                    .await?;
                    assert_public(&read);
                    let passage: KnowledgePassage = serde_json::from_value(read).unwrap();
                    assert!(passage.text.contains("residual checks"));
                }
                2 => {
                    // Reopening reconstructs the reference from durable native tool history.
                    let reference = request
                        .messages
                        .iter()
                        .find_map(|message| match message {
                            AgentMessage::ToolResult { tool, output, .. }
                                if tool == READ_KNOWLEDGE_TOOL =>
                            {
                                Some(output["reference"].clone())
                            }
                            _ => None,
                        })
                        .expect("persisted passage result");
                    let failure = tools
                        .call(ToolCall {
                            name: READ_KNOWLEDGE_TOOL.into(),
                            arguments: json!({"reference": reference}),
                        })
                        .await;
                    assert_eq!(failure.output["state"], "failed");
                    assert_eq!(failure.output["failure"]["code"], "result_unavailable");
                }
                _ => panic!("unexpected model call"),
            }
            Ok(AgentTurnResult {
                final_text: "Done.".into(),
            })
        })
    }
}

/// A successful call's model output; a fatal outcome ends the scripted run like a driver would.
async fn call(
    tools: &dyn AgentToolExecutor,
    name: &str,
    arguments: Value,
) -> Result<Value, AgentDriverFailure> {
    let outcome = tools
        .call(ToolCall {
            name: name.into(),
            arguments,
        })
        .await;
    if let Some(code) = outcome.fatal {
        return Err(AgentDriverFailure::new(code));
    }
    assert_ne!(outcome.output["state"], "failed", "{}", outcome.output);
    Ok(outcome.output)
}

fn assert_public(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                assert!(!matches!(
                    name.as_str(),
                    "version" | "sourceHash" | "revision" | "generation" | "project" | "sessionId"
                ));
                assert_public(value);
            }
        }
        serde_json::Value::Array(values) => values.iter().for_each(assert_public),
        _ => {}
    }
}

fn ports(
    store: Arc<InMemoryHarnessStore>,
    driver: Arc<KnowledgeDriver>,
    index: Arc<ObservedIndex>,
) -> HarnessPorts {
    HarnessPorts {
        models: crate::test_support::fixed_model(driver),
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        extension: Arc::new(NoteExtension::default()),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: index,
        clock: Arc::new(FixedClock::new(1000)),
        ids: Arc::new(SequentialIds::default()),
    }
}

fn project() -> ProjectSessionBinding {
    ProjectSessionBinding::new(
        ProjectInstanceId::new(),
        ProjectSessionId::new("knowledge-session"),
    )
}

#[tokio::test]
async fn knowledge_is_requested_by_tools_and_replays_without_internal_fields() {
    let store = Arc::new(InMemoryHarnessStore::default());
    install_guidance(&store).await;
    let driver = Arc::new(KnowledgeDriver::default());
    let index = Arc::new(ObservedIndex::default());
    let ports = ports(store.clone(), driver.clone(), index.clone());
    let host = HarnessHost::new(ports.clone()).unwrap();
    let project = project();
    let session = host
        .create_session(PrincipalId::try_new("user").unwrap(), project.clone())
        .await
        .unwrap();
    host.submit_turn(
        &session.id,
        &project,
        "Discuss regression later.".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    assert_eq!(index.builds.load(Ordering::Relaxed), 0);
    assert!(store.tool_invocations().is_empty());
    assert!(
        !store
            .published_events()
            .iter()
            .any(|event| matches!(event.event, HarnessEvent::KnowledgeCited { .. }))
    );

    host.submit_turn(
        &session.id,
        &project,
        "Read the regression guidance.".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    assert_eq!(index.builds.load(Ordering::Relaxed), 1);
    let citation = store
        .published_events()
        .into_iter()
        .find_map(|event| match event.event {
            HarnessEvent::KnowledgeCited { citation } => Some(citation),
            _ => None,
        })
        .unwrap();
    assert!(
        host.inspect_citation(&session.id, &citation)
            .await
            .unwrap()
            .is_some()
    );
    for record in store.tool_invocations() {
        assert_eq!(record.state, ToolInvocationState::Succeeded);
        assert!(record.finished_at.is_some());
        assert_public(&serde_json::to_value(record.request).unwrap());
        assert_public(&serde_json::to_value(record.result).unwrap());
    }
    store
        .mark_source_deleted(&citation.source_id, UnixMillis::from_existing(2))
        .await
        .unwrap();
    drop(host);
    let reopened = HarnessHost::new(ports).unwrap();
    reopened
        .submit_turn(
            &session.id,
            &project,
            "Read that passage again.".into(),
            vec![],
            None,
            Default::default(),
        )
        .await
        .unwrap();
    assert!(
        reopened
            .inspect_citation(&session.id, &citation)
            .await
            .unwrap()
            .is_none()
    );
    let requests = driver.requests.lock().unwrap();
    assert_eq!(
        requests[2]
            .messages
            .iter()
            .filter(|message| matches!(message, AgentMessage::ToolCall { .. }))
            .count(),
        2
    );
    assert_eq!(
        requests[2]
            .messages
            .iter()
            .filter(|message| matches!(message, AgentMessage::ToolResult { .. }))
            .count(),
        2
    );
    assert_eq!(
        store
            .published_events()
            .iter()
            .filter(|event| matches!(event.event, HarnessEvent::KnowledgeCited { .. }))
            .count(),
        1
    );
    assert_eq!(
        store
            .tool_invocations()
            .iter()
            .filter(|record| record.state == ToolInvocationState::Failed)
            .count(),
        1
    );
}

#[tokio::test]
async fn cancelling_knowledge_search_closes_the_ledger_without_a_late_citation() {
    let store = Arc::new(InMemoryHarnessStore::default());
    install_guidance(&store).await;
    let driver = Arc::new(KnowledgeDriver {
        step: AtomicUsize::new(1),
        ..Default::default()
    });
    let index = Arc::new(ObservedIndex {
        hold: true,
        ..Default::default()
    });
    let host = Arc::new(HarnessHost::new(ports(store.clone(), driver, index.clone())).unwrap());
    let project = project();
    let session = host
        .create_session(PrincipalId::try_new("user").unwrap(), project.clone())
        .await
        .unwrap();
    let task_host = host.clone();
    let session_id = session.id.clone();
    let task = tokio::spawn(async move {
        task_host
            .submit_turn(
                &session_id,
                &project,
                "Find regression guidance.".into(),
                vec![],
                None,
                Default::default(),
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), index.entered.notified())
        .await
        .unwrap();
    assert!(host.cancel_turn(&session.id));
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(outcome, Err(HarnessError::Cancelled)));
    let records = store.tool_invocations();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].state, ToolInvocationState::Failed);
    assert_eq!(
        records[0].failure.as_ref().unwrap().code,
        CapabilityFailureCode::CANCELLED
    );
    assert!(records[0].finished_at.is_some());
    assert!(store.load_running_invocations().await.unwrap().is_empty());
    assert!(
        !store
            .published_events()
            .iter()
            .any(|event| matches!(event.event, HarnessEvent::KnowledgeCited { .. }))
    );
}
