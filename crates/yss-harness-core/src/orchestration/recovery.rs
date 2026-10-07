use super::receipts::{Evidence, record_receipt};
use crate::{HarnessPorts, events::EventWriter};
use std::collections::BTreeMap;
use yss_harness_contract::*;

pub(crate) async fn pending_agent_turns(
    ports: &HarnessPorts,
) -> Result<Vec<HarnessTurnRecord>, PersistenceFailure> {
    let mut pending = BTreeMap::new();
    let mut controls = BTreeMap::new();
    for session in ports.sessions.load_active_sessions().await? {
        for envelope in ports.events.load_events_after(&session.id, 0).await? {
            if let Some((event, _)) = control_event(&envelope.event) {
                match event {
                    AgentEvent::ControlToolStarted { invocation_id, .. } => {
                        if let Some(turn_id) = &envelope.turn_id {
                            controls.insert(invocation_id.clone(), turn_id.clone());
                        }
                    }
                    AgentEvent::ControlToolFinished { invocation_id, .. } => {
                        controls.remove(invocation_id);
                    }
                    _ => {}
                }
            }
            match envelope.event {
                HarnessEvent::AgentRunResumed { request, .. } => {
                    if let Some(turn_id) = envelope.turn_id {
                        pending.insert(request.run_id, turn_id);
                    }
                }
                HarnessEvent::AgentRunStarted { run_id, .. } => {
                    if let Some(turn_id) = envelope.turn_id {
                        pending.insert(run_id, turn_id);
                    }
                }
                HarnessEvent::AgentRunFinished { outcome } => {
                    pending.remove(&outcome.run_id);
                }
                _ => {}
            }
        }
    }
    let mut turns = BTreeMap::new();
    for id in pending.into_values().chain(controls.into_values()) {
        if let Some(turn) = ports.sessions.load_turn(&id).await? {
            turns.insert(id, turn);
        }
    }
    Ok(turns.into_values().collect())
}

pub(crate) async fn recover_runs(
    ports: &HarnessPorts,
    writer: &EventWriter,
    turn: &HarnessTurnRecord,
) -> Result<(), crate::HarnessError> {
    let events = ports.events.load_events_after(&turn.session_id, 0).await?;
    let mut controls = BTreeMap::new();
    for envelope in &events {
        if envelope.turn_id.as_ref() != Some(&turn.id) {
            continue;
        }
        if let Some((event, run_id)) = control_event(&envelope.event) {
            match event {
                AgentEvent::ControlToolStarted {
                    invocation_id,
                    tool,
                } => {
                    controls.insert(invocation_id.clone(), (*tool, run_id.cloned()));
                }
                AgentEvent::ControlToolFinished { invocation_id, .. } => {
                    controls.remove(invocation_id);
                }
                _ => {}
            }
        }
    }
    for (invocation_id, (tool, run_id)) in controls {
        let event = AgentEvent::ControlToolFinished {
            invocation_id,
            tool,
            failure_code: Some(CapabilityFailureCode::OutcomeUnknown),
            failure_details: None,
        };
        writer
            .append(
                &turn.session_id,
                Some(&turn.id),
                match run_id {
                    Some(run_id) => HarnessEvent::AgentRunOutput { run_id, event },
                    None => HarnessEvent::Agent(event),
                },
            )
            .await?;
    }
    let mut runs = BTreeMap::new();
    for envelope in &events {
        if envelope.turn_id.as_ref() != Some(&turn.id) {
            continue;
        }
        match &envelope.event {
            HarnessEvent::AgentRunResumed {
                request,
                role,
                scope,
                ..
            } => {
                runs.insert(request.run_id.clone(), (*role, Some(scope.clone())));
            }
            HarnessEvent::AgentRunStarted {
                run_id, role, task, ..
            } => {
                runs.insert(
                    run_id.clone(),
                    (*role, task.as_ref().map(|task| task.scope.clone())),
                );
            }
            HarnessEvent::AgentRunFinished { outcome } => {
                runs.remove(&outcome.run_id);
            }
            _ => {}
        }
    }
    let mut runs: Vec<_> = runs.into_iter().collect();
    runs.sort_by_key(|(_, (role, _))| *role == AgentRole::Manager);
    for (run_id, (role, task)) in runs {
        let mut evidence = Evidence::default();
        let mut scope = AgentInvocationScope {
            run_id: run_id.clone(),
            role,
            task,
        };
        for envelope in &events {
            if envelope.turn_id.as_ref() != Some(&turn.id) {
                continue;
            }
            let event = match &envelope.event {
                HarnessEvent::AgentRunOutput { run_id: id, event } if id == &run_id => event,
                HarnessEvent::Agent(event) if role == AgentRole::Manager => event,
                _ => continue,
            };
            if let AgentEvent::PlanProposed { plan } = event {
                evidence.plan = Some(plan.clone());
            }
            if let AgentEvent::ToolInvocationStarted { invocation_id, .. }
            | AgentEvent::ToolInvocationCompleted { invocation_id, .. }
            | AgentEvent::ToolInvocationFailed { invocation_id, .. } = event
                && !evidence.invocations.contains(invocation_id)
                && let Some(record) = ports
                    .tool_ledger
                    .load_invocation(&turn.session_id, invocation_id)
                    .await?
                && record.agent_run_id.as_ref() == Some(&run_id)
            {
                evidence.invocations.push(invocation_id.clone());
                if let (Some(request), Some(result)) = (record.request.bound(), &record.result) {
                    record_receipt(request, result, &mut evidence, &mut scope);
                }
            }
        }
        writer.append(&turn.session_id, Some(&turn.id), HarnessEvent::AgentRunFinished {
            outcome: Box::new(AgentTaskOutcome {
                run_id, role, state: AgentRunState::Interrupted,
                report: (role != AgentRole::Manager).then(|| WorkerReport { summary: "The process stopped before this task finished. Inspect recorded effects before deciding on further work.".into(),
                    warnings: vec!["An interrupted operation may already have committed; do not automatically repeat it.".into()], blocked_reason: Some("interrupted".into()), next_steps: vec![] }),
                failure_code: Some(AgentDriverFailureCode::InternalFailure), artifacts: evidence.artifacts,
                results: evidence.results, evidence: evidence.invocations, plan: evidence.plan, invalidated_runs: vec![],
            }),
        }).await?;
    }
    Ok(())
}

fn control_event(event: &HarnessEvent) -> Option<(&AgentEvent, Option<&AgentRunId>)> {
    match event {
        HarnessEvent::Agent(event) => Some((event, None)),
        HarnessEvent::AgentRunOutput { run_id, event } => Some((event, Some(run_id))),
        _ => None,
    }
}
