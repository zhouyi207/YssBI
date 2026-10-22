use super::*;
use crate::contract::model::*;

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
                            CapabilityFailureCode::REVISION_CONFLICT,
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
                            CapabilityFailureCode::REVISION_CONFLICT,
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
    let scope = AgentInvocationScope {
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
    };
    let run = capabilities::executor(
        gateway.clone(),
        store.clone(),
        ids.clone(),
        scope.clone(),
        ResourceObservations::default(),
    );
    let read = CapabilityInput::ChartInspect(InspectChartInput { chart: chart() });
    let update = CapabilityInput::ChartUpdate(UpdateChartInput {
        chart: chart(),
        settings: ChartSettingsUpdate {
            y: Some(None),
            ..Default::default()
        },
    });
    let mut records = Vec::new();
    for input in [read.clone(), update.clone(), update.clone()] {
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
            serde_json::to_value(&input).unwrap()["payload"]
        );
        assert!(
            !model::capability_result(&outcome.result)
                .unwrap()
                .to_string()
                .contains("revision")
        );
        records.push(record);
    }
    let restored = capabilities::executor(
        gateway.clone(),
        store,
        ids,
        run.state().scope.lock().unwrap().clone(),
        ResourceObservations::default(),
    );
    for record in &records {
        restored.hooks.replay(record, &record.project);
    }
    restored.execute(update.clone()).await.unwrap();
    let source_change = CapabilityInput::ChartUpdate(UpdateChartInput {
        chart: chart(),
        settings: ChartSettingsUpdate {
            database_id: Some("unassigned".into()),
            ..Default::default()
        },
    });
    assert!(restored.execute(source_change).await.is_err());
    assert_eq!(gateway.0.load(Ordering::Acquire), 4);
    restored.state().scope.lock().unwrap().role = AgentRole::Report;
    assert!(restored.execute(update).await.is_err());
    restored.state().scope.lock().unwrap().role = AgentRole::Plot;
    gateway.0.store(5, Ordering::Release);
    assert_eq!(
        restored.execute(read).await.unwrap_err().code,
        CapabilityFailureCode::REVISION_CONFLICT
    );
    assert_eq!(
        restored
            .state()
            .scope
            .lock()
            .unwrap()
            .task
            .as_ref()
            .unwrap()
            .resources[0]
            .version
            .as_ref()
            .unwrap()
            .revision,
        4
    );
}
