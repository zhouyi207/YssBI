use super::*;
use serde_json::json;
use yss_harness_contract::model::*;

struct MindGateway(AtomicU64);

fn mind_resource() -> ProjectResourceRef {
    MindResourceRef::new("minds/plan.yssbi-mind".into()).resource()
}

impl CapabilityGatewayPort for MindGateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            crate::authorize_agent_capability(context.agent().unwrap(), &request)?;
            let current = version(self.0.load(Ordering::Acquire));
            let check = |expected: &ResourceVersion| {
                if expected == &current {
                    Ok(())
                } else {
                    Err(CapabilityFailure::new(
                        CapabilityFailureCode::RevisionConflict,
                    ))
                }
            };
            if let AutomationCapabilityRequest::ReadMind(request) = request {
                if let Some(expected) = request.version {
                    check(&expected)?;
                }
                let content = match request.input {
                    MindReadInput::Outline(_) => MindReadContent::Outline {
                        root_topic_id: "root".into(),
                        topic_count: 1,
                        subtree_count: 1,
                        topics: vec![],
                        page: InspectionPage::known(0, 0, 0),
                    },
                    MindReadInput::Find(_) => MindReadContent::Found {
                        topics: vec![],
                        page: InspectionPage::known(0, 0, 0),
                    },
                    MindReadInput::Topics(_) => MindReadContent::Topics { topics: vec![] },
                };
                return Ok(AutomationCapabilityResult::MindRead(MindReadResult {
                    mind: MindResourceRef::new(mind_resource().id),
                    version: current,
                    dirty: true,
                    content,
                }));
            }
            let (saved, edited) = match request {
                AutomationCapabilityRequest::ManageResource(ManageResourceRequest::Create {
                    specification: ResourceCreation::Mind { .. },
                }) => (false, false),
                AutomationCapabilityRequest::ManageResource(ManageResourceRequest::Save {
                    version,
                    ..
                }) => {
                    check(&version)?;
                    (true, false)
                }
                AutomationCapabilityRequest::EditResource(request) => {
                    check(&request.version)?;
                    (false, true)
                }
                _ => panic!("unexpected Mind operation"),
            };
            let revision = self.0.fetch_add(1, Ordering::AcqRel) + 1;
            let receipt = ResourceMutationReceipt {
                publication_revision: Some(revision),
                changes: vec![ResourceChange {
                    resource: mind_resource(),
                    revision,
                    revision_kind: ResourceRevisionKind::Resource,
                    deleted: false,
                }],
                moves: vec![],
                mind_edit: None,
                document_edit: None,
                database_edit: None,
                resources: vec![ResourceMutationState {
                    resource: mind_resource(),
                    name: "Plan".into(),
                    version: version(revision),
                    dirty: Some(!saved),
                    root_topic_id: Some("root".into()),
                }],
            };
            Ok(if edited {
                AutomationCapabilityResult::ResourceEdited(receipt)
            } else {
                AutomationCapabilityResult::ResourceManaged(receipt)
            })
        })
    }
}

