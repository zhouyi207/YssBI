use crate::error::map_prompt_failure;
mod delegation;
use crate::provider::RigProviderClient;
use crate::stream::consume_text_stream;
use futures_util::StreamExt;
use rig_agent::agent::MultiTurnStreamItem;
use rig_agent::completion::PromptError;
use rig_core::Model;
use rig_core::driver::{Exchange, Opened, Opening, Transport};
use rig_core::error::ProviderError;
use rig_core::operation::Finish;
use rig_core::streaming::{Item, StreamEvent, StreamRecord};
use rig_core::test_utils::{MockCompletionModel, MockFrame, MockScript, MockStreamEvent};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use yss_harness_contract::*;

use std::sync::{Arc, Mutex};

use super::*;
use rig_agent::completion::message::{ToolCall, ToolFunction, ToolName};
use rig_core::completion::{AssistantContent, CompletionRequest, Usage};
use yss_harness_contract::{AgentFuture, AgentOutputFailure};

fn scripted_events(
    choice: Vec<AssistantContent>,
    reason: Option<rig_core::completion::FinishReason>,
) -> Vec<MockStreamEvent> {
    let mut events = Vec::new();
    for part in choice {
        match part {
            AssistantContent::Text(text) => events.extend(
                text.text
                    .chars()
                    .map(|c| MockStreamEvent::text(c.to_string())),
            ),
            AssistantContent::ToolCall(call) => events.push(MockStreamEvent::tool_call(
                call.id.wire(),
                call.function.name.to_string(),
                Value::Object(call.function.arguments),
            )),
            _ => unreachable!(),
        }
    }
    events.push(MockStreamEvent::FinalResponse(Finish {
        reason,
        ..Default::default()
    }));
    events
}

fn scripted_model(turns: impl IntoIterator<Item = Vec<AssistantContent>>) -> MockCompletionModel {
    MockCompletionModel::from_stream_turns(
        turns
            .into_iter()
            .map(|choice| scripted_events(choice, None)),
    )
}

fn text_item(text: &str) -> MultiTurnStreamItem {
    let transcript = StreamRecord::parse(serde_json::json!([
        {"item":"event", "value":{"event":"start", "part":0, "kind":"text"}},
        {"item":"event", "value":{"event":"text", "part":0, "text":text}},
        {"item":"event", "value":{"event":"end", "part":0, "content":{"type":"text", "text":text}}}
    ]))
    .unwrap();
    MultiTurnStreamItem::StreamAssistantItem(Item::Event(
        transcript
            .events()
            .find(|event| matches!(event, StreamEvent::Text { .. }))
            .unwrap()
            .clone(),
    ))
}

fn local_provider_driver(base_url: String) -> Arc<dyn AgentDriverPort> {
    protocol_driver(LanguageModelProtocol::OpenAiChat, base_url)
}

fn protocol_driver(protocol: LanguageModelProtocol, base_url: String) -> Arc<dyn AgentDriverPort> {
    configured_driver(provider_config(protocol, base_url))
}

pub(crate) fn provider_config(
    protocol: LanguageModelProtocol,
    base_url: String,
) -> LanguageModelProviderConfig {
    let adapter = match protocol {
        LanguageModelProtocol::OpenAiChat | LanguageModelProtocol::OpenAiResponses => {
            "openai/openai"
        }
        LanguageModelProtocol::Anthropic => "anthropic/anthropic",
        LanguageModelProtocol::Gemini => "gcp.gemini/gemini",
    };
    LanguageModelProviderConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        custom_name: None,
        protocol,
        base_url,
        adapter: adapter.into(),
        authentication: LanguageModelAuthentication::ApiKey,
        models: Vec::new(),
    }
}

fn configured_driver(config: LanguageModelProviderConfig) -> Arc<dyn AgentDriverPort> {
    RigProviderClient::with_http_client(
        &config,
        Some(&SecretCredential::new("test-credential").unwrap()),
        rig_reqwest::reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap(),
    )
    .unwrap()
    .driver(config.models.first().unwrap_or(&LanguageModelConfig {
        reasoning_efforts: Vec::new(),
        id: "test-model".into(),
        name: "Test model".into(),
        context_window: None,
        max_output_tokens: Some(4096),
        temperature: None,
        top_p: None,
        additional_parameters: Default::default(),
    }))
    .unwrap()
}

pub(crate) const SCHEMA_TOOL: &str = "inspect_database_schema";

/// A fixture tool whose arguments name one database resource.
pub(crate) fn schema_tool() -> ToolSpec {
    tool(
        SCHEMA_TOOL,
        json!({
            "type": "object",
            "properties": {"database": {
                "type": "object",
                "properties": {"kind": {"type": "string"}, "id": {"type": "string"}},
                "required": ["kind", "id"]
            }},
            "required": ["database"]
        }),
    )
}

pub(crate) fn tool(name: &str, schema: Value) -> ToolSpec {
    ToolSpec::new(
        name,
        format!("Fixture {name}."),
        schema,
        ToolEffect::Inspect,
    )
}

fn plan_tool() -> ToolSpec {
    tool("propose_statistical_plan", json!({"type": "object"}))
}

fn schema_output() -> Value {
    json!({"type": "database_read", "payload": {
        "database": {"kind": "database", "id": "database-1"},
        "content": {"kind": "schema", "columns": []}
    }})
}

pub(crate) fn completed(output: Value) -> ToolCallOutcome {
    ToolCallOutcome {
        invocation_id: Some(ToolInvocationId::try_new("tool-1").unwrap()),
        output,
        fatal: None,
    }
}

fn failed(failure: Value) -> ToolCallOutcome {
    ToolCallOutcome {
        invocation_id: None,
        output: json!({"state": "failed", "failure": failure}),
        fatal: None,
    }
}

fn fatal(code: AgentDriverFailureCode) -> ToolCallOutcome {
    ToolCallOutcome {
        invocation_id: None,
        output: Value::Null,
        fatal: Some(code),
    }
}

/// Answers the schema tool with a fixed read and records any other call as accepted.
pub(crate) struct StaticExecutor;

