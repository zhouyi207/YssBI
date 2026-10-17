//! A minimal domain extension and scripted driver for exercising the Harness engine.
//!
//! Notes are versioned text values. `read_note` inspects, `write_note` mutates. A task scope is
//! `{"notes": [...], "write": bool}` and a run's facts are `{"changed": [...]}`.

use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use yss_harness_contract::*;

pub const READ_NOTE: &str = "read_note";
pub const WRITE_NOTE: &str = "write_note";

/// Holds owner calls until released, so tests can act while an invocation is in flight.
#[derive(Default)]
pub struct NoteGate {
    pub entered: tokio::sync::Notify,
    pub release: tokio::sync::Notify,
    control: Mutex<Option<CapabilityControl>>,
}

impl NoteGate {
    /// The control of the held invocation.
    pub fn control(&self) -> Option<CapabilityControl> {
        self.control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[derive(Default)]
struct NoteState {
    notes: Mutex<BTreeMap<String, (u64, String)>>,
    failures: Mutex<VecDeque<CapabilityFailure>>,
    recovery: Mutex<Option<ToolRecovery>>,
    executions: AtomicUsize,
    gate: Mutex<Option<Arc<NoteGate>>>,
    panic_next: std::sync::atomic::AtomicBool,
}

impl NoteState {
    async fn execute(
        &self,
        tool: &str,
        request: &Value,
        control: CapabilityControl,
    ) -> Result<ToolExecution, CapabilityFailure> {
        let gate = self.gate.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(gate) = gate {
            *gate.control.lock().unwrap_or_else(|e| e.into_inner()) = Some(control);
            gate.entered.notify_one();
            // Deliberately complete after cancellation; callers must fence late results.
            gate.release.notified().await;
        }
        if self.panic_next.swap(false, Ordering::AcqRel) {
            panic!("synthetic owner panic");
        }
        self.run_owner(tool, request)
    }

    fn run_owner(&self, tool: &str, request: &Value) -> Result<ToolExecution, CapabilityFailure> {
        self.executions.fetch_add(1, Ordering::AcqRel);
        if let Some(failure) = self
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
        {
            return Err(failure);
        }
        let id = request["id"]
            .as_str()
            .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST))?;
        let mut notes = self.notes.lock().unwrap_or_else(|e| e.into_inner());
        match tool {
            READ_NOTE => {
                let (revision, text) = notes.get(id).cloned().ok_or_else(|| {
                    CapabilityFailure::new(CapabilityFailureCode::RESOURCE_UNAVAILABLE)
                })?;
                Ok(ToolExecution::new(
                    json!({"id": id, "revision": revision, "text": text}),
                ))
            }
            WRITE_NOTE => {
                let entry = notes.entry(id.into()).or_insert((0, String::new()));
                entry.0 += 1;
                entry.1 = request["text"].as_str().unwrap_or_default().into();
                Ok(ToolExecution {
                    result: json!({"id": id, "revision": entry.0}),
                    notices: vec![AgentNotice {
                        kind: "note_written".into(),
                        payload: json!({"id": id}),
                    }],
                })
            }
            _ => Err(CapabilityFailure::new(
                CapabilityFailureCode::INVALID_REQUEST,
            )),
        }
    }
}

/// Lead delegates; helpers may write notes inside their scope; readers only read.
#[derive(Clone, Default)]
pub struct NoteExtension {
    state: Arc<NoteState>,
}

