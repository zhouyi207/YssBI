//! Julia worker lifecycle and Arrow IPC task exchange.

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{JuliaRuntimeState, background_command, get_runtime_status, system_julia_executable};

const WORKER_DIR: &str = "julia-worker";
const TASK_DIR: &str = "tasks";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);
const STDERR_BUFFER_LINES: usize = 100;
const WORKER_PROJECT: &str = include_str!("../../julia/Project.toml");
const WORKER_MANIFEST: &str = include_str!("../../julia/Manifest.toml");
const WORKER_SCRIPT: &str = include_str!("../../julia/worker.jl");
const WORKER_SCIENTIFIC_RUNTIME: &str = include_str!("../../julia/scientific_runtime.jl");
const WORKER_ACF_PACF_OP: &str = include_str!("../../julia/ops/acf_pacf.jl");
const WORKER_SERIAL_TESTS_OP: &str = include_str!("../../julia/ops/serial_tests.jl");
const WORKER_BAYES_FIT_OP: &str = include_str!("../../julia/ops/bayes_fit.jl");
const WORKER_BAYES_EXPRESSION_OP: &str = include_str!("../../julia/ops/bayes/expression.jl");
const WORKER_BAYES_RUNTIME_OP: &str = include_str!("../../julia/ops/bayes/runtime.jl");
const WORKER_BAYES_TURING_GENERIC_NORMAL_OP: &str =
    include_str!("../../julia/ops/bayes/turing_generic_normal.jl");

#[derive(Debug, Clone)]
pub struct JuliaWorkerTask {
    pub task_id: Option<String>,
    pub operation: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub struct JuliaWorkerTaskOutput {
    pub task_id: String,
    pub output_path: PathBuf,
    pub metadata_path: PathBuf,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JuliaWorkerProgress {
    pub task_id: String,
    pub stage: String,
    pub completed: Option<usize>,
    pub total: Option<usize>,
}

pub type JuliaWorkerProgressCallback = Arc<dyn Fn(JuliaWorkerProgress) + Send + Sync>;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JuliaWorkerEnvironmentState {
    Missing,
    Ready,
    Invalid,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JuliaWorkerProcessState {
    Stopped,
    Starting,
    Running,
    Crashed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JuliaWorkerStatus {
    pub runtime_state: JuliaRuntimeState,
    pub environment_state: JuliaWorkerEnvironmentState,
    pub process_state: JuliaWorkerProcessState,
    pub project_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Reuses one Julia process while serializing compute requests. Cancellation
/// notifications can still be sent while a request is waiting for its response.
#[derive(Clone)]
pub struct JuliaWorkerManager {
    inner: Arc<JuliaWorkerInner>,
}

struct JuliaWorkerInner {
    worker: Mutex<Option<Arc<WorkerProcess>>>,
    startup: Mutex<JuliaWorkerStartupState>,
    request_gate: Mutex<()>,
    active_task_id: Mutex<Option<String>>,
}

#[derive(Debug, Clone)]
enum JuliaWorkerStartupState {
    Idle,
    Preparing,
    Failed(String),
}

impl JuliaWorkerManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(JuliaWorkerInner {
                worker: Mutex::new(None),
                startup: Mutex::new(JuliaWorkerStartupState::Idle),
                request_gate: Mutex::new(()),
                active_task_id: Mutex::new(None),
            }),
        }
    }

    pub fn status(&self, app_data_dir: &Path) -> JuliaWorkerStatus {
        let worker_dir = app_data_dir.join(WORKER_DIR);
        let startup = self
            .inner
            .startup
            .lock()
            .map(|state| state.clone())
            .unwrap_or_else(|_| {
                JuliaWorkerStartupState::Failed(
                    "Julia worker startup state is unavailable.".to_string(),
                )
            });
        if matches!(startup, JuliaWorkerStartupState::Preparing) {
            return JuliaWorkerStatus {
                runtime_state: JuliaRuntimeState::Ready,
                environment_state: JuliaWorkerEnvironmentState::Missing,
                process_state: JuliaWorkerProcessState::Starting,
                project_dir: worker_dir.to_string_lossy().into_owned(),
                message: None,
            };
        }
        if self.process_state() == JuliaWorkerProcessState::Running {
            return JuliaWorkerStatus {
                runtime_state: JuliaRuntimeState::Ready,
                environment_state: JuliaWorkerEnvironmentState::Ready,
                process_state: JuliaWorkerProcessState::Running,
                project_dir: worker_dir.to_string_lossy().into_owned(),
                message: None,
            };
        }

        let runtime = get_runtime_status();
        let (worker_dir, asset_message) = ensure_worker_assets(app_data_dir)
            .map(|path| (path, None))
            .unwrap_or_else(|message| (worker_dir, Some(message)));
        let (environment_state, environment_message) = asset_message.map_or_else(
            || inspect_worker_environment(&worker_dir),
            |message| (JuliaWorkerEnvironmentState::Invalid, Some(message)),
        );
        let startup_message = match startup {
            JuliaWorkerStartupState::Failed(message) => Some(message),
            _ => None,
        };
        JuliaWorkerStatus {
            runtime_state: runtime.state,
            environment_state,
            process_state: self.process_state(),
            project_dir: worker_dir.to_string_lossy().into_owned(),
            message: runtime.message.or(environment_message).or(startup_message),
        }
    }

    pub fn warm_up(&self, app_data_dir: &Path) -> Result<(), String> {
        let _request_guard = self
            .inner
            .request_gate
            .lock()
            .map_err(|_| "Julia worker request gate is unavailable.".to_string())?;
        self.set_startup_state(JuliaWorkerStartupState::Preparing);
        let result = self.prepare(app_data_dir).and_then(|()| {
            let worker = self.worker(app_data_dir)?;
            let request_id = Uuid::new_v4().to_string();
            worker.send(json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "ping"
            }))?;
            worker.await_response(&request_id, "startup", None)
        });
        match &result {
            Ok(()) => self.set_startup_state(JuliaWorkerStartupState::Idle),
            Err(message) => {
                self.set_startup_state(JuliaWorkerStartupState::Failed(message.clone()))
            }
        }
        result
    }

