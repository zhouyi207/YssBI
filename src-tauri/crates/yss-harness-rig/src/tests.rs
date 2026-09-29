use crate::error::map_prompt_failure;
use crate::provider::openai_agent_driver_with_client;
use crate::stream::consume_text_stream;
use futures_util::StreamExt;
use rig_agent::agent::MultiTurnStreamItem;
use rig_agent::completion::PromptError;
use rig_agent::streaming::StreamedAssistantContent;
use rig_core::completion::CompletionModel;
use rig_core::{client::CompletionClient, providers::openai};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use yss_harness_contract::*;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::*;
use rig_agent::completion::message::{ToolCall, ToolFunction};
use rig_agent::streaming::StreamingCompletionResponse;
use rig_core::completion::{
    AssistantContent, CompletionError, CompletionRequest, CompletionResponse, Usage,
};
use yss_harness_contract::{
    AgentFuture, AgentOutputFailure, AutomationCapabilityResult, CapabilityFailure,
    DatasetSchemaInspection, ModelCapabilityOutcome, ToolInvocationId,
};

#[derive(Clone)]
struct ScriptedCompletionModel {
    turns: Arc<Mutex<VecDeque<Vec<AssistantContent>>>>,
    requests: Arc<Mutex<Vec<CompletionRequest>>>,
}

