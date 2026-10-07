use super::*;

#[tokio::test]
async fn resumed_workers_use_current_receipts_but_cannot_reuse_them_after_another_rebind() {
    #[derive(Default)]
    struct ResumeDriver {
        turns: AtomicUsize,
        run: Mutex<Option<AgentRunId>>,
    }
    impl AgentDriverPort for ResumeDriver {
        fn run_turn<'a>(
            &'a self,
            request: AgentTurnRequest,
            capabilities: Arc<dyn ModelCapabilityExecutor>,
            _: Arc<dyn AgentEventOutput>,
            _: CancellationToken,
        ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
            Box::pin(async move {
                if request.role == AgentRole::Manager {
                    let turn = self.turns.fetch_add(1, Ordering::Relaxed);
                    let previous = self.run.lock().unwrap().clone();
                    if let Some(run_id) = previous {
                        let followup = model::AgentFollowupInput {
                            run_id,
                            instruction: "Save the verified document again".into(),
                        };
                        if matches!(turn, 1 | 3) {
                            let failure =
                                capabilities.followup(followup.clone()).await.unwrap_err();
                            assert_eq!(model::failure(&failure)["code"], "resource_read_required");
                            inspect_resource(capabilities.as_ref(), document()).await;
                        }
                        // On turn 2, the current worker receipt is sufficient despite older history.
                        let outcome = capabilities.followup(followup).await.unwrap();
                        assert_eq!(outcome.state, AgentRunState::Completed);
                    } else {
                        let outcome = capabilities
                            .delegate(task("Save", AgentRole::Report, true))
                            .await
                            .unwrap();
                        assert_eq!(outcome.state, AgentRunState::Completed);
                        *self.run.lock().unwrap() = Some(outcome.run_id);
                    }
                } else {
                    capabilities
                        .execute(ModelCapabilityRequest {
                            request: model::CapabilityInput::SaveResource(
                                model::ResourceTargetInput {
                                    resource: document(),
                                },
                            ),
                        })
                        .await
                        .unwrap();
                }
                Ok(AgentTurnResult {
                    final_text: "Saved".into(),
                })
            })
        }
    }
    let driver = Arc::new(ResumeDriver::default());
    let (host, store, mut session) =
        setup_models(crate::test_support::fixed_model(driver), false).await;
    for turn in 0..4 {
        if matches!(turn, 1 | 3) {
            session.project = ProjectSessionBinding::new(
                ProjectInstanceId::new(),
                ProjectSessionId::new(format!("reopened-{turn}")),
            );
            store.update_session(&session).await.unwrap();
        }
        host.submit_turn(
            &session.id,
            &session.project,
            "Save".into(),
            vec![],
            None,
            Default::default(),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        store
            .tool_invocations()
            .iter()
            .filter(|record| record.capability_id == CapabilityId::SaveResource)
            .count(),
        4
    );
}