    pub fn prepare(&self, app_data_dir: &Path) -> Result<(), String> {
        let worker_dir = ensure_worker_assets(app_data_dir)?;
        let executable = system_julia_executable()?;
        let status = background_command(executable)
            .arg(format!("--project={}", worker_dir.display()))
            .args(["--startup-file=no", "-e", "using Pkg; Pkg.instantiate()"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| format!("Failed to prepare Julia worker packages: {error}"))?;
        if status.status.success() {
            Ok(())
        } else {
            let detail = String::from_utf8_lossy(&status.stderr).trim().to_string();
            Err(format!("Failed to prepare Julia worker packages: {detail}"))
        }
    }

    pub fn run_task(
        &self,
        app_data_dir: &Path,
        task: JuliaWorkerTask,
        write_input: impl FnOnce(&Path) -> Result<(), String>,
        progress: Option<JuliaWorkerProgressCallback>,
    ) -> Result<JuliaWorkerTaskOutput, String> {
        let _request_guard = self
            .inner
            .request_gate
            .lock()
            .map_err(|_| "Julia worker request gate is unavailable.".to_string())?;

        let task_id = task.task_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let task_dir = create_task_dir(app_data_dir, &task_id)?;
        let input_path = task_dir.join("input.arrow");
        let output_path = task_dir.join("output.arrow");
        let metadata_path = task_dir.join("metadata.json");
        write_input(&input_path)?;

        let result = (|| {
            let worker = self.worker(app_data_dir)?;
            let request_id = Uuid::new_v4().to_string();
            self.set_active_task(Some(task_id.clone()))?;
            let response = worker
                .send(json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "method": "run",
                    "params": {
                        "taskId": task_id,
                        "operation": task.operation,
                        "inputPath": input_path,
                        "outputPath": output_path,
                        "metadataPath": metadata_path,
                        "parameters": task.parameters
                    }
                }))
                .and_then(|()| worker.await_response(&request_id, &task_id, progress.as_ref()));
            self.clear_active_task(&task_id);
            response?;
            Ok(JuliaWorkerTaskOutput {
                task_id: task_id.clone(),
                output_path,
                metadata_path,
            })
        })();