impl ScriptedCompletionModel {
    fn new(turns: impl IntoIterator<Item = Vec<AssistantContent>>) -> Self {
        Self {
            turns: Arc::new(Mutex::new(turns.into_iter().collect())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CompletionModel for ScriptedCompletionModel {
    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> Result<CompletionResponse, CompletionError> {
        panic!("the driver must use the streaming provider interface")
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse, CompletionError> {
        self.requests.lock().unwrap().push(request);
        use rig_core::streaming::{RawStreamingChoice, RawStreamingToolCall, StreamFinal};
        let choice = self
            .turns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pop_front()
            .ok_or_else(|| CompletionError::ResponseError("script exhausted".to_owned()))?;
        let mut items = Vec::new();
        for part in choice {
            match part {
                AssistantContent::Text(text) => {
                    items.extend(
                        text.text.chars().map(|character| {
                            Ok(RawStreamingChoice::Message(character.to_string()))
                        }),
                    );
                }
                AssistantContent::ToolCall(call) => {
                    items.push(Ok(RawStreamingChoice::ToolCall(RawStreamingToolCall::new(
                        call.id.as_str(),
                        call.function.name,
                        call.function.arguments,
                    ))))
                }
                _ => unreachable!(),
            }
        }
        items.push(Ok(RawStreamingChoice::FinalResponse(StreamFinal::new(
            "test",
            Usage::new(),
        ))));
        Ok(StreamingCompletionResponse::stream(
            "test",
            Box::pin(futures_util::stream::iter(items)),
        ))
    }
}

struct StaticExecutor;

impl ModelCapabilityExecutor for StaticExecutor {
    fn execute<'a>(
        &'a self,
        request: ModelCapabilityRequest,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async move {
            assert!(matches!(
                request.request,
                AutomationCapabilityRequest::InspectDatasetSchema(_)
            ));
            Ok(ModelCapabilityOutcome {
                invocation_id: ToolInvocationId::try_new("tool-1").unwrap(),
                result: AutomationCapabilityResult::DatasetSchemaInspection(
                    DatasetSchemaInspection {
                        database_id: "database-1".to_owned(),
                        runtime_revision: 1,
                        schema_revision: 2,
                        columns: Vec::new(),
                    },
                ),
            })
        })
    }
}

#[derive(Default)]
struct CollectingOutput {
    events: Mutex<Vec<AgentEvent>>,
}

impl AgentEventOutput for CollectingOutput {
    fn emit<'a>(&'a self, event: AgentEvent) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
        Box::pin(async move {
            self.events
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(event);
            Ok(())
        })
    }
}

fn request(tools: Vec<ToolDescriptor>) -> AgentTurnRequest {
    AgentTurnRequest {
        role: yss_harness_contract::AgentRole::Stats,
        limits: yss_harness_contract::AgentRunLimits {
            maximum_model_turns: 128,
            maximum_output_tokens: 8192,
            maximum_duration_ms: 300_000,
            tool_concurrency: 1,
        },
        control_tools: vec![yss_harness_contract::AgentControlTool::ProposeStatisticalPlan],
        output_mode: yss_harness_contract::AgentOutputMode::FinalResponse,
        messages: vec![
            AgentMessage::System {
                content: "Use evidence.".to_owned(),
            },
            AgentMessage::User {
                content: "Inspect the schema.".to_owned(),
            },
        ],
        tools,
    }
}

#[test]
fn context_capacity_failure_is_explicit_without_treating_other_rejections_as_capacity() {
    for (code, expected) in [
        (
            "context_length_exceeded",
            AgentDriverFailureCode::ContextWindowExceeded,
        ),
        (
            "invalid_request",
            AgentDriverFailureCode::ProviderRequestRejected,
        ),
    ] {
        let error = CompletionError::from_http_response(
            400u16.try_into().unwrap(),
            serde_json::json!({"error": {"code": code}}).to_string(),
        );
        assert_eq!(
            map_prompt_failure(PromptError::CompletionError(error)).code,
            expected
        );
    }
}

#[tokio::test]
async fn manager_delegation_round_trips_through_rig_and_workers_do_not_receive_it() {
    use yss_harness_contract::{
        AgentRole, AgentRunId, AgentRunState, AgentTask, AgentTaskOutcome, AgentTaskScope,
        WorkerReport,
    };
    struct Delegate(std::sync::atomic::AtomicUsize);
    impl ModelCapabilityExecutor for Delegate {
        fn execute<'a>(
            &'a self,
            _request: ModelCapabilityRequest,
        ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
            Box::pin(async { panic!("no business capability requested") })
        }
        fn delegate<'a>(
            &'a self,
            task: AgentTask,
        ) -> AgentFuture<'a, Result<AgentTaskOutcome, CapabilityFailure>> {
            Box::pin(async move {
                task.validate().unwrap();
                assert_eq!(task.worker, AgentRole::Review);
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                Ok(AgentTaskOutcome {
                    run_id: AgentRunId::try_new("review-run").unwrap(),
                    role: task.worker,
                    state: AgentRunState::Completed,
                    report: Some(WorkerReport {
                        summary: "Checked".into(),
                        warnings: vec![],
                        blocked_reason: None,
                        next_steps: vec![],
                    }),
                    failure_code: None,
                    artifacts: vec![],
                    results: vec![],
                    evidence: vec![],
                    plan: None,
                    invalidated_runs: vec![],
                })
            })
        }
    }
    let task = AgentTask {
        key: "review".into(),
        worker: AgentRole::Review,
        objective: "Check evidence".into(),
        constraints: "Read only".into(),
        completion_criteria: "Return findings".into(),
        depends_on: vec![],
        scope: AgentTaskScope::default(),
    };
    let model = ScriptedCompletionModel::new([
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "delegate",
            ToolFunction::new("delegate_task".into(), serde_json::to_value(task).unwrap()),
        ))],
        vec![AssistantContent::text("Delivered")],
    ]);
    let executor = Arc::new(Delegate(std::sync::atomic::AtomicUsize::new(0)));
    let driver = RigAgentDriver::new(model.clone());
    let mut input = request(vec![]);
    input.role = AgentRole::Manager;
    input.control_tools = vec![yss_harness_contract::AgentControlTool::DelegateTask];
    input.output_mode = yss_harness_contract::AgentOutputMode::Transcript;
    driver
        .run_turn(
            input,
            executor.clone(),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(executor.0.load(std::sync::atomic::Ordering::Acquire), 1);
    let history = serde_json::to_string(&model.requests.lock().unwrap()[1].chat_history).unwrap();
    assert!(history.contains("review-run") && history.contains("Checked"));
    let model = ScriptedCompletionModel::new([vec![AssistantContent::text("Review result")]]);
    let driver = RigAgentDriver::new(model.clone());
    let mut input = request(vec![]);
    input.role = AgentRole::Review;
    input.control_tools.clear();
    driver
        .run_turn(
            input,
            executor,
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert!(model.requests.lock().unwrap()[0].tools.is_empty());
}

#[tokio::test]
async fn provider_receives_complete_history_and_correlated_tool_messages() {
    let model = ScriptedCompletionModel::new([vec![AssistantContent::text("continued")]]);
    let recorded = model.requests.clone();
    let driver = RigAgentDriver::new(model);
    let text = "long-history:".to_owned() + &"h".repeat(2_000_000) + ":text-tail";
    let payload = serde_json::json!({"complete": "r".repeat(2_000_000), "tail": "result-tail"});
    let invocation_id = ToolInvocationId::try_new("saved-call-1").unwrap();
    let mut input = request(Vec::new());
    input.messages = vec![
        AgentMessage::System {
            content: "Use recorded evidence.".into(),
        },
        AgentMessage::User {
            content: text.clone(),
        },
        AgentMessage::Assistant {
            content: "Checking.".into(),
        },
        AgentMessage::ToolCall {
            invocation_id: invocation_id.clone(),
            request: AutomationCapabilityRequest::InspectResult(InspectResultRequest {
                execution_session_id: "00000000-0000-0000-0000-000000000001".into(),
                result_id: 17,
                part: None,
                offset: 0,
                limit: 20,
            }),
        },
        AgentMessage::ToolResult {
            invocation_id,
            capability_id: CapabilityId::InspectResult,
            outcome: Ok(AutomationCapabilityResult::ResultInspection(
                yss_harness_contract::ResultInspection {
                    result_id: 17,
                    category: yss_harness_contract::ResultCategoryInspection::Value,
                    value: yss_harness_contract::ResultValueInspection::Json(payload.clone()),
                },
            )),
        },
        AgentMessage::Assistant {
            content: "Previous conclusion.".into(),
        },
        AgentMessage::User {
            content: "Continue.".into(),
        },
    ];
    driver
        .run_turn(
            input,
            Arc::new(StaticExecutor),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    let requests = recorded.lock().unwrap();
    let wire = requests[0]
        .chat_history
        .clone()
        .into_iter()
        .flat_map(|message| {
            Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
        })
        .map(|message| serde_json::to_value(message).unwrap())
        .collect::<Vec<_>>();
    let encoded = serde_json::to_string(&wire).unwrap();
    assert!(encoded.contains(&text));
    assert!(encoded.contains("result-tail"));
    let call = wire
        .iter()
        .find(|message| message.get("tool_calls").is_some())
        .unwrap();
    let result = wire
        .iter()
        .find(|message| message["role"] == "tool")
        .unwrap();
    assert_eq!(call["tool_calls"][0]["id"], result["tool_call_id"]);
    assert_eq!(call["tool_calls"][0]["function"]["name"], "inspect_result");
    let value: serde_json::Value =
        serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
    assert!(value["payload"]["value"]["value"] == payload);
}

fn sample_plan() -> serde_json::Value {
    serde_json::json!({
        "researchQuestion": "How does x relate to y?",
        "analysisMode": "exploratory",
        "studyDesign": { "kind": "cross_sectional", "description": "Observed data" },
        "estimands": [],
        "variableRoles": [
            { "resourceId": "database-1", "variable": "y", "role": "outcome" },
            { "resourceId": "database-1", "variable": "x", "role": "predictor" }
        ],
        "candidateMethods": ["yssbi.statistics.ols"],
        "selectedWorkflow": "dataset_quality_review",
        "requiredDiagnostics": ["measurement_scale", "missingness", "outliers", "model_assumptions", "influential_observations", "multiple_testing"],
        "robustnessChecks": [],
        "reportingContract": {
            "requireEffectSizes": true, "requireUncertainty": true,
            "requireDiagnostics": true, "requireLimitations": true, "confidenceLevel": 0.95
        }
    })
}

#[tokio::test]
async fn parallel_history_groups_business_and_delegation_results_before_continuing() {
    let model = ScriptedCompletionModel::new([vec![AssistantContent::text("continued")]]);
    let recorded = model.requests.clone();
    let driver = RigAgentDriver::new(model);
    let call = |id: &str| AgentMessage::ToolCall {
        invocation_id: ToolInvocationId::try_new(id).unwrap(),
        request: AutomationCapabilityRequest::InspectDatasetSchema(InspectDatasetSchemaRequest {
            database_id: id.into(),
        }),
    };
    let result = |id: &str| AgentMessage::ToolResult {
        invocation_id: ToolInvocationId::try_new(id).unwrap(),
        capability_id: CapabilityId::InspectDatasetSchema,
        outcome: Err(CapabilityFailure::new(
            CapabilityFailureCode::DatabaseUnavailable,
        )),
    };
    let run_id = AgentRunId::try_new("review-1").unwrap();
    let mut input = request(vec![]);
    input.messages = vec![
        AgentMessage::User {
            content: "Earlier work".into(),
        },
        call("call-a"),
        call("call-b"),
        result("call-b"),
        AgentMessage::DelegationCall {
            run_id: run_id.clone(),
            task: AgentTask {
                key: "review".into(),
                worker: AgentRole::Review,
                objective: "Review the evidence".into(),
                constraints: "Read only".into(),
                completion_criteria: "Return findings".into(),
                depends_on: vec![],
                scope: AgentTaskScope::default(),
            },
        },
        AgentMessage::Assistant {
            content: "Recorded progress must remain visible.".into(),
        },
        AgentMessage::DelegationResult {
            outcome: Box::new(AgentTaskOutcome {
                run_id,
                role: AgentRole::Review,
                state: AgentRunState::Completed,
                report: Some(WorkerReport {
                    summary: "Reviewed".into(),
                    warnings: vec![],
                    blocked_reason: None,
                    next_steps: vec![],
                }),
                failure_code: None,
                artifacts: vec![],
                results: vec![],
                evidence: vec![],
                plan: None,
                invalidated_runs: vec![],
            }),
        },
        result("call-a"),
        AgentMessage::User {
            content: "Continue with the report".into(),
        },
    ];
    driver
        .run_turn(
            input,
            Arc::new(StaticExecutor),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    let wire = recorded.lock().unwrap()[0]
        .chat_history
        .clone()
        .into_iter()
        .flat_map(|message| {
            Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
        })
        .map(|message| serde_json::to_value(message).unwrap())
        .collect::<Vec<_>>();
    let batch = wire
        .iter()
        .position(|message| message.get("tool_calls").is_some())
        .unwrap();
    let calls = wire[batch]["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 3);
    for (index, call) in calls.iter().enumerate() {
        assert_eq!(wire[batch + index + 1]["role"], "tool");
        assert_eq!(wire[batch + index + 1]["tool_call_id"], call["id"]);
    }
    let failure: serde_json::Value =
        serde_json::from_str(wire[batch + 2]["content"].as_str().unwrap()).unwrap();
    assert_eq!(failure["failure"]["code"], "database_unavailable");
    let review: serde_json::Value =
        serde_json::from_str(wire[batch + 3]["content"].as_str().unwrap()).unwrap();
    assert_eq!(review["report"]["summary"], "Reviewed");
    assert_eq!(wire[batch + 4]["role"], "assistant");
    assert!(
        wire[batch + 4]
            .to_string()
            .contains("Recorded progress must remain visible.")
    );
}

#[test]
fn unfinished_or_unmatched_tool_history_is_rejected_before_provider_submission() {
    let user = || AgentMessage::User {
        content: "Continue".into(),
    };
    for messages in [
        vec![
            user(),
            AgentMessage::ToolResult {
                invocation_id: ToolInvocationId::try_new("unknown-call").unwrap(),
                capability_id: CapabilityId::InspectDatasetSchema,
                outcome: Err(CapabilityFailure::new(
                    CapabilityFailureCode::DatabaseUnavailable,
                )),
            },
            user(),
        ],
        vec![
            user(),
            AgentMessage::ToolCall {
                invocation_id: ToolInvocationId::try_new("unfinished-call").unwrap(),
                request: AutomationCapabilityRequest::InspectDatasetSchema(
                    InspectDatasetSchemaRequest {
                        database_id: "dataset".into(),
                    },
                ),
            },
            user(),
        ],
    ] {
        let failure = crate::messages::prepare_messages(messages).err().unwrap();
        assert_eq!(
            failure.code,
            AgentDriverFailureCode::InvalidProviderResponse
        );
    }
}

#[tokio::test]
async fn plan_rejection_reaches_the_next_model_call_with_actionable_feedback() {
    struct RejectPlan;
    impl AgentEventOutput for RejectPlan {
        fn emit<'a>(
            &'a self,
            event: AgentEvent,
        ) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
            Box::pin(async move {
                if matches!(event, AgentEvent::PlanProposed { .. }) {
                    Err(AgentOutputFailure::PolicyRejected {
                        reason: "requiredDiagnostics is missing multiple_testing".into(),
                        available_methods: Vec::new(),
                    })
                } else {
                    Ok(())
                }
            })
        }
    }
    let model = ScriptedCompletionModel::new([
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "plan-call",
            ToolFunction::new("propose_statistical_plan".into(), sample_plan()),
        ))],
        vec![AssistantContent::text("I will correct the diagnostics.")],
    ]);
    let driver = RigAgentDriver::new(model.clone());
    driver
        .run_turn(
            request(Vec::new()),
            Arc::new(StaticExecutor),
            Arc::new(RejectPlan),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    let requests = model.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let history = serde_json::to_string(&requests[1].chat_history).unwrap();
    assert!(history.contains("requiredDiagnostics is missing multiple_testing"));
    assert!(history.contains("availableMethods"));
}

