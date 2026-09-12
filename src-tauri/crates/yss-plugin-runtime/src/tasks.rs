use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests {
    use super::*;
    struct Host;
    fn record(index: usize, instance: &str) -> TaskRecord {
        let task_id = format!("task-{index}");
        let operation = operation_id(crate::ledger::now_ms(), &format!("test-{index}")).unwrap();
        TaskRecord {
            snapshot: TaskSnapshot {
                task_id: task_id.clone(),
                operation_id: operation.clone(),
                plugin_id: "example.compute".into(),
                package_digest: "digest".into(),
                state: TaskState::Admitted,
                revision: "0".into(),
                error: None,
                result: None,
                progress: None,
            },
            context: CallContext {
                context_id: format!("context-{index}"),
                plugin_id: "example.compute".into(),
                installation_generation: "1".into(),
                instance_id: instance.into(),
                package_digest: "digest".into(),
                project: None,
                task_id: Some(task_id),
                operation_id: Some(operation),
                parameters_hash: Some("parameters".into()),
                granted_budget: ResourceBudget::default(),
            },
            parameters_hash: "parameters".into(),
        }
    }

    #[test]
    fn archived_tasks_release_admission_and_keep_retry_receipts_after_cleanup_and_restart() {
        let root =
            std::env::temp_dir().join(format!("yssbi-task-archive-{}", uuid::Uuid::new_v4()));
        let manager = PluginManager::new(&root, Arc::new(Host)).unwrap();
        let first = record(0, "one");
        for index in 0..160 {
            let task = if index == 0 {
                first.clone()
            } else {
                record(index, "one")
            };
            assert!(manager.admit_record(task.clone(), 2).unwrap().is_none());
            manager
                .update_task(
                    &task.snapshot.task_id,
                    TaskState::Succeeded,
                    None,
                    Some(
                        json!({"receipt":{"resource":"project-result"},"viewData":{"value":index}}),
                    ),
                )
                .unwrap();
        }
        assert!(manager.list_tasks().unwrap().is_empty());
        let page = manager.task_history("example.compute", None, 25).unwrap();
        assert_eq!(page.tasks.len(), 25);
        let second = manager
            .task_history("example.compute", page.next_cursor.as_deref(), 25)
            .unwrap();
        assert!(page.tasks.iter().all(|task| {
            second
                .tasks
                .iter()
                .all(|other| task.task_id != other.task_id)
        }));
        manager.clear_task_history("example.compute").unwrap();
        assert!(
            manager
                .task_history("example.compute", None, 25)
                .unwrap()
                .tasks
                .is_empty()
        );
        drop(manager);
        let manager = PluginManager::new(&root, Arc::new(Host)).unwrap();
        let replay = manager.admit_record(first.clone(), 2).unwrap().unwrap();
        assert_eq!(replay.state, TaskState::Succeeded);
        assert_eq!(
            replay.result.as_ref().unwrap()["receipt"]["resource"],
            "project-result"
        );
        assert!(replay.result.as_ref().unwrap().get("viewData").is_none());
        let mut conflict = first;
        conflict.parameters_hash = "changed".into();
        assert_eq!(
            manager.admit_record(conflict, 2).unwrap_err().code,
            "plugin_operation_conflict"
        );
        manager.admit_record(record(200, "one"), 2).unwrap();
        manager.admit_record(record(201, "one"), 2).unwrap();
        assert_eq!(
            manager
                .admit_record(record(202, "one"), 2)
                .unwrap_err()
                .code,
            "plugin_resource_exhausted"
        );
        drop(manager);
        std::fs::remove_dir_all(root).unwrap();
    }

    impl HostServices for Host {
        fn current_project(&self) -> Result<Option<ProjectContext>, PluginFailure> {
            Ok(None)
        }
        fn invoke(
            &self,
            _: &CallContext,
            _: &str,
            _: Value,
            _: &Path,
        ) -> Result<Value, PluginFailure> {
            unreachable!()
        }
    }
    #[test]
    fn late_poll_cannot_erase_cancel_or_change_a_terminal_result() {
        let root =
            std::env::temp_dir().join(format!("yssbi-cancel-ledger-{}", uuid::Uuid::new_v4()));
        let manager = PluginManager::new(&root, Arc::new(Host)).unwrap();
        let context = CallContext {
            context_id: "context".into(),
            plugin_id: "example.compute".into(),
            installation_generation: "1".into(),
            instance_id: "instance".into(),
            package_digest: "digest".into(),
            project: None,
            task_id: Some("task".into()),
            operation_id: Some("operation".into()),
            parameters_hash: Some("parameters".into()),
            granted_budget: ResourceBudget::default(),
        };
        manager
            .update_registry(|registry| {
                registry.tasks.insert(
                    "task".into(),
                    TaskRecord {
                        snapshot: TaskSnapshot {
                            task_id: "task".into(),
                            operation_id: "operation".into(),
                            plugin_id: "example.compute".into(),
                            package_digest: "digest".into(),
                            state: TaskState::Admitted,
                            revision: "0".into(),
                            error: None,
                            result: None,
                            progress: None,
                        },
                        context,
                        parameters_hash: "parameters".into(),
                    },
                );
                Ok(())
            })
            .unwrap();
        manager
            .update_task("task", TaskState::CancelRequested, None, None)
            .unwrap();
        manager
            .update_task("task", TaskState::Running, None, None)
            .unwrap();
        assert_eq!(
            manager.task("task").unwrap().snapshot.state,
            TaskState::CancelRequested
        );
        manager
            .update_task("task", TaskState::Cancelled, None, None)
            .unwrap();
        manager
            .update_task("task", TaskState::Succeeded, None, Some(json!({})))
            .unwrap();
        assert_eq!(
            manager.task("task").unwrap().snapshot.state,
            TaskState::Cancelled
        );
        drop(manager);
        fs::remove_dir_all(root).unwrap();
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct TaskRecord {
    pub snapshot: TaskSnapshot,
    pub(super) context: CallContext,
    pub(super) parameters_hash: String,
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
        context.parameters_hash = Some(parameters_hash.clone());
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
            parameters_hash: parameters_hash.clone(),
        };
        if let Some(snapshot) =
            self.admit_record(record.clone(), registration.granted_budget.active_tasks)?
        {
            return serde_json::to_value(snapshot).map_err(|_| fail("plugin_response_invalid"));
        }
        self.inner
            .state
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .contexts
            .insert(
                context.context_id.clone(),
                ContextBinding {
                    context: context.clone(),
                    window: String::new(),
                    view_id: String::new(),
                },
            );
        let manager = self.clone();
        let duration = Duration::from_millis(timeout_ms);
        std::thread::spawn(move || {
            manager.monitor_task(TaskExecution {
                task_id,
                task_type,
                parameters,
                context,
                lease,
                duration,
                produces_artifacts,
            })
        });
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
                if previous.parameters_hash != record.parameters_hash {
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
        lease.process.diagnostics.task(&task_id, true);
        let wire_context = json!({"contextId":context.context_id,"project":context.project,"remainingBudgetMs":duration.as_millis() as u64});
        let result = (|| {
            let mut cancellation_started = None;
            let mut last_storage_check = Instant::now();
            lease.process.request("tasks.start",json!({"context":wire_context,"input":{"taskId":task_id,"taskType":task_type,"parameters":parameters}}),Duration::from_secs(30).min(duration))?;
            loop {
                if last_storage_check.elapsed() >= Duration::from_secs(2) {
                    if let Err(error) = self.enforce_private_budget(&context.plugin_id, 0) {
                        self.fail_instance(&context.plugin_id, &lease.process, error)?;
                        return Ok(());
                    }
                    last_storage_check = Instant::now();
                }
                let state = self.task(&task_id)?.snapshot.state;
                if state.terminal() {
                    return Ok(());
                }
                let invalid = self.context(&context.context_id).is_err();
                if state == TaskState::CancelRequested || invalid || Instant::now() >= deadline {
                    let started = cancellation_started.get_or_insert_with(Instant::now);
                    if started.elapsed() > Duration::from_secs(10) {
                        self.fail_instance(
                            &context.plugin_id,
                            &lease.process,
                            fail("plugin_cancel_timeout"),
                        )?;
                        return Err(fail("plugin_outcome_unknown"));
                    }
                    let cancelled = lease.process.request(
                        "tasks.cancel",
                        json!({"context":wire_context,"input":{"taskId":task_id}}),
                        Duration::from_secs(10),
                    );
                    if let Err(error) = cancelled {
                        lease
                            .process
                            .diagnostics
                            .push(format!("tasks.cancel: {}\n", error.code).as_bytes());
                        self.fail_instance(
                            &context.plugin_id,
                            &lease.process,
                            fail("plugin_cancel_failed"),
                        )?;
                        return Err(fail("plugin_outcome_unknown"));
                    }
                }
                let snapshot = lease.process.request(
                    "tasks.get",
                    json!({"context":wire_context,"input":{"taskId":task_id}}),
                    Duration::from_secs(10),
                )?;
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
                self.update_progress(&task_id, progress)?;
                if remote == TaskState::Succeeded {
                    if invalid {
                        return Err(fail("plugin_stale_context"));
                    }
                    let result = lease.process.request(
                        "tasks.result",
                        json!({"context":wire_context,"input":{"taskId":task_id}}),
                        Duration::from_secs(30),
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
                    let error = snapshot
                        .get("error")
                        .filter(|value| !value.is_null())
                        .and_then(|value| serde_json::from_value(value.clone()).ok());
                    self.update_task(&task_id, remote, error, None)?;
                    return Ok(());
                }
                if state != TaskState::CancelRequested {
                    self.update_task(&task_id, remote, None, None)?;
                }
                std::thread::park_timeout(Duration::from_millis(250));
            }
        })();
        if let Err(error) = result {
            let state = if matches!(
                error.code.as_str(),
                "plugin_process_exited" | "plugin_request_timeout" | "plugin_outcome_unknown"
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
        let publication = self.record_instance_failure(plugin, &process.instance_id, error);
        let removed = {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?;
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
            removed
        };
        process.stop();
        for context in removed {
            self.inner.services.release_context(&context);
        }
        publication
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