        if result.is_err() {
            let _ = fs::remove_dir_all(&task_dir);
        }
        result
    }

    pub fn cancel(&self, task_id: &str) -> Result<bool, String> {
        let active_task_id = self
            .inner
            .active_task_id
            .lock()
            .map_err(|_| "Julia worker active task state is unavailable.".to_string())?;
        if active_task_id.as_deref() != Some(task_id) {
            return Ok(false);
        }
        let worker = self
            .inner
            .worker
            .lock()
            .map_err(|_| "Julia worker state is unavailable.".to_string())?
            .clone();
        match worker {
            Some(worker) => {
                worker.send(json!({
                    "jsonrpc": "2.0",
                    "method": "cancel",
                    "params": { "taskId": task_id }
                }))?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn restart_task(&self, task_id: &str) -> Result<bool, String> {
        let mut active_task_id = self
            .inner
            .active_task_id
            .lock()
            .map_err(|_| "Julia worker active task state is unavailable.".to_string())?;
        if active_task_id.as_deref() != Some(task_id) {
            return Ok(false);
        }
        let worker = self
            .inner
            .worker
            .lock()
            .map_err(|_| "Julia worker state is unavailable.".to_string())?
            .take();
        *active_task_id = None;
        if let Some(worker) = worker {
            worker.terminate();
        }
        Ok(true)
    }

    pub fn restart(&self) -> Result<(), String> {
        let worker = self
            .inner
            .worker
            .lock()
            .map_err(|_| "Julia worker state is unavailable.".to_string())?
            .take();
        if let Some(worker) = worker {
            worker.terminate();
        }
        Ok(())
    }

    fn set_startup_state(&self, state: JuliaWorkerStartupState) {
        if let Ok(mut startup) = self.inner.startup.lock() {
            *startup = state;
        }
    }

    fn set_active_task(&self, task_id: Option<String>) -> Result<(), String> {
        *self
            .inner
            .active_task_id
            .lock()
            .map_err(|_| "Julia worker active task state is unavailable.".to_string())? = task_id;
        Ok(())
    }

    fn clear_active_task(&self, task_id: &str) {
        if let Ok(mut active_task_id) = self.inner.active_task_id.lock()
            && active_task_id.as_deref() == Some(task_id)
        {
            *active_task_id = None;
        }
    }

    fn process_state(&self) -> JuliaWorkerProcessState {
        let worker = self.inner.worker.lock().ok().and_then(|slot| slot.clone());
        worker.map_or(JuliaWorkerProcessState::Stopped, |worker| {
            worker.process_state()
        })
    }

    fn worker(&self, app_data_dir: &Path) -> Result<Arc<WorkerProcess>, String> {
        let mut slot = self
            .inner
            .worker
            .lock()
            .map_err(|_| "Julia worker state is unavailable.".to_string())?;
        if let Some(worker) = slot.as_ref().filter(|worker| worker.is_running()) {
            return Ok(worker.clone());
        }

        let worker_dir = ensure_worker_assets(app_data_dir)?;
        let worker = Arc::new(WorkerProcess::spawn(&worker_dir)?);
        *slot = Some(worker.clone());
        Ok(worker)
    }
}

impl Default for JuliaWorkerManager {
    fn default() -> Self {
        Self::new()
    }
}

struct WorkerProcess {
    child: Mutex<Child>,
    stdin: Mutex<BufWriter<ChildStdin>>,
    messages: Mutex<mpsc::Receiver<Value>>,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

impl WorkerProcess {
    fn spawn(worker_dir: &Path) -> Result<Self, String> {
        let executable = system_julia_executable()?;
        let script = worker_dir.join("worker.jl");
        let mut child = background_command(executable)
            .arg(format!("--project={}", worker_dir.display()))
            .args(["--startup-file=no", "--history-file=no"])
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Failed to start Julia worker: {error}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Julia worker stdin is unavailable.".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Julia worker stdout is unavailable.".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Julia worker stderr is unavailable.".to_string())?;
        let (sender, receiver) = mpsc::channel();
        let stderr_buffer = Arc::new(Mutex::new(VecDeque::new()));

        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(message) = serde_json::from_str(&line) {
                    if sender.send(message).is_err() {
                        break;
                    }
                }
            }
        });
        let stderr_target = stderr_buffer.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Ok(mut lines) = stderr_target.lock() {
                    if lines.len() == STDERR_BUFFER_LINES {
                        lines.pop_front();
                    }
                    lines.push_back(line);
                }
            }
        });

        Ok(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(BufWriter::new(stdin)),
            messages: Mutex::new(receiver),
            stderr: stderr_buffer,
        })
    }

    fn is_running(&self) -> bool {
        self.process_state() == JuliaWorkerProcessState::Running
    }

    fn process_state(&self) -> JuliaWorkerProcessState {
        match self
            .child
            .lock()
            .ok()
            .and_then(|mut child| child.try_wait().ok())
        {
            Some(None) => JuliaWorkerProcessState::Running,
            Some(Some(_)) => JuliaWorkerProcessState::Crashed,
            None => JuliaWorkerProcessState::Crashed,
        }
    }

    fn send(&self, message: Value) -> Result<(), String> {
        let encoded = serde_json::to_string(&message).map_err(|error| error.to_string())?;
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| "Julia worker stdin is unavailable.".to_string())?;
        writeln!(stdin, "{encoded}")
            .map_err(|error| format!("Failed to write Julia request: {error}"))?;
        stdin
            .flush()
            .map_err(|error| format!("Failed to flush Julia request: {error}"))
    }

    fn await_response(
        &self,
        request_id: &str,
        task_id: &str,
        progress: Option<&JuliaWorkerProgressCallback>,
    ) -> Result<(), String> {
        let receiver = self
            .messages
            .lock()
            .map_err(|_| "Julia worker response channel is unavailable.".to_string())?;
        loop {
            let message = receiver
                .recv_timeout(RESPONSE_TIMEOUT)
                .map_err(|_| self.worker_failure("Julia worker did not return a response."))?;
            if message.get("method").and_then(Value::as_str) == Some("progress") {
                if let Some(progress) = progress
                    && let Some(params) = message.get("params")
                    && let Ok(update) =
                        serde_json::from_value::<JuliaWorkerProgress>(params.clone())
                    && update.task_id == task_id
                {
                    progress(update);
                }
                continue;
            }
            if message.get("id").and_then(Value::as_str) != Some(request_id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(worker_error(error));
            }
            if message.get("result").is_some() {
                return Ok(());
            }
            return Err(format!(
                "Julia worker returned an invalid response for task {task_id}."
            ));
        }
    }

    fn terminate(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn worker_failure(&self, fallback: &str) -> String {
        let detail = self
            .stderr
            .lock()
            .ok()
            .map(|lines| lines.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();
        if detail.is_empty() {
            fallback.to_string()
        } else {
            format!("{fallback} {detail}")
        }
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn inspect_worker_environment(worker_dir: &Path) -> (JuliaWorkerEnvironmentState, Option<String>) {
    let required_files = [
        worker_dir.join("Project.toml"),
        worker_dir.join("Manifest.toml"),
        worker_dir.join("worker.jl"),
        worker_dir.join("scientific_runtime.jl"),
        worker_dir.join("ops").join("acf_pacf.jl"),
        worker_dir.join("ops").join("serial_tests.jl"),
        worker_dir.join("ops").join("bayes_fit.jl"),
        worker_dir.join("ops").join("bayes").join("expression.jl"),
        worker_dir.join("ops").join("bayes").join("runtime.jl"),
        worker_dir
            .join("ops")
            .join("bayes")
            .join("turing_generic_normal.jl"),
    ];
    if required_files.iter().any(|path| !path.is_file()) {
        return (
            JuliaWorkerEnvironmentState::Missing,
            Some("Julia worker assets were not prepared.".to_string()),
        );
    }

    let executable = match system_julia_executable() {
        Ok(executable) => executable,
        Err(message) => return (JuliaWorkerEnvironmentState::Invalid, Some(message)),
    };
    let output = background_command(executable)
        .arg(format!("--project={}", worker_dir.display()))
        .args(["--startup-file=no", "-e", "using Arrow, JSON3"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match output {
        Ok(output) if output.status.success() => (JuliaWorkerEnvironmentState::Ready, None),
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            (
                JuliaWorkerEnvironmentState::Invalid,
                Some(if detail.is_empty() {
                    "Julia worker packages are not available.".to_string()
                } else {
                    detail
                }),
            )
        }
        Err(error) => (
            JuliaWorkerEnvironmentState::Invalid,
            Some(format!(
                "Failed to inspect Julia worker environment: {error}"
            )),
        ),
    }
}

fn ensure_worker_assets(app_data_dir: &Path) -> Result<PathBuf, String> {
    let worker_dir = app_data_dir.join(WORKER_DIR);
    fs::create_dir_all(&worker_dir)
        .map_err(|error| format!("Failed to create Julia worker directory: {error}"))?;
    write_asset(&worker_dir.join("Project.toml"), WORKER_PROJECT)?;
    write_asset(&worker_dir.join("Manifest.toml"), WORKER_MANIFEST)?;
    write_asset(&worker_dir.join("worker.jl"), WORKER_SCRIPT)?;
    write_asset(
        &worker_dir.join("scientific_runtime.jl"),
        WORKER_SCIENTIFIC_RUNTIME,
    )?;
    let ops_dir = worker_dir.join("ops");
    fs::create_dir_all(&ops_dir)
        .map_err(|error| format!("Failed to create Julia worker ops directory: {error}"))?;
    write_asset(&ops_dir.join("acf_pacf.jl"), WORKER_ACF_PACF_OP)?;
    write_asset(&ops_dir.join("serial_tests.jl"), WORKER_SERIAL_TESTS_OP)?;
    write_asset(&ops_dir.join("bayes_fit.jl"), WORKER_BAYES_FIT_OP)?;
    let bayes_ops_dir = ops_dir.join("bayes");
    fs::create_dir_all(&bayes_ops_dir).map_err(|error| {
        format!("Failed to create Julia Bayesian worker ops directory: {error}")
    })?;
    write_asset(
        &bayes_ops_dir.join("expression.jl"),
        WORKER_BAYES_EXPRESSION_OP,
    )?;
    write_asset(&bayes_ops_dir.join("runtime.jl"), WORKER_BAYES_RUNTIME_OP)?;
    write_asset(
        &bayes_ops_dir.join("turing_generic_normal.jl"),
        WORKER_BAYES_TURING_GENERIC_NORMAL_OP,
    )?;
    Ok(worker_dir)
}

fn write_asset(path: &Path, contents: &str) -> Result<(), String> {
    if fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, contents)
        .map_err(|error| format!("Failed to write Julia worker asset: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("Failed to update Julia worker asset: {error}"))
}

fn create_task_dir(app_data_dir: &Path, task_id: &str) -> Result<PathBuf, String> {
    let safe_task_id = task_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let task_dir = app_data_dir
        .join(WORKER_DIR)
        .join(TASK_DIR)
        .join(safe_task_id);
    fs::create_dir_all(&task_dir)
        .map_err(|error| format!("Failed to create Julia task directory: {error}"))?;
    Ok(task_dir)
}

fn worker_error(error: &Value) -> String {
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("internal_error");
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Julia worker task failed.");
    format!("{code}: {message}")
}

#[cfg(test)]
mod tests {
    use super::{JuliaWorkerManager, worker_error};
    use serde_json::json;

    #[test]
    fn preserves_structured_worker_error_code() {
        assert_eq!(
            worker_error(&json!({ "code": "invalid_parameters", "message": "bad column" })),
            "invalid_parameters: bad column"
        );
    }

    #[test]
    fn cancellation_and_restart_ignore_non_active_task() {
        let manager = JuliaWorkerManager::new();
        manager
            .set_active_task(Some("active-task".to_string()))
            .expect("set active task");

        assert!(!manager.cancel("other-task").expect("cancel task"));
        assert!(!manager.restart_task("other-task").expect("restart task"));
        assert_eq!(
            manager
                .inner
                .active_task_id
                .lock()
                .expect("active task lock")
                .as_deref(),
            Some("active-task")
        );
    }
}
