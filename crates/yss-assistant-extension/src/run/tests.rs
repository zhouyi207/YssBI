use crate::contract::*;
use crate::run::observations::ResourceObservations;
use crate::tests::support::{StoreSlot, TaskOutcome, TestRun, Tools, ports};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use yss_harness_core::HarnessHost;
use yss_harness_core::test_support::{InMemoryHarnessStore, SequentialIds};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

mod capabilities;
mod chart;
mod database;
mod document;
mod mind;
mod observations;
mod run_options;

fn document() -> ProjectResourceRef {
    ProjectResourceRef {
        kind: ProjectResourceKind::Doc,
        id: "docs/report.md".into(),
    }
}
fn version(revision: u64) -> ResourceVersion {
    ResourceVersion {
        revision,
        session_id: Some("document-session".into()),
    }
}
fn task(label: &str, worker: AgentRole, write: bool) -> model::AgentTaskInput {
    model::AgentTaskInput {
        worker,
        objective: format!("{label}: Check the specified report"),
        constraints: "Preserve the scientific intent".into(),
        completion_criteria: "Return evidence and limitations".into(),
        depends_on: vec![],
        scope: model::AgentTaskScopeInput {
            resources: vec![model::AgentResourceInput {
                resource: document(),
                operations: if write {
                    vec![
                        AgentResourceOperation::Inspect,
                        AgentResourceOperation::Save,
                    ]
                } else {
                    vec![AgentResourceOperation::Inspect]
                },
            }],
            ..Default::default()
        },
    }
}

async fn inspect_resource(capabilities: &Tools<'_>, resource: ProjectResourceRef) {
    capabilities
        .execute(AutomationCapabilityRequest::InspectResource(
            InspectResourceRequest { resource },
        ))
        .await
        .unwrap();
}