impl AgentToolExecutor for StaticExecutor {
    fn call<'a>(
        &'a self,
        call: yss_harness_contract::ToolCall,
    ) -> AgentFuture<'a, ToolCallOutcome> {
        Box::pin(async move {
            if call.name == SCHEMA_TOOL {
                assert_eq!(call.arguments["database"]["id"], "database-1");
                return completed(schema_output());
            }
            completed(json!({"accepted": true, "tool": call.name}))
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

pub(crate) fn request(mut tools: Vec<ToolSpec>) -> AgentTurnRequest {
    tools.push(plan_tool());
    AgentTurnRequest {
        options: Default::default(),
        role: AgentRoleId::try_new("stats").unwrap(),
        tool_concurrency: 1,
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

fn chat_wire(
    history: impl IntoIterator<Item = impl Into<rig_core::completion::Message>>,
) -> Vec<Value> {
    use rig_core::providers::openai::{OpenAIConfig, wire::Chat};
    use rig_core::wire::{Body, Mode, Wire};
    // 0.44 removed the typed OpenAI messages; encode the real Chat request body instead.
    let request = CompletionRequest::from(history.into_iter().map(Into::into).collect::<Vec<_>>());
    let encoded = Chat::new(OpenAIConfig::new("test"), "test-model")
        .encode(request, Mode::Unary)
        .unwrap();
    let Body::Bytes(bytes) = encoded.request.body() else {
        panic!("Chat requests have a JSON body");
    };
    let mut body: Value = serde_json::from_slice(bytes).unwrap();
    match body["messages"].take() {
        Value::Array(messages) => messages,
        other => panic!("Chat body has no messages: {other}"),
    }
}

#[tokio::test]
async fn delivers_reasoning_and_usage_and_applies_only_explicit_effort_restrictions() {
    let model = MockCompletionModel::from_stream_turns([vec![
        MockStreamEvent::reasoning_delta("Checking evidence"),
        MockStreamEvent::text("Answer"),
        MockStreamEvent::FinalResponse(Finish {
            usage: Usage::new()
                .input_tokens(100)
                .output_tokens(20)
                .total_tokens(120)
                .cached_input_tokens(60)
                .reasoning_tokens(10),
            ..Default::default()
        }),
    ]]);
    let driver = RigAgentDriver::new(model.clone());
    let restricted_driver = RigAgentDriver::new(model.clone()).with_model_config(
        &LanguageModelConfig {
            reasoning_efforts: vec![ReasoningEffort::Low],
            id: "test-model".into(),
            name: "Test model".into(),
            context_window: None,
            max_output_tokens: None,
            temperature: None,
            top_p: None,
            additional_parameters: Default::default(),
        },
        LanguageModelProtocol::OpenAiChat,
    );
    let output = Arc::new(CollectingOutput::default());
    let mut input = request(vec![]);
    input.options.reasoning_effort = Some(ReasoningEffort::High);
    assert_eq!(
        restricted_driver
            .run_turn(
                input.clone(),
                Arc::new(StaticExecutor),
                output.clone(),
                CancellationToken::default()
            )
            .await
            .unwrap_err()
            .code,
        AgentDriverFailureCode::ProviderRequestRejected
    );
    assert!(model.requests().is_empty());
    driver
        .run_turn(
            input,
            Arc::new(StaticExecutor),
            output.clone(),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        model.requests()[0].additional_params,
        Some(serde_json::json!({"reasoning_effort": "high"}))
    );
    let events = output.events.lock().unwrap();
    assert!(
        matches!(&events[0], AgentEvent::ReasoningDelta { delta } if delta == "Checking evidence")
    );
    let usages: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::UsageReported { usage, .. } => Some(usage),
            _ => None,
        })
        .collect();
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].input_tokens, Some(100));
    assert_eq!(usages[0].output_tokens, Some(20));
    assert_eq!(usages[0].cached_input_tokens, Some(60));
    assert_eq!(usages[0].reasoning_tokens, Some(10));
    assert_eq!(usages[0].cache_creation_input_tokens, None);
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
        let error = ProviderError::from_http_response(
            400u16.try_into().unwrap(),
            serde_json::json!({"error": {"code": code}}).to_string(),
        );
        assert_eq!(
            map_prompt_failure(PromptError::Provider(error)).code,
            expected
        );
    }
}

#[tokio::test]
async fn delegation_outcomes_round_trip_and_runs_without_tools_send_none() {
    struct Delegate(std::sync::atomic::AtomicUsize);
    impl AgentToolExecutor for Delegate {
        fn call<'a>(
            &'a self,
            call: yss_harness_contract::ToolCall,
        ) -> AgentFuture<'a, ToolCallOutcome> {
            Box::pin(async move {
                assert_eq!(call.name, DELEGATE_TASK_TOOL);
                assert_eq!(call.arguments["worker"], "review");
                self.0.fetch_add(1, Ordering::AcqRel);
                completed(json!({
                    "runId": "review-run", "state": "completed",
                    "report": {"summary": "Checked"}
                }))
            })
        }
    }
    let model = scripted_model([
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "delegate",
            ToolFunction::new(
                ToolName::new(DELEGATE_TASK_TOOL).unwrap(),
                json!({"worker": "review", "objective": "Check evidence"}),
            ),
        ))],
        vec![AssistantContent::text("Delivered")],
    ]);
    let executor = Arc::new(Delegate(std::sync::atomic::AtomicUsize::new(0)));
    let driver = RigAgentDriver::new(model.clone());
    let mut input = request(vec![tool(DELEGATE_TASK_TOOL, json!({"type": "object"}))]);
    input.role = AgentRoleId::try_new("manager").unwrap();
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
    assert_eq!(executor.0.load(Ordering::Acquire), 1);
    let history = serde_json::to_string(&model.requests()[1].chat_history).unwrap();
    assert!(history.contains("review-run") && history.contains("Checked"));
    let model = scripted_model([vec![AssistantContent::text("Review result")]]);
    let driver = RigAgentDriver::new(model.clone());
    let mut input = request(vec![]);
    input.tools.clear();
    driver
        .run_turn(
            input,
            executor,
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert!(model.requests()[0].tools.is_empty());
}

