use super::*;
use crate::contract::model::*;
use serde_json::json;

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
                        CapabilityFailureCode::REVISION_CONFLICT,
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
    let task_scope = AgentTaskScope {
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
    };
    let scope = AgentInvocationScope {
        run_id: AgentRunId::try_new("report-mind").unwrap(),
        role: AgentRole::Report,
        task: Some(task_scope),
    };
    let run = capabilities::executor(
        gateway.clone(),
        store.clone(),
        Arc::new(SequentialIds::default()),
        scope.clone(),
        ResourceObservations::default(),
    );
    assert!(run.state().evidence.lock().unwrap().delivery_pending());
    run.execute(CapabilityInput::ProjectCreateResource(
        CreateResourceInput::Mind {
            name: "Plan".into(),
        },
    ))
    .await
    .unwrap();
    assert!(run.state().evidence.lock().unwrap().delivery_pending());
    let mind = MindResourceRef::new(mind_resource().id);
    let inputs: Vec<CapabilityInput> = vec![
        CapabilityInput::MindInspect(serde_json::from_value(json!({"mind":mind})).unwrap()),
        CapabilityInput::MindSearchTopics(serde_json::from_value(json!({"mind":mind,"query":"result"})).unwrap()),
        CapabilityInput::MindInspectTopics(serde_json::from_value(json!({"mind":mind,"topicIds":["root"]})).unwrap()),
        CapabilityInput::MindCreateTopics(serde_json::from_value(json!({"mind":mind,"topics":[{"clientId":"branch","parentId":"root","content":"Branch"}]})).unwrap()),
        CapabilityInput::MindUpdateTopics(serde_json::from_value(json!({"mind":mind,"topics":[{"topicId":"root","reference":null}]})).unwrap()),
        CapabilityInput::MindMoveTopics(serde_json::from_value(json!({"mind":mind,"topics":[{"topicId":"branch","parentId":"root"}]})).unwrap()),
        CapabilityInput::MindDuplicateTopics(serde_json::from_value(json!({"mind":mind,"topicIds":["branch"],"parentId":"root"})).unwrap()),
        CapabilityInput::MindDeleteTopics(serde_json::from_value(json!({"mind":mind,"topicIds":["branch"]})).unwrap()),
    ];
    let mut replay = ResourceObservations::default();
    for input in &inputs {
        let outcome = run.execute(input.clone()).await.unwrap();
        let record = store
            .load_invocation(
                &HarnessSessionId::try_new("session").unwrap(),
                &outcome.invocation_id,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.tool, input.capability_id().as_str());
        assert_eq!(
            record.arguments,
            serde_json::to_value(input).unwrap()["payload"]
        );
        assert_eq!(record.state, ToolInvocationState::Succeeded);
        let visible = model::capability_result(&outcome.result)
            .unwrap()
            .to_string();
        assert!(!visible.contains("revision") && !visible.contains("sessionId"));
        replay.record(&outcome.result, true);
    }
    assert_eq!(
        replay
            .version(&mind_resource(), false)
            .unwrap()
            .unwrap()
            .revision,
        13
    );
    assert!(run.state().evidence.lock().unwrap().delivery_pending());
    let save = CapabilityInput::ProjectSaveResource(ResourceTargetInput {
        resource: mind_resource(),
    });
    run.execute(save).await.unwrap();
    assert!(!run.state().evidence.lock().unwrap().delivery_pending());
    let finish = run.hooks.finish();
    assert!(finish.blocked_reason.is_none());
    assert!(
        AgentTaskFacts::from_value(&finish.facts)
            .unwrap()
            .artifacts
            .iter()
            .all(|artifact| artifact.resource.kind == ProjectResourceKind::Mind)
    );
    run.execute(inputs[4].clone()).await.unwrap();
    assert!(run.state().evidence.lock().unwrap().delivery_pending());
    let before = run
        .state()
        .scope
        .lock()
        .unwrap()
        .task
        .as_ref()
        .unwrap()
        .resources[0]
        .version
        .clone();
    gateway.0.fetch_add(1, Ordering::AcqRel);
    for input in [inputs[0].clone(), inputs[4].clone()] {
        assert_eq!(
            run.execute(input).await.unwrap_err().code,
            CapabilityFailureCode::REVISION_CONFLICT
        );
    }
    assert_eq!(
        run.state()
            .scope
            .lock()
            .unwrap()
            .task
            .as_ref()
            .unwrap()
            .resources[0]
            .version,
        before
    );
    let mut unauthorized = run.state().scope.lock().unwrap().clone();
    unauthorized.role = AgentRole::Review;
    assert!(crate::agents::authorize_model_capability(&unauthorized, &inputs[4]).is_err());
    unauthorized.role = AgentRole::Report;
    unauthorized.task.as_mut().unwrap().resources.clear();
    assert!(crate::agents::authorize_model_capability(&unauthorized, &inputs[4]).is_err());
}
