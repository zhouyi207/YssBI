use super::*;
use yss_harness_contract::model::*;

struct ChartGateway(AtomicU64);
fn chart() -> ChartResourceRef {
    ChartResourceRef::new("charts/Trend.yssbi-chart".into())
}
fn chart_version(revision: u64) -> ResourceVersion {
    ResourceVersion {
        revision,
        session_id: None,
    }
}
impl CapabilityGatewayPort for ChartGateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            crate::authorize_agent_capability(context.agent().unwrap(), &request)?;
            let current = chart_version(self.0.load(Ordering::Acquire));
            match request {
                AutomationCapabilityRequest::InspectChart(request) => {
                    if request
                        .version
                        .as_ref()
                        .is_some_and(|version| version != &current)
                    {
                        return Err(CapabilityFailure::new(
                            CapabilityFailureCode::RevisionConflict,
                        ));
                    }
                    Ok(AutomationCapabilityResult::ChartInspection(
                        ChartInspection {
                            chart: request.chart,
                            version: current,
                            settings: ChartSettings {
                                database_id: "sales".into(),
                                chart_type: ChartType::Line,
                                x: Some("time".into()),
                                y: None,
                            },
                        },
                    ))
                }
                AutomationCapabilityRequest::EditResource(request) => {
                    assert!(matches!(request.edit, ResourceEdit::UpdateChart { .. }));
                    if request.version != current {
                        return Err(CapabilityFailure::new(
                            CapabilityFailureCode::RevisionConflict,
                        ));
                    }
                    let revision = self.0.fetch_add(1, Ordering::AcqRel) + 1;
                    Ok(AutomationCapabilityResult::ResourceEdited(
                        ResourceMutationReceipt {
                            publication_revision: Some(revision),
                            changes: vec![ResourceChange {
                                resource: chart().resource(),
                                revision,
                                revision_kind: ResourceRevisionKind::Resource,
                                deleted: false,
                            }],
                            moves: vec![],
                            mind_edit: None,
                            database_edit: None,
                            document_edit: None,
                            resources: vec![ResourceMutationState {
                                resource: chart().resource(),
                                name: "Trend".into(),
                                version: chart_version(revision),
                                dirty: Some(false),
                                root_topic_id: None,
                            }],
                        },
                    ))
                }
                _ => panic!("unexpected chart capability"),
            }
        })
    }
}

#[tokio::test]
async fn chart_tools_bind_versions_preserve_public_identity_and_authorize_source_changes() {
    let gateway = Arc::new(ChartGateway(AtomicU64::new(1)));
    let store = Arc::new(InMemoryHarnessStore::default());
    let ids = Arc::new(SequentialIds::default());
    let scope = Arc::new(Mutex::new(AgentInvocationScope {
        run_id: AgentRunId::try_new("chart-worker").unwrap(),
        role: AgentRole::Plot,
        task: Some(AgentTaskScope {
            resources: vec![AgentResourceAccess {
                resource: chart().resource(),
                version: None,
                operations: vec![
                    AgentResourceOperation::Inspect,
                    AgentResourceOperation::Edit,
                ],
            }],
            ..Default::default()
        }),
    }));
    let run = capabilities::executor(
        gateway.clone(),
        store.clone(),
        ids.clone(),
        scope.clone(),
        ResourceObservations::default(),
    );
    let read = CapabilityInput::InspectChart(InspectChartInput { chart: chart() });
    let update = CapabilityInput::UpdateChart(UpdateChartInput {
        chart: chart(),
        settings: ChartSettingsUpdate {
            y: Some(None),
            ..Default::default()
        },
    });
    let mut replay = ResourceObservations::default();
    for input in [read.clone(), update.clone(), update.clone()] {
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
        assert_eq!(record.request.model_input().unwrap(), input);
        assert!(
            !model::capability_result(&outcome.result)
                .unwrap()
                .to_string()
                .contains("revision")
        );
        replay.replay(&record, &record.project);
    }
    let restored = capabilities::executor(gateway.clone(), store, ids, scope.clone(), replay);
    restored
        .execute(ModelCapabilityRequest {
            request: update.clone(),
        })
        .await
        .unwrap();
    let source_change = CapabilityInput::UpdateChart(UpdateChartInput {
        chart: chart(),
        settings: ChartSettingsUpdate {
            database_id: Some("unassigned".into()),
            ..Default::default()
        },
    });
    assert!(
        restored
            .execute(ModelCapabilityRequest {
                request: source_change
            })
            .await
            .is_err()
    );
    assert_eq!(gateway.0.load(Ordering::Acquire), 4);
    scope.lock().unwrap().role = AgentRole::Report;
    assert!(
        restored
            .execute(ModelCapabilityRequest { request: update })
            .await
            .is_err()
    );
    scope.lock().unwrap().role = AgentRole::Plot;
    gateway.0.store(5, Ordering::Release);
    assert_eq!(
        restored
            .execute(ModelCapabilityRequest { request: read })
            .await
            .unwrap_err()
            .code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(
        scope.lock().unwrap().task.as_ref().unwrap().resources[0]
            .version
            .as_ref()
            .unwrap()
            .revision,
        4
    );
}
