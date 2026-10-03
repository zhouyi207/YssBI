use arrow::array::{ArrayRef, Float64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier, Mutex},
    time::{Duration, Instant},
};
use yss_plugin_protocol::*;
use yss_plugin_runtime::PluginManager;

fn operation(nonce: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    operation_id(now, nonce).unwrap()
}

struct Host {
    project: Mutex<Option<ProjectContext>>,
    root: PathBuf,
}
impl HostServices for Host {
    fn current_project(&self) -> Result<Option<ProjectContext>, PluginFailure> {
        Ok(self.project.lock().unwrap().clone())
    }
    fn invoke(
        &self,
        context: &CallContext,
        method: &str,
        input: Value,
        exchange: &Path,
    ) -> Result<Value, PluginFailure> {
        if context.project != *self.project.lock().unwrap() {
            return Err(PluginFailure::new("plugin_stale_context"));
        }
        match method {
            "data.snapshot" => {
                let names = input["columns"].as_array().unwrap();
                let columns = names
                    .iter()
                    .map(|name| {
                        let name = name.as_str().unwrap();
                        let values = match name {
                            "x" => vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
                            "y" => vec![3.1, 5.0, 7.2, 8.9, 11.1, 13.0],
                            _ => panic!("unexpected column"),
                        };
                        Arc::new(Float64Array::from(values)) as ArrayRef
                    })
                    .collect();
                let schema = Arc::new(Schema::new(
                    names
                        .iter()
                        .map(|name| Field::new(name.as_str().unwrap(), DataType::Float64, false))
                        .collect::<Vec<_>>(),
                ));
                let data = RecordBatch::try_new(schema.clone(), columns).unwrap();
                let path = exchange.join("fixture.arrow");
                let mut writer = arrow::ipc::writer::FileWriter::try_new_with_options(
                    fs::File::create(&path).unwrap(),
                    &schema,
                    arrow::ipc::writer::IpcWriteOptions::try_new(
                        8,
                        false,
                        arrow::ipc::MetadataVersion::V5,
                    )
                    .unwrap(),
                )
                .unwrap();
                writer.write(&data).unwrap();
                writer.finish().unwrap();
                Ok(json!({"leaseId":"fixture","path":path,"sourceRevision":"1"}))
            }
            "data.release" => Ok(Value::Null),
            "results.commit" => {
                let result = input["viewData"].clone();
                fs::write(
                    self.root.join("retained-result.json"),
                    serde_json::to_vec(&result).unwrap(),
                )
                .unwrap();
                Ok(json!({"resourceRef":"retained-result.json"}))
            }
            _ => Err(PluginFailure::new("plugin_method_unknown")),
        }
    }
}
fn wait(manager: &PluginManager, session: &str, id: &str, timeout: Duration) -> Value {
    let start = Instant::now();
    loop {
        let value = manager
            .call_view(session, "test", "tasks.get", json!({"taskId":id}))
            .unwrap_or_else(|error| {
                panic!(
                    "{error:?}; diagnostics: {:?}",
                    manager.diagnostics("yssbi.julia")
                )
            });
        let state = value["state"].as_str().unwrap();
        if ["succeeded", "failed", "cancelled", "outcomeUnknown"].contains(&state) {
            return value;
        }
        assert!(start.elapsed() < timeout, "task did not terminate");
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[test]
#[ignore = "requires a packaged executable and a compatible Julia installation"]
fn independently_installs_executes_cancels_and_uninstalls_a_native_extension() {
    let package = PathBuf::from(
        std::env::var_os("YSSBI_PLUGIN_TEST_PACKAGE").expect("YSSBI_PLUGIN_TEST_PACKAGE"),
    );
    let root =
        std::env::temp_dir().join(format!("yssbi-native-extension-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let host = Arc::new(Host {
        project: Mutex::new(Some(ProjectContext {
            project_instance_id: "project".into(),
            project_session_id: "session".into(),
        })),
        root: root.clone(),
    });
    let manager = PluginManager::new(&root, host.clone()).unwrap();
    let package = manager
        .inspect(&package)
        .map(|inspection| (package, inspection))
        .unwrap();
    manager
        .install(
            &package.0,
            &package.1.package_digest,
            &operation("install-test"),
            true,
            None,
        )
        .unwrap();
    let runtime = manager
        .attach_view("yssbi.julia", "runtime", "test")
        .unwrap();
    assert!(runtime.html.contains("Content-Security-Policy"));
    let dependency = manager
        .call_view(
            &runtime.session_id,
            "test",
            "dependencies.inspect",
            Value::Null,
        )
        .unwrap();
    assert_eq!(
        dependency["runtimeState"], "ready",
        "compatible Julia must be installed for this test"
    );
    let interrupted = manager.call_view(&runtime.session_id, "test", "tasks.start", json!({"operationId":operation("cancel-prepare"),"taskType":"runtime.prepare","parameters":{},"timeoutMs":600000})).unwrap();
    let interrupted_id = interrupted["taskId"].as_str().unwrap();
    // Give the real preparation process time to enter Pkg, then cancel this worker.
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(manager.call_view(&runtime.session_id, "test", "tasks.start", json!({"operationId":operation("overlap-prepare"),"taskType":"runtime.prepare","parameters":{},"timeoutMs":600000})).unwrap_err().code, "plugin_resource_exhausted");
    let cancel_started = Instant::now();
    manager
        .call_view(
            &runtime.session_id,
            "test",
            "tasks.cancel",
            json!({"taskId":interrupted_id}),
        )
        .unwrap();
    let interrupted = wait(
        &manager,
        &runtime.session_id,
        interrupted_id,
        Duration::from_secs(15),
    );
    assert_eq!(interrupted["state"], "cancelled", "{interrupted}");
    assert!(cancel_started.elapsed() < Duration::from_secs(15));
    assert!(
        manager
            .call_view(
                &runtime.session_id,
                "test",
                "dependencies.inspect",
                Value::Null
            )
            .is_ok()
    );
    let preparation=manager.call_view(&runtime.session_id,"test","tasks.start",json!({"operationId":operation("prepare-test"),"taskType":"runtime.prepare","parameters":{},"timeoutMs":600000})).unwrap();
    let prepared = wait(
        &manager,
        &runtime.session_id,
        preparation["taskId"].as_str().unwrap(),
        Duration::from_secs(620),
    );
    assert_eq!(prepared["state"], "succeeded", "{prepared}");
    let analysis = manager
        .attach_view("yssbi.julia", "analysis", "test")
        .unwrap();
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/bayes/linear_normal/simple.json")).unwrap();
    let spec = &fixture["modelSpec"];
    let mut draft = json!({"formulaText":"y = a*x+b","rawResponse":{"type":"symbol","name":"y"},"boundResponse":{"type":"data_variable","name":"y"},"symbols":[{"name":"y","role":"dependent","inferredRole":"dependent","userEdited":false},{"name":"x","role":"independent","inferredRole":"independent","userEdited":false},{"name":"a","role":"parameter","inferredRole":"parameter","userEdited":false},{"name":"b","role":"parameter","inferredRole":"parameter","userEdited":false},{"name":"sigma","role":"parameter","inferredRole":"parameter","userEdited":false}],"dataset":{"sourceType":"table","sourceId":"fixture-linear-normal","columns":[{"name":"x","dtype":"number","nullable":false},{"name":"y","dtype":"number","nullable":false}]},"responseBinding":{"symbol":"y","column":"y"},"dataBindings":{"x":"x"},"boundPredictor":spec["predictor"],"likelihood":spec["likelihood"],"parameters":spec["parameters"],"sampler":spec["sampler"]});
    draft["sampler"]["chains"] = json!(1);
    draft["sampler"]["samples"] = json!(64);
    draft["sampler"]["warmup"] = json!(64);
    let fit_operation = operation("fit-test");
    let admitted=manager.call_view(&analysis.session_id,"test","tasks.start",json!({"operationId":fit_operation,"taskType":"bayes.inference","parameters":draft,"timeoutMs":180000})).unwrap();
    let task_id = admitted["taskId"].as_str().unwrap();
    let completed = wait(
        &manager,
        &analysis.session_id,
        task_id,
        Duration::from_secs(200),
    );
    assert_eq!(completed["state"], "succeeded", "{completed}");
    let result = manager
        .call_view(
            &analysis.session_id,
            "test",
            "tasks.result",
            json!({"taskId":task_id}),
        )
        .unwrap();
    assert!(result["summaries"].as_array().unwrap().len() >= 3);
    assert!(root.join("retained-result.json").exists());
    let duplicate=manager.call_view(&analysis.session_id,"test","tasks.start",json!({"operationId":fit_operation,"taskType":"bayes.inference","parameters":draft,"timeoutMs":180000})).unwrap();
    assert_eq!(duplicate["taskId"], task_id);
    draft["sampler"]["samples"] = json!(100_000);
    let long=manager.call_view(&analysis.session_id,"test","tasks.start",json!({"operationId":operation("cancel-test"),"taskType":"bayes.inference","parameters":draft,"timeoutMs":180000})).unwrap();
    let long_id = long["taskId"].as_str().unwrap();
    let warmup = draft["sampler"]["warmup"].as_u64().unwrap();
    let sampling_started = Instant::now();
    loop {
        let snapshot = manager
            .call_view(
                &analysis.session_id,
                "test",
                "tasks.get",
                json!({"taskId":long_id}),
            )
            .unwrap();
        let state = snapshot["state"].as_str().unwrap();
        let progress = &snapshot["progress"];
        if state == "running"
            && progress["stage"] == "sampling"
            && let (Some(completed), Some(total)) =
                (progress["completed"].as_u64(), progress["total"].as_u64())
            && completed > warmup
            && completed < total
        {
            eprintln!(
                "Sampling cancellation gate: completed={completed}, total={total}, warmup={warmup}"
            );
            break;
        }
        assert!(
            !["succeeded", "failed", "cancelled", "outcomeUnknown"].contains(&state),
            "task ended before sampling cancellation gate: {snapshot}"
        );
        assert!(
            sampling_started.elapsed() < Duration::from_secs(180),
            "task did not reach sampling cancellation gate: {snapshot}"
        );
        std::thread::park_timeout(Duration::from_millis(250));
    }
    let cancel_started = Instant::now();
    manager
        .call_view(
            &analysis.session_id,
            "test",
            "tasks.cancel",
            json!({"taskId":long_id}),
        )
        .unwrap();
    let cancelled = wait(
        &manager,
        &analysis.session_id,
        long_id,
        Duration::from_secs(30),
    );
    assert_eq!(cancelled["state"], "cancelled", "{cancelled}");
    let cancel_elapsed = cancel_started.elapsed();
    eprintln!("Sampling cancellation terminal latency: {cancel_elapsed:?}");
    assert!(cancel_elapsed < Duration::from_secs(30));
    *host.project.lock().unwrap() = None;
    assert_eq!(
        manager
            .call_view(
                &analysis.session_id,
                "test",
                "tasks.get",
                json!({"taskId":task_id})
            )
            .unwrap_err()
            .code,
        "plugin_stale_context"
    );
    manager.detach_view(&analysis.session_id, "test").unwrap();
    manager.detach_view(&runtime.session_id, "test").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    manager.uninstall("yssbi.julia").unwrap();
    assert!(manager.list().unwrap().is_empty());
    assert!(root.join("retained-result.json").exists());
    drop(manager);
    drop(host);
    let _ = fs::remove_dir_all(root);
}

#[test]
#[ignore = "requires a packaged extension; does not prepare or launch Julia"]
fn concurrent_activation_shares_process_and_respects_view_leases() {
    let package =
        PathBuf::from(std::env::var_os("YSSBI_PLUGIN_TEST_PACKAGE").expect("test package"));
    let root =
        std::env::temp_dir().join(format!("yssbi-plugin-activation-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let manager = PluginManager::new(
        &root,
        Arc::new(Host {
            project: Mutex::new(None),
            root: root.clone(),
        }),
    )
    .unwrap();
    let inspected = manager.inspect(&package).unwrap();
    let id = inspected.manifest.id.clone();
    manager
        .install(
            &package,
            &inspected.package_digest,
            &operation("activation-install"),
            true,
            None,
        )
        .unwrap();
    let barrier = Arc::new(Barrier::new(4));
    let threads = (0..3)
        .map(|_| {
            let manager = manager.clone();
            let id = id.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                manager.acquire(&id)
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let leases = threads
        .into_iter()
        .map(|thread| thread.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert!(
        leases
            .iter()
            .all(|lease| Arc::ptr_eq(&lease.process, &leases[0].process))
    );
    assert_eq!(manager.uninstall(&id).unwrap_err().code, "plugin_busy");
    drop(leases);

    let view = &inspected
        .manifest
        .contributes
        .views
        .iter()
        .find(|view| view.scope == ViewScope::Application)
        .unwrap()
        .id;
    let limit = inspected.manifest.resource_budget.views;
    let sessions = (0..limit)
        .map(|index| {
            manager
                .attach_view(&id, view, &format!("window-{index}"))
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        manager.attach_view(&id, view, "extra").unwrap_err().code,
        "plugin_view_limit"
    );
    assert_eq!(
        manager
            .detach_view(&sessions[0].session_id, "wrong-window")
            .unwrap_err()
            .code,
        "plugin_permission_denied"
    );
    manager
        .detach_view(&sessions[0].session_id, "window-0")
        .unwrap();
    manager
        .detach_view(&sessions[0].session_id, "window-0")
        .unwrap();
    let replacement = manager.attach_view(&id, view, "extra").unwrap();
    manager
        .detach_view(&replacement.session_id, "extra")
        .unwrap();
    for (index, session) in sessions.iter().enumerate().skip(1) {
        manager
            .detach_view(&session.session_id, &format!("window-{index}"))
            .unwrap();
    }
    manager.uninstall(&id).unwrap();
    drop(manager);
    fs::remove_dir_all(root).unwrap();
}

type ProjectReadGate = (std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>);

#[derive(Default)]
struct GatedViewHost {
    next_project_read: Mutex<Option<ProjectReadGate>>,
    next_invoke: Mutex<Option<ProjectReadGate>>,
    resources: Mutex<std::collections::BTreeSet<String>>,
}
impl GatedViewHost {
    fn pause_next_project_read(&self) -> ProjectReadGate {
        let (entered, ready) = std::sync::mpsc::channel();
        let (resume, proceed) = std::sync::mpsc::channel();
        *self.next_project_read.lock().unwrap() = Some((entered, proceed));
        (resume, ready)
    }
    fn pause_next_invoke(&self) -> ProjectReadGate {
        let (entered, ready) = std::sync::mpsc::channel();
        let (resume, proceed) = std::sync::mpsc::channel();
        *self.next_invoke.lock().unwrap() = Some((entered, proceed));
        (resume, ready)
    }
}
impl HostServices for GatedViewHost {
    fn current_project(&self) -> Result<Option<ProjectContext>, PluginFailure> {
        let gate = self.next_project_read.lock().unwrap().take();
        if let Some((entered, proceed)) = gate {
            entered.send(()).unwrap();
            proceed.recv_timeout(Duration::from_secs(30)).unwrap();
        }
        Ok(Some(ProjectContext {
            project_instance_id: "project".into(),
            project_session_id: "session".into(),
        }))
    }
    fn invoke(
        &self,
        context: &CallContext,
        method: &str,
        _: Value,
        _: &Path,
    ) -> Result<Value, PluginFailure> {
        if method != "data.list" {
            return Err(PluginFailure::new("plugin_method_unknown"));
        }
        let gate = self.next_invoke.lock().unwrap().take();
        if let Some((entered, proceed)) = gate {
            entered.send(()).unwrap();
            proceed.recv_timeout(Duration::from_secs(30)).unwrap();
        }
        self.resources
            .lock()
            .unwrap()
            .insert(context.context_id.clone());
        Ok(json!({"datasets":[]}))
    }
    fn release_context(&self, context_id: &str) {
        self.resources.lock().unwrap().remove(context_id);
    }
}

fn view_race_fixture() -> (PluginManager, Arc<GatedViewHost>, PathBuf) {
    let package =
        PathBuf::from(std::env::var_os("YSSBI_PLUGIN_TEST_PACKAGE").expect("test package"));
    let root = std::env::temp_dir().join(format!("yssbi-plugin-view-{}", uuid::Uuid::new_v4()));
    let host = Arc::new(GatedViewHost::default());
    let manager = PluginManager::new(&root, host.clone()).unwrap();
    let inspected = manager.inspect(&package).unwrap();
    manager
        .install(
            &package,
            &inspected.package_digest,
            &operation("view-race-install"),
            true,
            None,
        )
        .unwrap();
    (manager, host, root)
}

#[test]
#[ignore = "requires a packaged extension; does not prepare or launch Julia"]
fn view_attach_rejects_a_process_lost_during_context_preparation() {
    let (manager, host, root) = view_race_fixture();
    let lease = manager.acquire("yssbi.julia").unwrap();
    let (resume, ready) = host.pause_next_project_read();
    let attaching = {
        let manager = manager.clone();
        std::thread::spawn(move || manager.attach_view("yssbi.julia", "analysis", "test"))
    };
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    lease.process.stop();
    resume.send(()).unwrap();
    assert_eq!(
        attaching.join().unwrap().err().map(|error| error.code),
        Some("plugin_process_exited".into())
    );
    drop(lease);

    let replacement = manager
        .attach_view("yssbi.julia", "analysis", "test")
        .unwrap();
    manager
        .detach_view(&replacement.session_id, "test")
        .unwrap();
    manager.uninstall("yssbi.julia").unwrap();
    drop(manager);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires a packaged extension; does not prepare or launch Julia"]
fn view_export_rejects_a_context_detached_after_initial_validation() {
    let (manager, host, root) = view_race_fixture();
    let view = manager
        .attach_view("yssbi.julia", "analysis", "test")
        .unwrap();
    let (resume, ready) = host.pause_next_project_read();
    let granting = {
        let manager = manager.clone();
        let session = view.session_id.clone();
        let path = root.join("export.csv");
        std::thread::spawn(move || manager.grant_export(&session, "test", path))
    };
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    manager.detach_view(&view.session_id, "test").unwrap();
    resume.send(()).unwrap();
    assert_eq!(
        granting.join().unwrap().err().map(|error| error.code),
        Some("plugin_stale_context".into())
    );

    let replacement = manager
        .attach_view("yssbi.julia", "analysis", "test")
        .unwrap();
    assert!(
        !manager
            .grant_export(&replacement.session_id, "test", root.join("export.csv"))
            .unwrap()
            .is_empty()
    );
    manager
        .detach_view(&replacement.session_id, "test")
        .unwrap();
    manager.uninstall("yssbi.julia").unwrap();
    drop(manager);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires a packaged extension; does not prepare or launch Julia"]
fn view_host_reply_rejects_revocation_and_releases_late_context_resources() {
    let (manager, host, root) = view_race_fixture();
    let view = manager
        .attach_view("yssbi.julia", "analysis", "test")
        .unwrap();
    let (resume, ready) = host.pause_next_invoke();
    let reading = {
        let manager = manager.clone();
        let session = view.session_id.clone();
        std::thread::spawn(move || manager.call_view(&session, "test", "data.list", json!({})))
    };
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    manager.detach_view(&view.session_id, "test").unwrap();
    let replacement = manager
        .attach_view("yssbi.julia", "analysis", "test")
        .unwrap();
    let current = manager
        .call_view(&replacement.session_id, "test", "data.list", json!({}))
        .unwrap();
    resume.send(()).unwrap();
    let outcome = reading.join().unwrap();
    let resources = host.resources.lock().unwrap().clone();

    // Finish this fixture's process and directory cleanup even when the regression fails.
    manager
        .detach_view(&replacement.session_id, "test")
        .unwrap();
    manager.uninstall("yssbi.julia").unwrap();
    drop(manager);
    fs::remove_dir_all(root).unwrap();

    assert_eq!(
        outcome.err().map(|error| error.code),
        Some("plugin_stale_context".into())
    );
    assert_eq!(current, json!({"datasets":[]}));
    assert_eq!(resources, [replacement.session_id].into_iter().collect());
}