#[tokio::test]
async fn driver_continues_within_the_host_model_and_output_budgets() {
    let mut turns = (0..20)
        .map(|index| {
            vec![AssistantContent::ToolCall(ToolCall::from_wire(
                format!("plan-{index}"),
                ToolFunction::new("propose_statistical_plan".into(), sample_plan()),
            ))]
        })
        .collect::<Vec<_>>();
    turns.push(vec![AssistantContent::text("Analysis complete.")]);
    let model = ScriptedCompletionModel::new(turns);
    let driver = RigAgentDriver::new(model.clone());
    let result = driver
        .run_turn(
            request(Vec::new()),
            Arc::new(StaticExecutor),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(result.final_text, "Analysis complete.");
    let requests = model.requests.lock().unwrap();
    assert_eq!(requests.len(), 21);
    assert!(
        requests
            .iter()
            .all(|request| request.max_tokens == Some(8192))
    );
}

#[tokio::test]
async fn model_call_budget_exhaustion_has_a_distinct_failure_code() {
    let model =
        ScriptedCompletionModel::new([vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "plan-call",
            ToolFunction::new("propose_statistical_plan".into(), sample_plan()),
        ))]]);
    let driver = RigAgentDriver::new(model.clone());
    let mut input = request(Vec::new());
    input.limits.maximum_model_turns = 1;
    let error = driver
        .run_turn(
            input,
            Arc::new(StaticExecutor),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, AgentDriverFailureCode::ModelTurnLimitExceeded);
    assert_eq!(model.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn rig_driver_maps_typed_tool_calls_and_emits_ordered_events() {
    let plan = sample_plan();
    serde_json::from_value::<StatisticalPlan>(plan.clone())
        .unwrap()
        .validate()
        .unwrap();
    struct ObservingExecutor(Arc<CollectingOutput>);
    impl ModelCapabilityExecutor for ObservingExecutor {
        fn execute<'a>(
            &'a self,
            request: ModelCapabilityRequest,
        ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
            Box::pin(async move {
                let text = self
                    .0
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|event| match event {
                        AgentEvent::TextDelta { delta } => delta.as_str(),
                        _ => "",
                    })
                    .collect::<String>();
                assert_eq!(text, "Checking schema.");
                StaticExecutor.execute(request).await
            })
        }
    }
    let model = ScriptedCompletionModel::new([
        vec![
            AssistantContent::text("Checking schema."),
            AssistantContent::ToolCall(ToolCall::from_wire(
                "call-1",
                ToolFunction::new(
                    "inspect_dataset_schema".to_owned(),
                    serde_json::json!({ "databaseId": "database-1" }),
                ),
            )),
        ],
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "plan-call",
            ToolFunction::new("propose_statistical_plan".into(), plan.clone()),
        ))],
        vec![AssistantContent::text(
            "The schema inspection completed.".to_owned(),
        )],
    ]);
    let requests = model.requests.clone();
    let driver = RigAgentDriver::new(model);
    let output = Arc::new(CollectingOutput::default());

    let result = driver
        .run_turn(
            request(vec![ToolDescriptor::for_capability(
                CapabilityId::InspectDatasetSchema,
            )]),
            Arc::new(ObservingExecutor(output.clone())),
            output.clone(),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    let events = output
        .events
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();

    assert_eq!(result.final_text, "The schema inspection completed.");
    assert!(events.len() > 1);
    assert_eq!(
        events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TextDelta { delta } => Some(delta.as_str()),
                AgentEvent::PlanProposed { .. } => None,
                _ => panic!("unexpected event"),
            })
            .collect::<String>(),
        "Checking schema.\n\nThe schema inspection completed."
    );
    assert!(events.iter().any(|event| matches!(event, AgentEvent::PlanProposed { plan: recorded } if serde_json::to_value(recorded).unwrap() == plan)));
    let requests = requests.lock().unwrap();
    let wire = requests[2]
        .chat_history
        .clone()
        .into_iter()
        .flat_map(|message| {
            Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
        })
        .map(|message| serde_json::to_value(message).unwrap())
        .collect::<Vec<_>>();
    let receipt = wire
        .iter()
        .find(|message| message["tool_call_id"] == "plan-call")
        .unwrap();
    let receipt: serde_json::Value =
        serde_json::from_str(receipt["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        receipt,
        serde_json::json!({ "accepted": true, "plan": plan })
    );
}