#[derive(Clone, Copy)]
enum Scenario {
    Parallel,
    Cancel,
    Stale,
    QueuedStale,
    Report(ReportDelivery),
    Long,
    Continue,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ReportDelivery {
    TextOnly,
    TextOnlyExistingDocument,
    Unsaved,
    Saved,
    FailedSave,
    EditedAfterSave,
}

async fn report_document_operations(capabilities: &Tools<'_>, delivery: ReportDelivery) {
    if matches!(
        delivery,
        ReportDelivery::TextOnly | ReportDelivery::TextOnlyExistingDocument
    ) {
        return;
    }
    capabilities
        .execute(model::CapabilityInput::CreateResource(
            model::CreateResourceInput::Doc {
                name: "Report".into(),
            },
        ))
        .await
        .unwrap();
    let edit = || {
        model::CapabilityInput::WriteDocument(model::WriteDocumentInput {
            document: model::DocumentResourceRef::new(document().id),
            markdown: "# Analysis report\n\nVerified findings and limitations.".into(),
        })
    };
    capabilities.execute(edit()).await.unwrap();
    if delivery == ReportDelivery::Unsaved {
        return;
    }
    let saved = capabilities
        .execute(model::CapabilityInput::SaveResource(
            model::ResourceTargetInput {
                resource: document(),
            },
        ))
        .await;
    if delivery == ReportDelivery::FailedSave {
        assert_eq!(
            saved.unwrap_err().code,
            CapabilityFailureCode::REVISION_CONFLICT
        );
        return;
    }
    assert!(matches!(
        saved.unwrap().result,
        AutomationCapabilityResult::ResourceManaged(_)
    ));
    if delivery == ReportDelivery::EditedAfterSave {
        capabilities.execute(edit()).await.unwrap();
    }
}

struct Driver {
    scenario: Scenario,
    worker_calls: AtomicUsize,
    previous_run: Mutex<Option<AgentRunId>>,
    requests: Mutex<Vec<AgentTurnRequest>>,
    barrier: tokio::sync::Barrier,
    ready: tokio::sync::Notify,
    proceed: tokio::sync::Notify,
    store: StoreSlot,
}
impl Driver {
    fn new(scenario: Scenario) -> Self {
        Self {
            store: StoreSlot::default(),
            scenario,
            worker_calls: AtomicUsize::new(0),
            previous_run: Mutex::new(None),
            requests: Mutex::new(vec![]),
            barrier: tokio::sync::Barrier::new(2),
            ready: tokio::sync::Notify::new(),
            proceed: tokio::sync::Notify::new(),
        }
    }
}
impl AgentDriverPort for Driver {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        executor: Arc<dyn AgentToolExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            let capabilities = Tools {
                executor: executor.as_ref(),
                store: self.store.get(),
            };
            self.requests.lock().unwrap().push(request.clone());
            if request.role != AgentRole::Manager.id() {
                self.worker_calls.fetch_add(1, Ordering::AcqRel);
                let failure = capabilities
                    .delegate(task("forbidden", AgentRole::Review, false))
                    .await
                    .unwrap_err();
                assert_eq!(failure.code, CapabilityFailureCode::INVALID_REQUEST);
                assert_eq!(failure.details["reason"], "workers_cannot_delegate");
                if matches!(self.scenario, Scenario::Parallel) {
                    self.barrier.wait().await;
                }
                if matches!(self.scenario, Scenario::Cancel) {
                    self.ready.notify_one();
                    cancellation.cancelled().await;
                    return Err(AgentDriverFailure::new(AgentDriverFailureCode::Cancelled));
                }
                if request.role == AgentRole::Review.id() {
                    assert!(
                        capabilities
                            .execute(AutomationCapabilityRequest::ManageResource(
                                ManageResourceRequest::Save {
                                    resource: document(),
                                    version: version(1)
                                }
                            ))
                            .await
                            .is_err()
                    );
                }
                if matches!(self.scenario, Scenario::Stale | Scenario::QueuedStale)
                    && request.role == AgentRole::Report.id()
                {
                    if matches!(self.scenario, Scenario::QueuedStale) {
                        self.ready.notify_one();
                        self.proceed.notified().await;
                    }
                    capabilities
                        .execute(AutomationCapabilityRequest::ManageResource(
                            ManageResourceRequest::Save {
                                resource: document(),
                                version: version(1),
                            },
                        ))
                        .await
                        .unwrap();
                }
                if matches!(self.scenario, Scenario::Continue) {
                    if self.worker_calls.load(Ordering::Acquire) == 1 {
                        output
                            .emit(AgentEvent::ContextCompacted {
                                summary: "Goal: save the report; preserve exact result references."
                                    .into(),
                            })
                            .await
                            .unwrap();
                        report_document_operations(&capabilities, ReportDelivery::Unsaved).await;
                        output
                            .emit(AgentEvent::ContextCompactionProgress {
                                completed_bytes: 10,
                                total_bytes: 20,
                                checkpoint: Some(ContextCompactionCheckpoint {
                                    processed_bytes: 10,
                                    prefix_hash: "worker-prefix".into(),
                                    summary: "Partial worker checkpoint".into(),
                                }),
                            })
                            .await
                            .unwrap();
                    } else {
                        assert!(
                            capabilities
                                .execute(AutomationCapabilityRequest::InspectResource(
                                    InspectResourceRequest {
                                        resource: ProjectResourceRef {
                                            kind: ProjectResourceKind::Doc,
                                            id: "docs/foreign.md".into()
                                        },
                                    }
                                ))
                                .await
                                .is_err()
                        );
                        assert!(request.messages.iter().any(|message| matches!(message,
                            AgentMessage::CompactionCheckpoint { checkpoint } if checkpoint.prefix_hash == "worker-prefix"
                        )));
                        assert!(request.messages.iter().any(|message| matches!(message, AgentMessage::Assistant { content } if content.contains("Continuation checkpoint"))));
                        assert!(request.messages.iter().any(|message| matches!(
                            message,
                            AgentMessage::ToolResult { output, .. } if output["type"] == "resource_edited"
                        )));
                        capabilities
                            .execute(AutomationCapabilityRequest::ManageResource(
                                ManageResourceRequest::Save {
                                    resource: document(),
                                    version: version(3),
                                },
                            ))
                            .await
                            .unwrap();
                    }
                }
                if let Scenario::Report(delivery) = self.scenario {
                    report_document_operations(&capabilities, delivery).await;
                }
                if matches!(self.scenario, Scenario::Long) {
                    for _ in 0..10 {
                        capabilities
                            .execute(AutomationCapabilityRequest::InspectResource(
                                InspectResourceRequest {
                                    resource: document(),
                                },
                            ))
                            .await
                            .unwrap();
                    }
                }
                output
                    .emit(AgentEvent::TextDelta {
                        delta: "worker-private-progress".into(),
                    })
                    .await
                    .unwrap();
                return Ok(AgentTurnResult {
                    final_text:
                        "Evidence checked.\n\nThe original result references were preserved.".into(),
                });
            }
            // On a follow-up, verify the parent only receives delegated outcomes and its own history.
            if request.messages.iter().any(|message| matches!(message, AgentMessage::User { content } if content == "Follow up")) {
                assert_eq!(request.messages.iter().filter(|message| matches!(message, AgentMessage::ToolResult { tool, .. } if tool == DELEGATE_TASK_TOOL)).count(), 2);
                assert!(!request.messages.iter().any(|message| matches!(message, AgentMessage::Assistant { content } if content.contains("worker-private-progress"))));
                return Ok(AgentTurnResult { final_text: "Follow-up answer".into() });
            }
            match self.scenario {
                Scenario::Long => {
                    for index in 0..26 {
                        let mut task = task(&format!("section-{index}"), AgentRole::Review, false);
                        task.objective = format!(
                            "Section {index}: {}",
                            "Review the authorized report evidence. ".repeat(2200)
                        );
                        let outcome = capabilities.delegate(task).await.unwrap();
                        assert_eq!(outcome.state, AgentRunState::Completed);
                        assert_eq!(outcome.evidence.len(), 12);
                    }
                }
                Scenario::Parallel => {
                    let first = task("report", AgentRole::Report, false);
                    let (a, b) = tokio::join!(
                        capabilities.delegate(first.clone()),
                        capabilities.delegate(task("review", AgentRole::Review, false))
                    );
                    let a = a.unwrap();
                    let b = b.unwrap();
                    assert_eq!(a.state, AgentRunState::Completed);
                    assert_eq!(b.state, AgentRunState::Completed);
                    assert!(!a.evidence.is_empty());
                    assert_eq!(capabilities.delegate(first.clone()).await.unwrap(), a);
                    let mut changed = first;
                    changed.worker = AgentRole::Review;
                    changed.scope.resources[0]
                        .operations
                        .push(AgentResourceOperation::Edit);
                    assert!(capabilities.delegate(changed).await.is_err());
                }
                Scenario::Cancel => {
                    let _ = capabilities
                        .delegate(task("review", AgentRole::Review, false))
                        .await;
                }
                Scenario::Stale => {
                    let review = capabilities
                        .delegate(task("review", AgentRole::Review, false))
                        .await
                        .unwrap();
                    let write = capabilities
                        .delegate(task("save", AgentRole::Report, true))
                        .await
                        .unwrap();
                    assert_eq!(write.invalidated_runs, vec![review.run_id.clone()]);
                    let mut dependent = task("dependent", AgentRole::Review, false);
                    dependent.depends_on.push(review.run_id);
                    assert!(capabilities.delegate(dependent).await.is_err());
                    let stale = capabilities
                        .delegate(task("old-version", AgentRole::Review, false))
                        .await
                        .unwrap_err();
                    assert_eq!(stale.code, CapabilityFailureCode::REVISION_CONFLICT);
                    inspect_resource(&capabilities, document()).await;
                    let refreshed = capabilities
                        .delegate(task("new-version", AgentRole::Review, false))
                        .await
                        .unwrap();
                    assert_eq!(refreshed.state, AgentRunState::Completed);
                }
                Scenario::QueuedStale => {
                    let review = capabilities
                        .delegate(task("review", AgentRole::Review, false))
                        .await
                        .unwrap();
                    let write = capabilities.delegate(task("save", AgentRole::Report, true));
                    tokio::pin!(write);
                    tokio::select! {
                        _ = self.ready.notified() => {}
                        _ = &mut write => panic!("writer must wait while holding the access gate"),
                    }
                    let mut dependent = task("dependent", AgentRole::Review, false);
                    dependent.scope.resources.clear();
                    dependent.depends_on.push(review.run_id.clone());
                    let dependent = capabilities.delegate(dependent);
                    tokio::pin!(dependent);
                    std::future::poll_fn(|context| {
                        assert!(
                            std::future::Future::poll(dependent.as_mut(), context).is_pending()
                        );
                        std::task::Poll::Ready(())
                    })
                    .await;
                    self.proceed.notify_one();
                    let write = write.await.unwrap();
                    assert_eq!(write.invalidated_runs, vec![review.run_id]);
                    let dependent = dependent.await.unwrap();
                    assert_eq!(dependent.state, AgentRunState::Blocked);
                    assert_eq!(self.worker_calls.load(Ordering::Acquire), 2);
                }
                Scenario::Large => {
                    return Ok(AgentTurnResult {
                        final_text: "Completed evidence.\n".repeat(70_000),
                    });
                }
                Scenario::Continue => {
                    let previous = self.previous_run.lock().unwrap().clone();
                    if let Some(run_id) = previous {
                        // A Manager's unrelated read cannot add that resource to the resumed grant.
                        inspect_resource(
                            &capabilities,
                            ProjectResourceRef {
                                kind: ProjectResourceKind::Doc,
                                id: "docs/foreign.md".into(),
                            },
                        )
                        .await;
                        let outcome = capabilities.followup(AgentFollowupInput {
                            run_id: run_id.clone(),
                            instruction: "Save the existing document; do not repeat its creation or edit.".into(),
                        }).await.unwrap();
                        assert_eq!(outcome.run_id, run_id);
                        assert_eq!(outcome.state, AgentRunState::Completed);
                        assert!(capabilities.completion_feedback().is_none());
                    } else {
                        let mut report = task("report", AgentRole::Report, true);
                        report.scope.resources.clear();
                        report.scope.creations.push(AgentCreationAccess {
                            specification: ResourceCreation::Doc {
                                name: "Report".into(),
                            },
                            operations: vec![
                                AgentResourceOperation::Inspect,
                                AgentResourceOperation::Edit,
                                AgentResourceOperation::Save,
                            ],
                        });
                        let outcome = capabilities.delegate(report).await.unwrap();
                        assert_eq!(outcome.state, AgentRunState::Blocked);
                        assert!(capabilities.completion_feedback().is_some());
                        *self.previous_run.lock().unwrap() = Some(outcome.run_id);
                    }
                }
                Scenario::Report(delivery) => {
                    let mut report = task("write-report", AgentRole::Report, true);
                    report.objective = "Create and save the analysis report Doc".into();
                    report.completion_criteria =
                        "A saved Doc backed by successful tool receipts".into();
                    if delivery != ReportDelivery::TextOnlyExistingDocument {
                        report.scope.resources.clear();
                        report.scope.creations.push(AgentCreationAccess {
                            specification: ResourceCreation::Doc {
                                name: "Report".into(),
                            },
                            operations: vec![
                                AgentResourceOperation::Inspect,
                                AgentResourceOperation::Edit,
                                AgentResourceOperation::Save,
                            ],
                        });
                    }
                    capabilities.delegate(report).await.unwrap();
                }
            }
            Ok(AgentTurnResult {
                final_text: "Manager answer".into(),
            })
        })
    }
}