#[tokio::test]
async fn project_rebind_warns_before_delegation_and_requires_fresh_reads() {
    #[derive(Default)]
    struct ReboundDriver {
        turns: AtomicUsize,
        workers: AtomicUsize,
    }
    impl AgentDriverPort for ReboundDriver {
        fn run_turn<'a>(
            &'a self,
            request: AgentTurnRequest,
            capabilities: Arc<dyn ModelCapabilityExecutor>,
            _: Arc<dyn AgentEventOutput>,
            _: CancellationToken,
        ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
            Box::pin(async move {
                if request.role != AgentRole::Manager {
                    self.workers.fetch_add(1, Ordering::Relaxed);
                    return Ok(AgentTurnResult {
                        final_text: "Checked".into(),
                    });
                }
                let turn = self.turns.fetch_add(1, Ordering::Relaxed);
                let notice = request.messages.iter().find_map(|message| {
                    let AgentMessage::System { content } = message else {
                        return None;
                    };
                    let value: serde_json::Value = serde_json::from_str(content).ok()?;
                    (value["reason"] == "resource_read_required").then_some(value)
                });
                if turn == 1 {
                    let notice = notice.expect(
                        "rebound history must identify unverified resources before sampling",
                    );
                    assert_eq!(notice["resourceIds"], serde_json::json!([document().id]));
                    let encoded = notice.to_string();
                    for private in ["revision", "session", "hash"] {
                        assert!(!encoded.contains(private), "{encoded}");
                    }
                    // Ignoring the notice still cannot silently admit the historical basis.
                    let failure = capabilities
                        .delegate(task("Review", AgentRole::Review, false))
                        .await
                        .unwrap_err();
                    assert_eq!(model::failure(&failure)["code"], "resource_read_required");
                    assert_eq!(self.workers.load(Ordering::Relaxed), 0);
                } else {
                    assert!(notice.is_none());
                }
                if turn < 2 {
                    inspect_resource(capabilities.as_ref(), document()).await;
                }
                if turn == 1 {
                    let outcome = capabilities
                        .delegate(task("Review", AgentRole::Review, false))
                        .await
                        .unwrap();
                    assert_eq!(outcome.state, AgentRunState::Completed);
                }
                Ok(AgentTurnResult {
                    final_text: "Done".into(),
                })
            })
        }
    }
    let driver = Arc::new(ReboundDriver::default());
    let (host, store, mut session) =
        setup_models(crate::test_support::fixed_model(driver.clone()), false).await;
    host.submit_turn(
        &session.id,
        &session.project,
        "Read".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    session.project =
        ProjectSessionBinding::new(ProjectInstanceId::new(), ProjectSessionId::new("reopened"));
    store.update_session(&session).await.unwrap();
    for _ in 0..2 {
        host.submit_turn(
            &session.id,
            &session.project,
            "Continue".into(),
            vec![],
            None,
            Default::default(),
        )
        .await
        .unwrap();
    }
    assert_eq!(driver.workers.load(Ordering::Relaxed), 1);
}

#[test]
fn dataset_reads_bind_owner_resource_versions_and_reject_a_changed_runtime() {
    let resource = ProjectResourceRef {
        kind: ProjectResourceKind::Database,
        id: "survey".into(),
    };
    let schema = |runtime_revision| DatasetSchemaInspection {
        database_id: resource.id.clone(),
        runtime_revision,
        schema_revision: 7,
        columns: vec![],
    };
    let inspection = |runtime, revision| ResourceInspection {
        resource: resource.clone(),
        name: "Survey".into(),
        version: ResourceVersion {
            revision,
            session_id: None,
        },
        dirty: false,
        content: ResourceContent::DatabaseMetadata {
            runtime_revision: runtime,
            schema_revision: 7,
        },
    };
    let mut observations = ResourceObservations::default();
    observations.record(&AutomationCapabilityResult::DatasetSchemaInspection(
        schema(12),
    ));
    assert!(observations.version(&resource, false).unwrap().is_none());
    assert!(observations.needs_database_binding(&resource));
    assert_eq!(
        observations.bind(&inspection(12, 800)).unwrap().revision,
        800
    );
    observations.record(&AutomationCapabilityResult::DatasetSchemaInspection(
        schema(13),
    ));
    assert_eq!(
        observations.bind(&inspection(14, 802)).unwrap_err().code,
        CapabilityFailureCode::RevisionConflict
    );
    assert!(observations.version(&resource, false).unwrap().is_none());
    observations.record(&AutomationCapabilityResult::ResourceInspection(inspection(
        14, 802,
    )));
    assert_eq!(
        observations
            .version(&resource, false)
            .unwrap()
            .unwrap()
            .revision,
        802
    );
    observations.invalidate(&[ResourceChange {
        resource: resource.clone(),
        revision: 803,
        revision_kind: ResourceRevisionKind::Resource,
        deleted: false,
    }]);
    let failure = observations.version(&resource, false).unwrap_err();
    assert_eq!(model::failure(&failure)["code"], "resource_changed");
}

#[derive(Default)]
struct ObservedDriver {
    turns: AtomicUsize,
    writes: AtomicUsize,
}

