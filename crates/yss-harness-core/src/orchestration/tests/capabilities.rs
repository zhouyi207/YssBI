use super::*;

struct GraphState {
    revision: u64,
    content: u64,
    signature: u64,
    inputs: String,
    writes: usize,
    reads: usize,
}
struct GraphGateway(Mutex<GraphState>);

fn graph_resource() -> ProjectResourceRef {
    ProjectResourceRef {
        kind: ProjectResourceKind::FunctionGraph,
        id: "functions/fit.yssbi-function".into(),
    }
}

impl GraphState {
    fn inspection(&self) -> GraphInspection {
        GraphInspection {
            version: version(self.revision),
            graph_path: graph_resource().id,
            graph_hash: format!("{:064x}", self.content),
            semantic_input_hash: self.inputs.clone(),
            revision: self.revision,
            ready: true,
            nodes: vec![],
            connections: vec![],
            constants: BTreeMap::new(),
            diagnostics: vec![],
        }
    }
}

impl CapabilityGatewayPort for GraphGateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            crate::authorize_agent_capability(context.agent().unwrap(), &request)?;
            let request = match request {
                AutomationCapabilityRequest::GraphMutation(request) => {
                    AutomationCapabilityRequest::ApplyGraphEdit(request.edit_request())
                }
                request => request,
            };
            let mut state = self.0.lock().unwrap();
            let conflict = || CapabilityFailure::new(CapabilityFailureCode::RevisionConflict);
            Ok(match request {
                AutomationCapabilityRequest::InspectGraph(_) => {
                    state.reads += 1;
                    AutomationCapabilityResult::GraphInspection(state.inspection())
                }
                AutomationCapabilityRequest::ValidateGraph(request) => {
                    if request.graph_hash != format!("{:064x}", state.content) {
                        return Err(conflict());
                    }
                    AutomationCapabilityResult::GraphValidation(GraphValidation {
                        graph_path: request.graph.id,
                        graph_hash: request.graph_hash,
                        ready: true,
                        node_ids: request.node_ids,
                        scope_node_count: 0,
                        page: InspectionPage::known(0, 0, 0),
                        diagnostics: vec![],
                    })
                }
                AutomationCapabilityRequest::InspectResource(request) => {
                    state.reads += 1;
                    AutomationCapabilityResult::ResourceInspection(ResourceInspection {
                        resource: request.resource,
                        name: "Fit".into(),
                        version: version(state.revision),
                        dirty: false,
                        content: ResourceContent::Function {
                            signature: FunctionSignatureInspection {
                                revision: state.signature,
                                parameters: vec![],
                                return_type: None,
                            },
                        },
                    })
                }
                AutomationCapabilityRequest::ApplyGraphEdit(request) => {
                    if request.base_revision != state.revision
                        || request.graph_hash != format!("{:064x}", state.content)
                    {
                        return Err(conflict());
                    }
                    state.revision += 1;
                    state.content += 1;
                    state.writes += 1;
                    AutomationCapabilityResult::GraphEditReceipt(GraphEditReceipt {
                        created_constants: Default::default(),
                        graph_path: request.graph_path,
                        from_revision: request.base_revision,
                        to_revision: state.revision,
                        graph_hash: format!("{:064x}", state.content),
                        client_key: request.client_key,
                        created_nodes: BTreeMap::new(),
                        created_ports: BTreeMap::new(),
                        changes: GraphEditChanges {
                            base_semantic_input_hash: state.inputs.clone(),
                            semantic_input_hash: state.inputs.clone(),
                            nodes: vec![],
                            removed_node_ids: vec![],
                            connections: vec![],
                            removed_connection_ids: vec![],
                            constants: BTreeMap::new(),
                            removed_constant_ids: vec![],
                            ready: true,
                            diagnostics: vec![],
                        },
                    })
                }
                AutomationCapabilityRequest::ManageResource(ManageResourceRequest::Save {
                    resource,
                    version: expected,
                }) => {
                    if expected != version(state.revision) {
                        return Err(conflict());
                    }
                    state.revision += 1;
                    state.writes += 1;
                    AutomationCapabilityResult::ResourceManaged(ResourceMutationReceipt {
                        database_edit: None,
                        publication_revision: None,
                        changes: vec![ResourceChange {
                            resource: resource.clone(),
                            revision: state.revision,
                            revision_kind: ResourceRevisionKind::Resource,
                            deleted: false,
                        }],
                        resources: vec![ResourceMutationState {
                            resource,
                            name: "Fit".into(),
                            version: version(state.revision),
                            dirty: Some(false),
                            root_topic_id: None,
                        }],
                        moves: vec![],
                        mind_edit: None,
                        document_edit: None,
                    })
                }
                AutomationCapabilityRequest::ExecuteGraph(request) => {
                    assert_eq!(context.graph_observation(), Some(state.inputs.as_str()));
                    state.inputs = "e".repeat(64);
                    AutomationCapabilityResult::GraphExecution(GraphExecution {
                        graph_path: request.graph.id,
                        graph_hash: request.graph_hash,
                        run_id: Some(1),
                        status: "succeeded".into(),
                        timing: None,
                        failure_code: None,
                        failure_location: None,
                        result_count: Some(0),
                        results_complete: true,
                        results: vec![],
                    })
                }
                AutomationCapabilityRequest::EditResource(request) => {
                    if request.version != version(state.revision) {
                        return Err(conflict());
                    }
                    if let ResourceEdit::GraphHistory { graph_hash, redo } = &request.edit {
                        if graph_hash != &format!("{:064x}", state.content) {
                            return Err(conflict());
                        }
                        state.content = if *redo {
                            state.content + 1
                        } else {
                            state.content - 1
                        };
                        state.revision += 1;
                        state.writes += 1;
                        return Ok(AutomationCapabilityResult::ResourceEdited(
                            ResourceMutationReceipt {
                                database_edit: None,
                                publication_revision: None,
                                changes: vec![ResourceChange {
                                    resource: request.resource.clone(),
                                    revision: state.revision,
                                    revision_kind: ResourceRevisionKind::Resource,
                                    deleted: false,
                                }],
                                resources: vec![ResourceMutationState {
                                    resource: request.resource,
                                    name: "Fit".into(),
                                    version: version(state.revision),
                                    dirty: Some(true),
                                    root_topic_id: None,
                                }],
                                moves: vec![],
                                mind_edit: None,
                                document_edit: None,
                            },
                        ));
                    }
                    let ResourceEdit::FunctionSignature { signature } = request.edit else {
                        panic!("signature");
                    };
                    if signature.revision != state.signature {
                        return Err(conflict());
                    }
                    state.signature += 1;
                    state.writes += 1;
                    AutomationCapabilityResult::ResourceEdited(ResourceMutationReceipt {
                        database_edit: None,
                        resources: vec![],
                        publication_revision: None,
                        moves: vec![],
                        mind_edit: None,
                        document_edit: None,
                        changes: vec![ResourceChange {
                            resource: request.resource,
                            revision: state.signature,
                            revision_kind: ResourceRevisionKind::FunctionSignature,
                            deleted: false,
                        }],
                    })
                }
                _ => panic!("unexpected graph operation"),
            })
        })
    }
}