#[tokio::test]
async fn argument_diagnostics_reach_the_model_and_allow_correction() {
    let call = |id: &str, name: &str, arguments| {
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            id,
            ToolFunction::new(name.to_owned(), arguments),
        ))]
    };
    let model = ScriptedCompletionModel::new([
        call(
            "malformed",
            "inspect_dataset_schema",
            serde_json::json!({"databaseId": {"secret": "secret"}}),
        ),
        call(
            "malformed-plan",
            "propose_statistical_plan",
            serde_json::json!({"analysisMode": "secret"}),
        ),
        call(
            "corrected",
            "inspect_dataset_schema",
            serde_json::json!({"databaseId": "database-1"}),
        ),
        vec![AssistantContent::text("Corrected.")],
    ]);
    let requests = model.requests.clone();
    let driver = RigAgentDriver::new(model);
    let result = driver
        .run_turn(
            request(vec![ToolDescriptor::for_capability(
                CapabilityId::InspectDatasetSchema,
            )]),
            Arc::new(StaticExecutor),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(result.final_text, "Corrected.");
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    for (index, id, path) in [
        (1, "malformed", "$.databaseId"),
        (2, "malformed-plan", "$.analysisMode"),
    ] {
        let wire = requests[index]
            .chat_history
            .clone()
            .into_iter()
            .flat_map(|message| {
                Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
            })
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>();
        let message = wire
            .iter()
            .find(|message| message["tool_call_id"] == id)
            .unwrap();
        let content = message["content"].as_str().unwrap();
        let feedback: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(feedback["state"], "failed");
        assert_eq!(feedback["failure"]["code"], "invalid_request");
        assert_eq!(feedback["failure"]["details"]["path"], path);
        assert!(!content.contains("secret"));
    }
}

