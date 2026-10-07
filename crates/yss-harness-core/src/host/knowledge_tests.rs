use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use yss_harness_contract::*;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

use crate::test_support::{
    FixedClock, InMemoryHarnessStore, RejectingCapabilityGateway, SequentialIds,
};
use crate::{HarnessError, HarnessHost, HarnessPorts, install_builtin_statistical_knowledge};

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
        tools: Arc<dyn ModelCapabilityExecutor>,
        _output: Arc<dyn AgentEventOutput>,
        _cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            assert!(
                request
                    .tools
                    .iter()
                    .any(|tool| tool.capability_id == CapabilityId::SearchKnowledge)
            );
            assert!(
                request
                    .tools
                    .iter()
                    .any(|tool| tool.capability_id == CapabilityId::ReadKnowledge)
            );
            match self.step.fetch_add(1, Ordering::Relaxed) {
                0 => {}
                1 => {
                    let search = tools
                        .execute(ModelCapabilityRequest {
                            request: (AutomationCapabilityRequest::SearchKnowledge(
                                SearchKnowledgeRequest {
                                    query: "regression diagnostics".into(),
                                    scopes: vec![],
                                    limit: 1,
                                },
                            ))
                            .into(),
                        })
                        .await
                        .map_err(driver_failure)?;
                    assert_public(&serde_json::to_value(&search.result).unwrap());
                    let AutomationCapabilityResult::KnowledgeSearch(result) = search.result else {
                        panic!("search result")
                    };
                    assert_eq!(result.matches.len(), 1);
                    let read = tools
                        .execute(ModelCapabilityRequest {
                            request: (AutomationCapabilityRequest::ReadKnowledge(
                                ReadKnowledgeRequest {
                                    reference: result.matches[0].reference.clone(),
                                },
                            ))
                            .into(),
                        })
                        .await
                        .map_err(driver_failure)?;
                    assert_public(&serde_json::to_value(&read.result).unwrap());
                    let AutomationCapabilityResult::KnowledgePassage(passage) = read.result else {
                        panic!("passage result")
                    };
                    assert!(passage.text.contains("residual checks"));
                }
                2 => {
                    // Reopening reconstructs the reference from durable native tool history.
                    let reference = request
                        .messages
                        .iter()
                        .find_map(|message| match message {
                            AgentMessage::ToolResult {
                                outcome: Ok(AutomationCapabilityResult::KnowledgePassage(passage)),
                                ..
                            } => Some(passage.reference.clone()),
                            _ => None,
                        })
                        .expect("persisted passage result");
                    let failure = tools
                        .execute(ModelCapabilityRequest {
                            request: (AutomationCapabilityRequest::ReadKnowledge(
                                ReadKnowledgeRequest { reference },
                            ))
                            .into(),
                        })
                        .await
                        .unwrap_err();
                    assert_eq!(failure.code, CapabilityFailureCode::ResultUnavailable);
                }
                _ => panic!("unexpected model call"),
            }
            Ok(AgentTurnResult {
                final_text: "Done.".into(),
            })
        })
    }
}

fn driver_failure(failure: CapabilityFailure) -> AgentDriverFailure {
    AgentDriverFailure::new(if failure.code == CapabilityFailureCode::Cancelled {
        AgentDriverFailureCode::Cancelled
    } else {
        AgentDriverFailureCode::InternalFailure
    })
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
        capability_gateway: Arc::new(RejectingCapabilityGateway),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: index,
        approvals: store,
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
    install_builtin_statistical_knowledge(store.clone(), UnixMillis::from_existing(1))
        .await
        .unwrap();
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
    install_builtin_statistical_knowledge(store.clone(), UnixMillis::from_existing(1))
        .await
        .unwrap();
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
        CapabilityFailureCode::Cancelled
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