#[test]
fn message_mapping_preserves_complete_history_and_correlated_tool_messages() {
    let text = "long-history:".to_owned() + &"h".repeat(2_000_000) + ":text-tail";
    let payload = json!({"complete": "r".repeat(2_000_000), "tail": "result-tail"});
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
            invocation_id: "saved-call-1".into(),
            tool: "inspect_result".into(),
            arguments: json!({"resultRef": {"executionSessionId": "session", "resultId": "17"}}),
        },
        AgentMessage::ToolResult {
            invocation_id: "saved-call-1".into(),
            tool: "inspect_result".into(),
            output: json!({"type": "result_inspection", "payload": {"value": {"value": payload}}}),
        },
        AgentMessage::Assistant {
            content: "Previous conclusion.".into(),
        },
        AgentMessage::User {
            content: "Continue.".into(),
        },
    ];
    let prepared = crate::messages::prepare_messages(input.messages).unwrap();
    let wire = chat_wire(
        prepared
            .history
            .into_iter()
            .chain(std::iter::once(prepared.prompt)),
    );
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
    let value: Value = serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
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
async fn parallel_history_groups_tool_and_delegation_results_before_continuing() {
    let model = scripted_model([vec![AssistantContent::text("continued")]]);
    let recorded = model.clone();
    let driver = RigAgentDriver::new(model);
    let call = |id: &str| AgentMessage::ToolCall {
        invocation_id: id.into(),
        tool: SCHEMA_TOOL.into(),
        arguments: json!({"database": {"kind": "database", "id": id}}),
    };
    let result = |id: &str| AgentMessage::ToolResult {
        invocation_id: id.into(),
        tool: SCHEMA_TOOL.into(),
        output: json!({"state": "failed", "failure": {"code": "database_unavailable", "details": {}}}),
    };
    let mut input = request(vec![]);
    input.messages = vec![
        AgentMessage::User {
            content: "Earlier work".into(),
        },
        call("call-a"),
        call("call-b"),
        result("call-b"),
        AgentMessage::ToolCall {
            invocation_id: "review-1".into(),
            tool: DELEGATE_TASK_TOOL.into(),
            arguments: json!({
                "worker": "review", "objective": "Review the evidence",
                "scope": {"resources": [{"resource": {"kind": "doc", "id": "docs/report.md"}, "operations": ["inspect"]}]}
            }),
        },
        AgentMessage::Assistant {
            content: "Recorded progress must remain visible.".into(),
        },
        AgentMessage::ToolResult {
            invocation_id: "review-1".into(),
            tool: DELEGATE_TASK_TOOL.into(),
            output: json!({
                "runId": "review-1", "report": {"summary": "Reviewed"},
                "artifacts": [{"resource": {"kind": "doc", "id": "docs/report.md"}, "deleted": false}]
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
    let wire = chat_wire(recorded.requests()[0].chat_history.clone());
    let batch = wire
        .iter()
        .position(|message| message.get("tool_calls").is_some())
        .unwrap();
    let calls = wire[batch]["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 3);
    let delegated: Value =
        serde_json::from_str(calls[2]["function"]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(
        delegated["scope"]["resources"][0]["operations"],
        json!(["inspect"])
    );
    for (index, call) in calls.iter().enumerate() {
        assert_eq!(wire[batch + index + 1]["role"], "tool");
        assert_eq!(wire[batch + index + 1]["tool_call_id"], call["id"]);
    }
    let failure: Value =
        serde_json::from_str(wire[batch + 2]["content"].as_str().unwrap()).unwrap();
    assert_eq!(failure["failure"]["code"], "database_unavailable");
    let review: Value = serde_json::from_str(wire[batch + 3]["content"].as_str().unwrap()).unwrap();
    assert_eq!(review["report"]["summary"], "Reviewed");
    assert_eq!(review["artifacts"][0]["resource"]["id"], "docs/report.md");
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
                invocation_id: "unknown-call".into(),
                tool: SCHEMA_TOOL.into(),
                output: json!({"state": "failed"}),
            },
            user(),
        ],
        vec![
            user(),
            AgentMessage::ToolCall {
                invocation_id: "unfinished-call".into(),
                tool: SCHEMA_TOOL.into(),
                arguments: json!({"database": {"kind": "database", "id": "dataset"}}),
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
async fn driver_continues_past_previous_model_and_output_limits() {
    let mut turns = (0..40)
        .map(|index| {
            vec![AssistantContent::ToolCall(ToolCall::from_wire(
                format!("plan-{index}"),
                ToolFunction::new(
                    ToolName::new("propose_statistical_plan").unwrap(),
                    sample_plan(),
                ),
            ))]
        })
        .collect::<Vec<_>>();
    let final_text = "Verified report section.\n".repeat(1500);
    turns.push(vec![AssistantContent::text(final_text.clone())]);
    let model = scripted_model(turns);
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
    assert_eq!(result.final_text, final_text);
    let requests = model.requests();
    assert_eq!(requests.len(), 41);
    assert!(requests.iter().all(|request| request.max_tokens.is_none()));
}

#[tokio::test]
async fn incomplete_model_responses_have_distinct_failures_and_preserve_progress() {
    use rig_core::completion::FinishReason;
    struct UnusedExecutor;
    impl AgentToolExecutor for UnusedExecutor {
        fn call<'a>(
            &'a self,
            _: yss_harness_contract::ToolCall,
        ) -> AgentFuture<'a, ToolCallOutcome> {
            Box::pin(async { panic!("a truncated model response must not execute its tool calls") })
        }
    }
    // Exercise the actual Rig loop as well as its public stream surface below.
    for content in [
        vec![AssistantContent::text("partial answer")],
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "cut-short",
            ToolFunction::new(
                ToolName::new("inspect_database_schema").unwrap(),
                serde_json::json!({"database":{"kind":"database","id":"database-1"}}),
            ),
        ))],
        vec![],
    ] {
        let model = MockCompletionModel::from_stream_turns([scripted_events(
            content,
            Some(FinishReason::Length),
        )]);
        let result = RigAgentDriver::new(model)
            .run_turn(
                request(vec![schema_tool()]),
                Arc::new(UnusedExecutor),
                Arc::new(CollectingOutput::default()),
                CancellationToken::default(),
            )
            .await;
        assert_eq!(
            result.unwrap_err().code,
            AgentDriverFailureCode::ProviderOutputTruncated
        );
    }
    for (reason, expected) in [
        (
            Some(FinishReason::Length),
            AgentDriverFailureCode::ProviderOutputTruncated,
        ),
        (
            Some(FinishReason::ContentFilter),
            AgentDriverFailureCode::ProviderContentFiltered,
        ),
        (None, AgentDriverFailureCode::ProviderStreamInterrupted),
    ] {
        let mut items = vec![Ok(text_item("partial progress"))];
        if let Some(reason) = reason {
            items.push(Ok(MultiTurnStreamItem::CompletionCall(
                rig_agent::agent::CompletionCall::new(0, Usage::default(), serde_json::Value::Null)
                    .with_finish_reason(Some(reason)),
            )));
            items.push(Ok(MultiTurnStreamItem::final_response(
                vec![AssistantContent::text("must not become a completed answer")],
                Usage::default(),
            )));
        }
        let output = Arc::new(CollectingOutput::default());
        let (_sender, receiver) = tokio::sync::watch::channel(None);
        let error = consume_text_stream(
            Box::pin(futures_util::stream::iter(items)),
            output.clone(),
            CancellationToken::default(),
            receiver,
            true,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, expected);
        let mut expected_events = vec![AgentEvent::TextDelta {
            delta: "partial progress".into(),
        }];
        if expected != AgentDriverFailureCode::ProviderStreamInterrupted {
            expected_events.push(crate::run_options::usage_event(
                Usage::default(),
                None,
                ModelCallPurpose::Response,
            ));
        }
        assert_eq!(*output.events.lock().unwrap(), expected_events);
    }
    assert_eq!(
        map_prompt_failure(PromptError::Provider(ProviderError::Truncated)).code,
        AgentDriverFailureCode::ProviderStreamInterrupted
    );
    assert_eq!(
        map_prompt_failure(PromptError::Provider(
            ProviderError::from_http_response(402u16.try_into().unwrap(), "payment required"),
        ))
        .code,
        AgentDriverFailureCode::ProviderPaymentRequired
    );
}

#[tokio::test]
async fn rig_driver_passes_tool_calls_through_and_emits_ordered_events() {
    struct ObservingExecutor(Arc<CollectingOutput>);
    impl AgentToolExecutor for ObservingExecutor {
        fn call<'a>(
            &'a self,
            call: yss_harness_contract::ToolCall,
        ) -> AgentFuture<'a, ToolCallOutcome> {
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
                StaticExecutor.call(call).await
            })
        }
    }
    let plan = sample_plan();
    let model = scripted_model([
        vec![
            AssistantContent::text("Checking schema."),
            AssistantContent::ToolCall(ToolCall::from_wire(
                "call-1",
                ToolFunction::new(
                    ToolName::new(SCHEMA_TOOL).unwrap(),
                    json!({ "database":{"kind":"database","id":"database-1"} }),
                ),
            )),
        ],
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            "plan-call",
            ToolFunction::new(
                ToolName::new("propose_statistical_plan").unwrap(),
                plan.clone(),
            ),
        ))],
        vec![AssistantContent::text(
            "The schema inspection completed.".to_owned(),
        )],
    ]);
    let requests = model.clone();
    let driver = RigAgentDriver::new(model);
    let output = Arc::new(CollectingOutput::default());

    let result = driver
        .run_turn(
            request(vec![schema_tool()]),
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
                AgentEvent::UsageReported { .. } => None,
                _ => panic!("unexpected event"),
            })
            .collect::<String>(),
        "Checking schema.\n\nThe schema inspection completed."
    );
    let wire = chat_wire(requests.requests()[2].chat_history.clone());
    let receipt = wire
        .iter()
        .find(|message| message["tool_call_id"] == "plan-call")
        .unwrap();
    let receipt: Value = serde_json::from_str(receipt["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        receipt,
        json!({ "accepted": true, "tool": "propose_statistical_plan" })
    );
}

