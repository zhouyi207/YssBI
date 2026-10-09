use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct TaskRecord {
    pub snapshot: TaskSnapshot,
    pub(super) context: CallContext,
}

struct TaskExecution {
    task_id: String,
    task_type: String,
    parameters: Value,
    context: CallContext,
    lease: Arc<PluginLease>,
    duration: Duration,
    produces_artifacts: bool,
}

impl PluginManager {
    pub(super) fn start_task(
        &self,
        binding: ContextBinding,
        input: Value,
    ) -> Result<Value, PluginFailure> {
        let operation_id = input["operationId"]
            .as_str()
            .filter(|value| valid_id(value))
            .ok_or_else(|| fail("plugin_operation_invalid"))?
            .to_owned();
        let task_type = input["taskType"]
            .as_str()
            .ok_or_else(|| fail("plugin_task_type_unknown"))?
            .to_owned();
        let registration = self.registration(&binding.context.plugin_id)?;
        let task_descriptor = registration
            .manifest
            .contributes
            .task_types
            .iter()
            .find(|task| task.id == task_type)
            .ok_or_else(|| fail("plugin_task_type_unknown"))?;
        let produces_artifacts = task_descriptor.produces_artifacts;
        if produces_artifacts && binding.context.project.is_none() {
            return Err(fail("plugin_project_required"));
        }
        if produces_artifacts
            && !registration
                .manifest
                .permissions
                .iter()
                .any(|permission| permission == "results.write")
        {
            return Err(fail("plugin_permission_denied"));
        }
        let parameters = input["parameters"].clone();
        let timeout_ms = input["timeoutMs"].as_u64().unwrap_or(600_000);
        if !(1_000..=86_400_000).contains(&timeout_ms) {
            return Err(fail("plugin_task_invalid"));
        }
        let parameters_hash = package::hash(&serde_jcs::to_vec(&json!({"parameters":parameters,"project":binding.context.project,"taskType":task_type,"packageDigest":registration.digest,"timeoutMs":timeout_ms})).map_err(|_| fail("plugin_task_invalid"))?);
        let lease = Arc::new(self.acquire(&binding.context.plugin_id)?);
        if lease.process.instance_id != binding.context.instance_id {
            return Err(fail("plugin_stale_context"));
        }
        let task_id = format!("task-{}", uuid::Uuid::new_v4());
        let mut context = binding.context.clone();
        context.context_id = uuid::Uuid::new_v4().to_string();
        context.task_id = Some(task_id.clone());
        context.operation_id = Some(operation_id.clone());
        context.parameters_hash = Some(parameters_hash);
        let record = TaskRecord {
            snapshot: TaskSnapshot {
                task_id: task_id.clone(),
                operation_id: operation_id.clone(),
                plugin_id: binding.context.plugin_id.clone(),
                package_digest: registration.digest,
                state: TaskState::Admitted,
                revision: "0".into(),
                error: None,
                result: None,
                progress: None,
            },
            context: context.clone(),
        };
        if let Some(snapshot) =
            self.admit_record(record.clone(), registration.granted_budget.active_tasks)?
        {
            return serde_json::to_value(snapshot).map_err(|_| fail("plugin_response_invalid"));
        }
        let manager = self.clone();
        let duration = Duration::from_millis(timeout_ms);
        if std::thread::Builder::new()
            .spawn(move || {
                manager.monitor_task(TaskExecution {
                    task_id,
                    task_type,
                    parameters,
                    context,
                    lease,
                    duration,
                    produces_artifacts,
                })
            })
            .is_err()
        {
            let error = fail("plugin_resource_exhausted");
            self.update_task(
                &record.snapshot.task_id,
                TaskState::Failed,
                Some(error.clone()),
                None,
            )?;
            return Err(error);
        }
        serde_json::to_value(record.snapshot).map_err(|_| fail("plugin_response_invalid"))
    }