#[tokio::test]
async fn domain_failure_is_structured_feedback_and_the_model_can_correct_its_request() {
    struct CorrectingExecutor;
    impl ModelCapabilityExecutor for CorrectingExecutor {
        fn execute<'a>(
            &'a self,
            request: ModelCapabilityRequest,
        ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
            Box::pin(async move {
                if matches!(&request.request, AutomationCapabilityRequest::InspectDatasetSchema(request) if request.database_id == "missing")
                {
                    return Err(
                        CapabilityFailure::new(CapabilityFailureCode::DatabaseUnavailable)
                            .with_detail("databaseId", "missing"),
                    );
                }
                StaticExecutor.execute(request).await
            })
        }
    }
    let call = |id: &str, database: &str| {
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            id,
            ToolFunction::new(
                "inspect_dataset_schema".into(),
                serde_json::json!({"databaseId": database}),
            ),
        ))]
    };
    let model = ScriptedCompletionModel::new([
        call("first", "missing"),
        call("second", "database-1"),
        vec![AssistantContent::text("Corrected and completed.")],
    ]);
    let requests = model.requests.clone();
    let driver = RigAgentDriver::new(model);
    let result = driver
        .run_turn(
            request(vec![ToolDescriptor::for_capability(
                CapabilityId::InspectDatasetSchema,
            )]),
            Arc::new(CorrectingExecutor),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await;
    assert!(
        result.is_ok(),
        "ordinary tool failure must allow another model step"
    );
    assert_eq!(result.unwrap().final_text, "Corrected and completed.");
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    let wire = requests[1]
        .chat_history
        .clone()
        .into_iter()
        .flat_map(|message| {
            Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
        })
        .map(|message| serde_json::to_value(message).unwrap())
        .collect::<Vec<_>>();
    let tool = wire
        .iter()
        .find(|message| message["role"] == "tool")
        .unwrap();
    let call = wire
        .iter()
        .find(|message| message.get("tool_calls").is_some())
        .unwrap();
    assert_eq!(call["tool_calls"][0]["id"], tool["tool_call_id"]);
    let feedback = serde_json::from_str::<serde_json::Value>(tool["content"].as_str().unwrap());
    assert!(
        feedback.is_ok(),
        "domain failure must use the same structured JSON as replayed history"
    );
    let feedback = feedback.unwrap();
    assert_eq!(feedback["state"], "failed");
    assert_eq!(feedback["failure"]["code"], "database_unavailable");
    assert_eq!(feedback["failure"]["details"]["databaseId"], "missing");
}