#[tokio::test]
async fn unavailable_tools_return_feedback_without_dispatch_or_replaying_completed_work() {
    #[derive(Default)]
    struct CountingExecutor(std::sync::atomic::AtomicUsize);
    impl AgentToolExecutor for CountingExecutor {
        fn call<'a>(
            &'a self,
            call: yss_harness_contract::ToolCall,
        ) -> AgentFuture<'a, ToolCallOutcome> {
            self.0.fetch_add(1, Ordering::SeqCst);
            StaticExecutor.call(call)
        }
    }
    let call = |id: &str, name: &str, arguments| {
        AssistantContent::ToolCall(ToolCall::from_wire(
            id,
            ToolFunction::new(ToolName::new(name).unwrap(), arguments),
        ))
    };
    let valid = || {
        call(
            "valid",
            "inspect_database_schema",
            serde_json::json!({"database":{"kind":"database","id":"database-1"}}),
        )
    };
    let model = scripted_model([
        vec![valid()],
        vec![call(
            "unavailable",
            "delegate_task",
            serde_json::json!({"private":"secret"}),
        )],
        vec![AssistantContent::text(
            "The requested worker is not available to this role.",
        )],
    ]);
    let executor = Arc::new(CountingExecutor::default());
    let result = RigAgentDriver::new(model.clone())
        .run_turn(
            request(vec![schema_tool()]),
            executor.clone(),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        result.final_text,
        "The requested worker is not available to this role."
    );
    assert_eq!(executor.0.load(Ordering::SeqCst), 1);
    let requests = model.requests();
    assert_eq!(requests.len(), 3);
    let wire = chat_wire(requests[2].chat_history.clone());
    let feedback = wire
        .iter()
        .find(|message| message["tool_call_id"] == "unavailable")
        .unwrap();
    let text = feedback["content"].as_str().unwrap();
    let failure: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        failure["failure"]["details"]["reason"],
        "tool_not_available"
    );
    assert!(
        !failure["failure"]["details"]["allowedTools"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("delegate_task"))
    );
    assert!(!text.contains("secret"));

    // A valid-looking sibling in an abandoned streamed response must not execute.
    let model = scripted_model([
        vec![
            valid(),
            call("missing", "unknown_tool", serde_json::json!({})),
        ],
        vec![valid()],
        vec![AssistantContent::text("Corrected.")],
    ]);
    let executor = Arc::new(CountingExecutor::default());
    let result = RigAgentDriver::new(model)
        .run_turn(
            request(vec![schema_tool()]),
            executor.clone(),
            Arc::new(CollectingOutput::default()),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(result.final_text, "Corrected.");
    assert_eq!(executor.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn domain_failure_is_structured_feedback_and_the_model_can_correct_its_request() {
    struct CorrectingExecutor;
    impl AgentToolExecutor for CorrectingExecutor {
        fn call<'a>(
            &'a self,
            call: yss_harness_contract::ToolCall,
        ) -> AgentFuture<'a, ToolCallOutcome> {
            Box::pin(async move {
                if call.arguments["database"]["id"] == "missing" {
                    return failed(json!({
                        "code": "database_unavailable", "details": {"databaseId": "missing"}
                    }));
                }
                StaticExecutor.call(call).await
            })
        }
    }
    let call = |id: &str, database: &str| {
        vec![AssistantContent::ToolCall(ToolCall::from_wire(
            id,
            ToolFunction::new(
                ToolName::new(SCHEMA_TOOL).unwrap(),
                json!({"database":{"kind":"database","id":database}}),
            ),
        ))]
    };
    let model = scripted_model([
        call("first", "missing"),
        call("second", "database-1"),
        vec![AssistantContent::text("Corrected and completed.")],
    ]);
    let requests = model.clone();
    let driver = RigAgentDriver::new(model);
    let result = driver
        .run_turn(
            request(vec![schema_tool()]),
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
    let requests = requests.requests();
    assert_eq!(requests.len(), 3);
    let wire = chat_wire(requests[1].chat_history.clone());
    let tool = wire
        .iter()
        .find(|message| message["role"] == "tool")
        .unwrap();
    let call = wire
        .iter()
        .find(|message| message.get("tool_calls").is_some())
        .unwrap();
    assert_eq!(call["tool_calls"][0]["id"], tool["tool_call_id"]);
    let feedback = serde_json::from_str::<Value>(tool["content"].as_str().unwrap());
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
    struct FatalExecutor(Option<AgentDriverFailureCode>, Arc<AtomicBool>);
    impl AgentToolExecutor for FatalExecutor {
        fn call<'a>(
            &'a self,
            _call: yss_harness_contract::ToolCall,
        ) -> AgentFuture<'a, ToolCallOutcome> {
            Box::pin(async move {
                self.1.store(true, Ordering::SeqCst);
                match self.0 {
                    Some(code) => fatal(code),
                    None => panic!("simulated executor failure"),
                }
            })
        }
    }
    for (failure, expected) in [
        (
            Some(AgentDriverFailureCode::InternalFailure),
            AgentDriverFailureCode::InternalFailure,
        ),
        (
            Some(AgentDriverFailureCode::Cancelled),
            AgentDriverFailureCode::Cancelled,
        ),
        (
            Some(AgentDriverFailureCode::DeadlineElapsed),
            AgentDriverFailureCode::DeadlineElapsed,
        ),
        (None, AgentDriverFailureCode::InternalFailure),
    ] {
        let model = scripted_model([
            vec![
                AssistantContent::text("Before tool."),
                AssistantContent::ToolCall(ToolCall::from_wire(
                    "fatal-call",
                    ToolFunction::new(
                        ToolName::new(SCHEMA_TOOL).unwrap(),
                        json!({"database":{"kind":"database","id":"database-1"}}),
                    ),
                )),
            ],
            vec![AssistantContent::text("Must not execute this model step.")],
        ]);
        let requests = model.clone();
        let settled = Arc::new(AtomicBool::new(false));
        let output = Arc::new(CollectingOutput::default());
        let driver = RigAgentDriver::new(model);
        let result = driver
            .run_turn(
                request(vec![schema_tool()]),
                Arc::new(FatalExecutor(failure, settled.clone())),
                output.clone(),
                CancellationToken::default(),
            )
            .await;
        assert_eq!(result.unwrap_err().code, expected);
        assert!(settled.load(Ordering::SeqCst));
        assert_eq!(
            requests.request_count(),
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
    for cancel in [false, true] {
        let output = Arc::new(CollectingOutput::default());
        let token = CancellationToken::default();
        let (release, wait) = tokio::sync::oneshot::channel::<()>();
        let initial = futures_util::stream::iter([Ok(text_item("开始")), Ok(text_item("分析"))]);
        let ending = futures_util::stream::once(async move {
            let _ = wait.await;
            Ok(MultiTurnStreamItem::final_response(
                vec![AssistantContent::text("开始分析")],
                Usage::default(),
            ))
        });
        let (_tool_failure, failure_receiver) = tokio::sync::watch::channel(None);
        let task = tokio::spawn(consume_text_stream(
            Box::pin(initial.chain(ending)),
            output.clone(),
            token.clone(),
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
    let stream = futures_util::stream::iter([Ok(text_item("first")), Ok(text_item("pending"))])
        .inspect(move |item| {
            if matches!(item, Ok(MultiTurnStreamItem::StreamAssistantItem(Item::Event(StreamEvent::Text { text, .. }))) if text == "pending") {
                cancel_on_last.cancel(CancellationReason::User);
            }
        });
    let (_tool_failure, failure_receiver) = tokio::sync::watch::channel(None);
    let result = consume_text_stream(
        Box::pin(stream),
        output.clone(),
        cancellation,
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
    use yss_harness_contract::CancellationReason;

    for reason in [
        CancellationReason::User,
        CancellationReason::ProjectReplaced,
        CancellationReason::DeadlineElapsed,
    ] {
        let token = CancellationToken::default();
        assert_eq!(token.reason(), None);
        assert!(token.cancel(reason));
        assert!(!token.cancel(CancellationReason::User));
        assert!(!token.cancel(CancellationReason::DeadlineElapsed));
        assert_eq!(token.reason(), Some(reason));
    }
}

#[tokio::test]
async fn cancelling_a_model_turn_waits_for_admitted_tool_cleanup() {
    struct WaitingExecutor {
        cancellation: CancellationToken,
        entered: tokio::sync::Notify,
        settled: AtomicBool,
    }
    impl AgentToolExecutor for WaitingExecutor {
        fn call<'a>(
            &'a self,
            _: yss_harness_contract::ToolCall,
        ) -> AgentFuture<'a, ToolCallOutcome> {
            Box::pin(async move {
                self.entered.notify_one();
                self.cancellation.cancelled().await;
                tokio::task::yield_now().await;
                self.settled.store(true, Ordering::Release);
                fatal(AgentDriverFailureCode::Cancelled)
            })
        }
    }
    let cancellation = CancellationToken::default();
    let executor = Arc::new(WaitingExecutor {
        cancellation: cancellation.clone(),
        entered: tokio::sync::Notify::new(),
        settled: AtomicBool::new(false),
    });
    let model = scripted_model([vec![AssistantContent::ToolCall(ToolCall::from_wire(
        "call-1",
        ToolFunction::new(
            ToolName::new("inspect_database_schema").unwrap(),
            serde_json::json!({"database":{"kind":"database","id":"database-1"}}),
        ),
    ))]]);
    let driver = RigAgentDriver::new(model);
    let (result, ()) = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(
            driver.run_turn(
                request(vec![schema_tool()]),
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
async fn stalled_provider_remains_cancellable_and_panic_is_a_terminal_failure() {
    #[derive(Clone)]
    struct BrokenModel {
        panic: bool,
    }
    impl Transport<MockScript> for BrokenModel {
        fn send(&self, _: CompletionRequest, _: Exchange) -> Opening<MockFrame> {
            let panic = self.panic;
            Opening::new(async move {
                assert!(!panic, "synthetic provider panic");
                std::future::pending().await
            })
        }
    }
    for (panic, expected) in [
        (false, AgentDriverFailureCode::Cancelled),
        (true, AgentDriverFailureCode::InternalFailure),
    ] {
        let driver = RigAgentDriver::new(Model::new(MockScript::default(), BrokenModel { panic }));
        let input = request(Vec::new());
        let cancellation = CancellationToken::default();
        let cancel = cancellation.clone();
        let stop = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            cancel.cancel(CancellationReason::User);
        });
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            driver.run_turn(
                input,
                Arc::new(StaticExecutor),
                Arc::new(CollectingOutput::default()),
                cancellation,
            ),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err().code, expected);
        stop.await.unwrap();
    }
}

#[test]
fn native_provider_configuration_rejects_invalid_urls_and_models() {
    let credential = SecretCredential::new("test-key").unwrap();
    for url in [
        "",
        "file:///v1",
        "https://user:secret@example.com/v1",
        "https://example.com/v1?key=secret",
    ] {
        assert!(
            RigProviderClient::new(
                &provider_config(LanguageModelProtocol::OpenAiChat, url.into()),
                Some(&credential)
            )
            .is_err()
        );
    }
    let mut ids = std::collections::BTreeSet::new();
    for preset in crate::provider_presets().unwrap() {
        assert!(ids.insert(preset.id.clone()));
        let config = LanguageModelProviderConfig {
            id: preset.id,
            name: preset.name,
            custom_name: None,
            protocol: preset.protocol,
            adapter: preset.adapter,
            authentication: preset.authentication,
            base_url: if preset.base_url.is_empty() {
                "https://example.com".into()
            } else {
                preset.base_url
            },
            models: vec![],
        };
        let key =
            (config.authentication == LanguageModelAuthentication::ApiKey).then_some(&credential);
        let client = RigProviderClient::new(&config, key).unwrap();
        let mut model = LanguageModelConfig {
            reasoning_efforts: Vec::new(),
            id: "model".into(),
            name: "Model".into(),
            context_window: Some(32_000),
            max_output_tokens: Some(8_000),
            temperature: None,
            top_p: None,
            additional_parameters: Default::default(),
        };
        assert!(client.driver(&model).is_ok());
        model
            .additional_parameters
            .insert("messages".into(), serde_json::json!([]));
        assert!(client.driver(&model).is_err());
        model.additional_parameters.clear();
        model.top_p = Some(1.1);
        assert!(client.driver(&model).is_err());
        model.top_p = None;
        model.id.clear();
        assert!(client.driver(&model).is_err());
    }
}

fn provider_stream(protocol: LanguageModelProtocol) -> String {
    use serde_json::json;
    let frames = match protocol {
        LanguageModelProtocol::Gemini => vec![
            json!({"event_type":"step.start","index":0,"step":{"type":"model_output","content":[]}}),
            json!({"event_type":"step.delta","index":0,"delta":{"type":"text","text":"Hello"}}),
            json!({"event_type":"step.stop","index":0}),
            json!({"event_type":"interaction.completed","interaction":{"id":"int_1","status":"completed","usage":{"total_input_tokens":4,"total_output_tokens":1,"total_tokens":5}}}),
        ],
        LanguageModelProtocol::Anthropic => vec![
            json!({"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"test-model","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":4,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":1}}),
            json!({"type":"message_stop"}),
        ],
        LanguageModelProtocol::OpenAiResponses => vec![
            json!({"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"content_index":0,"sequence_number":1,"delta":"Hello"}),
            json!({"type":"response.output_text.done","item_id":"msg_1","output_index":0,"content_index":0,"sequence_number":2,"text":"Hello"}),
            json!({"type":"response.completed","sequence_number":3,"response":{"id":"resp_1","object":"response","created_at":0,"status":"completed","model":"test-model","output":[{"id":"msg_1","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Hello","annotations":[]}]}],"usage":{"input_tokens":4,"output_tokens":1,"total_tokens":5}}}),
        ],
        _ => vec![
            json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}),
            json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
        ],
    };
    let mut events: String = frames
        .iter()
        .map(|frame| format!("data: {frame}\n\n"))
        .collect();
    if matches!(protocol, LanguageModelProtocol::OpenAiChat) {
        events.push_str("data: [DONE]\n\n");
    }
    events
}

