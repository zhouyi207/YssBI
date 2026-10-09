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
    }
}

#[test]
fn archived_tasks_release_admission_and_keep_retry_receipts_after_cleanup_and_restart() {
    let root = std::env::temp_dir().join(format!("yssbi-task-archive-{}", uuid::Uuid::new_v4()));
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
                Some(json!({"receipt":{"resource":"project-result"},"viewData":{"value":index}})),
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
    conflict.context.parameters_hash = Some("changed".into());
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
    fn invoke(&self, _: &CallContext, _: &str, _: Value, _: &Path) -> Result<Value, PluginFailure> {
        unreachable!()
    }
}
#[test]
fn late_poll_cannot_erase_cancel_or_change_a_terminal_result() {
    let root = std::env::temp_dir().join(format!("yssbi-cancel-ledger-{}", uuid::Uuid::new_v4()));
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

#[cfg(unix)]
mod observation {
    use super::*;
    use std::{
        net::Shutdown,
        os::unix::net::UnixStream,
        sync::atomic::{AtomicUsize, Ordering},
    };
    use yss_plugin_sdk::{Handler, Peer};

    struct Fixture {
        root: PathBuf,
        manager: PluginManager,
        process: Arc<PluginProcess>,
        remote: Arc<Peer>,
        socket: UnixStream,
    }

    impl Fixture {
        fn new(handler: Handler) -> Self {
            let root = std::env::temp_dir()
                .join(format!("yssbi-task-observation-{}", uuid::Uuid::new_v4()));
            let manager = PluginManager::new(&root, Arc::new(Host)).unwrap();
            let budget = ResourceBudget::default();
            let manifest = PluginManifest {
                schema_version: 1,
                id: "example.compute".into(),
                name: "Compute".into(),
                description: "Task observation fixture".into(),
                publisher: "example".into(),
                version: "1.0.0".into(),
                host_api: "^1".into(),
                protocol: ProtocolRange {
                    major: PROTOCOL_MAJOR,
                    min_minor: 0,
                    max_minor: 0,
                    required_features: vec![],
                },
                target: current_target().unwrap().into(),
                executable: "plugin".into(),
                execution: ExecutionMode::TrustedNative,
                contributes: Contributions {
                    views: vec![],
                    commands: vec![],
                    task_types: vec![],
                },
                permissions: vec![],
                ui_methods: vec![],
                resource_budget: budget.clone(),
                cache_directories: vec![],
            };
            manager
                .update_registry(|registry| {
                    registry.entries.insert(
                        manifest.id.clone(),
                        Registration {
                            manifest,
                            digest: "digest".into(),
                            generation: 1,
                            signer: "fixture".into(),
                            enabled: true,
                            files: vec![],
                            granted_budget: budget.clone(),
                        },
                    );
                    Ok(())
                })
                .unwrap();
            let (socket, remote_socket) = UnixStream::pair().unwrap();
            let peer = Peer::connect(
                socket.try_clone().unwrap(),
                socket.try_clone().unwrap(),
                budget.clone(),
                Arc::new(|_, _| Err(fail("plugin_method_unknown"))),
            );
            let remote = Peer::connect(
                remote_socket.try_clone().unwrap(),
                remote_socket,
                budget,
                handler,
            );
            let process = PluginProcess::with_test_peer("one", peer);
            for (index, instance) in ["one", "one", "two", "one"].into_iter().enumerate() {
                let task = record(index, instance);
                manager.admit_record(task.clone(), 4).unwrap();
                if index == 3 {
                    manager
                        .update_task(&task.snapshot.task_id, TaskState::Succeeded, None, None)
                        .unwrap();
                } else {
                    manager.inner.state.lock().unwrap().contexts.insert(
                        task.context.context_id.clone(),
                        ContextBinding {
                            context: task.context,
                            window: String::new(),
                            view_id: String::new(),
                        },
                    );
                }
            }
            manager
                .inner
                .state
                .lock()
                .unwrap()
                .processes
                .insert("example.compute".into(), process.clone());
            Self {
                root,
                manager,
                process,
                remote,
                socket,
            }
        }

        fn run(&self) {
            let task = self.manager.task("task-0").unwrap();
            self.process.leases.fetch_add(1, Ordering::AcqRel);
            self.manager.monitor_task(TaskExecution {
                task_id: task.snapshot.task_id,
                task_type: "compute".into(),
                parameters: Value::Null,
                context: task.context,
                lease: Arc::new(PluginLease {
                    process: self.process.clone(),
                    data_dir: self.root.join("extensions/data/example.compute"),
                }),
                duration: Duration::from_secs(5),
                produces_artifacts: false,
            });
        }

        fn snapshot(&self, index: usize) -> TaskSnapshot {
            self.manager
                .list_tasks()
                .unwrap()
                .into_iter()
                .chain(
                    self.manager
                        .task_history("example.compute", None, 10)
                        .unwrap()
                        .tasks,
                )
                .find(|task| task.task_id == format!("task-{index}"))
                .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.process.stop();
            self.remote.close();
            let _ = self.socket.shutdown(Shutdown::Both);
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn temporary_poll_pressure_keeps_observing_the_started_task() {
        let polls = Arc::new(AtomicUsize::new(0));
        let counted = polls.clone();
        let fixture = Fixture::new(Arc::new(move |_, request| match request.method.as_str() {
            "tasks.start" => Ok(Value::Null),
            "tasks.get" if counted.fetch_add(1, Ordering::AcqRel) == 0 => {
                Err(fail("plugin_resource_exhausted"))
            }
            "tasks.get" => Ok(json!({"taskId":"task-0","state":"succeeded"})),
            "tasks.result" => Ok(json!({"viewData":{"value":42}})),
            _ => Err(fail("plugin_method_unknown")),
        }));
        fixture.run();
        let task = fixture.snapshot(0);
        assert_eq!(task.state, TaskState::Succeeded);
        let binding = fixture.manager.inner.state.lock().unwrap().contexts["context-1"].clone();
        let result = fixture
            .manager
            .task_view_call(&binding, "tasks.result", json!({"taskId":"task-0"}))
            .unwrap();
        assert_eq!(result["value"], 42);
        assert_eq!(polls.load(Ordering::Acquire), 2);
        assert!(fixture.process.is_running());
        assert_eq!(fixture.snapshot(1).state, TaskState::Admitted);
    }

    #[test]
    fn invalid_poll_retires_the_instance_without_claiming_task_failure() {
        let fixture = Fixture::new(Arc::new(|_, request| match request.method.as_str() {
            "tasks.start" => Ok(Value::Null),
            "tasks.get" => Ok(json!({"taskId":"wrong-task","state":"running"})),
            _ => Err(fail("plugin_method_unknown")),
        }));
        fixture.run();
        for index in [0, 1] {
            let task = fixture.snapshot(index);
            assert_eq!(task.state, TaskState::OutcomeUnknown);
            assert_eq!(task.error.unwrap().code, "plugin_response_invalid");
        }
        assert_eq!(fixture.snapshot(2).state, TaskState::Admitted);
        assert_eq!(fixture.snapshot(3).state, TaskState::Succeeded);
        assert!(!fixture.process.is_running());
        let state = fixture.manager.inner.state.lock().unwrap();
        assert!(!state.processes.contains_key("example.compute"));
        assert!(!state.contexts.contains_key("context-0"));
        assert!(!state.contexts.contains_key("context-1"));
        assert!(state.contexts.contains_key("context-2"));
    }

    #[test]
    fn result_rejection_after_remote_completion_preserves_the_instance() {
        let fixture = Fixture::new(Arc::new(|_, request| match request.method.as_str() {
            "tasks.start" => Ok(Value::Null),
            "tasks.get" => Ok(json!({"taskId":"task-0","state":"succeeded"})),
            "tasks.result" => Err(fail("plugin_result_missing")),
            _ => Err(fail("plugin_method_unknown")),
        }));
        fixture.run();
        let task = fixture.snapshot(0);
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.error.unwrap().code, "plugin_result_missing");
        assert!(fixture.process.is_running());
        assert_eq!(fixture.snapshot(1).state, TaskState::Admitted);
    }

    #[test]
    fn fault_teardown_precedes_blocked_task_publication() {
        let fixture = Fixture::new(Arc::new(|_, _| Err(fail("plugin_method_unknown"))));
        let publication = fixture.manager.inner.commit.lock().unwrap();
        let manager = fixture.manager.clone();
        let process = fixture.process.clone();
        let fault = std::thread::spawn(move || {
            manager.fail_instance("example.compute", &process, fail("plugin_response_invalid"))
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let retired = loop {
            let state = fixture.manager.inner.state.lock().unwrap();
            let retired = !fixture.process.is_running()
                && !state.contexts.contains_key("context-0")
                && !state.contexts.contains_key("context-1");
            drop(state);
            if retired || Instant::now() >= deadline {
                break retired;
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        drop(publication);
        fault.join().unwrap().unwrap();
        assert!(retired, "fault teardown waited for task publication");
        assert_eq!(fixture.snapshot(0).state, TaskState::OutcomeUnknown);
        assert_eq!(fixture.snapshot(1).state, TaskState::OutcomeUnknown);
        assert_eq!(fixture.snapshot(2).state, TaskState::Admitted);
        assert_eq!(fixture.snapshot(3).state, TaskState::Succeeded);
    }

    #[test]
    fn fault_teardown_survives_poisoned_runtime_state() {
        let fixture = Fixture::new(Arc::new(|_, _| Err(fail("plugin_method_unknown"))));
        {
            let mut state = fixture.manager.inner.state.lock().unwrap();
            for index in 0..3 {
                state.exports.insert(
                    format!("export-{index}"),
                    (
                        format!("context-{index}"),
                        fixture.root.join(format!("lease-{index}")),
                    ),
                );
            }
        }
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = fixture.manager.inner.state.lock().unwrap();
            panic!("runtime-state fault fixture");
        }));
        assert!(poisoned.is_err());
        let error = fixture
            .manager
            .fail_instance(
                "example.compute",
                &fixture.process,
                fail("plugin_response_invalid"),
            )
            .unwrap_err();
        assert_eq!(error.code, "plugin_state_unavailable");
        assert!(!fixture.process.is_running());
        assert_eq!(fixture.snapshot(0).state, TaskState::OutcomeUnknown);
        assert_eq!(fixture.snapshot(1).state, TaskState::OutcomeUnknown);
        assert_eq!(fixture.snapshot(2).state, TaskState::Admitted);
        assert_eq!(fixture.snapshot(3).state, TaskState::Succeeded);
        assert_eq!(
            fixture.manager.list().err().unwrap().code,
            "plugin_state_unavailable"
        );
        let state = fixture
            .manager
            .inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert!(!state.processes.contains_key("example.compute"));
        assert!(!state.contexts.contains_key("context-0"));
        assert!(!state.contexts.contains_key("context-1"));
        assert!(state.contexts.contains_key("context-2"));
        assert!(!state.exports.contains_key("export-0"));
        assert!(!state.exports.contains_key("export-1"));
        assert!(state.exports.contains_key("export-2"));
    }

    #[test]
    fn task_start_panic_retires_unconfirmed_remote_work() {
        let started = Arc::new(AtomicUsize::new(0));
        let observed = started.clone();
        let fixture = Fixture::new(Arc::new(move |_, request| {
            if request.method == "tasks.start" {
                observed.fetch_add(1, Ordering::AcqRel);
                panic!("task-start handler after remote admission");
            }
            Err(fail("plugin_method_unknown"))
        }));
        fixture.run();
        assert_eq!(started.load(Ordering::Acquire), 1);
        for index in [0, 1] {
            let task = fixture.snapshot(index);
            assert_eq!(task.state, TaskState::OutcomeUnknown);
            assert_eq!(task.error.unwrap().code, "plugin_handler_failed");
        }
        assert_eq!(fixture.snapshot(2).state, TaskState::Admitted);
        assert_eq!(fixture.snapshot(3).state, TaskState::Succeeded);
        assert!(!fixture.process.is_running());
        let state = fixture.manager.inner.state.lock().unwrap();
        assert!(!state.contexts.contains_key("context-0"));
        assert!(!state.contexts.contains_key("context-1"));
        assert!(state.contexts.contains_key("context-2"));
    }

    #[test]
    fn task_start_local_rejection_keeps_the_instance() {
        let fixture = Fixture::new(Arc::new(|_, _| {
            panic!("a locally rejected task must not reach its handler")
        }));
        fixture
            .process
            .peer
            .apply_budget(ResourceBudget {
                frame_bytes: 64,
                ..ResourceBudget::default()
            })
            .unwrap();
        fixture.run();
        let task = fixture.snapshot(0);
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.error.unwrap().code, "plugin_payload_too_large");
        assert!(fixture.process.is_running());
        assert_eq!(fixture.snapshot(1).state, TaskState::Admitted);
        assert_eq!(fixture.snapshot(2).state, TaskState::Admitted);
        assert_eq!(fixture.snapshot(3).state, TaskState::Succeeded);
    }
}