impl NoteExtension {
    pub fn with_note(self, id: &str, text: &str) -> Self {
        self.state
            .notes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.into(), (1, text.into()));
        self
    }

    pub fn with_gate(self) -> (Self, Arc<NoteGate>) {
        let gate = Arc::new(NoteGate::default());
        *self.state.gate.lock().unwrap_or_else(|e| e.into_inner()) = Some(gate.clone());
        (self, gate)
    }

    /// The next owner call fails with `failure` instead of executing.
    pub fn fail_next(&self, failure: CapabilityFailure) {
        self.state
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(failure);
    }

    pub fn panic_next(&self) {
        self.state.panic_next.store(true, Ordering::Release);
    }

    pub fn recover_with(&self, recovery: ToolRecovery) {
        *self
            .state
            .recovery
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(recovery);
    }

    pub fn executions(&self) -> usize {
        self.state.executions.load(Ordering::Acquire)
    }

    pub fn note(&self, id: &str) -> Option<(u64, String)> {
        self.state
            .notes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    /// Change a note outside the Harness, as another editor would.
    pub fn edit_externally(&self, id: &str, text: &str) {
        let mut notes = self.state.notes.lock().unwrap_or_else(|e| e.into_inner());
        let entry = notes.entry(id.into()).or_insert((0, String::new()));
        entry.0 += 1;
        entry.1 = text.into();
    }
}

pub fn note_specs(write: bool) -> Vec<ToolSpec> {
    let mut specs = vec![
        ToolSpec::new(
            READ_NOTE,
            "Read a note.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
            ToolEffect::Inspect,
        ),
        ToolSpec::new(
            SEARCH_KNOWLEDGE_TOOL,
            "Search knowledge.",
            search_knowledge_schema(),
            ToolEffect::Inspect,
        ),
        ToolSpec::new(
            READ_KNOWLEDGE_TOOL,
            "Read knowledge.",
            read_knowledge_schema(),
            ToolEffect::Inspect,
        ),
    ];
    if write {
        specs.push(ToolSpec::new(
            WRITE_NOTE,
            "Write a note.",
            json!({"type": "object", "properties": {"id": {"type": "string"}, "text": {"type": "string"}, "key": {"type": "string"}}, "required": ["id", "text"]}),
            ToolEffect::Mutate,
        ));
    }
    specs
}

pub fn role(name: &str) -> AgentRoleId {
    AgentRoleId::try_new(name).unwrap_or_else(|_| unreachable!("fixture role ids are valid"))
}

fn scope_notes(scope: &Value) -> Vec<String> {
    scope["notes"]
        .as_array()
        .map(|notes| {
            notes
                .iter()
                .filter_map(|note| note.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

impl HarnessExtension for NoteExtension {
    fn root_role(&self) -> AgentRoleId {
        role("lead")
    }

    fn profile(&self, id: &AgentRoleId) -> Option<AgentProfile> {
        let (write, delegates, output_mode) = match id.as_str() {
            "lead" => (false, true, AgentOutputMode::Transcript),
            "helper" => (true, false, AgentOutputMode::FinalResponse),
            "reader" => (false, false, AgentOutputMode::FinalResponse),
            _ => return None,
        };
        Some(AgentProfile {
            role: id.clone(),
            instructions: vec![format!("You are the {id}.")],
            tools: note_specs(write),
            tool_concurrency: if delegates { 4 } else { 1 },
            delegates,
            output_mode,
        })
    }

    fn mode_instructions(&self, mode: HarnessMode) -> Option<String> {
        (mode == HarnessMode::Ask).then(|| "Ask mode.".into())
    }

    fn delegate_task_schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn decode_task_input(&self, arguments: Value) -> Result<AgentTaskInput, CapabilityFailure> {
        serde_json::from_value(arguments)
            .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST))
    }

    fn validate_task(&self, task: &AgentTask) -> Result<(), CapabilityFailure> {
        if task.worker.as_str() == "reader" && task.scope["write"] == true {
            return Err(
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                    .with_detail("reason", "reader_cannot_write"),
            );
        }
        Ok(())
    }

    fn scope_is_read_only(&self, scope: &Value) -> bool {
        scope["write"] != true
    }

    fn task_scope_input(&self, scope: &Value) -> Value {
        scope.clone()
    }

    fn task_invalidated(&self, scope: &Value, facts: &Value) -> bool {
        let changed = scope_notes(&json!({"notes": facts["changed"]}));
        scope_notes(scope).iter().any(|note| changed.contains(note))
    }

    fn project_result(&self, _tool: &str, result: &Value) -> Result<Value, CapabilityFailure> {
        Ok(result.clone())
    }

    fn project_arguments(&self, _tool: &str, request: &Value) -> Value {
        request.clone()
    }

    fn project_failure(&self, failure: &CapabilityFailure) -> Value {
        json!({"code": failure.code, "details": failure.details})
    }

    fn project_task_facts(&self, facts: &Value) -> Value {
        json!({"changed": facts["changed"]})
    }

    fn tool_spec(&self, tool: &str) -> Option<ToolSpec> {
        note_specs(true).into_iter().find(|spec| spec.name == tool)
    }

    fn validate_request(&self, _tool: &str, request: &Value) -> Result<(), CapabilityFailure> {
        if request["id"].as_str().is_some_and(|id| !id.is_empty()) {
            Ok(())
        } else {
            Err(CapabilityFailure::new(
                CapabilityFailureCode::INVALID_REQUEST,
            ))
        }
    }

    fn execute<'a>(
        &'a self,
        _context: ToolInvocationContext,
        tool: &'a str,
        request: &'a Value,
        control: CapabilityControl,
    ) -> AgentFuture<'a, Result<ToolExecution, CapabilityFailure>> {
        Box::pin(self.state.execute(tool, request, control))
    }

    fn recover<'a>(
        &'a self,
        _context: ToolInvocationContext,
        _tool: &'a str,
        _request: &'a Value,
    ) -> AgentFuture<'a, Result<ToolRecovery, CapabilityFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .recovery
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .unwrap_or(ToolRecovery::Unsupported))
        })
    }

    fn start_run(&self, run: RunStart) -> Arc<dyn AgentRunHooks> {
        Arc::new(NoteRun {
            state: Arc::clone(&self.state),
            scope: Mutex::new(run.task_scope.unwrap_or(Value::Null)),
            changed: Mutex::new(Vec::new()),
            replayed: Mutex::new(Vec::new()),
        })
    }
}