fn ui_intent_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"id": {"type": "string", "description": "Exact intent receipt."}},
        "required": ["id"],
        "additionalProperties": false
    })
}

fn protocol_tools() -> Vec<ToolSpec> {
    vec![schema_tool(), tool("inspect_ui_intent", ui_intent_schema())]
}

#[tokio::test]
async fn native_provider_protocols_preserve_routes_auth_tools_and_streaming() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for (protocol, adapter, authentication, endpoint) in [
        (
            LanguageModelProtocol::OpenAiChat,
            "openai/openai",
            LanguageModelAuthentication::ApiKey,
            "/v1/chat/completions",
        ),
        (
            LanguageModelProtocol::OpenAiChat,
            "moonshot/openai",
            LanguageModelAuthentication::ApiKey,
            "/v1/chat/completions",
        ),
        (
            LanguageModelProtocol::OpenAiResponses,
            "openai/openai",
            LanguageModelAuthentication::ApiKey,
            "/v1/responses",
        ),
        (
            LanguageModelProtocol::Anthropic,
            "anthropic/anthropic",
            LanguageModelAuthentication::ApiKey,
            "/v1/messages",
        ),
        (
            LanguageModelProtocol::Gemini,
            "gcp.gemini/gemini",
            LanguageModelAuthentication::ApiKey,
            "/v1beta/interactions?alt=sse",
        ),
        (
            LanguageModelProtocol::OpenAiChat,
            "openai/openai",
            LanguageModelAuthentication::None,
            "/v1/chat/completions",
        ),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let suffix = if matches!(
            protocol,
            LanguageModelProtocol::Anthropic | LanguageModelProtocol::Gemini
        ) {
            ""
        } else {
            "/v1"
        };
        let mut config = provider_config(
            protocol,
            format!("http://{}{suffix}", listener.local_addr().unwrap()),
        );
        config.adapter = adapter.into();
        config.authentication = authentication;
        let parameters = match protocol {
            LanguageModelProtocol::Gemini => {
                serde_json::json!({"generation_config": {"seed": 7, "thinking_level": "medium"}})
            }
            LanguageModelProtocol::Anthropic => {
                serde_json::json!({"stop_sequences": ["HALT"], "output_config": {"effort": "low"}})
            }
            LanguageModelProtocol::OpenAiResponses => {
                serde_json::json!({"reasoning": {"effort": "low"}})
            }
            LanguageModelProtocol::OpenAiChat => {
                serde_json::json!({"seed": 7, "reasoning_effort": "medium"})
            }
        };
        config.models.push(LanguageModelConfig {
            reasoning_efforts: Vec::new(),
            id: "test-model".into(),
            name: "Test model".into(),
            context_window: None,
            max_output_tokens: Some(4096),
            temperature: Some(0.5),
            top_p: Some(0.8),
            additional_parameters: parameters.as_object().unwrap().clone(),
        });
        assert_eq!(
            crate::model_default_reasoning_effort(&config.models[0], protocol),
            Some(match protocol {
                LanguageModelProtocol::Gemini | LanguageModelProtocol::OpenAiChat =>
                    ReasoningEffort::Medium,
                LanguageModelProtocol::Anthropic | LanguageModelProtocol::OpenAiResponses =>
                    ReasoningEffort::Low,
            })
        );
        let driver = configured_driver(config);
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
                    assert!(
                        headers.starts_with(&format!("POST {endpoint} HTTP/1.1\r\n")),
                        "{protocol:?}: {headers}"
                    );
                    let headers = headers.to_ascii_lowercase();
                    if authentication == LanguageModelAuthentication::None {
                        assert!(
                            !headers.contains("authorization:") && !headers.contains("api-key:")
                        );
                    } else if protocol == LanguageModelProtocol::Anthropic {
                        assert!(headers.contains("x-api-key: test-credential"));
                        assert!(headers.contains("anthropic-version:"));
                    } else if protocol == LanguageModelProtocol::Gemini {
                        assert!(headers.contains("x-goog-api-key: test-credential"));
                    } else {
                        assert!(headers.contains("authorization: bearer test-credential"));
                    }
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
            if protocol == LanguageModelProtocol::Gemini {
                assert_eq!(body["store"], false);
                assert_eq!(body["generation_config"]["max_output_tokens"], 4096);
            }
            let sampling = if protocol == LanguageModelProtocol::Gemini {
                &body["generation_config"]
            } else {
                &body
            };
            assert_eq!(sampling["temperature"], 0.5);
            assert_eq!(sampling["top_p"], 0.8);
            match protocol {
                LanguageModelProtocol::Gemini => assert_eq!(sampling["seed"], 7),
                LanguageModelProtocol::Anthropic => {
                    assert_eq!(body["stop_sequences"], parameters["stop_sequences"])
                }
                LanguageModelProtocol::OpenAiResponses => {
                    assert_eq!(body["reasoning"]["effort"], "high")
                }
                LanguageModelProtocol::OpenAiChat => assert_eq!(body["seed"], 7),
            }
            match protocol {
                LanguageModelProtocol::OpenAiResponses => {
                    assert_eq!(body["reasoning"]["effort"], "high")
                }
                LanguageModelProtocol::OpenAiChat => assert_eq!(body["reasoning_effort"], "high"),
                LanguageModelProtocol::Anthropic => {
                    assert_eq!(body["output_config"]["effort"], "high")
                }
                LanguageModelProtocol::Gemini => {
                    assert_eq!(body["generation_config"]["thinking_level"], "high")
                }
            }
            assert!(
                body[if matches!(
                    protocol,
                    LanguageModelProtocol::OpenAiResponses | LanguageModelProtocol::Gemini
                ) {
                    "input"
                } else {
                    "messages"
                }]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| {
                    (message["role"] == "user" || message["type"] == "user_input")
                        && message["content"]
                            .to_string()
                            .contains("Inspect the schema.")
                })
            );
            let tools = body["tools"].as_array().unwrap();
            let specs = protocol_tools();
            assert_eq!(tools.len(), specs.len() + 1);
            for tool in tools {
                let parameters = match protocol {
                    LanguageModelProtocol::Anthropic => &tool["input_schema"],
                    LanguageModelProtocol::OpenAiResponses | LanguageModelProtocol::Gemini => {
                        &tool["parameters"]
                    }
                    _ => &tool["function"]["parameters"],
                };
                assert_eq!(parameters["type"], "object", "{protocol:?}: {tool}");
                if let Some(spec) = specs
                    .iter()
                    .find(|spec| tool["name"] == spec.name || tool["function"]["name"] == spec.name)
                {
                    assert_eq!(*parameters, spec.input_schema);
                }
            }
            let inspect = tools
                .iter()
                .find(|tool| {
                    tool["name"] == "inspect_ui_intent"
                        || tool["function"]["name"] == "inspect_ui_intent"
                })
                .unwrap();
            let parameters = match protocol {
                LanguageModelProtocol::Anthropic => &inspect["input_schema"],
                LanguageModelProtocol::OpenAiResponses | LanguageModelProtocol::Gemini => {
                    &inspect["parameters"]
                }
                _ => &inspect["function"]["parameters"],
            };
            assert_eq!(*parameters, ui_intent_schema());
            if protocol == LanguageModelProtocol::Anthropic {
                assert_eq!(body["max_tokens"], 4096);
            }

            let events = provider_stream(protocol);
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
                    {
                        let mut turn = request(protocol_tools());
                        turn.options.reasoning_effort = Some(ReasoningEffort::High);
                        turn
                    },
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
        let usage = output
            .events
            .lock()
            .unwrap()
            .iter()
            .find_map(|event| match event {
                AgentEvent::UsageReported { usage, .. } => Some(usage.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            usage.input_tokens,
            if protocol == LanguageModelProtocol::OpenAiChat {
                None
            } else {
                Some(4)
            }
        );
        assert!(
            output.events.lock().unwrap().iter().any(|event| {
                matches!(event, AgentEvent::TextDelta { delta } if delta == "Hello")
            })
        );
    }
}