#[tokio::test]
async fn fatal_tool_failures_stop_before_another_model_request_and_settle_admitted_work() {
    struct FatalExecutor(Option<CapabilityFailureCode>, Arc<AtomicBool>);
    impl ModelCapabilityExecutor for FatalExecutor {
        fn execute<'a>(
            &'a self,
            _request: ModelCapabilityRequest,
        ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
            Box::pin(async move {
                self.1.store(true, Ordering::SeqCst);
                match self.0 {
                    Some(code) => Err(CapabilityFailure::new(code)),
                    None => panic!("simulated executor failure"),
                }
            })
        }
    }
    for (failure, expected) in [
        (
            Some(CapabilityFailureCode::InternalFailure),
            AgentDriverFailureCode::InternalFailure,
        ),
        (
            Some(CapabilityFailureCode::PersistenceUnavailable),
            AgentDriverFailureCode::InternalFailure,
        ),
        (
            Some(CapabilityFailureCode::Cancelled),
            AgentDriverFailureCode::Cancelled,
        ),
        (
            Some(CapabilityFailureCode::ProjectSessionChanged),
            AgentDriverFailureCode::Cancelled,
        ),
        (
            Some(CapabilityFailureCode::DeadlineElapsed),
            AgentDriverFailureCode::DeadlineElapsed,
        ),
        (None, AgentDriverFailureCode::InternalFailure),
    ] {
        let model = ScriptedCompletionModel::new([
            vec![
                AssistantContent::text("Before tool."),
                AssistantContent::ToolCall(ToolCall::from_wire(
                    "fatal-call",
                    ToolFunction::new(
                        "inspect_dataset_schema".into(),
                        serde_json::json!({"databaseId": "database-1"}),
                    ),
                )),
            ],
            vec![AssistantContent::text("Must not execute this model step.")],
        ]);
        let requests = model.requests.clone();
        let settled = Arc::new(AtomicBool::new(false));
        let output = Arc::new(CollectingOutput::default());
        let driver = RigAgentDriver::new(model);
        let result = driver
            .run_turn(
                request(vec![ToolDescriptor::for_capability(
                    CapabilityId::InspectDatasetSchema,
                )]),
                Arc::new(FatalExecutor(failure, settled.clone())),
                output.clone(),
                CancellationToken::default(),
            )
            .await;
        assert_eq!(result.unwrap_err().code, expected);
        assert!(settled.load(Ordering::SeqCst));
        assert_eq!(
            requests.lock().unwrap().len(),
            1,
            "fatal failure must not reach a subsequent completion request"
        );
        let text = output
            .events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TextDelta { delta } => Some(delta.clone()),
                _ => None,
            })
            .collect::<String>();
        assert_eq!(text, "Before tool.");
    }
}

