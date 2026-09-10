mod commands;
mod inputs;

use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use yss_bayes_runtime::BayesInferenceService;
use yss_plugin_protocol::{
    PROTOCOL_MAJOR, PROTOCOL_MINOR, PluginFailure, ResourceBudget, RpcRequest,
};
use yss_plugin_sdk::Peer;

struct DependencyTask {
    state: yss_plugin_protocol::TaskState,
    error: Option<PluginFailure>,
    cancellation: Arc<AtomicBool>,
}
type DependencyTasks = Arc<Mutex<BTreeMap<String, DependencyTask>>>;

struct Extension {
    version: String,
    data_dir: PathBuf,
    worker: yss_julia_worker::JuliaWorkerManager,
    bayes: BayesInferenceService,
    initialized: AtomicBool,
    stopping: AtomicBool,
    task_contexts: Mutex<BTreeMap<String, Value>>,
    dependency_tasks: DependencyTasks,
    admission: Mutex<()>,
}

fn input(request: &RpcRequest) -> Value {
    request.params.get("input").cloned().unwrap_or(Value::Null)
}
fn fail(code: &str) -> PluginFailure {
    PluginFailure::new(code)
}

impl Extension {
    fn handle(&self, peer: &Peer, request: RpcRequest) -> Result<Value, PluginFailure> {
        let args = input(&request);
        if request.method == "lifecycle.initialize" {
            if args["protocolMajor"].as_u64() != Some(u64::from(PROTOCOL_MAJOR))
                || self.initialized.swap(true, Ordering::AcqRel)
            {
                return Err(fail("plugin_protocol_incompatible"));
            }
            peer.apply_budget(
                serde_json::from_value(args["resourceBudget"].clone())
                    .map_err(|_| fail("plugin_budget_invalid"))?,
            )?;
            return Ok(
                json!({"pluginId":"yssbi.julia", "version": self.version, "protocolMajor":PROTOCOL_MAJOR,"protocolMinor":PROTOCOL_MINOR}),
            );
        }
        if !self.initialized.load(Ordering::Acquire) {
            return Err(fail("plugin_not_initialized"));
        }
        if request.method == "lifecycle.shutdown" {
            if self.bayes.has_active_tasks()
                || self
                    .dependency_tasks
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?
                    .values()
                    .any(|task| !task.state.terminal())
            {
                return Err(fail("plugin_busy"));
            }
            self.worker
                .restart()
                .map_err(|_| fail("plugin_shutdown_failed"))?;
            self.stopping.store(true, Ordering::Release);
            return Ok(Value::Null);
        }
        let context = request
            .params
            .get("context")
            .cloned()
            .ok_or_else(|| fail("plugin_context_required"))?;
        match request.method.as_str() {
            "dependencies.inspect" => serde_json::to_value(self.worker.status(&self.data_dir))
                .map_err(|_| fail("plugin_response_invalid")),
            "commands.execute" => {
                if let Some(task_id) = args["args"]["taskId"].as_str() {
                    let expected = self
                        .task_contexts
                        .lock()
                        .map_err(|_| fail("plugin_state_unavailable"))?
                        .get(task_id)
                        .cloned()
                        .ok_or_else(|| fail("plugin_task_missing"))?;
                    if expected["project"] != context["project"] {
                        return Err(fail("plugin_stale_context"));
                    }
                }
                commands::execute(&self.bayes, args, peer, &context, &self.data_dir)
            }
            "tasks.start" => {
                let _admission = self
                    .admission
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?;
                let task_id = args["taskId"]
                    .as_str()
                    .filter(|id| yss_plugin_protocol::valid_id(id))
                    .ok_or_else(|| fail("plugin_task_invalid"))?
                    .to_owned();
                if let Some(expected) = self
                    .task_contexts
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?
                    .get(&task_id)
                    .cloned()
                {
                    if expected["contextId"] != context["contextId"] {
                        return Err(fail("plugin_stale_context"));
                    }
                    if let Some(task) = self
                        .dependency_tasks
                        .lock()
                        .map_err(|_| fail("plugin_state_unavailable"))?
                        .get(&task_id)
                    {
                        return Ok(json!({"taskId":task_id,"state":task.state,"error":task.error}));
                    }
                    return task_snapshot(
                        self.bayes.status(&task_id).map_err(commands::bayes_error)?,
                    );
                }
                if self.bayes.has_active_tasks()
                    || self
                        .dependency_tasks
                        .lock()
                        .map_err(|_| fail("plugin_state_unavailable"))?
                        .values()
                        .any(|task| !task.state.terminal())
                {
                    return Err(fail("plugin_busy"));
                }
                if args["taskType"].as_str() == Some("runtime.prepare") {
                    let cancellation = Arc::new(AtomicBool::new(false));
                    self.task_contexts
                        .lock()
                        .map_err(|_| fail("plugin_state_unavailable"))?
                        .insert(task_id.clone(), context);
                    self.dependency_tasks
                        .lock()
                        .map_err(|_| fail("plugin_state_unavailable"))?
                        .insert(
                            task_id.clone(),
                            DependencyTask {
                                state: yss_plugin_protocol::TaskState::Running,
                                error: None,
                                cancellation: cancellation.clone(),
                            },
                        );
                    let tasks = self.dependency_tasks.clone();
                    let worker = self.worker.clone();
                    let data_dir = self.data_dir.clone();
                    let id = task_id.clone();
                    std::thread::spawn(move || {
                        let result = worker.warm_up_cancellable(&data_dir, &cancellation);
                        if let Ok(mut tasks) = tasks.lock()
                            && let Some(task) = tasks.get_mut(&id)
                        {
                            if cancellation.load(Ordering::Acquire) {
                                task.state = yss_plugin_protocol::TaskState::Cancelled;
                            } else if result.is_ok() {
                                task.state = yss_plugin_protocol::TaskState::Succeeded;
                            } else {
                                task.state = yss_plugin_protocol::TaskState::Failed;
                                task.error = Some(fail("plugin_dependency_unavailable"));
                            }
                        }
                    });
                    return Ok(json!({"taskId":task_id,"state":"running","error":null}));
                }
                if args["taskType"].as_str() != Some("bayes.inference") {
                    return Err(fail("plugin_task_type_unknown"));
                }
                let draft: yss_bayes_model::BayesModelDraft =
                    serde_json::from_value(args["parameters"].clone())
                        .map_err(|_| fail("plugin_task_invalid"))?;
                let spec = yss_bayes_model::draft_to_model_spec(draft.clone())
                    .map_err(|_| fail("bayes_validation_failed"))?;
                let descriptor = peer.call("data.snapshot", json!({"context":context,"input":{"datasetId":spec.dataset().source_id, "columns":yss_bayes_runtime::required_input_columns(&spec)}}), Duration::from_secs(30))?;
                let dataset_path = descriptor["path"]
                    .as_str()
                    .ok_or_else(|| fail("plugin_snapshot_invalid"))?;
                let values = inputs::read_inputs(dataset_path)?;
                let _ = peer.call(
                    "data.release",
                    json!({"context":context,"input":{"leaseId":descriptor["leaseId"]}}),
                    Duration::from_secs(5),
                );
                let budget = context["remainingBudgetMs"]
                    .as_u64()
                    .unwrap_or(600_000)
                    .min(86_400_000);
                let task = self
                    .bayes
                    .submit(
                        task_id.clone(),
                        draft,
                        values,
                        Duration::from_millis(budget),
                    )
                    .map_err(commands::bayes_error)?;
                self.task_contexts
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?
                    .insert(task_id, context);
                task_snapshot(task)
            }
            "tasks.get" | "tasks.cancel" | "tasks.result" => {
                let task_id = args["taskId"]
                    .as_str()
                    .ok_or_else(|| fail("plugin_task_invalid"))?;
                let expected = self
                    .task_contexts
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?
                    .get(task_id)
                    .cloned()
                    .ok_or_else(|| fail("plugin_task_missing"))?;
                if expected["contextId"] != context["contextId"] {
                    return Err(fail("plugin_stale_context"));
                }
                {
                    let mut tasks = self
                        .dependency_tasks
                        .lock()
                        .map_err(|_| fail("plugin_state_unavailable"))?;
                    if let Some(task) = tasks.get_mut(task_id) {
                        if request.method == "tasks.cancel" {
                            if !task.state.terminal() {
                                task.state = yss_plugin_protocol::TaskState::CancelRequested;
                                task.cancellation.store(true, Ordering::Release);
                            }
                            return Ok(Value::Null);
                        }
                        if request.method == "tasks.result" {
                            return Ok(json!({"artifacts":[],"viewData":null}));
                        }
                        return Ok(
                            json!({"taskId":task_id,"state":task.state,"error":task.error,"progress":{"stage":"preparing_dependencies"}}),
                        );
                    }
                }
                if request.method == "tasks.cancel" {
                    self.bayes.cancel(task_id).map_err(commands::bayes_error)?;
                    return Ok(Value::Null);
                }
                if request.method == "tasks.result" {
                    let result = serde_json::to_value(
                        self.bayes.result(task_id).map_err(commands::bayes_error)?,
                    )
                    .map_err(|_| fail("plugin_response_invalid"))?;
                    return archive_result(&self.data_dir, task_id, result);
                }
                task_snapshot(self.bayes.status(task_id).map_err(commands::bayes_error)?)
            }
            _ => Err(fail("plugin_method_unknown")),
        }
    }
}

