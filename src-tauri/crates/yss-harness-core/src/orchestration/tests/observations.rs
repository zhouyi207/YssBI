use super::*;

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
        content: ResourceContent::Database {
            schema: schema(runtime),
            rows: vec![],
            row_ids: vec![],
            next_offset: None,
            can_undo: false,
            can_redo: false,
        },
    };
    let mut observations = ResourceObservations::default();
    observations.record(&AutomationCapabilityResult::DatasetSchemaInspection(
        schema(12),
    ));
    assert!(observations.version(&resource, false).unwrap().is_none());
    assert!(observations.needs_database_content(&resource));
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
                record.request,
                AutomationCapabilityRequest::ManageResource(ManageResourceRequest::Save { .. })
            ))
            .count(),
        1
    );
}