#[tokio::test]
async fn text_is_published_while_provider_waits_and_pending_text_survives_cancellation() {
    use rig_core::message::Text;
    for cancel in [false, true] {
        let output = Arc::new(CollectingOutput::default());
        let token = CancellationToken::default();
        let (release, wait) = tokio::sync::oneshot::channel::<()>();
        let initial = futures_util::stream::iter([
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::Text(Text::new("开始")),
            )),
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::Text(Text::new("分析")),
            )),
        ]);
        let ending = futures_util::stream::once(async move {
            let _ = wait.await;
            Ok(MultiTurnStreamItem::final_response(
                vec![AssistantContent::text("开始分析")],
                Usage::new(),
            ))
        });
        let (_tool_failure, failure_receiver) = tokio::sync::watch::channel(None);
        let task = tokio::spawn(consume_text_stream(
            Box::pin(initial.chain(ending)),
            output.clone(),
            token.clone(),
            tokio::time::Instant::now() + Duration::from_secs(2),
            failure_receiver,
            false,
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if output
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|event| match event {
                        AgentEvent::TextDelta { delta } => delta.as_str(),
                        _ => "",
                    })
                    .collect::<String>()
                    == "开始分析"
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("text must be published before releasing the provider");
        assert!(!task.is_finished());
        if cancel {
            token.cancel(CancellationReason::User);
        } else {
            release.send(()).unwrap();
        }
        let result = task.await.unwrap();
        if cancel {
            assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
        } else {
            assert_eq!(result.unwrap(), "开始分析");
        }
        let events = output.events.lock().unwrap();
        assert_eq!(events.len(), 2);
    }
    let output = Arc::new(CollectingOutput::default());
    let cancellation = CancellationToken::default();
    let cancel_on_last = cancellation.clone();
    let stream = futures_util::stream::iter(["first", "pending"]).map(move |text| {
        if text == "pending" {
            cancel_on_last.cancel(CancellationReason::User);
        }
        Ok(MultiTurnStreamItem::StreamAssistantItem(
            StreamedAssistantContent::Text(Text::new(text)),
        ))
    });
    let (_tool_failure, failure_receiver) = tokio::sync::watch::channel(None);
    let result = consume_text_stream(
        Box::pin(stream),
        output.clone(),
        cancellation,
        tokio::time::Instant::now() + Duration::from_secs(1),
        failure_receiver,
        false,
    )
    .await;
    assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
    assert_eq!(
        output.events.lock().unwrap().as_slice(),
        &[
            AgentEvent::TextDelta {
                delta: "first".into()
            },
            AgentEvent::TextDelta {
                delta: "pending".into()
            },
        ]
    );
}

#[test]
fn cancellation_token_preserves_the_first_reason() {
    let token = CancellationToken::default();
    assert!(token.cancel(yss_harness_contract::CancellationReason::User));
    assert!(!token.cancel(yss_harness_contract::CancellationReason::DeadlineElapsed));
    assert_eq!(
        token.reason(),
        Some(yss_harness_contract::CancellationReason::User)
    );
}

#[tokio::test]
async fn cancelling_a_model_turn_waits_for_admitted_tool_cleanup() {
    struct WaitingExecutor {
        cancellation: CancellationToken,
        entered: tokio::sync::Notify,
        settled: AtomicBool,
    }
    impl ModelCapabilityExecutor for WaitingExecutor {
        fn execute<'a>(
            &'a self,
            _: ModelCapabilityRequest,
        ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
            Box::pin(async move {
                self.entered.notify_one();
                self.cancellation.cancelled().await;
                tokio::task::yield_now().await;
                self.settled.store(true, Ordering::Release);
                Err(CapabilityFailure::new(CapabilityFailureCode::Cancelled))
            })
        }
    }
    let cancellation = CancellationToken::default();
    let executor = Arc::new(WaitingExecutor {
        cancellation: cancellation.clone(),
        entered: tokio::sync::Notify::new(),
        settled: AtomicBool::new(false),
    });
    let model =
        ScriptedCompletionModel::new([vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "call-1",
            ToolFunction::new(
                "inspect_dataset_schema".into(),
                serde_json::json!({"databaseId": "database-1"}),
            ),
        ))]]);
    let driver = RigAgentDriver::new(model);
    let (result, ()) = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(
            driver.run_turn(
                request(vec![ToolDescriptor::for_capability(
                    CapabilityId::InspectDatasetSchema
                )]),
                executor.clone(),
                Arc::new(CollectingOutput::default()),
                cancellation.clone()
            ),
            async {
                executor.entered.notified().await;
                cancellation.cancel(CancellationReason::User);
            }
        )
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
    assert!(executor.settled.load(Ordering::Acquire));
}