#[tokio::test]
async fn configured_https_provider_attempts_a_tls_handshake() {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let cancellation = CancellationToken::default();
    let driver = local_provider_driver(format!("https://{}/v1", listener.local_addr().unwrap()));
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

#[derive(Default)]
struct WritingExecutor(std::sync::atomic::AtomicUsize, bool);
impl AgentToolExecutor for WritingExecutor {
    fn call<'a>(
        &'a self,
        call: yss_harness_contract::ToolCall,
    ) -> AgentFuture<'a, ToolCallOutcome> {
        Box::pin(async move {
            assert_eq!(call.name, "create_resource");
            self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            completed(json!({"type": "resource_managed", "payload": {
                "changes": [{"resource": {"kind": "doc", "id": "docs/retry-proof.md"}, "deleted": false}]
            }}))
        })
    }
    fn completion_feedback(&self) -> Option<String> {
        (self.1 && self.0.load(std::sync::atomic::Ordering::Acquire) == 0).then(|| "The requested document has not been delivered. Complete the authorized write or explain its blocker.".into())
    }
}
fn document_tool() -> ToolSpec {
    tool("create_resource", json!({"type": "object"}))
}
fn document_call() -> AssistantContent {
    AssistantContent::ToolCall(ToolCall::from_wire(
        "write-doc",
        ToolFunction::new(
            ToolName::new("create_resource").unwrap(),
            json!({"kind":"doc", "name":"Retry proof"}),
        ),
    ))
}
#[derive(Clone)]
struct InterruptedModel {
    inner: MockCompletionModel,
    requests: Arc<Mutex<Vec<CompletionRequest>>>,
}
impl Transport<MockScript> for InterruptedModel {
    fn send(&self, request: CompletionRequest, exchange: Exchange) -> Opening<MockFrame> {
        let interrupt = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request.clone());
            requests.len() == 2
        };
        if interrupt {
            return Opening::ready(Opened::new(futures_util::stream::iter([
                Ok(MockFrame::Event(MockStreamEvent::reasoning_delta(
                    "Partial reasoning before interruption",
                ))),
                Ok(MockFrame::Event(MockStreamEvent::text(
                    "Interrupted partial response",
                ))),
                Err(ProviderError::Truncated),
            ])));
        }
        self.inner.transport.send(request, exchange)
    }
}
#[tokio::test]
async fn reconnect_reuses_the_sampling_boundary_without_repeating_a_committed_write() {
    let inner = scripted_model([
        vec![document_call()],
        vec![AssistantContent::text("Saved document verified.")],
    ]);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let model = Model::new(
        MockScript::default(),
        InterruptedModel {
            inner,
            requests: requests.clone(),
        },
    );
    let executor = Arc::new(WritingExecutor::default());
    let output = Arc::new(CollectingOutput::default());
    let result = RigAgentDriver::new(model)
        .run_turn(
            request(vec![document_tool()]),
            executor.clone(),
            output.clone(),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(executor.0.load(std::sync::atomic::Ordering::Acquire), 1);
    assert_eq!(result.final_text, "Saved document verified.");
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        serde_json::to_value(&requests[1].chat_history).unwrap(),
        serde_json::to_value(&requests[2].chat_history).unwrap()
    );
    assert!(
        serde_json::to_string(&requests[2].chat_history)
            .unwrap()
            .contains("docs/retry-proof.md")
    );
    let events = output.events.lock().unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::RuntimeStatus {
            phase: AgentRuntimePhase::Reconnecting,
            attempt: 1
        }
    )));
    assert!(
        events.iter().any(
            |event| matches!(event, AgentEvent::TextRetracted { characters } if *characters > 0)
        )
    );
}