    fn admit_record(
        &self,
        record: TaskRecord,
        active_limit: u32,
    ) -> Result<Option<TaskSnapshot>, PluginFailure> {
        if record.snapshot.state != TaskState::Admitted {
            return Err(fail("plugin_task_invalid"));
        }
        let mut existing = None;
        self.ledger()?.prune()?;
        self.update_registry(|registry| {
            let previous = registry
                .tasks
                .values()
                .find(|task| {
                    task.snapshot.plugin_id == record.snapshot.plugin_id
                        && task.snapshot.operation_id == record.snapshot.operation_id
                })
                .cloned()
                .or(self
                    .ledger()?
                    .task_operation(&record.snapshot.plugin_id, &record.snapshot.operation_id)?);
            if let Some(previous) = previous {
                if previous.context.parameters_hash != record.context.parameters_hash {
                    return Err(fail("plugin_operation_conflict"));
                }
                existing = Some(previous.snapshot);
                return Ok(());
            }
            validate_operation_id(&record.snapshot.operation_id, crate::ledger::now_ms())?;
            if registry
                .tasks
                .values()
                .filter(|task| !task.snapshot.state.terminal())
                .count()
                >= 128
                || registry
                    .tasks
                    .values()
                    .filter(|task| {
                        task.snapshot.plugin_id == record.snapshot.plugin_id
                            && !task.snapshot.state.terminal()
                    })
                    .count()
                    >= active_limit as usize
            {
                return Err(fail("plugin_resource_exhausted"));
            }
            registry
                .tasks
                .insert(record.snapshot.task_id.clone(), record);
            Ok(())
        })?;
        Ok(existing)
    }
    fn monitor_task(&self, execution: TaskExecution) {
        let TaskExecution {
            task_id,
            task_type,
            parameters,
            context,
            lease,
            duration,
            produces_artifacts,
        } = execution;
        let deadline = Instant::now() + duration;
        let poll_interval = Duration::from_millis(250);
        lease.process.diagnostics.task(&task_id, true);
        let wire_context = |request_deadline: Instant| json!({"contextId":context.context_id,"project":context.project,"remainingBudgetMs":request_deadline.saturating_duration_since(Instant::now()).as_millis() as u64});
        let mut start_attempted = false;
        let mut remote_may_run = false;
        let result = (|| {
            let state = self.task(&task_id)?.snapshot.state;
            if state.terminal() {
                return Ok(());
            }
            if state == TaskState::CancelRequested {
                self.update_task(&task_id, TaskState::Cancelled, None, None)?;
                return Ok(());
            }
            {
                let mut state = self
                    .inner
                    .state
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?;
                state.ensure_active_process(&context)?;
                state.contexts.insert(
                    context.context_id.clone(),
                    ContextBinding {
                        context: context.clone(),
                        window: String::new(),
                        view_id: String::new(),
                    },
                );
            }
            let mut cancellation_deadline = None;
            let mut last_storage_check = Instant::now();
            start_attempted = true;
            lease
                .process
                .request(
                    "tasks.start",
                    json!({"context":wire_context(deadline),"input":{"taskId":task_id,"taskType":task_type,"parameters":parameters}}),
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_secs(30)),
                )
                .inspect_err(|error| {
                    // A caught handler panic may follow remote work admission.
                    if error.code == "plugin_handler_failed" {
                        remote_may_run = true;
                    }
                })?;
            remote_may_run = true;
            loop {
                if last_storage_check.elapsed() >= Duration::from_secs(2) {
                    self.enforce_private_budget(&context.plugin_id, 0)?;
                    last_storage_check = Instant::now();
                }
                let state = self.task(&task_id)?.snapshot.state;
                if state.terminal() {
                    return Ok(());
                }
                let invalid = self.context(&context.context_id).is_err();
                if state == TaskState::CancelRequested || invalid || Instant::now() >= deadline {
                    let grace = cancellation_deadline
                        .get_or_insert_with(|| Instant::now() + Duration::from_secs(10));
                    let remaining = grace.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(fail("plugin_cancel_timeout"));
                    }
                    let cancelled = lease.process.request(
                        "tasks.cancel",
                        json!({"context":wire_context(*grace),"input":{"taskId":task_id}}),
                        remaining,
                    );
                    if let Err(error) = cancelled {
                        lease
                            .process
                            .diagnostics
                            .push(format!("tasks.cancel: {}\n", error.code).as_bytes());
                        return Err(fail("plugin_cancel_failed"));
                    }
                }
                let reply_deadline = cancellation_deadline.unwrap_or(deadline);
                let remaining = reply_deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    continue;
                }
                let snapshot = match lease.process.request(
                    "tasks.get",
                    json!({"context":wire_context(reply_deadline),"input":{"taskId":task_id}}),
                    remaining.min(Duration::from_secs(10)),
                ) {
                    Ok(snapshot) => snapshot,
                    Err(error) if error.code == "plugin_resource_exhausted" => {
                        std::thread::park_timeout(
                            poll_interval
                                .min(reply_deadline.saturating_duration_since(Instant::now())),
                        );
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                if Instant::now() >= reply_deadline {
                    return Err(fail("plugin_request_timeout"));
                }
                let remote: TaskState = serde_json::from_value(snapshot["state"].clone())
                    .map_err(|_| fail("plugin_response_invalid"))?;
                if snapshot["taskId"].as_str() != Some(task_id.as_str()) {
                    return Err(fail("plugin_response_invalid"));
                }
                let progress = snapshot
                    .get("progress")
                    .filter(|value| !value.is_null())
                    .cloned();
                if let Some(value) = &progress
                    && serde_json::to_vec(value)
                        .map_err(|_| fail("plugin_response_invalid"))?
                        .len()
                        > 16 * 1024
                {
                    return Err(fail("plugin_response_invalid"));
                }
                let error = snapshot
                    .get("error")
                    .filter(|value| !value.is_null())
                    .map(|value| serde_json::from_value::<PluginFailure>(value.clone()))
                    .transpose()
                    .map_err(|_| fail("plugin_response_invalid"))?;
                if error.is_some()
                    && !matches!(
                        remote,
                        TaskState::Failed | TaskState::Cancelled | TaskState::OutcomeUnknown
                    )
                {
                    return Err(fail("plugin_response_invalid"));
                }
                if Instant::now() >= reply_deadline {
                    return Err(fail("plugin_request_timeout"));
                }
                // Retire started remote work only after a validated terminal reply.
                remote_may_run = !remote.terminal();
                self.update_progress(&task_id, progress)?;
                if remote == TaskState::Succeeded {
                    if invalid {
                        return Err(fail("plugin_stale_context"));
                    }
                    let handoff_timeout = Duration::from_secs(30);
                    let result = lease.process.request(
                        "tasks.result",
                        json!({"context":wire_context(Instant::now() + handoff_timeout),"input":{"taskId":task_id}}),
                        handoff_timeout,
                    )?;
                    self.context(&context.context_id)?;
                    let receipt = if produces_artifacts {
                        self.inner.services.invoke(
                            &context,
                            "results.commit",
                            result.clone(),
                            &lease.data_dir,
                        )?
                    } else {
                        Value::Null
                    };
                    self.update_task(
                        &task_id,
                        TaskState::Succeeded,
                        None,
                        Some(json!({"receipt":receipt,"viewData":result["viewData"]})),
                    )?;
                    return Ok(());
                }
                if remote.terminal() {
                    self.update_task(&task_id, remote, error, None)?;
                    return Ok(());
                }
                if state != TaskState::CancelRequested {
                    self.update_task(&task_id, remote, None, None)?;
                }
                std::thread::park_timeout(
                    poll_interval.min(reply_deadline.saturating_duration_since(Instant::now())),
                );
            }
        })();
        if let Err(error) = result {
            let state = if remote_may_run
                || start_attempted
                    && matches!(
                        error.code.as_str(),
                        "plugin_process_exited"
                            | "plugin_request_timeout"
                            | "plugin_outcome_unknown"
                    ) {
                TaskState::OutcomeUnknown
            } else {
                TaskState::Failed
            };
            if state == TaskState::OutcomeUnknown {
                let _ = self.fail_instance(&context.plugin_id, &lease.process, error.clone());
            }
            let _ = self.update_task(&task_id, state, Some(error), None);
        }
        if let Ok(mut state) = self.inner.state.lock() {
            state.contexts.remove(&context.context_id);
        }
        self.inner.services.release_context(&context.context_id);
        lease.process.diagnostics.task(&task_id, false);
    }

    fn update_task(
        &self,
        id: &str,
        state: TaskState,
        error: Option<PluginFailure>,
        result: Option<Value>,
    ) -> Result<(), PluginFailure> {
        let current = self.task(id)?;
        if current.snapshot.state.terminal()
            || current.snapshot.state == state && error.is_none() && result.is_none()
        {
            return Ok(());
        }
        self.update_registry(|registry| {
            let task = registry
                .tasks
                .get_mut(id)
                .ok_or_else(|| fail("plugin_task_missing"))?;
            if task.snapshot.state.terminal() {
                return Ok(());
            }
            // A poll admitted before cancellation must never erase the durable intent.
            if task.snapshot.state == TaskState::CancelRequested && !state.terminal()
                || task.snapshot.state == TaskState::Running && state == TaskState::Admitted
            {
                return Ok(());
            }
            if task.snapshot.state == state && error.is_none() && result.is_none() {
                return Ok(());
            }
            task.snapshot.state = state;
            task.snapshot.error = error;
            task.snapshot.result = result;
            task.snapshot.revision = task
                .snapshot
                .revision
                .parse::<u64>()
                .unwrap_or_default()
                .saturating_add(1)
                .to_string();
            Ok(())
        })
    }
    fn update_progress(&self, id: &str, progress: Option<Value>) -> Result<(), PluginFailure> {
        let task = self.task(id)?;
        if task.snapshot.state.terminal() || task.snapshot.progress == progress {
            return Ok(());
        }
        self.update_registry(|registry| {
            if let Some(task) = registry.tasks.get_mut(id)
                && !task.snapshot.state.terminal()
                && task.snapshot.progress != progress
            {
                task.snapshot.progress = progress;
                task.snapshot.revision = task
                    .snapshot
                    .revision
                    .parse::<u64>()
                    .unwrap_or_default()
                    .saturating_add(1)
                    .to_string();
            }
            Ok(())
        })
    }

    pub(super) fn fail_instance(
        &self,
        plugin: &str,
        process: &Arc<PluginProcess>,
        error: PluginFailure,
    ) -> Result<(), PluginFailure> {
        process
            .diagnostics
            .push(format!("plugin process fault: {}\n", error.code).as_bytes());
        process.stop();
        let (removed, state_unavailable) = {
            // A poisoned state still owns bindings that must be revoked.
            let (mut state, unavailable) = match self.inner.state.lock() {
                Ok(state) => (state, false),
                Err(error) => (error.into_inner(), true),
            };
            if state
                .processes
                .get(plugin)
                .is_some_and(|current| Arc::ptr_eq(current, process))
            {
                state.processes.remove(plugin);
            }
            let removed = state
                .contexts
                .iter()
                .filter(|(_, binding)| {
                    binding.context.plugin_id == plugin
                        && binding.context.instance_id == process.instance_id
                })
                .map(|(id, _)| id.clone())
                .collect::<BTreeSet<_>>();
            state.contexts.retain(|id, _| !removed.contains(id));
            state
                .exports
                .retain(|_, (context, _)| !removed.contains(context));
            (removed, unavailable)
        };
        let publication = self.record_instance_failure(plugin, &process.instance_id, error);
        for context in removed {
            self.inner.services.release_context(&context);
        }
        if state_unavailable {
            Err(fail("plugin_state_unavailable"))
        } else {
            publication
        }
    }

    fn record_instance_failure(
        &self,
        plugin: &str,
        instance: &str,
        error: PluginFailure,
    ) -> Result<(), PluginFailure> {
        self.update_registry(|registry| {
            for task in registry.tasks.values_mut().filter(|task| {
                task.context.plugin_id == plugin
                    && task.context.instance_id == instance
                    && !task.snapshot.state.terminal()
            }) {
                task.snapshot.state = TaskState::OutcomeUnknown;
                task.snapshot.error = Some(error.clone());
                task.snapshot.revision = task
                    .snapshot
                    .revision
                    .parse::<u64>()
                    .unwrap_or_default()
                    .saturating_add(1)
                    .to_string();
            }
            Ok(())
        })
    }

    pub fn diagnostics(&self, plugin: &str) -> Result<Vec<PluginDiagnostic>, PluginFailure> {
        if !valid_id(plugin) {
            return Err(fail("plugin_id_invalid"));
        }
        Ok(self
            .inner
            .state
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .diagnostics
            .iter()
            .filter(|buffer| buffer.plugin == plugin)
            .flat_map(|buffer| buffer.snapshot())
            .collect())
    }
    fn task(&self, id: &str) -> Result<TaskRecord, PluginFailure> {
        self.available()?;
        let active = self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .tasks
            .get(id)
            .cloned();
        active.map(Ok).unwrap_or_else(|| {
            self.ledger()?
                .task(id)?
                .ok_or_else(|| fail("plugin_task_missing"))
        })
    }
    pub fn list_tasks(&self) -> Result<Vec<TaskSnapshot>, PluginFailure> {
        self.available()?;
        Ok(self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .tasks
            .values()
            .map(|task| task.snapshot.clone())
            .collect())
    }
    pub fn task_history(
        &self,
        plugin: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<TaskHistoryPage, PluginFailure> {
        if !valid_id(plugin) {
            return Err(fail("plugin_id_invalid"));
        }
        self.ledger()?.prune()?;
        self.ledger()?.history(plugin, cursor, limit)
    }
    pub fn clear_task_history(&self, plugin: &str) -> Result<(), PluginFailure> {
        if !valid_id(plugin) {
            return Err(fail("plugin_id_invalid"));
        }
        self.ledger()?.clear_history(plugin)
    }
    pub(super) fn task_view_call(
        &self,
        binding: &ContextBinding,
        method: &str,
        input: Value,
    ) -> Result<Value, PluginFailure> {
        let id = input["taskId"]
            .as_str()
            .ok_or_else(|| fail("plugin_task_missing"))?;
        let task = self.task(id)?;
        if task.snapshot.plugin_id != binding.context.plugin_id
            || task.context.project != binding.context.project
        {
            return Err(fail("plugin_permission_denied"));
        }
        match method {
            "tasks.cancel" => {
                self.update_task(id, TaskState::CancelRequested, None, None)?;
                Ok(Value::Null)
            }
            "tasks.result" => task
                .snapshot
                .result
                .and_then(|result| result.get("viewData").cloned())
                .ok_or_else(|| fail("plugin_result_missing")),
            _ => serde_json::to_value(task.snapshot).map_err(|_| fail("plugin_response_invalid")),
        }
    }
}