fn archive_result(
    data_dir: &std::path::Path,
    task_id: &str,
    result: Value,
) -> Result<Value, PluginFailure> {
    let root = fs::canonicalize(data_dir).map_err(|_| fail("plugin_storage_failed"))?;
    let directory = data_dir.join("bayes-results").join(task_id);
    let mut archived = result.clone();
    let mut artifacts = Vec::new();
    for artifact in archived["artifactManifest"]["artifacts"]
        .as_array_mut()
        .ok_or_else(|| fail("plugin_artifact_invalid"))?
    {
        let path = fs::canonicalize(
            artifact["path"]
                .as_str()
                .ok_or_else(|| fail("plugin_artifact_invalid"))?,
        )
        .map_err(|_| fail("plugin_artifact_invalid"))?;
        let relative = path
            .strip_prefix(&root)
            .map_err(|_| fail("plugin_artifact_invalid"))?
            .to_str()
            .ok_or_else(|| fail("plugin_artifact_invalid"))?
            .replace('\\', "/");
        let media_type = match artifact["format"].as_str() {
            Some("arrow_ipc") => "application/vnd.apache.arrow.file",
            Some("json") => "application/json",
            _ => "text/plain",
        };
        artifacts.push(json!({"path":relative,"mediaType":media_type}));
        artifact["path"] = Value::String(
            path.file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| fail("plugin_artifact_invalid"))?
                .into(),
        );
    }
    fs::write(
        directory.join("result.json"),
        serde_json::to_vec_pretty(&archived).map_err(|_| fail("plugin_response_invalid"))?,
    )
    .map_err(|_| fail("plugin_storage_failed"))?;
    let columns = [
        "parameter",
        "mean",
        "sd",
        "median",
        "q025",
        "q975",
        "rhat",
        "essBulk",
        "essTail",
    ];
    let mut csv = columns.join(",");
    csv.push('\n');
    for summary in result["summaries"]
        .as_array()
        .ok_or_else(|| fail("plugin_response_invalid"))?
    {
        let cells = columns
            .iter()
            .map(|column| match &summary[*column] {
                Value::String(text) => format!(
                    "\"{}{}\"",
                    if text.starts_with(['=', '+', '-', '@']) {
                        "'"
                    } else {
                        ""
                    },
                    text.replace('"', "\"\"")
                ),
                Value::Number(number) => number.to_string(),
                _ => String::new(),
            })
            .collect::<Vec<_>>();
        csv.push_str(&cells.join(","));
        csv.push('\n');
    }
    fs::write(directory.join("summary.csv"), csv).map_err(|_| fail("plugin_storage_failed"))?;
    artifacts.push(json!({"path":format!("bayes-results/{task_id}/result.json"),"mediaType":"application/json"}));
    artifacts.push(
        json!({"path":format!("bayes-results/{task_id}/summary.csv"),"mediaType":"text/csv"}),
    );
    Ok(json!({"viewData":result,"artifacts":artifacts}))
}