struct Gateway(AtomicU64, bool);
impl CapabilityGatewayPort for Gateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _control: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            crate::authorize_agent_capability(context.agent().unwrap(), &request)?;
            match request {
                AutomationCapabilityRequest::InspectResource(request) => Ok(
                    AutomationCapabilityResult::ResourceInspection(ResourceInspection {
                        resource: request.resource,
                        name: "Report".into(),
                        version: version(self.0.load(Ordering::Acquire)),
                        dirty: false,
                        content: ResourceContent::Metadata,
                    }),
                ),
                AutomationCapabilityRequest::ManageResource(request) => {
                    match &request {
                        ManageResourceRequest::Create {
                            specification: ResourceCreation::Doc { .. },
                        } => {}
                        ManageResourceRequest::Save { version, .. } => {
                            if self.1 {
                                // Simulate a GUI edit between the worker's write and Save.
                                self.0.fetch_add(1, Ordering::AcqRel);
                            }
                            if version.revision != self.0.load(Ordering::Acquire) {
                                return Err(CapabilityFailure::new(
                                    CapabilityFailureCode::REVISION_CONFLICT,
                                ));
                            }
                        }
                        _ => panic!("unexpected resource operation"),
                    }
                    let revision = self.0.fetch_add(1, Ordering::AcqRel) + 1;
                    Ok(AutomationCapabilityResult::ResourceManaged(
                        ResourceMutationReceipt {
                            database_edit: None,
                            publication_revision: Some(revision),
                            changes: vec![ResourceChange {
                                resource: document(),
                                revision,
                                revision_kind: ResourceRevisionKind::Resource,
                                deleted: false,
                            }],
                            moves: vec![],
                            mind_edit: None,
                            document_edit: None,
                            resources: vec![ResourceMutationState {
                                resource: document(),
                                name: "Report".into(),
                                version: version(revision),
                                dirty: Some(false),
                                root_topic_id: None,
                            }],
                        },
                    ))
                }
                AutomationCapabilityRequest::EditResource(request) => {
                    assert_eq!(request.version.revision, self.0.load(Ordering::Acquire));
                    let revision = self.0.fetch_add(1, Ordering::AcqRel) + 1;
                    Ok(AutomationCapabilityResult::ResourceEdited(
                        ResourceMutationReceipt {
                            database_edit: None,
                            publication_revision: Some(revision),
                            changes: vec![ResourceChange {
                                resource: request.resource,
                                revision,
                                revision_kind: ResourceRevisionKind::Resource,
                                deleted: false,
                            }],
                            moves: vec![],
                            mind_edit: None,
                            document_edit: None,
                            resources: Vec::new(),
                        },
                    ))
                }
                _ => panic!("unexpected capability"),
            }
        })
    }
}