#[tokio::test]
async fn provider_timeout_and_panic_return_terminal_failures() {
    #[derive(Clone)]
    struct BrokenModel {
        panic: bool,
    }
    impl CompletionModel for BrokenModel {
        async fn completion(
            &self,
            _: CompletionRequest,
        ) -> Result<CompletionResponse, CompletionError> {
            assert!(!self.panic, "synthetic provider panic");
            std::future::pending().await
        }
        async fn stream(
            &self,
            _: CompletionRequest,
        ) -> Result<StreamingCompletionResponse, CompletionError> {
            assert!(!self.panic, "synthetic provider panic");
            std::future::pending().await
        }
    }
    for (panic, expected) in [
        (false, AgentDriverFailureCode::DeadlineElapsed),
        (true, AgentDriverFailureCode::InternalFailure),
    ] {
        let driver = RigAgentDriver::new(BrokenModel { panic });
        let mut input = request(Vec::new());
        input.limits.maximum_duration_ms = 20;
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            driver.run_turn(
                input,
                Arc::new(StaticExecutor),
                Arc::new(CollectingOutput::default()),
                CancellationToken::default(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err().code, expected);
    }
}

#[tokio::test]
async fn configurable_driver_starts_unavailable_and_can_be_cleared() {
    let driver = ConfigurableAgentDriver::new();
    assert!(!driver.is_configured());
    assert_eq!(
        driver
            .run_turn(
                request(vec![]),
                Arc::new(StaticExecutor),
                Arc::new(CollectingOutput::default()),
                CancellationToken::default(),
            )
            .await
            .unwrap_err()
            .code,
        AgentDriverFailureCode::ProviderUnavailable
    );
    assert!(
        driver
            .configure(
                "https://api.openai.com/v1".to_owned(),
                "gpt-test".to_owned(),
                Some(SecretCredential::new("test-credential").unwrap()),
            )
            .unwrap()
    );
    assert!(driver.is_configured());
    assert!(
        !driver
            .configure(
                "https://api.openai.com/v1".to_owned(),
                "gpt-test".to_owned(),
                None
            )
            .expect("clearing provider settings is valid")
    );
    assert!(!driver.is_configured());
    assert_eq!(
        driver
            .run_turn(
                request(vec![]),
                Arc::new(StaticExecutor),
                Arc::new(CollectingOutput::default()),
                CancellationToken::default(),
            )
            .await
            .unwrap_err()
            .code,
        AgentDriverFailureCode::ProviderUnavailable
    );
}

#[tokio::test]
async fn configured_provider_uses_chat_completions_and_streams_text() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let driver = openai_agent_driver_with_client(
        SecretCredential::new("test-credential").unwrap(),
        format!("http://{}/v1", listener.local_addr().unwrap()),
        "test-model",
        rig_core::http_client::ReqwestClient::builder()
            .no_proxy()
            .build()
            .unwrap(),
    )
    .unwrap();
    let output = Arc::new(CollectingOutput::default());
    let server = async {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        let mut buffer = [0; 4096];
        let (header_end, content_length) = loop {
            let count = stream.read(&mut buffer).await.unwrap();
            assert!(count > 0, "request ended before headers");
            received.extend_from_slice(&buffer[..count]);
            if let Some(end) = received.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&received[..end]).unwrap();
                assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (end + 4, length);
            }
        };
        while received.len() < header_end + content_length {
            let count = stream.read(&mut buffer).await.unwrap();
            assert!(count > 0, "request ended before body");
            received.extend_from_slice(&buffer[..count]);
        }
        let body: serde_json::Value =
            serde_json::from_slice(&received[header_end..header_end + content_length]).unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["stream"], true);
        assert!(body["messages"].as_array().unwrap().iter().any(|message| {
            message["role"] == "user"
                && message["content"]
                    .to_string()
                    .contains("Inspect the schema.")
        }));
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(
            tools.len(),
            yss_harness_contract::CAPABILITY_DESCRIPTORS.len() + 1
        );
        for tool in tools {
            assert_eq!(tool["type"], "function");
            assert_eq!(
                tool["function"]["parameters"]["type"], "object",
                "{}",
                tool["function"]["name"]
            );
        }
        let inspect_ui = tools
            .iter()
            .find(|tool| tool["function"]["name"] == "inspect_ui")
            .unwrap();
        let original = serde_json::to_value(yss_harness_contract::capability_input_schema(
            CapabilityId::InspectUi,
        ))
        .unwrap();
        assert_eq!(inspect_ui["function"]["parameters"], original);
        assert!(body.get("input").is_none());

        let events = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            events.len(),
            events,
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            driver.run_turn(
                request(
                    yss_harness_contract::CAPABILITY_DESCRIPTORS
                        .iter()
                        .map(|descriptor| { ToolDescriptor::for_capability(descriptor.id) })
                        .collect()
                ),
                Arc::new(StaticExecutor),
                output.clone(),
                CancellationToken::default(),
            ),
            server,
        )
    })
    .await
    .expect("local provider request must complete");
    assert_eq!(result.unwrap().final_text, "Hello");
    assert!(
        output
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| { matches!(event, AgentEvent::TextDelta { delta } if delta == "Hello") })
    );
}

#[tokio::test]
async fn configured_https_provider_attempts_a_tls_handshake() {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let cancellation = CancellationToken::default();
    let client = openai::Client::builder()
        .api_key("test-credential")
        .base_url(format!("https://{}/v1", listener.local_addr().unwrap()))
        .http_client(
            rig_core::http_client::ReqwestClient::builder()
                .no_proxy()
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .completions_api();
    let driver = RigAgentDriver::new(client.completion_model("test-model"));
    let server = async {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut record = [0; 5];
        stream.read_exact(&mut record).await.unwrap();
        // Stop after ClientHello so the probe needs neither a trusted certificate nor an API key.
        cancellation.cancel(yss_harness_contract::CancellationReason::User);
        record
    };
    let (result, record) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(
            driver.run_turn(
                request(vec![]),
                Arc::new(StaticExecutor),
                Arc::new(CollectingOutput::default()),
                cancellation.clone()
            ),
            server,
        )
    })
    .await
    .expect("the HTTPS transport must initiate TLS");
    assert_eq!(&record[..2], &[22, 3]);
    assert_eq!(result.unwrap_err().code, AgentDriverFailureCode::Cancelled);
}