pub(super) fn executor(
    gateway: Arc<dyn CapabilityGatewayPort>,
    store: Arc<InMemoryHarnessStore>,
    ids: Arc<SequentialIds>,
    scope: Arc<Mutex<AgentInvocationScope>>,
    observations: ResourceObservations,
) -> RunExecutor {
    let project = ProjectSessionBinding::new(
        ProjectInstanceId::from_existing("project".into()),
        ProjectSessionId::new("project-session"),
    );
    let knowledge = Arc::new(crate::KnowledgeService::new(
        store.clone(),
        Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
    ));
    let role = scope.lock().unwrap().role;
    RunExecutor {
        tools: HarnessToolExecutor::new(
            ToolRegistry::for_agent(role),
            gateway,
            knowledge,
            store,
            Arc::new(FixedClock::new(1000)),
            ids,
            PrincipalId::try_new("user").unwrap(),
            HarnessSessionId::try_new("session").unwrap(),
            HarnessTurnId::try_new("turn").unwrap(),
            project,
            CancellationToken::default(),
        )
        .with_agent(scope.clone()),
        scope,
        observations: Mutex::new(observations),
        evidence: Arc::new(Mutex::new(Evidence::default())),
        manager: None,
    }
}

#[tokio::test]
async fn model_graph_writes_bind_receipts_restore_after_compaction_and_reject_gui_changes() {
    let gateway = Arc::new(GraphGateway(Mutex::new(GraphState {
        revision: 1,
        content: 1,
        signature: 41,
        inputs: "a".repeat(64),
        writes: 0,
        reads: 0,
    })));
    let store = Arc::new(InMemoryHarnessStore::default());
    let ids = Arc::new(SequentialIds::default());
    let scope = Arc::new(Mutex::new(AgentInvocationScope {
        run_id: AgentRunId::try_new("worker").unwrap(),
        role: AgentRole::Stats,
        task: Some(AgentTaskScope {
            resources: vec![AgentResourceAccess {
                resource: graph_resource(),
                version: Some(ResourceVersion {
                    revision: 1,
                    session_id: None,
                }),
                operations: vec![
                    AgentResourceOperation::Inspect,
                    AgentResourceOperation::Edit,
                    AgentResourceOperation::Save,
                    AgentResourceOperation::Execute,
                ],
            }],
            ..Default::default()
        }),
    }));
    let run = executor(
        gateway.clone(),
        store.clone(),
        ids.clone(),
        scope.clone(),
        ResourceObservations::default(),
    );
    let edit = || ModelCapabilityRequest {
        request: model::CapabilityInput::MoveNodes(model::MoveNodesInput {
            graph: GraphResourceRef::for_path(graph_resource().id),
            positions: vec![GraphEditPosition {
                node_id: "node".into(),
                x: 1.0,
                y: 2.0,
            }],
        }),
    };
    assert_eq!(
        run.execute(ModelCapabilityRequest {
            request: model::CapabilityInput::WriteDocument(model::WriteDocumentInput {
                document: model::DocumentResourceRef::new(graph_resource().id),
                markdown: String::new(),
            }),
        })
        .await
        .unwrap_err()
        .code,
        CapabilityFailureCode::InvalidRequest
    );
    let rejected = store.tool_invocations().pop().unwrap();
    assert_eq!(rejected.state, ToolInvocationState::Failed);
    assert_eq!(rejected.capability_id, CapabilityId::WriteDocument);
    assert!(rejected.request.bound().is_none());
    assert_eq!(
        rejected.request.model_arguments().unwrap()["document"]["id"],
        graph_resource().id
    );
    assert!(rejected.finished_at.is_some());
    assert!(run.evidence.lock().unwrap().invocations.is_empty());
    assert_eq!(gateway.0.lock().unwrap().writes, 0);
    run.execute(edit()).await.unwrap();
    run.execute(ModelCapabilityRequest {
        request: model::CapabilityInput::SaveResource(model::ResourceTargetInput {
            resource: graph_resource(),
        }),
    })
    .await
    .unwrap();
    run.execute(edit()).await.unwrap();
    assert_eq!(
        gateway.0.lock().unwrap().reads,
        2,
        "edits reuse committed facts; save metadata requires one internal graph-basis read"
    );
    run.execute(ModelCapabilityRequest {
        request: model::CapabilityInput::ExecuteGraph(model::ExecuteGraphInput {
            graph: GraphResourceRef::for_path(graph_resource().id),
            node_id: None,
            mode: None,
        }),
    })
    .await
    .unwrap();
    let signature = || ModelCapabilityRequest {
        request: model::CapabilityInput::EditResource(model::EditResourceInput {
            resource: graph_resource(),
            edit: model::ResourceEditInput::FunctionSignature {
                signature: model::FunctionSignatureInput {
                    parameters: vec![],
                    return_type: Some("core.numeric".into()),
                },
            },
        }),
    };
    assert_eq!(
        run.execute(signature()).await.unwrap_err().details["reason"],
        "resource_read_required"
    );
    run.execute(ModelCapabilityRequest {
        request: model::CapabilityInput::InspectResource(model::InspectResourceInput {
            resource: graph_resource(),
        }),
    })
    .await
    .unwrap();
    run.execute(signature()).await.unwrap();

    // The model's compacted text contains no concurrency fields. Full ledger receipts restore them.
    let mut observations = ResourceObservations::default();
    let invocations = run.evidence.lock().unwrap().invocations.clone();
    let project = ProjectSessionBinding::new(
        ProjectInstanceId::from_existing("project".into()),
        ProjectSessionId::new("project-session"),
    );
    let mut public_edits = 0;
    for id in invocations {
        let record = store
            .load_invocation(&HarnessSessionId::try_new("session").unwrap(), &id)
            .await
            .unwrap()
            .unwrap();
        if matches!(
            record.request.bound(),
            Some(AutomationCapabilityRequest::GraphMutation(_))
        ) {
            public_edits += 1;
            assert_eq!(record.capability_id, CapabilityId::MoveNodes);
            let input = serde_json::to_value(record.request.model_input().unwrap()).unwrap();
            assert_eq!(input["type"], "move_nodes");
            assert_eq!(input["payload"]["graph"]["id"], graph_resource().id);
            assert!(input["payload"].get("graphHash").is_none());
        }
        observations.replay(&record, &project);
    }
    assert_eq!(public_edits, 2);
    let resumed = executor(
        gateway.clone(),
        store.clone(),
        ids.clone(),
        scope.clone(),
        observations,
    );
    resumed.execute(signature()).await.unwrap();
    assert_eq!(
        gateway.0.lock().unwrap().signature,
        43,
        "signature counters are distinct from graph resource revisions"
    );
    let inspect = || ModelCapabilityRequest {
        request: model::CapabilityInput::InspectGraph(
            (&InspectGraphRequest::overview(graph_resource().id)).into(),
        ),
    };
    resumed.execute(inspect()).await.unwrap();
    let writes = gateway.0.lock().unwrap().writes;
    gateway.0.lock().unwrap().inputs = "b".repeat(64);
    assert_eq!(
        resumed.execute(inspect()).await.unwrap_err().code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(
        resumed.execute(edit()).await.unwrap_err().code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(
        gateway.0.lock().unwrap().writes,
        writes,
        "semantic changes with unchanged document revisions still stop writes"
    );
    let mut fresh = ResourceObservations::default();
    fresh.record(&AutomationCapabilityResult::GraphInspection(
        gateway.0.lock().unwrap().inspection(),
    ));
    let mut restored = std::mem::take(&mut *resumed.observations.lock().unwrap());
    restored.refresh_from(&fresh, &graph_resource());
    let resumed = executor(
        gateway.clone(),
        store.clone(),
        ids.clone(),
        scope.clone(),
        restored,
    );
    resumed.execute(edit()).await.unwrap();
    let writes = gateway.0.lock().unwrap().writes;
    {
        let mut state = gateway.0.lock().unwrap();
        state.revision += 1;
        state.content += 1;
    }
    assert_eq!(
        resumed.execute(edit()).await.unwrap_err().code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(
        resumed.execute(inspect()).await.unwrap_err().code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(gateway.0.lock().unwrap().writes, writes);
    assert_eq!(
        scope.lock().unwrap().task.as_ref().unwrap().resources[0].version,
        Some(version(5))
    );
    // A coordinator's pure validation captures its own current read. It has no
    // worker task envelope and must not borrow the stale worker's authority.
    let manager = executor(
        gateway.clone(),
        store,
        ids,
        Arc::new(Mutex::new(AgentInvocationScope {
            run_id: AgentRunId::try_new("manager-validation").unwrap(),
            role: AgentRole::Manager,
            task: None,
        })),
        ResourceObservations::default(),
    );
    let outcome = manager
        .execute(ModelCapabilityRequest {
            request: model::CapabilityInput::ValidateGraph(model::ValidateGraphInput {
                graph: GraphResourceRef::for_path(graph_resource().id),
                node_ids: vec![],
                offset: 0,
                limit: 50,
            }),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome.result,
        AutomationCapabilityResult::GraphValidation(GraphValidation { ready: true, .. })
    ));
    assert_eq!(gateway.0.lock().unwrap().writes, writes);
}

#[tokio::test]
async fn resource_history_keeps_public_ledger_identity_and_rejects_changed_inputs() {
    let gateway = Arc::new(GraphGateway(Mutex::new(GraphState {
        revision: 1,
        content: 1,
        signature: 1,
        inputs: "a".repeat(64),
        writes: 0,
        reads: 0,
    })));
    let store = Arc::new(InMemoryHarnessStore::default());
    let scope = Arc::new(Mutex::new(AgentInvocationScope {
        run_id: AgentRunId::try_new("history-worker").unwrap(),
        role: AgentRole::Stats,
        task: Some(AgentTaskScope {
            resources: vec![AgentResourceAccess {
                resource: graph_resource(),
                version: Some(version(1)),
                operations: vec![
                    AgentResourceOperation::Inspect,
                    AgentResourceOperation::Edit,
                ],
            }],
            ..Default::default()
        }),
    }));
    let run = executor(
        gateway.clone(),
        store.clone(),
        Arc::new(SequentialIds::default()),
        scope.clone(),
        ResourceObservations::default(),
    );
    let target = || model::ResourceTargetInput {
        resource: graph_resource(),
    };
    for input in [
        model::CapabilityInput::UndoResource(target()),
        model::CapabilityInput::RedoResource(target()),
    ] {
        let expected = input.capability_id();
        run.execute(ModelCapabilityRequest {
            request: input.clone(),
        })
        .await
        .unwrap();
        let invocation = run
            .evidence
            .lock()
            .unwrap()
            .invocations
            .last()
            .unwrap()
            .clone();
        let record = store
            .load_invocation(&HarnessSessionId::try_new("session").unwrap(), &invocation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.capability_id, expected);
        assert_eq!(record.request.capability_id(), expected);
        assert_eq!(record.request.model_input().unwrap(), input);
        assert_eq!(record.state, ToolInvocationState::Succeeded);
        let public = serde_json::to_value(record.request.model_input().unwrap()).unwrap();
        assert_eq!(
            public["payload"],
            serde_json::json!({"resource":graph_resource()})
        );
    }
    assert_eq!(gateway.0.lock().unwrap().content, 1);
    assert_eq!(gateway.0.lock().unwrap().writes, 2);
    gateway.0.lock().unwrap().revision += 1;
    assert_eq!(
        run.execute(ModelCapabilityRequest {
            request: model::CapabilityInput::UndoResource(target())
        })
        .await
        .unwrap_err()
        .code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(gateway.0.lock().unwrap().writes, 2);
    scope.lock().unwrap().task.as_mut().unwrap().resources[0].operations =
        vec![AgentResourceOperation::Inspect];
    assert!(
        run.bind_input(model::CapabilityInput::RedoResource(target()))
            .await
            .is_err()
    );
}