fn task_snapshot(task: yss_bayes_result::BayesInferenceTask) -> Result<Value, PluginFailure> {
    let value = serde_json::to_value(task).map_err(|_| fail("plugin_response_invalid"))?;
    let state = match value["status"].as_str() {
        Some("queued") => "admitted",
        Some("running") => "running",
        Some("cancelling") => "cancelRequested",
        Some("completed") => "succeeded",
        Some("cancelled") => "cancelled",
        _ => "failed",
    };
    Ok(
        json!({"taskId":value["taskId"],"state":state,"error":value["error"],"progress":value["progress"]}),
    )
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", error.code);
        std::process::exit(1);
    }
}
fn run() -> Result<(), PluginFailure> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--data-dir")) {
        return Err(fail("plugin_arguments_invalid"));
    }
    let data_dir = PathBuf::from(
        arguments
            .next()
            .ok_or_else(|| fail("plugin_arguments_invalid"))?,
    );
    fs::create_dir_all(&data_dir).map_err(|_| fail("plugin_storage_failed"))?;
    let worker = yss_julia_worker::JuliaWorkerManager::new();
    let bayes = BayesInferenceService::with_worker(
        data_dir.clone(),
        Arc::new(yss_bayes_worker_julia::JuliaBayesWorkerAdapter::new(
            data_dir.clone(),
            worker.clone(),
        )),
        Arc::new(
            yss_bayes_artifact_datafusion::DataFusionBayesArtifactReader::new()
                .map_err(|_| fail("plugin_storage_failed"))?,
        ),
    );
    let version = fs::read("plugin.json")
        .ok()
        .and_then(|bytes| {
            serde_json::from_slice::<yss_plugin_protocol::PluginManifest>(&bytes).ok()
        })
        .map(|manifest| manifest.version)
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into());
    let extension = Arc::new(Extension {
        version,
        data_dir,
        worker,
        bayes,
        initialized: AtomicBool::new(false),
        stopping: AtomicBool::new(false),
        task_contexts: Mutex::new(BTreeMap::new()),
        dependency_tasks: Arc::new(Mutex::new(BTreeMap::new())),
        admission: Mutex::new(()),
    });
    let handler_extension = extension.clone();
    let peer = Peer::connect(
        std::io::stdin(),
        std::io::stdout(),
        ResourceBudget::default(),
        Arc::new(move |peer, request| handler_extension.handle(peer, request)),
    );
    while peer.is_alive() && !extension.stopping.load(Ordering::Acquire) {
        std::thread::park_timeout(Duration::from_millis(100));
    }
    let _ = extension.worker.restart();
    peer.close();
    Ok(())
}
