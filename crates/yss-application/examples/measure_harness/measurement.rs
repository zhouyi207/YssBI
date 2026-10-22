//! Measurements derived from durable events and the public result projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use yss_assistant_contract::*;
use yss_harness_contract::*;
use yss_harness_core::{HarnessError, HarnessHost};

use super::model_calls::{CallMeasurement, fingerprint, is_control};
use super::{Error, runtime::SchemaMeasurement};

pub struct TaskMeasurement<'a> {
    pub label: &'a str,
    pub elapsed: Duration,
    pub outcome: &'a Result<AgentTurnResult, HarnessError>,
    pub schemas: Vec<SchemaMeasurement>,
    pub model_calls: Vec<CallMeasurement>,
}

pub async fn write(
    host: &HarnessHost,
    session: &HarnessSessionRecord,
    sequence: u64,
    directory: &Path,
    measurement: TaskMeasurement<'_>,
) -> Result<(), Error> {
    let TaskMeasurement {
        label,
        elapsed,
        outcome,
        schemas,
        mut model_calls,
    } = measurement;
    let events = host.events_after(&session.id, sequence).await?;
    let mut ids = BTreeSet::new();
    let mut invocations = Vec::new();
    let mut usage = Vec::new();
    let mut controls = Vec::new();
    let mut control_calls = BTreeMap::new();
    let mut agent_outcomes = Vec::new();
    let mut model = Value::Null;
    let mut reconnects = 0;
    for envelope in &events {
        match &envelope.event {
            HarnessEvent::AgentRunFinished { outcome } => agent_outcomes.push(json!({
                "role": outcome.role, "state": outcome.state, "failureCode": outcome.failure_code,
            })),
            HarnessEvent::TurnStarted {
                model: identity, ..
            } => model = serde_json::to_value(identity)?,
            HarnessEvent::AgentRunStarted {
                role,
                task: Some(task),
                ..
            } => controls.push(
                json!({"event": "agent_delegate_task", "role": role, "task": task.objective}),
            ),
            HarnessEvent::AgentRunResumed { .. } => {
                controls.push(json!({"event": "agent_continue_task"}))
            }
            HarnessEvent::Agent(event) | HarnessEvent::AgentRunOutput { event, .. } => {
                match event {
                    AgentEvent::ControlToolStarted {
                        invocation_id,
                        tool,
                    } => {
                        control_calls.insert(invocation_id.clone(), json!({
                            "id": invocation_id, "tool": tool.as_str(), "startedAt": envelope.occurred_at,
                            "finishedAt": null, "elapsedMs": null, "failureCode": null,
                        }));
                    }
                    AgentEvent::ControlToolFinished {
                        invocation_id,
                        failure_code,
                        ..
                    } => {
                        if let Some(call) = control_calls.get_mut(invocation_id) {
                            call["finishedAt"] = json!(envelope.occurred_at);
                            call["elapsedMs"] = json!(
                                envelope
                                    .occurred_at
                                    .get()
                                    .saturating_sub(call["startedAt"].as_u64().unwrap())
                            );
                            call["failureCode"] = json!(failure_code);
                        }
                    }
                    AgentEvent::ToolInvocationStarted { invocation_id, .. }
                        if ids.insert(invocation_id.clone()) =>
                    {
                        invocations.push(
                            host.inspect_tool_invocation(&session.id, invocation_id)
                                .await?
                                .ok_or("event has no ledger record")?,
                        );
                    }
                    AgentEvent::UsageReported {
                        usage: tokens,
                        purpose,
                        ..
                    } => usage.push(json!({"purpose": purpose, "tokens": tokens})),
                    AgentEvent::Custom { kind, .. } if kind == PLAN_PROPOSED_NOTICE => {
                        controls.push(json!({"event": PROPOSE_STATISTICAL_PLAN_TOOL}))
                    }
                    AgentEvent::RuntimeStatus {
                        phase: AgentRuntimePhase::Reconnecting,
                        ..
                    } => reconnects += 1,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    invocations.sort_by_key(|record| record.started_at);
    for call in &mut model_calls {
        let Some(id) = &call.invocation_id else {
            continue;
        };
        let recovered;
        let record = if let Some(record) = invocations.iter().find(|record| &record.id == id) {
            record
        } else {
            // Idempotent recovery can return a durable identity without a new
            // start event in this turn.
            recovered = host
                .inspect_tool_invocation(&session.id, id)
                .await?
                .ok_or("model call has no ledger record")?;
            &recovered
        };
        // Rejection stores {} when decoding failed, not the provider payload.
        // Decoded Assistant inputs retain their fields even when binding fails.
        if record.request.is_some()
            || record
                .arguments
                .as_object()
                .is_some_and(|arguments| !arguments.is_empty())
        {
            call.argument_hash = fingerprint(&record.arguments);
        }
        call.failure_code = record.failure.as_ref().map(|failure| failure.code.clone());
        if record.result.is_some() || record.failure.is_some() {
            call.record_projection(&public_result(record)?);
        }
    }
    // Plan proposals now use the extension's ledger path, not Core's control
    // lifecycle. Keep them beside delegation rather than counting business work.
    for record in invocations.iter().filter(|record| is_control(&record.tool)) {
        let result = public_result(record)?;
        control_calls.insert(record.id.clone(), json!({
            "id": record.id, "tool": record.tool, "startedAt": record.started_at,
            "finishedAt": record.finished_at,
            "elapsedMs": record.finished_at.map(|finish| finish.get().saturating_sub(record.started_at.get())),
            "failureCode": record.failure.as_ref().map(|failure| &failure.code),
            "arguments": record.arguments, "result": result,
        }));
    }
    invocations.retain(|record| !is_control(&record.tool));
    let mut counts = BTreeMap::<String, usize>::new();
    let mut failures = BTreeMap::<String, usize>::new();
    let mut read_results = BTreeSet::new();
    let mut failed_arguments = BTreeSet::new();
    let mut duplicate_reads = 0;
    let mut retries = 0;
    let mut result_bytes = 0;
    let mut result_characters = 0;
    let mut tool_elapsed_ms = 0;
    let mut missing_terminal = 0;
    let mut details = Vec::new();
    for record in &invocations {
        let name = record.tool.as_str();
        *counts.entry(name.into()).or_default() += 1;
        let arguments = &record.arguments;
        let key = (name.to_owned(), serde_json::to_string(arguments)?);
        if failed_arguments.remove(&key) {
            retries += 1;
        }
        if let Some(failure) = &record.failure {
            *failures.entry(failure.code.to_string()).or_default() += 1;
            failed_arguments.insert(key.clone());
        }
        let result = public_result(record)?;
        let encoded = serde_json::to_string(&result)?;
        if record.state == ToolInvocationState::Succeeded
            && record.effect == ToolEffect::Inspect
            && !read_results.insert((key, encoded.clone()))
        {
            duplicate_reads += 1;
        }
        let duration = record
            .finished_at
            .map(|finish| finish.get().saturating_sub(record.started_at.get()));
        tool_elapsed_ms += duration.unwrap_or(0);
        if duration.is_none() {
            missing_terminal += 1;
        }
        result_bytes += encoded.len();
        result_characters += encoded.chars().count();
        details.push(json!({
            "id": record.id, "agentRunId": record.agent_run_id, "tool": name,
            "state": record.state, "arguments": arguments, "result": result,
            "startedAt": record.started_at, "finishedAt": record.finished_at,
            "elapsedMs": duration, "resultBytes": encoded.len(), "resultCharacters": encoded.chars().count(),
        }));
    }
    let mut tokens = serde_json::Map::new();
    for field in [
        "inputTokens",
        "outputTokens",
        "cachedInputTokens",
        "cacheCreationInputTokens",
        "reasoningTokens",
    ] {
        let samples: Vec<_> = usage
            .iter()
            .filter_map(|sample| sample["tokens"][field].as_u64())
            .collect();
        tokens.insert(field.into(), json!({
            "reportedSum": if samples.is_empty() { None } else { Some(samples.iter().sum::<u64>()) },
            "reportingCalls": samples.len(),
        }));
    }
    let summary = json!({
        "label": label, "model": model, "succeeded": outcome.is_ok(),
        "error": outcome.as_ref().err().map(ToString::to_string),
        "wallElapsedMs": elapsed.as_millis(), "toolElapsedMsSum": tool_elapsed_ms,
        "businessToolCalls": invocations.len(), "callsByTool": counts,
        "failuresByCode": failures, "missingTerminalTimes": missing_terminal,
        "resultBytes": result_bytes, "resultCharacters": result_characters,
        "duplicateSuccessfulReads": duplicate_reads, "sameArgumentRetries": retries,
        "providerReconnectEvents": reconnects, "usageReports": usage.len(), "tokens": tokens,
        "agentAdmissions": schemas, "acceptedControlEvents": controls,
        "agentOutcomes": agent_outcomes,
        "modelBusiness": model_summary(&model_calls),
        "controlToolCalls": control_calls.len(),
        "controlCalls": control_calls.values().collect::<Vec<_>>(),
    });
    fs::write(
        directory.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    fs::write(
        directory.join("invocations.json"),
        serde_json::to_vec_pretty(&details)?,
    )?;
    fs::write(
        directory.join("usage.json"),
        serde_json::to_vec_pretty(&usage)?,
    )?;
    fs::write(
        directory.join("model-calls.json"),
        serde_json::to_vec_pretty(&model_calls)?,
    )?;
    if let Ok(result) = outcome {
        fs::write(directory.join("response.md"), &result.final_text)?;
    }
    println!(
        "Calls: {}; results: {} chars; failed: {}; elapsed: {} ms",
        invocations.len(),
        result_characters,
        invocations
            .iter()
            .filter(|entry| entry.failure.is_some())
            .count(),
        elapsed.as_millis()
    );
    Ok(())
}

fn public_result(record: &ToolInvocationRecord) -> Result<Value, serde_json::Error> {
    match (&record.result, &record.failure) {
        (Some(result), None) => project_tool_result(&record.tool, result),
        (None, Some(failure)) => Ok(model::failure(failure)),
        _ => Ok(Value::Null),
    }
}

fn model_summary(calls: &[CallMeasurement]) -> Value {
    let mut reads = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut duplicate_reads = 0;
    let mut retries = 0;
    for call in calls {
        let Some(arguments) = &call.argument_hash else {
            continue;
        };
        let key = (call.capability_id.as_str(), arguments);
        if failed.remove(&key) {
            retries += 1;
        }
        if call.failure_code.is_some() || call.fatal.is_some() {
            failed.insert(key);
        } else if call.effect == Some(ToolEffect::Inspect)
            && let Some(result) = &call.result_hash
            && !reads.insert((key, result))
        {
            duplicate_reads += 1;
        }
    }
    json!({
        "calls": calls.len(),
        "failures": calls.iter().filter(|call| call.failure_code.is_some() || call.fatal.is_some()).count(),
        "resultBytes": calls.iter().filter_map(|call| call.result_bytes).sum::<usize>(),
        "resultCharacters": calls.iter().filter_map(|call| call.result_characters).sum::<usize>(),
        "projectionFailures": calls.iter().filter(|call| call.result_bytes.is_none()).count(),
        "duplicateSuccessfulReads": duplicate_reads, "sameArgumentRetries": retries,
    })
}