async fn setup(
    driver: Arc<Driver>,
) -> (
    Arc<HarnessHost>,
    Arc<InMemoryHarnessStore>,
    HarnessSessionRecord,
) {
    let fail_save = matches!(
        driver.scenario,
        Scenario::Report(ReportDelivery::FailedSave)
    );
    let setup = setup_models(
        yss_harness_core::test_support::fixed_model(driver.clone()),
        fail_save,
    )
    .await;
    driver.store.set(setup.1.clone());
    setup
}

async fn setup_models(
    models: Arc<dyn LanguageModelResolverPort>,
    fail_save: bool,
) -> (
    Arc<HarnessHost>,
    Arc<InMemoryHarnessStore>,
    HarnessSessionRecord,
) {
    let store = Arc::new(InMemoryHarnessStore::default());
    let host = Arc::new(
        HarnessHost::new(ports(
            store.clone(),
            models,
            Arc::new(Gateway(AtomicU64::new(1), fail_save)),
        ))
        .unwrap(),
    );
    let session = host
        .create_session(
            PrincipalId::try_new("user").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project".into()),
                ProjectSessionId::new("project-session"),
            ),
        )
        .await
        .unwrap();
    (host, store, session)
}

#[tokio::test]
async fn a_turn_freezes_its_model_for_all_workers_and_records_the_executed_identity() {
    struct SwitchingModels {
        calls: AtomicUsize,
        first: Arc<Driver>,
    }
    impl LanguageModelResolverPort for SwitchingModels {
        fn resolve<'a>(
            &'a self,
            _: Option<&'a LanguageModelSelection>,
        ) -> AgentFuture<'a, Result<ResolvedLanguageModel, AgentDriverFailure>> {
            Box::pin(async {
                let mut identity = yss_harness_core::test_support::model_identity();
                let driver: Arc<dyn AgentDriverPort> =
                    if self.calls.fetch_add(1, Ordering::AcqRel) == 0 {
                        self.first.clone()
                    } else {
                        identity.selection.model_id = "next-model".into();
                        identity.model_name = "Next model".into();
                        Arc::new(yss_harness_core::test_support::MockAgentDriver::new(
                            "Next reply",
                        ))
                    };
                Ok(ResolvedLanguageModel { identity, driver })
            })
        }
    }
    let models = Arc::new(SwitchingModels {
        calls: AtomicUsize::new(0),
        first: Arc::new(Driver::new(Scenario::Parallel)),
    });
    let (host, store, session) = setup_models(models.clone(), false).await;
    models.first.store.set(store);
    let session = host
        .session_access()
        .await
        .create_conversation(
            session.principal_id.clone(),
            "project-key".into(),
            session.project,
        )
        .await
        .unwrap();
    let next_model = LanguageModelSelection {
        provider_id: "test-provider".into(),
        model_id: "next-model".into(),
    };
    host.session_access()
        .await
        .select_model(
            &session.id,
            &session.principal_id,
            "project-key",
            next_model.clone(),
        )
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        host.submit_turn(
            &session.id,
            &session.project,
            "Inspect report".into(),
            vec![],
            Some(yss_harness_core::test_support::model_identity().selection),
            Default::default(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(models.calls.load(Ordering::Acquire), 1);
    assert_eq!(models.first.worker_calls.load(Ordering::Acquire), 2);
    let selected = host
        .session_access()
        .await
        .list_conversations(&session.principal_id, "project-key")
        .await
        .unwrap();
    assert_eq!(
        selected[0].conversation.as_ref().unwrap().model,
        Some(next_model)
    );
    assert_eq!(
        host.submit_turn(
            &session.id,
            &session.project,
            "Next turn".into(),
            vec![],
            None,
            Default::default(),
        )
        .await
        .unwrap()
        .final_text,
        "Next reply"
    );
    assert_eq!(models.calls.load(Ordering::Acquire), 2);
    let identities: Vec<_> = host
        .events_after(&session.id, 0)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|envelope| match envelope.event {
            HarnessEvent::TurnStarted { model, .. } => Some(model.selection.model_id),
            _ => None,
        })
        .collect();
    assert_eq!(identities, ["test-model", "next-model"]);
}