#[tokio::test]
async fn delivery_hook_continues_missing_work_and_stops_when_feedback_has_no_progress() {
    for correct in [true, false] {
        let turns = if correct {
            vec![
                vec![AssistantContent::text("Done without writing")],
                vec![document_call()],
                vec![AssistantContent::text("Delivered")],
            ]
        } else {
            vec![
                vec![AssistantContent::text("Done without writing")],
                vec![AssistantContent::text(
                    "Cannot deliver: missing external input",
                )],
            ]
        };
        let model = scripted_model(turns);
        let requests = model.clone();
        let output = Arc::new(CollectingOutput::default());
        let executor = Arc::new(WritingExecutor(
            std::sync::atomic::AtomicUsize::new(0),
            true,
        ));
        let result = RigAgentDriver::new(model)
            .run_turn(
                request(vec![document_tool()]),
                executor.clone(),
                output.clone(),
                CancellationToken::default(),
            )
            .await
            .unwrap();
        assert!(!result.final_text.contains("Done without writing"));
        assert_eq!(requests.request_count(), if correct { 3 } else { 2 });
        assert_eq!(
            executor.0.load(std::sync::atomic::Ordering::Acquire),
            usize::from(correct)
        );
        assert_eq!(
            output
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, AgentEvent::DeliveryBlocked { .. })),
            !correct
        );
    }
}

