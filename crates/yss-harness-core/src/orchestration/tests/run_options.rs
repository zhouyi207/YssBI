use super::*;

#[derive(Default)]
struct OptionsDriver {
    writable_run: Mutex<Option<AgentRunId>>,
    calls: AtomicUsize,
}

impl AgentDriverPort for OptionsDriver {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        _: Arc<dyn AgentEventOutput>,
        _: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::Relaxed);
            assert_eq!(
                request.options.reasoning_effort,
                Some(ReasoningEffort::High)
            );
            if request.options.mode == HarnessMode::Ask {
                assert!(
                    request
                        .tools
                        .iter()
                        .all(|tool| tool.capability_id.descriptor().effect == ToolEffect::Inspect)
                );
                if request.role == AgentRole::Manager {
                    inspect_resource(capabilities.as_ref(), document()).await;
                    // This operation is normally authorized for a manager, but Ask removes it at execution too.
                    let intent = serde_json::from_value(
                        serde_json::json!({"intent": {"kind": "showPanel", "panel": "project"}}),
                    )
                    .unwrap();
                    assert!(
                        capabilities
                            .execute(ModelCapabilityRequest {
                                request: model::CapabilityInput::RequestUiIntent(intent)
                            })
                            .await
                            .is_err()
                    );
                    assert!(
                        capabilities
                            .delegate(task("write", AgentRole::Report, true))
                            .await
                            .is_err()
                    );
                    let run_id = self.writable_run.lock().unwrap().clone().unwrap();
                    assert!(
                        capabilities
                            .followup(model::AgentFollowupInput {
                                run_id,
                                instruction: "Save again".into()
                            })
                            .await
                            .is_err()
                    );
                    let outcome = capabilities
                        .delegate(task("read", AgentRole::Report, false))
                        .await
                        .unwrap();
                    assert_eq!(outcome.state, AgentRunState::Completed);
                    let resumed = capabilities
                        .followup(model::AgentFollowupInput {
                            run_id: outcome.run_id,
                            instruction: "Read again".into(),
                        })
                        .await
                        .unwrap();
                    assert_eq!(resumed.state, AgentRunState::Completed);
                } else {
                    inspect_resource(capabilities.as_ref(), document()).await;
                    assert!(
                        capabilities
                            .execute(ModelCapabilityRequest {
                                request: model::CapabilityInput::SaveResource(
                                    model::ResourceTargetInput {
                                        resource: document()
                                    }
                                )
                            })
                            .await
                            .is_err()
                    );
                }
            } else if request.role == AgentRole::Manager {
                let outcome = capabilities
                    .delegate(task("write", AgentRole::Report, true))
                    .await
                    .unwrap();
                assert_eq!(outcome.state, AgentRunState::Completed);
                *self.writable_run.lock().unwrap() = Some(outcome.run_id);
            } else {
                inspect_resource(capabilities.as_ref(), document()).await;
                capabilities
                    .execute(ModelCapabilityRequest {
                        request: model::CapabilityInput::SaveResource(model::ResourceTargetInput {
                            resource: document(),
                        }),
                    })
                    .await
                    .unwrap();
            }
            Ok(AgentTurnResult {
                final_text: "Checked".into(),
            })
        })
    }
}

#[tokio::test]
async fn ask_mode_constrains_manager_workers_and_restored_tasks_and_preserves_turn_options() {
    let driver = Arc::new(OptionsDriver::default());
    let (host, store, session) =
        setup_models(crate::test_support::fixed_model(driver.clone()), false).await;
    for mode in [HarnessMode::Write, HarnessMode::Ask] {
        let options = HarnessTurnOptions {
            mode,
            reasoning_effort: Some(ReasoningEffort::High),
        };
        host.submit_turn(
            &session.id,
            &session.project,
            "Check report".into(),
            vec![],
            None,
            options,
        )
        .await
        .unwrap();
    }
    assert_eq!(driver.calls.load(Ordering::Relaxed), 5);
    let events = store.load_events_after(&session.id, 0).await.unwrap();
    let modes: Vec<_> = events
        .iter()
        .filter_map(|event| match event.event {
            HarnessEvent::TurnConfigured { options } => Some(options.mode),
            _ => None,
        })
        .collect();
    assert_eq!(modes, vec![HarnessMode::Write, HarnessMode::Ask]);
}