#[tokio::test]
async fn long_turns_can_finish_more_than_twenty_four_tasks_and_256_tools() {
    let driver = Arc::new(Driver::new(Scenario::Long));
    let (host, store, session) = setup(driver.clone()).await;
    host.submit_turn(
        &session.id,
        &session.project,
        "Review all sections".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    assert_eq!(driver.worker_calls.load(Ordering::Acquire), 26);
    let outcomes = host
        .events_after(&session.id, 0)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.event {
            HarnessEvent::AgentRunFinished { outcome } => Some(outcome),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(outcomes.len(), 27);
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.state == AgentRunState::Completed)
    );
    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome.evidence.len())
            .sum::<usize>(),
        313
    );
    let rejected: Vec<_> = store
        .tool_invocations()
        .into_iter()
        .filter(|record| record.state == ToolInvocationState::Failed)
        .collect();
    assert_eq!(rejected.len(), 26);
    assert!(
        rejected
            .iter()
            .all(|record| record.tool == CapabilityId::SaveResource.as_str()
                && record.request.is_none()
                && record.failure.as_ref().is_some_and(|failure| {
                    failure.code == CapabilityFailureCode::INVALID_REQUEST
                        && failure.details.get("capabilityId").map(String::as_str)
                            == Some(CapabilityId::SaveResource.as_str())
                }))
    );
}