/// Per-run fixture state: the task scope, changed notes and replayed invocations.
pub struct NoteRun {
    state: Arc<NoteState>,
    scope: Mutex<Value>,
    changed: Mutex<Vec<String>>,
    replayed: Mutex<Vec<ToolInvocationId>>,
}

impl NoteRun {
    pub fn replayed(&self) -> Vec<ToolInvocationId> {
        self.replayed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn record_write(&self, result: &Value) {
        if let Some(id) = result["id"].as_str() {
            self.changed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(id.into());
        }
    }
}

impl AgentRunHooks for NoteRun {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn replay(&self, record: &ToolInvocationRecord, _project: &ProjectSessionBinding) {
        self.replayed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(record.id.clone());
        if record.tool == WRITE_NOTE
            && let Some(result) = &record.result
        {
            self.record_write(result);
        }
    }

    fn invalidate(&self, _facts: &Value) {}

    fn resume(&self, scope: &Value) {
        *self.scope.lock().unwrap_or_else(|e| e.into_inner()) = scope.clone();
    }

    fn prepare<'a>(
        &'a self,
        call: ToolCall,
        _runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<PreparedToolCall, RejectedToolCall>> {
        Box::pin(async move {
            if !call.arguments["id"].is_string() {
                return Err(CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST).into());
            }
            let scope = self.scope.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if call.name == WRITE_NOTE
                && (scope["write"] != true
                    || !scope_notes(&scope)
                        .iter()
                        .any(|note| call.arguments["id"] == **note))
            {
                return Err(RejectedToolCall {
                    failure: CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                        .with_detail("reason", "scope_denied"),
                    arguments: Some(call.arguments),
                });
            }
            Ok(PreparedToolCall {
                tool: call.name,
                idempotency_token: call.arguments["key"].as_str().map(str::to_owned),
                request: call.arguments.clone(),
                arguments: call.arguments,
                internal: false,
            })
        })
    }