impl AgentDriverPort for ObservedDriver {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        output: Arc<dyn AgentEventOutput>,
        _cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            if request.role == AgentRole::Report {
                let input = request
                    .messages
                    .iter()
                    .rev()
                    .find_map(|message| {
                        let AgentMessage::User { content } = message else {
                            return None;
                        };
                        serde_json::from_str::<serde_json::Value>(content).ok()
                    })
                    .unwrap();
                assert!(input["task"].get("key").is_none());
                assert!(
                    input["task"]["scope"]["resources"][0]
                        .get("version")
                        .is_none()
                );
                capabilities
                    .execute(ModelCapabilityRequest {
                        request: (AutomationCapabilityRequest::ManageResource(
                            ManageResourceRequest::Save {
                                resource: document(),
                                version: version(2),
                            },
                        ))
                        .into(),
                    })
                    .await
                    .unwrap();
                self.writes.fetch_add(1, Ordering::Relaxed);
            } else if self.turns.fetch_add(1, Ordering::Relaxed) == 0 {
                inspect_resource(capabilities.as_ref(), document()).await;
                output
                    .emit(AgentEvent::ContextCompacted {
                        summary: "The report was inspected; save it when the user continues."
                            .into(),
                    })
                    .await
                    .unwrap();
            } else {
                // The raw receipt was compacted away, but its version must still bind delegation.
                assert!(
                    !request
                        .messages
                        .iter()
                        .any(|message| matches!(message, AgentMessage::ToolResult { .. }))
                );
                let specification = task("Save inspected report", AgentRole::Report, true);
                let blocked = capabilities.delegate(specification.clone()).await.unwrap();
                assert_eq!(blocked.state, AgentRunState::Blocked);
                assert_eq!(self.writes.load(Ordering::Relaxed), 0);
                assert_eq!(
                    capabilities.delegate(specification).await.unwrap().run_id,
                    blocked.run_id
                );

                inspect_resource(capabilities.as_ref(), document()).await;
                let resumed = capabilities
                    .followup(model::AgentFollowupInput {
                        run_id: blocked.run_id.clone(),
                        instruction: "Save the newly inspected report.".into(),
                    })
                    .await
                    .unwrap();
                assert_eq!(resumed.run_id, blocked.run_id);
                assert_eq!(resumed.state, AgentRunState::Completed);
                assert_eq!(self.writes.load(Ordering::Relaxed), 1);
            }
            Ok(AgentTurnResult {
                final_text: "Done".into(),
            })
        })
    }
}

#[tokio::test]
async fn compacted_reads_survive_reopen_and_stale_delegation_cannot_write_until_reinspected() {
    let driver = Arc::new(ObservedDriver::default());
    let store = Arc::new(InMemoryHarnessStore::default());
    let gateway = Arc::new(Gateway(AtomicU64::new(1), false));
    let ports = HarnessPorts {
        resources: Arc::new(crate::test_support::FixtureResourceResolver),
        models: crate::test_support::fixed_model(driver.clone()),
        capability_gateway: gateway.clone(),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store.clone(),
        clock: Arc::new(FixedClock::new(1000)),
        ids: Arc::new(SequentialIds::default()),
    };
    let host = HarnessHost::new(ports.clone()).unwrap();
    let session = host
        .create_session(
            PrincipalId::try_new("user").unwrap(),
            ProjectSessionBinding::new(ProjectInstanceId::new(), ProjectSessionId::new("project")),
        )
        .await
        .unwrap();
    host.submit_turn(
        &session.id,
        &session.project,
        "Read the report".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    drop(host);
    // A GUI edit after the captured read must not become an implicit new write baseline.
    gateway.0.store(2, Ordering::Release);
    let host = HarnessHost::new(ports).unwrap();
    host.submit_turn(
        &session.id,
        &session.project,
        "Continue".into(),
        vec![],
        None,
        Default::default(),
    )
    .await
    .unwrap();
    assert_eq!(gateway.0.load(Ordering::Acquire), 3);
    let records = store.tool_invocations();
    let replaced =
        ProjectSessionBinding::new(ProjectInstanceId::new(), ProjectSessionId::new("reopened"));
    let mut observations = ResourceObservations::default();
    for record in &records {
        observations.replay(record, &replaced);
    }
    assert!(observations.version(&document(), false).is_err());
    assert!(observations.version(&document(), true).is_err());
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record.request.bound(),
                Some(AutomationCapabilityRequest::ManageResource(
                    ManageResourceRequest::Save { .. }
                ))
            ))
            .count(),
        1
    );
}