#[tokio::test]
async fn mind_only_report_binds_topic_tools_replays_receipts_and_requires_a_final_save() {
    let gateway = Arc::new(MindGateway(AtomicU64::new(7)));
    let store = Arc::new(InMemoryHarnessStore::default());
    let task = AgentTask {
        key: "mind-plan".into(),
        worker: AgentRole::Report,
        objective: "Create a saved Mind".into(),
        constraints: "Only the authorized Mind".into(),
        completion_criteria: "Saved Mind".into(),
        depends_on: vec![],
        scope: AgentTaskScope {
            creations: vec![AgentCreationAccess {
                specification: ResourceCreation::Mind {
                    name: "Plan".into(),
                },
                operations: vec![
                    AgentResourceOperation::Inspect,
                    AgentResourceOperation::Edit,
                    AgentResourceOperation::Save,
                ],
            }],
            ..Default::default()
        },
    };
    let scope = Arc::new(Mutex::new(AgentInvocationScope {
        run_id: AgentRunId::try_new("report-mind").unwrap(),
        role: AgentRole::Report,
        task: Some(task.scope.clone()),
    }));
    let run = capabilities::executor(
        gateway.clone(),
        store.clone(),
        Arc::new(SequentialIds::default()),
        scope.clone(),
        ResourceObservations::default(),
    );
    *run.evidence.lock().unwrap() = Evidence::for_task(&task);
    assert!(run.evidence.lock().unwrap().delivery_pending());
    run.execute(ModelCapabilityRequest {
        request: CapabilityInput::CreateResource(CreateResourceInput::Mind {
            name: "Plan".into(),
        }),
    })
    .await
    .unwrap();
    assert!(run.evidence.lock().unwrap().delivery_pending());
    let mind = MindResourceRef::new(mind_resource().id);
    let inputs: Vec<CapabilityInput> = vec![
        CapabilityInput::InspectMind(serde_json::from_value(json!({"mind":mind})).unwrap()),
        CapabilityInput::FindTopics(serde_json::from_value(json!({"mind":mind,"query":"result"})).unwrap()),
        CapabilityInput::InspectTopics(serde_json::from_value(json!({"mind":mind,"topicIds":["root"]})).unwrap()),
        CapabilityInput::CreateTopics(serde_json::from_value(json!({"mind":mind,"topics":[{"clientId":"branch","parentId":"root","content":"Branch"}]})).unwrap()),
        CapabilityInput::UpdateTopics(serde_json::from_value(json!({"mind":mind,"topics":[{"topicId":"root","reference":null}]})).unwrap()),
        CapabilityInput::MoveTopics(serde_json::from_value(json!({"mind":mind,"topics":[{"topicId":"branch","parentId":"root"}]})).unwrap()),
        CapabilityInput::DuplicateTopics(serde_json::from_value(json!({"mind":mind,"topicIds":["branch"],"parentId":"root"})).unwrap()),
        CapabilityInput::DeleteTopics(serde_json::from_value(json!({"mind":mind,"topicIds":["branch"]})).unwrap()),
    ];
    let mut replay = ResourceObservations::default();
    for input in &inputs {
        let outcome = run
            .execute(ModelCapabilityRequest {
                request: input.clone(),
            })
            .await
            .unwrap();
        let record = store
            .load_invocation(
                &HarnessSessionId::try_new("session").unwrap(),
                &outcome.invocation_id,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.capability_id, input.capability_id());
        assert_eq!(record.request.model_input().unwrap(), *input);
        assert_eq!(record.state, ToolInvocationState::Succeeded);
        let visible = model::capability_result(&outcome.result)
            .unwrap()
            .to_string();
        assert!(!visible.contains("revision") && !visible.contains("sessionId"));
        replay.record(&outcome.result);
    }
    assert_eq!(
        replay
            .version(&mind_resource(), false)
            .unwrap()
            .unwrap()
            .revision,
        13
    );
    assert!(run.evidence.lock().unwrap().delivery_pending());
    let save = CapabilityInput::SaveResource(ResourceTargetInput {
        resource: mind_resource(),
    });
    run.execute(ModelCapabilityRequest { request: save })
        .await
        .unwrap();
    assert!(!run.evidence.lock().unwrap().delivery_pending());
    let outcome = finish_outcome(
        AgentRunId::try_new("report-mind").unwrap(),
        AgentRole::Report,
        &Ok(AgentTurnResult {
            final_text: "Saved the Mind".into(),
        }),
        &run.evidence,
    );
    assert_eq!(outcome.state, AgentRunState::Completed);
    assert!(
        outcome
            .artifacts
            .iter()
            .all(|artifact| artifact.resource.kind == ProjectResourceKind::Mind)
    );
    run.execute(ModelCapabilityRequest {
        request: inputs[4].clone(),
    })
    .await
    .unwrap();
    assert!(run.evidence.lock().unwrap().delivery_pending());
    let before = scope.lock().unwrap().task.as_ref().unwrap().resources[0]
        .version
        .clone();
    gateway.0.fetch_add(1, Ordering::AcqRel);
    for input in [inputs[0].clone(), inputs[4].clone()] {
        assert_eq!(
            run.execute(ModelCapabilityRequest { request: input })
                .await
                .unwrap_err()
                .code,
            CapabilityFailureCode::RevisionConflict
        );
    }
    assert_eq!(
        scope.lock().unwrap().task.as_ref().unwrap().resources[0].version,
        before
    );
    let mut unauthorized = scope.lock().unwrap().clone();
    unauthorized.role = AgentRole::Review;
    assert!(crate::agents::authorize_model_capability(&unauthorized, &inputs[4]).is_err());
    unauthorized.role = AgentRole::Report;
    unauthorized.task.as_mut().unwrap().resources.clear();
    assert!(crate::agents::authorize_model_capability(&unauthorized, &inputs[4]).is_err());
}