    fn execute<'a>(
        &'a self,
        _context: ToolInvocationContext,
        call: &'a PreparedToolCall,
        control: CapabilityControl,
    ) -> AgentFuture<'a, Result<ToolExecution, CapabilityFailure>> {
        Box::pin(self.state.execute(&call.tool, &call.request, control))
    }

    fn observe(
        &self,
        call: &PreparedToolCall,
        outcome: &Result<ToolInvocationOutcome, CapabilityFailure>,
    ) {
        if call.tool == WRITE_NOTE
            && let Ok(outcome) = outcome
        {
            self.record_write(&outcome.result);
        }
    }

    fn bind_task<'a>(
        &'a self,
        input: AgentTaskInput,
        _runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<Value, CapabilityFailure>> {
        Box::pin(async move { Ok(input.scope) })
    }

    fn resume_task<'a>(
        &'a self,
        worker: &'a dyn AgentRunHooks,
        _runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<(Value, Value), CapabilityFailure>> {
        Box::pin(async move {
            let worker = worker
                .as_any()
                .downcast_ref::<NoteRun>()
                .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE))?;
            let scope = worker
                .scope
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            Ok((scope, json!({"rebound": true})))
        })
    }

    fn completion_feedback(&self, _delegated: &[DelegatedTask<'_>]) -> Option<String> {
        None
    }

    fn finish(&self) -> RunFinish {
        RunFinish {
            blocked_reason: None,
            next_steps: Vec::new(),
            facts: json!({"changed": *self.changed.lock().unwrap_or_else(|e| e.into_inner())}),
        }
    }
}

pub enum ScriptedStep {
    Call(&'static str, Value),
    Text(&'static str),
}

/// Plays one script per run of a role, in delegation order, and records every call outcome.
#[derive(Default)]
pub struct ScriptedDriver {
    scripts: Mutex<BTreeMap<String, VecDeque<Vec<ScriptedStep>>>>,
    requests: Mutex<Vec<AgentTurnRequest>>,
    outcomes: Mutex<Vec<(String, String, ToolCallOutcome)>>,
}

impl ScriptedDriver {
    pub fn script(self, role: &str, steps: Vec<ScriptedStep>) -> Self {
        self.push(role, steps);
        self
    }

    /// Queue the script for the next run of `role`.
    pub fn push(&self, role: &str, steps: Vec<ScriptedStep>) {
        self.scripts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(role.into())
            .or_default()
            .push_back(steps);
    }

    pub fn requests(&self) -> Vec<AgentTurnRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Every executed call as (role, tool, outcome), in completion order.
    pub fn outcomes(&self) -> Vec<(String, String, ToolCallOutcome)> {
        self.outcomes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl AgentDriverPort for ScriptedDriver {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        tools: Arc<dyn AgentToolExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            let role = request.role.to_string();
            self.requests
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(request);
            let steps = self
                .scripts
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_mut(&role)
                .and_then(VecDeque::pop_front)
                .unwrap_or_default();
            let mut text = String::new();
            for step in steps {
                if cancellation.is_cancelled() {
                    return Err(AgentDriverFailure::new(AgentDriverFailureCode::Cancelled));
                }
                match step {
                    ScriptedStep::Call(name, arguments) => {
                        let outcome = tools
                            .call(ToolCall {
                                name: name.into(),
                                arguments,
                            })
                            .await;
                        let fatal = outcome.fatal;
                        self.outcomes
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push((role.clone(), name.into(), outcome));
                        if let Some(code) = fatal {
                            return Err(AgentDriverFailure::new(code));
                        }
                    }
                    ScriptedStep::Text(delta) => {
                        output
                            .emit(AgentEvent::TextDelta {
                                delta: delta.into(),
                            })
                            .await
                            .map_err(|_| {
                                AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable)
                            })?;
                        text.push_str(delta);
                    }
                }
            }
            Ok(AgentTurnResult {
                final_text: if text.is_empty() { "done".into() } else { text },
            })
        })
    }
}