#[derive(Clone, Default)]
struct CompactingModel(Arc<Mutex<Vec<CompletionRequest>>>);
impl Transport<MockScript> for CompactingModel {
    fn send(&self, request: CompletionRequest, exchange: Exchange) -> Opening<MockFrame> {
        let serialized = serde_json::to_string(&request.chat_history).unwrap();
        let summary = serialized.contains("Create a continuation checkpoint");
        if !summary {
            assert!(serialized.contains("docs/exact-report.md"));
            assert!(!serialized.contains("unneeded detail unneeded detail"));
        }
        self.0.lock().unwrap().push(request.clone());
        let mut events = scripted_events(
            vec![AssistantContent::text(if summary {
                "Goal: finish docs/exact-report.md. Existing edit receipt is committed; save remains pending. Preserve real group labels."
            } else {
                "Continued from the checkpoint."
            })],
            None,
        );
        if summary {
            events.insert(
                0,
                MockStreamEvent::reasoning_delta("Preserve the committed evidence."),
            );
        }
        MockCompletionModel::from_stream_turns([events])
            .transport
            .send(request, exchange)
    }
}
#[tokio::test]
async fn compaction_checkpoints_working_context_without_replaying_large_history() {
    let model = CompactingModel::default();
    let requests = model.0.clone();
    let mut request = request(vec![]);
    request.messages = vec![
        AgentMessage::User {
            content: "Finish docs/exact-report.md".into(),
        },
        AgentMessage::Assistant {
            content: "unneeded detail ".repeat(14_000),
        },
        AgentMessage::User {
            content: "Continue, keeping committed edit receipts and real group labels.".into(),
        },
    ];
    let output = Arc::new(CollectingOutput::default());
    let result = RigAgentDriver::new(Model::new(MockScript::default(), model))
        .with_model_config(
            &LanguageModelConfig {
                reasoning_efforts: Vec::new(),
                id: "model".into(),
                name: "Model".into(),
                context_window: None,
                max_output_tokens: Some(4096),
                temperature: Some(0.3),
                top_p: Some(0.8),
                additional_parameters: serde_json::json!({"generation_config": {"seed": 7}})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
            LanguageModelProtocol::Gemini,
        )
        .run_turn(
            request,
            Arc::new(StaticExecutor),
            output.clone(),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    assert_eq!(result.final_text, "Continued from the checkpoint.");
    assert!(output.events.lock().unwrap().iter().any(|event| matches!(event, AgentEvent::ContextCompacted { summary } if summary.contains("docs/exact-report.md"))));
    let mut summary_text = String::new();
    let mut summary_reasoning = String::new();
    let mut summary_calls = 0;
    for event in output.events.lock().unwrap().iter() {
        match event {
            AgentEvent::ContextCompactionDelta { delta, reasoning } => {
                if *reasoning {
                    summary_reasoning.push_str(delta);
                } else {
                    summary_text.push_str(delta);
                }
            }
            AgentEvent::UsageReported {
                purpose: ModelCallPurpose::Compaction,
                ..
            } => {
                assert_eq!(summary_reasoning, "Preserve the committed evidence.");
                assert!(summary_text.contains("docs/exact-report.md"));
                summary_calls += 1;
                summary_text.clear();
                summary_reasoning.clear();
            }
            _ => {}
        }
    }
    assert_eq!(
        summary_calls, 2,
        "each summary delivers its reasoning and text before usage"
    );
    let requests = requests.lock().unwrap();
    for request in requests.iter() {
        assert_eq!(request.temperature, Some(0.3));
        assert_eq!(request.max_tokens, Some(4096));
        assert_eq!(
            request.additional_params,
            Some(
                serde_json::json!({"generation_config": {"seed": 7, "top_p": 0.8}, "store": false})
            )
        );
    }
    assert_eq!(
        requests.len(),
        3,
        "large history requires two summaries and one continuation"
    );
    assert!(
        serde_json::to_vec(&requests.last().unwrap().chat_history)
            .unwrap()
            .len()
            < 4096
    );
}

#[tokio::test]
async fn interrupted_compaction_resumes_verified_prefix_across_driver_instances() {
    let mut original = request(vec![]);
    original.messages = vec![
        AgentMessage::User {
            content: "Finish docs/exact-report.md".into(),
        },
        AgentMessage::Assistant {
            content: "unneeded detail ".repeat(14_000),
        },
        AgentMessage::User {
            content: "Continue".into(),
        },
    ];
    let inner = scripted_model([
        vec![AssistantContent::text(
            "Goal: finish docs/exact-report.md. Existing receipt is committed.",
        )],
        vec![AssistantContent::text(
            "Goal: finish docs/exact-report.md; continue safely.",
        )],
        vec![AssistantContent::text("Continued.")],
    ]);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let model = Model::new(
        MockScript::default(),
        InterruptedModel {
            inner,
            requests: requests.clone(),
        },
    );
    let output = Arc::new(CollectingOutput::default());
    RigAgentDriver::new(model)
        .run_turn(
            original.clone(),
            Arc::new(StaticExecutor),
            output.clone(),
            CancellationToken::default(),
        )
        .await
        .unwrap();
    {
        let requests = requests.lock().unwrap();
        assert_eq!(
            requests.len(),
            4,
            "the completed first fragment must not be requested again"
        );
        assert_eq!(requests[1].chat_history, requests[2].chat_history);
    }
    {
        let events = output.events.lock().unwrap();
        let partial = events
            .iter()
            .position(|event| {
                matches!(event,
                    AgentEvent::ContextCompactionDelta { delta, reasoning: false }
                    if delta == "Interrupted partial response"
                )
            })
            .expect("interrupted summary text must remain visible");
        assert!(events[..partial].iter().any(|event| matches!(event,
            AgentEvent::ContextCompactionDelta { delta, reasoning: true }
            if delta == "Partial reasoning before interruption"
        )));
        assert!(
            events[partial + 1..].iter().any(|event| matches!(
                event,
                AgentEvent::RuntimeStatus {
                    phase: AgentRuntimePhase::Reconnecting,
                    ..
                }
            )),
            "received summary output must be flushed before retrying"
        );
    }
    let checkpoint = output
        .events
        .lock()
        .unwrap()
        .iter()
        .find_map(|event| match event {
            AgentEvent::ContextCompactionProgress {
                checkpoint: Some(checkpoint),
                ..
            } => Some(checkpoint.clone()),
            _ => None,
        })
        .unwrap();
    assert!(checkpoint.processed_bytes > 100_000);
    for unchanged_prefix in [true, false] {
        let model = CompactingModel::default();
        let recorded = model.0.clone();
        let mut resumed = original.clone();
        if !unchanged_prefix {
            resumed.messages[0] = AgentMessage::User {
                content: "Changed request: finish docs/exact-report.md".into(),
            };
        }
        resumed.messages.insert(
            0,
            AgentMessage::CompactionCheckpoint {
                checkpoint: checkpoint.clone(),
            },
        );
        resumed.messages.push(AgentMessage::User {
            content: "Resume after restart".into(),
        });
        let output = Arc::new(CollectingOutput::default());
        RigAgentDriver::new(Model::new(MockScript::default(), model))
            .run_turn(
                resumed,
                Arc::new(StaticExecutor),
                output.clone(),
                CancellationToken::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            recorded.lock().unwrap().len(),
            if unchanged_prefix { 2 } else { 3 }
        );
        let events = output.events.lock().unwrap();
        let start = events
            .iter()
            .find_map(|event| match event {
                AgentEvent::ContextCompactionProgress {
                    completed_bytes, ..
                } => Some(*completed_bytes),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            start,
            if unchanged_prefix {
                checkpoint.processed_bytes
            } else {
                0
            }
        );
    }
}
