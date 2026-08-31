//! Julia worker lifecycle and Arrow IPC task exchange.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use yss_julia_runtime::{
    JuliaRuntimeState, background_command, get_runtime_status, system_julia_executable,
};

mod assets;
mod error;
mod task_directory;

use assets::ensure_worker_assets;
#[cfg(test)]
use assets::write_asset;
pub use error::{JuliaWorkerError, JuliaWorkerErrorCode, JuliaWorkerErrorDetails};
pub use task_directory::JuliaWorkerTaskDirectory;

const WORKER_DIR: &str = "julia-worker";
const TASK_DIR: &str = "tasks";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);
const STDERR_BUFFER_LINES: usize = 100;

#[derive(Debug, Clone)]
pub struct JuliaWorkerTask {
    pub task_id: Option<String>,
    pub operation: String,
    pub parameters: Value,
}

#[derive(Debug)]
pub struct JuliaWorkerTaskOutput {
    pub task_id: String,
    pub output_path: PathBuf,
    pub metadata_path: PathBuf,
    task_directory: Option<JuliaWorkerTaskDirectory>,
}

impl JuliaWorkerTaskOutput {
    pub(crate) fn take_task_directory(&mut self) -> Option<JuliaWorkerTaskDirectory> {
        self.task_directory.take()
    }
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
    Failed,
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
        self.status_with_runtime_state(app_data_dir, get_runtime_status().state)
    }

    fn status_with_runtime_state(
        &self,
        app_data_dir: &Path,
        runtime_state: JuliaRuntimeState,
    ) -> JuliaWorkerStatus {
        let worker_dir = app_data_dir.join(WORKER_DIR);
        let startup = self
            .inner
            .startup
            .lock()
            .map(|state| state.clone())
            .unwrap_or(JuliaWorkerStartupState::Failed);
        let observed_process = self.process_state();
        let (environment_state, process_state) =
            if matches!(startup, JuliaWorkerStartupState::Preparing) {
                (
                    JuliaWorkerEnvironmentState::Missing,
                    JuliaWorkerProcessState::Starting,
                )
            } else if observed_process == JuliaWorkerProcessState::Running {
                (
                    JuliaWorkerEnvironmentState::Ready,
                    JuliaWorkerProcessState::Running,
                )
            } else {
                match startup {
                    JuliaWorkerStartupState::Failed => {
                        (JuliaWorkerEnvironmentState::Invalid, observed_process)
                    }
                    JuliaWorkerStartupState::Idle => {
                        (JuliaWorkerEnvironmentState::Missing, observed_process)
                    }
                    JuliaWorkerStartupState::Preparing => unreachable!(),
                }
            };

        JuliaWorkerStatus {
            runtime_state,
            environment_state,
            process_state,
            project_dir: worker_dir.to_string_lossy().into_owned(),
        }
    }

    pub fn warm_up(&self, app_data_dir: &Path) -> Result<(), JuliaWorkerError> {
        let _request_guard = self.inner.request_gate.lock().map_err(|_| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StateUnavailable,
                "Julia worker request gate is unavailable.",
            )
        })?;
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
            Err(_) => self.set_startup_state(JuliaWorkerStartupState::Failed),
        }
        result
    }

    pub fn prepare(&self, app_data_dir: &Path) -> Result<(), JuliaWorkerError> {
        let worker_dir = ensure_worker_assets(app_data_dir)?;
        let executable = system_julia_executable().map_err(|error| {
            JuliaWorkerError::new(JuliaWorkerErrorCode::RuntimeUnavailable, error.to_string())
        })?;
        let status = background_command(executable)
            .arg(format!("--project={}", worker_dir.display()))
            .args(["--startup-file=no", "-e", "using Pkg; Pkg.instantiate()"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::EnvironmentUnavailable,
                    format!("Failed to prepare Julia worker packages: {error}"),
                )
            })?;
        if status.status.success() {
            Ok(())
        } else {
            let detail = String::from_utf8_lossy(&status.stderr).trim().to_string();
            Err(JuliaWorkerError::new(
                JuliaWorkerErrorCode::EnvironmentUnavailable,
                format!("Failed to prepare Julia worker packages: {detail}"),
            ))
        }
    }

    pub fn run_task(
        &self,
        app_data_dir: &Path,
        task: JuliaWorkerTask,
        write_input: impl FnOnce(&Path) -> Result<(), String>,
        progress: Option<JuliaWorkerProgressCallback>,
    ) -> Result<JuliaWorkerTaskOutput, JuliaWorkerError> {
        self.run_task_with_typed_input(
            app_data_dir,
            task,
            |path| {
                write_input(path).map_err(|diagnostic| {
                    JuliaWorkerError::new(JuliaWorkerErrorCode::InputWriteFailed, diagnostic)
                })
            },
            progress,
        )
    }

    pub(crate) fn run_task_with_typed_input(
        &self,
        app_data_dir: &Path,
        task: JuliaWorkerTask,
        write_input: impl FnOnce(&Path) -> Result<(), JuliaWorkerError>,
        progress: Option<JuliaWorkerProgressCallback>,
    ) -> Result<JuliaWorkerTaskOutput, JuliaWorkerError> {
        let _request_guard = self.inner.request_gate.lock().map_err(|_| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StateUnavailable,
                "Julia worker request gate is unavailable.",
            )
        })?;

        let task_id = task.task_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let task_directory = JuliaWorkerTaskDirectory::create(app_data_dir, &task_id)?;
        let input_path = task_directory.path().join("input.arrow");
        let output_path = task_directory.path().join("output.arrow");
        let metadata_path = task_directory.path().join("metadata.json");

        let result = (|| {
            write_input(&input_path)?;
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
            response
        })();
        result?;

        Ok(JuliaWorkerTaskOutput {
            task_id,
            output_path,
            metadata_path,
            task_directory: Some(task_directory),
        })
    }

    pub fn cancel(&self, task_id: &str) -> Result<bool, JuliaWorkerError> {
        self.cancel_with_io_hook(task_id, || {})
    }

    fn cancel_with_io_hook(
        &self,
        task_id: &str,
        before_io: impl FnOnce(),
    ) -> Result<bool, JuliaWorkerError> {
        let is_active = self
            .inner
            .active_task_id
            .lock()
            .map_err(|_| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::StateUnavailable,
                    "Julia worker active task state is unavailable.",
                )
            })?
            .as_deref()
            == Some(task_id);
        if !is_active {
            return Ok(false);
        }
        before_io();
        let worker = self
            .inner
            .worker
            .lock()
            .map_err(|_| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::StateUnavailable,
                    "Julia worker state is unavailable.",
                )
            })?
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

    pub fn restart_task(&self, task_id: &str) -> Result<bool, JuliaWorkerError> {
        self.restart_task_with_io_hook(task_id, || {})
    }

    fn restart_task_with_io_hook(
        &self,
        task_id: &str,
        before_io: impl FnOnce(),
    ) -> Result<bool, JuliaWorkerError> {
        {
            let mut active_task_id = self.inner.active_task_id.lock().map_err(|_| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::StateUnavailable,
                    "Julia worker active task state is unavailable.",
                )
            })?;
            if active_task_id.as_deref() != Some(task_id) {
                return Ok(false);
            }
            *active_task_id = None;
        }
        before_io();
        let worker = self
            .inner
            .worker
            .lock()
            .map_err(|_| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::StateUnavailable,
                    "Julia worker state is unavailable.",
                )
            })?
            .take();
        if let Some(worker) = worker {
            worker.terminate();
        }
        Ok(true)
    }

    pub fn restart(&self) -> Result<(), JuliaWorkerError> {
        let worker = self
            .inner
            .worker
            .lock()
            .map_err(|_| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::StateUnavailable,
                    "Julia worker state is unavailable.",
                )
            })?
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

    fn set_active_task(&self, task_id: Option<String>) -> Result<(), JuliaWorkerError> {
        *self.inner.active_task_id.lock().map_err(|_| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StateUnavailable,
                "Julia worker active task state is unavailable.",
            )
        })? = task_id;
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

    fn worker(&self, app_data_dir: &Path) -> Result<Arc<WorkerProcess>, JuliaWorkerError> {
        let mut slot = self.inner.worker.lock().map_err(|_| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StateUnavailable,
                "Julia worker state is unavailable.",
            )
        })?;
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
    fn spawn(worker_dir: &Path) -> Result<Self, JuliaWorkerError> {
        let executable = system_julia_executable().map_err(|error| {
            JuliaWorkerError::new(JuliaWorkerErrorCode::RuntimeUnavailable, error.to_string())
        })?;
        let script = worker_dir.join("worker.jl");
        let mut child = background_command(executable)
            .arg(format!("--project={}", worker_dir.display()))
            .args(["--startup-file=no", "--history-file=no"])
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::StartFailed,
                    format!("Failed to start Julia worker: {error}"),
                )
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StartFailed,
                "Julia worker stdin is unavailable.",
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StartFailed,
                "Julia worker stdout is unavailable.",
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StartFailed,
                "Julia worker stderr is unavailable.",
            )
        })?;
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

    fn send(&self, message: Value) -> Result<(), JuliaWorkerError> {
        let encoded = serde_json::to_string(&message).map_err(|error| {
            JuliaWorkerError::new(JuliaWorkerErrorCode::RequestFailed, error.to_string())
        })?;
        let mut stdin = self.stdin.lock().map_err(|_| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StateUnavailable,
                "Julia worker stdin is unavailable.",
            )
        })?;
        writeln!(stdin, "{encoded}").map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::RequestFailed,
                format!("Failed to write Julia request: {error}"),
            )
        })?;
        stdin.flush().map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::RequestFailed,
                format!("Failed to flush Julia request: {error}"),
            )
        })
    }

    fn await_response(
        &self,
        request_id: &str,
        task_id: &str,
        progress: Option<&JuliaWorkerProgressCallback>,
    ) -> Result<(), JuliaWorkerError> {
        let receiver = self.messages.lock().map_err(|_| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::StateUnavailable,
                "Julia worker response channel is unavailable.",
            )
        })?;
        loop {
            let message = receiver.recv_timeout(RESPONSE_TIMEOUT).map_err(|_| {
                JuliaWorkerError::new(
                    JuliaWorkerErrorCode::ResponseTimeout,
                    self.worker_failure("Julia worker did not return a response."),
                )
            })?;
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
            return Err(JuliaWorkerError::new(
                JuliaWorkerErrorCode::InvalidResponse,
                format!("Julia worker returned an invalid response for task {task_id}."),
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

fn worker_error(error: &Value) -> JuliaWorkerError {
    JuliaWorkerError::from_json_rpc_error(error)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Barrier};

    use super::{
        JuliaRuntimeState, JuliaWorkerErrorCode, JuliaWorkerManager, JuliaWorkerStartupState,
        JuliaWorkerTask, TASK_DIR, WORKER_DIR, worker_error, write_asset,
    };
    use serde_json::json;
    use uuid::Uuid;

    struct TemporaryAppRoot {
        path: PathBuf,
    }

    impl TemporaryAppRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!("yssbi-{label}-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create temporary app root");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TemporaryAppRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn preserves_structured_worker_error_code_and_safe_details() {
        let error = worker_error(&json!({
            "code": "invalid_parameters",
            "message": "private worker prose",
            "data": {
                "column": "predictor_x",
                "row": 7,
                "parameter": "beta",
                "path": "parameters.beta"
            }
        }));

        assert_eq!(error.code(), JuliaWorkerErrorCode::InvalidParameters);
        let details = error.details().expect("safe worker error details");
        assert_eq!(details.column.as_deref(), Some("predictor_x"));
        assert_eq!(details.row, Some(7));
        assert_eq!(details.parameter.as_deref(), Some("beta"));
        assert_eq!(details.path.as_deref(), Some("parameters.beta"));
        assert_eq!(error.diagnostic(), "private worker prose");
        assert_eq!(error.to_string(), "julia_worker_invalid_parameters");
    }

    #[test]
    fn replaces_existing_worker_asset_without_leaving_temporary_files() {
        let app_root = TemporaryAppRoot::new("julia-asset-replace");
        let worker_dir = app_root.path().join(WORKER_DIR);
        fs::create_dir_all(&worker_dir).expect("create worker directory");
        let target = worker_dir.join("worker.jl");
        fs::write(&target, "old").expect("write existing worker asset");

        write_asset(&target, "new").expect("replace existing worker asset");

        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(fs::read_dir(&worker_dir).unwrap().count(), 1);
    }

    #[test]
    fn removes_temporary_worker_asset_when_publication_fails() {
        let app_root = TemporaryAppRoot::new("julia-asset-failure");
        let worker_dir = app_root.path().join(WORKER_DIR);
        fs::create_dir_all(&worker_dir).expect("create worker directory");
        let target = worker_dir.join("worker.jl");
        fs::create_dir(&target).expect("reserve target as a directory");

        let error = write_asset(&target, "new").expect_err("asset publication must fail");

        assert_eq!(error.code(), JuliaWorkerErrorCode::AssetUpdateFailed);
        assert_eq!(fs::read_dir(&worker_dir).unwrap().count(), 1);
    }

    #[test]
    fn cleans_task_directory_when_input_write_fails() {
        let app_root = TemporaryAppRoot::new("julia-input-failure");
        let manager = JuliaWorkerManager::new();
        let task_id = "input-failure";

        let error = manager
            .run_task(
                app_root.path(),
                JuliaWorkerTask {
                    task_id: Some(task_id.to_string()),
                    operation: "bayes_fit".to_string(),
                    parameters: json!({}),
                },
                |_| Err("private input write failure".to_string()),
                None,
            )
            .expect_err("input write must fail");

        assert_eq!(error.code(), JuliaWorkerErrorCode::InputWriteFailed);
        assert!(
            !app_root
                .path()
                .join(WORKER_DIR)
                .join(TASK_DIR)
                .join(task_id)
                .exists()
        );
    }

    #[test]
    fn rejects_noncanonical_task_id_before_writing_input() {
        let app_root = TemporaryAppRoot::new("julia-invalid-task-id");
        let manager = JuliaWorkerManager::new();
        let wrote_input = Cell::new(false);

        let error = manager
            .run_task(
                app_root.path(),
                JuliaWorkerTask {
                    task_id: Some("../escape".to_string()),
                    operation: "bayes_fit".to_string(),
                    parameters: json!({}),
                },
                |_| {
                    wrote_input.set(true);
                    Err("input writer must not run".to_string())
                },
                None,
            )
            .expect_err("noncanonical task id must be rejected");

        assert_eq!(error.code(), JuliaWorkerErrorCode::TaskDirectoryInvalid);
        assert!(!wrote_input.get());
        assert!(!app_root.path().join(WORKER_DIR).join(TASK_DIR).exists());
    }

    #[test]
    fn worker_startup_status_preserves_authoritative_runtime_probe() {
        let app_root = TemporaryAppRoot::new("julia-worker-status");
        let manager = JuliaWorkerManager::new();
        manager.set_startup_state(JuliaWorkerStartupState::Preparing);

        let status = manager.status_with_runtime_state(app_root.path(), JuliaRuntimeState::Invalid);

        assert_eq!(status.runtime_state, JuliaRuntimeState::Invalid);
        assert_eq!(
            status.process_state,
            super::JuliaWorkerProcessState::Starting
        );
        assert_eq!(
            status.environment_state,
            super::JuliaWorkerEnvironmentState::Missing
        );
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            json!({
                "runtimeState": "invalid",
                "environmentState": "missing",
                "processState": "starting",
                "projectDir": app_root.path().join(WORKER_DIR).to_string_lossy(),
            })
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

    #[test]
    fn cancellation_and_restart_release_active_task_lock_before_worker_io() {
        let manager = JuliaWorkerManager::new();
        manager
            .set_active_task(Some("active-task".to_owned()))
            .expect("set active task");
        let cancel_entered = Arc::new(Barrier::new(2));
        let cancel_release = Arc::new(Barrier::new(2));
        let thread_manager = manager.clone();
        let thread_entered = Arc::clone(&cancel_entered);
        let thread_release = Arc::clone(&cancel_release);
        let cancel = std::thread::spawn(move || {
            thread_manager.cancel_with_io_hook("active-task", || {
                thread_entered.wait();
                thread_release.wait();
            })
        });
        cancel_entered.wait();
        assert!(manager.inner.active_task_id.try_lock().is_ok());
        cancel_release.wait();
        assert!(!cancel.join().expect("cancel thread must finish").unwrap());

        manager
            .set_active_task(Some("active-task".to_owned()))
            .expect("reset active task");
        let restart_entered = Arc::new(Barrier::new(2));
        let restart_release = Arc::new(Barrier::new(2));
        let thread_manager = manager.clone();
        let thread_entered = Arc::clone(&restart_entered);
        let thread_release = Arc::clone(&restart_release);
        let restart = std::thread::spawn(move || {
            thread_manager.restart_task_with_io_hook("active-task", || {
                thread_entered.wait();
                thread_release.wait();
            })
        });
        restart_entered.wait();
        assert!(manager.inner.active_task_id.try_lock().is_ok());
        restart_release.wait();
        assert!(restart.join().expect("restart thread must finish").unwrap());
    }
}
