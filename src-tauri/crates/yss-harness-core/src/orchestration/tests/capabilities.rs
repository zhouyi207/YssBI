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
            let mut state = self.0.lock().unwrap();
            let conflict = || CapabilityFailure::new(CapabilityFailureCode::RevisionConflict);
            Ok(match request {
                AutomationCapabilityRequest::InspectGraph(_) => {
                    state.reads += 1;
                    AutomationCapabilityResult::GraphInspection(state.inspection())
                }
                AutomationCapabilityRequest::InspectResource(request) => {
                    state.reads += 1;
                    AutomationCapabilityResult::ResourceInspection(ResourceInspection {
                        resource: request.resource,
                        name: "Fit".into(),
                        version: version(state.revision),
                        dirty: false,
                        content: ResourceContent::Graph {
                            graph: state.inspection(),
                            can_undo: true,
                            can_redo: false,
                            function: Some(FunctionSignatureInspection {
                                revision: state.signature,
                                parameters: vec![],
                                return_type: None,
                            }),
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
                AutomationCapabilityRequest::SaveGraph(request) => {
                    assert_eq!(
                        context.agent().unwrap().task.as_ref().unwrap().resources[0].version,
                        Some(version(state.revision))
                    );
                    if request.graph_hash != format!("{:064x}", state.content) {
                        return Err(conflict());
                    }
                    let from_revision = state.revision;
                    state.revision += 1;
                    state.writes += 1;
                    AutomationCapabilityResult::GraphSaved(GraphSaved {
                        graph_path: request.graph_path,
                        graph_hash: request.graph_hash,
                        from_revision,
                        resource_revision: state.revision,
                        dirty: false,
                        can_undo: false,
                        can_redo: false,
                    })
                }
                AutomationCapabilityRequest::ExecuteGraph(request) => {
                    assert_eq!(context.graph_observation(), Some(state.inputs.as_str()));
                    state.inputs = "e".repeat(64);
                    AutomationCapabilityResult::GraphExecution(GraphExecution {
                        graph_path: request.graph_path,
                        graph_hash: request.graph_hash,
                        run_id: Some(1),
                        status: "succeeded".into(),
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
                    let ResourceEdit::FunctionSignature { signature } = request.edit else {
                        panic!("signature");
                    };
                    if signature.revision != state.signature {
                        return Err(conflict());
                    }
                    state.signature += 1;
                    state.writes += 1;
                    AutomationCapabilityResult::ResourceEdited(ResourceMutationReceipt {
                        publication_revision: None,
                        moves: vec![],
                        created_nodes: BTreeMap::new(),
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

fn executor(
    gateway: Arc<GraphGateway>,
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
    RunExecutor {
        tools: HarnessToolExecutor::new(
            ToolRegistry::for_agent(AgentRole::Stats),
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
                version: Some(version(1)),
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
        request: model::CapabilityInput::ApplyGraphEdit(model::ApplyGraphEditInput {
            graph_path: graph_resource().id,
            locale: "en-US".into(),
            operations: vec![GraphEditOperation::MoveNodes {
                positions: vec![GraphEditPosition {
                    node_id: "node".into(),
                    x: 1.0,
                    y: 2.0,
                }],
            }],
        }),
    };
    assert_eq!(
        run.execute(ModelCapabilityRequest {
            request: model::CapabilityInput::EditResource(model::EditResourceInput {
                resource: graph_resource(),
                edit: model::ResourceEditInput::Doc { operations: vec![] },
            }),
        })
        .await
        .unwrap_err()
        .code,
        CapabilityFailureCode::InvalidRequest
    );
    run.execute(edit()).await.unwrap();
    run.execute(ModelCapabilityRequest {
        request: model::CapabilityInput::SaveGraph(model::GraphTargetInput {
            graph_path: graph_resource().id,
        }),
    })
    .await
    .unwrap();
    run.execute(edit()).await.unwrap();
    assert_eq!(
        gateway.0.lock().unwrap().reads,
        1,
        "committed graph facts avoid refresh-only reads"
    );
    run.execute(ModelCapabilityRequest {
        request: model::CapabilityInput::ExecuteGraph(model::ExecuteGraphInput {
            graph_path: graph_resource().id,
            demand: GraphExecutionDemand::Default,
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
        request: model::CapabilityInput::InspectResource(InspectResourceRequest {
            resource: graph_resource(),
            metadata_only: false,
            graph_view: GraphInspectionView::Overview,
            offset: 0,
            limit: 1,
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
    for id in invocations {
        let record = store
            .load_invocation(&HarnessSessionId::try_new("session").unwrap(), &id)
            .await
            .unwrap()
            .unwrap();
        observations.replay(&record, &project);
    }
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
    let resumed = executor(gateway.clone(), store, ids, scope.clone(), restored);
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
}
