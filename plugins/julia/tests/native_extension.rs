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
    draft["sampler"]["samples"] = json!(1_000_000);
    let long=manager.call_view(&analysis.session_id,"test","tasks.start",json!({"operationId":operation("cancel-test"),"taskType":"bayes.inference","parameters":draft,"timeoutMs":180000})).unwrap();
    let long_id = long["taskId"].as_str().unwrap();
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