#[tokio::test]
async fn report_completion_requires_a_successful_save_after_the_final_document_edit() {
    for delivery in [
        ReportDelivery::TextOnly,
        ReportDelivery::TextOnlyExistingDocument,
        ReportDelivery::Unsaved,
        ReportDelivery::Saved,
        ReportDelivery::FailedSave,
        ReportDelivery::EditedAfterSave,
    ] {
        let driver = Arc::new(Driver::new(Scenario::Report(delivery)));
        let (host, store, session) = setup(driver).await;
        host.submit_turn(
            &session.id,
            &session.project,
            "Analyze the data and output an analysis report".into(),
            vec![],
            None,
            Default::default(),
        )
        .await
        .unwrap();
        let outcome = store
            .published_events()
            .into_iter()
            .find_map(|event| match event.event {
                HarnessEvent::AgentRunFinished { outcome }
                    if outcome.role == AgentRole::Report.id() =>
                {
                    Some(TaskOutcome::from(*outcome))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(
            outcome.state,
            if delivery == ReportDelivery::Saved {
                AgentRunState::Completed
            } else {
                AgentRunState::Blocked
            },
            "{delivery:?}"
        );
        let report = outcome.report.unwrap();
        assert_eq!(
            report.summary,
            "Evidence checked.\n\nThe original result references were preserved."
        );
        if delivery == ReportDelivery::Saved {
            assert!(report.blocked_reason.is_none());
            assert!(!outcome.artifacts.is_empty());
            assert!(!outcome.evidence.is_empty());
        } else {
            assert_eq!(
                report.blocked_reason.as_deref(),
                Some(if delivery == ReportDelivery::FailedSave {
                    "input_changed"
                } else {
                    "report_resource_not_saved"
                })
            );
        }
    }
}

#[tokio::test]
async fn delegates_parallel_readers_once_and_replays_only_parent_conversation() {
    let driver = Arc::new(Driver::new(Scenario::Parallel));
    let (host, store, session) = setup(driver.clone()).await;
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        host.submit_turn(
            &session.id,
            &session.project,
            "Inspect report".into(),
            vec![],
            None,
            Default::default(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(driver.worker_calls.load(Ordering::Acquire), 2);
    let events = host.events_after(&session.id, 0).await.unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        (1..=events.len() as u64).collect::<Vec<_>>()
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, HarnessEvent::TurnStarted { .. }))
            .count(),
        1
    );
    for event in &events {
        if let HarnessEvent::AgentRunOutput {
            run_id,
            event: AgentEvent::ToolInvocationStarted { invocation_id, .. },
        } = &event.event
        {
            assert_eq!(
                store
                    .load_invocation(&session.id, invocation_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .agent_run_id
                    .as_ref(),
                Some(run_id)
            );
        }
    }
    assert_eq!(store.published_events(), events);
    host.submit_turn(
        &session.id,
        &session.project,
        "Follow up".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn parent_cancellation_finishes_worker_and_parent_without_extra_user_turn() {
    let driver = Arc::new(Driver::new(Scenario::Cancel));
    let (host, _, session) = setup(driver.clone()).await;
    let host_task = host.clone();
    let session_id = session.id.clone();
    let project = session.project.clone();
    let run = tokio::spawn(async move {
        host_task
            .submit_turn(
                &session_id,
                &project,
                "Review".into(),
                vec![],
                None,
                Default::default(),
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), driver.ready.notified())
        .await
        .unwrap();
    assert!(host.cancel_turn(&session.id));
    assert!(run.await.unwrap().is_err());
    let events = host.events_after(&session.id, 0).await.unwrap();
    let completed = events
        .iter()
        .filter_map(|event| match &event.event {
            HarnessEvent::AgentRunFinished { outcome } => Some(outcome.state),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        completed,
        vec![AgentRunState::Cancelled, AgentRunState::Cancelled]
    );
}

#[tokio::test]
async fn changed_inputs_invalidate_dependents_and_block_stale_versions() {
    let driver = Arc::new(Driver::new(Scenario::Stale));
    let (host, _, session) = setup(driver.clone()).await;
    host.submit_turn(
        &session.id,
        &session.project,
        "Revise the report".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    assert_eq!(driver.worker_calls.load(Ordering::Acquire), 3);
    assert!(
        host.events_after(&session.id, 0)
            .await
            .unwrap()
            .iter()
            .any(|event| matches!(event.event, HarnessEvent::AgentRunInvalidated { .. }))
    );
}

#[tokio::test]
async fn queued_dependencies_are_rechecked_before_worker_execution() {
    let driver = Arc::new(Driver::new(Scenario::QueuedStale));
    let (host, _, session) = setup(driver.clone()).await;
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        host.submit_turn(
            &session.id,
            &session.project,
            "Review after the pending revision".into(),
            vec![],
            None,
            Default::default(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(driver.worker_calls.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn recovery_preserves_committed_worker_receipts_without_repeating_the_task() {
    let driver = Arc::new(Driver::new(Scenario::Stale));
    let (host, store, session) = setup(driver.clone()).await;
    let turn_id = HarnessTurnId::try_new("interrupted-turn").unwrap();
    store
        .create_turn(&HarnessTurnRecord {
            id: turn_id.clone(),
            session_id: session.id.clone(),
            // A failure to persist the worker terminal event can precede a parent failure.
            state: HarnessTurnState::Failed,
            user_message: "Save report".into(),
            final_text: None,
            started_at: UnixMillis::from_existing(1000),
            finished_at: None,
        })
        .await
        .unwrap();
    let manager_id = AgentRunId::try_new("abandoned-manager").unwrap();
    let worker_id = AgentRunId::try_new("abandoned-worker").unwrap();
    let invocation_id = ToolInvocationId::try_new("committed-save").unwrap();
    let control_id = ToolInvocationId::try_new("abandoned-delegation").unwrap();
    let mut record = ToolInvocationRecord {
        id: invocation_id.clone(),
        idempotency_key: IdempotencyKey::try_new("save-key").unwrap(),
        session_id: session.id.clone(),
        turn_id: turn_id.clone(),
        agent_run_id: Some(worker_id.clone()),
        workflow_run_id: None,
        workflow_step_id: None,
        project: session.project.clone(),
        tool: CapabilityId::SaveResource.as_str().into(),
        effect: ToolEffect::Mutate,
        arguments: serde_json::json!({"resource": document()}),
        request: Some(
            encode_request(&AutomationCapabilityRequest::ManageResource(
                ManageResourceRequest::Save {
                    resource: document(),
                    version: version(1),
                },
            ))
            .unwrap(),
        ),
        state: ToolInvocationState::Running,
        result: None,
        failure: None,
        started_at: UnixMillis::from_existing(1000),
        deadline: UnixMillis::from_existing(2000),
        finished_at: None,
    };
    store.begin(&record).await.unwrap();
    record.state = ToolInvocationState::Succeeded;
    record.finished_at = Some(UnixMillis::from_existing(1100));
    record.result = Some(
        encode_result(&AutomationCapabilityResult::ResourceManaged(
            ResourceMutationReceipt {
                database_edit: None,
                publication_revision: Some(2),
                changes: vec![ResourceChange {
                    resource: document(),
                    revision: 2,
                    revision_kind: ResourceRevisionKind::Resource,
                    deleted: false,
                }],
                moves: vec![],
                mind_edit: None,
                document_edit: None,
                resources: Vec::new(),
            },
        ))
        .unwrap(),
    );
    store.finish(&record).await.unwrap();
    let events = [
        HarnessEvent::TurnStarted {
            resources: vec![],
            model: yss_harness_core::test_support::model_identity(),
            user_message: "Save report".into(),
        },
        HarnessEvent::AgentRunStarted {
            run_id: manager_id.clone(),
            parent_run_id: None,
            role: AgentRole::Manager.id(),
            task: None,
        },
        HarnessEvent::AgentRunStarted {
            run_id: worker_id.clone(),
            parent_run_id: Some(manager_id.clone()),
            role: AgentRole::Report.id(),
            task: Some(Box::new(AgentTask {
                key: "save".into(),
                worker: AgentRole::Report.id(),
                objective: "Save the report".into(),
                constraints: String::new(),
                completion_criteria: "Saved Doc".into(),
                depends_on: vec![],
                scope: serde_json::to_value(AgentTaskScope {
                    resources: vec![AgentResourceAccess {
                        resource: document(),
                        version: Some(version(1)),
                        operations: vec![
                            AgentResourceOperation::Inspect,
                            AgentResourceOperation::Save,
                        ],
                    }],
                    ..Default::default()
                })
                .unwrap(),
            })),
        },
        HarnessEvent::AgentRunOutput {
            run_id: worker_id.clone(),
            event: AgentEvent::ToolInvocationStarted {
                invocation_id: invocation_id.clone(),
                tool: CapabilityId::SaveResource.as_str().into(),
            },
        },
        HarnessEvent::TurnFailed,
        HarnessEvent::AgentRunOutput {
            run_id: manager_id,
            event: AgentEvent::ControlToolStarted {
                invocation_id: control_id.clone(),
                tool: DELEGATE_TASK_TOOL.into(),
            },
        },
    ];
    for event in events {
        store
            .append_event(
                &session.id,
                Some(&turn_id),
                UnixMillis::from_existing(1100),
                event,
            )
            .await
            .unwrap();
    }
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 1);
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 0);
    assert_eq!(driver.worker_calls.load(Ordering::Acquire), 0);
    let events = host.events_after(&session.id, 0).await.unwrap();
    assert_eq!(events.iter().filter(|envelope| matches!(&envelope.event,
        HarnessEvent::AgentRunOutput { event: AgentEvent::ControlToolFinished { invocation_id, failure_code: Some(code), .. }, .. }
        if invocation_id == &control_id && code == &CapabilityFailureCode::OUTCOME_UNKNOWN
    )).count(), 1);
    let outcomes = events
        .iter()
        .filter_map(|event| match &event.event {
            HarnessEvent::AgentRunFinished { outcome } => {
                Some(TaskOutcome::from((**outcome).clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(outcomes[0].run_id, worker_id);
    assert_eq!(outcomes[0].state, AgentRunState::Interrupted);
    assert_eq!(outcomes[0].artifacts[0].revision, 2);
    assert_eq!(outcomes[0].evidence, vec![invocation_id]);
    assert_eq!(outcomes[1].role, AgentRole::Manager);
    assert!(outcomes[1].report.is_none());
    // A failed control handoff can outlive an already-terminal parent run.
    store
        .append_event(
            &session.id,
            Some(&turn_id),
            UnixMillis::from_existing(1200),
            HarnessEvent::Agent(AgentEvent::ControlToolStarted {
                invocation_id: ToolInvocationId::try_new("orphan-control").unwrap(),
                tool: FOLLOWUP_TASK_TOOL.into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 1);
    assert_eq!(host.recover_interrupted_turns().await.unwrap(), 0);
    assert_eq!(driver.worker_calls.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn followup_restores_a_worker_across_turns_without_repeating_committed_writes() {
    let driver = Arc::new(Driver::new(Scenario::Continue));
    let (host, store, session) = setup(driver.clone()).await;
    for instruction in ["Write the report", "Continue"] {
        host.submit_turn(
            &session.id,
            &session.project,
            instruction.into(),
            vec![],
            None,
            Default::default(),
        )
        .await
        .unwrap();
    }
    let records = store.tool_invocations();
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record
                    .request
                    .as_ref()
                    .map(|request| decode_request(request).unwrap()),
                Some(AutomationCapabilityRequest::ManageResource(
                    ManageResourceRequest::Create { .. }
                ))
            ))
            .count(),
        1
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record
                    .request
                    .as_ref()
                    .map(|request| decode_request(request).unwrap()),
                Some(AutomationCapabilityRequest::EditResource(_))
            ))
            .count(),
        1
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record
                    .request
                    .as_ref()
                    .map(|request| decode_request(request).unwrap()),
                Some(AutomationCapabilityRequest::ManageResource(
                    ManageResourceRequest::Save { .. }
                ))
            ))
            .count(),
        1
    );
    let events = host.events_after(&session.id, 0).await.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, HarnessEvent::AgentRunResumed { .. }))
            .count(),
        1
    );
    assert!(events.iter().any(|event| matches!(&event.event, HarnessEvent::AgentRunFinished { outcome } if outcome.role == AgentRole::Manager.id() && outcome.state == AgentRunState::Blocked)));
    // Every recorded run, including the resumed one, projects into Assistant events.
    for event in &events {
        AssistantEvent::try_from(event).unwrap();
    }
}

#[tokio::test]
async fn host_persists_complete_replies_above_one_mebibyte() {
    let (host, store, session) = setup(Arc::new(Driver::new(Scenario::Large))).await;
    let result = host
        .submit_turn(
            &session.id,
            &session.project,
            "Read the complete report".into(),
            vec![],
            None,
            Default::default(),
        )
        .await
        .unwrap();
    assert!(result.final_text.len() > 1024 * 1024);
    assert!(store.published_events().iter().any(|event| matches!(&event.event, HarnessEvent::TurnCompleted { final_text } if final_text == &result.final_text)));
}
