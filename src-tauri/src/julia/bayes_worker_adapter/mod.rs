use std::collections::BTreeMap;
use std::fs;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::julia::worker::{JuliaWorkerError, JuliaWorkerManager, JuliaWorkerTaskDirectory};
use crate::sci::api::bayes::worker::{
    BayesArtifact, BayesArtifactHandle, BayesCancelTerminal, BayesTaskHandle, BayesTaskId,
    BayesTaskResult, BayesWorkerError, BayesWorkerPhase, BayesWorkerPort, BayesWorkerTerminalCode,
    ValidatedBayesTask,
};
use crate::sci::api::control::{CancelDeliveryControl, ExecutionControl};

mod fit;
mod predictor;

use fit::{CompletedJuliaTask, OwnedArtifact, PreparedJuliaTask};

const CONTROL_POLL_INTERVAL: Duration = Duration::from_millis(20);

struct JuliaTaskCompletion {
    worker_task_id: Box<str>,
    metadata_path: PathBuf,
    task_directory: JuliaWorkerTaskDirectory,
}

trait JuliaBayesRuntime: Send + Sync {
    fn run_task(
        &self,
        app_data_dir: &Path,
        worker_task_id: &str,
        task: &PreparedJuliaTask,
    ) -> Result<JuliaTaskCompletion, JuliaWorkerError>;

    fn cancel(&self, worker_task_id: &str) -> Result<bool, JuliaWorkerError>;
}

struct ManagerJuliaBayesRuntime {
    worker: JuliaWorkerManager,
}

impl JuliaBayesRuntime for ManagerJuliaBayesRuntime {
    fn run_task(
        &self,
        app_data_dir: &Path,
        worker_task_id: &str,
        task: &PreparedJuliaTask,
    ) -> Result<JuliaTaskCompletion, JuliaWorkerError> {
        fit::run_manager_task(&self.worker, app_data_dir, worker_task_id, task)
    }

    fn cancel(&self, worker_task_id: &str) -> Result<bool, JuliaWorkerError> {
        self.worker.cancel(worker_task_id)
    }
}

pub struct JuliaBayesWorkerAdapter {
    app_data_dir: PathBuf,
    runtime: Arc<dyn JuliaBayesRuntime>,
    next_generation: AtomicU64,
    state: Mutex<AdapterState>,
}

#[derive(Default)]
struct AdapterState {
    current: BTreeMap<BayesTaskId, BayesTaskHandle>,
    tasks: BTreeMap<BayesTaskHandle, AdapterTaskState>,
}

enum AdapterTaskState {
    Active {
        worker_task_id: Box<str>,
        completion: Option<mpsc::Receiver<Result<JuliaTaskCompletion, JuliaWorkerError>>>,
    },
    Completed {
        result: Option<BayesTaskResult>,
        artifacts: BTreeMap<BayesArtifactHandle, OwnedArtifact>,
        _task_directory: JuliaWorkerTaskDirectory,
    },
    Cancelled,
    Failed,
}

impl JuliaBayesWorkerAdapter {
    pub fn new(app_data_dir: impl Into<PathBuf>, worker: JuliaWorkerManager) -> Self {
        Self::from_runtime(
            app_data_dir.into(),
            Arc::new(ManagerJuliaBayesRuntime { worker }),
        )
    }

    fn from_runtime(app_data_dir: PathBuf, runtime: Arc<dyn JuliaBayesRuntime>) -> Self {
        Self {
            app_data_dir,
            runtime,
            next_generation: AtomicU64::new(1),
            state: Mutex::new(AdapterState::default()),
        }
    }

    #[cfg(test)]
    fn with_runtime(app_data_dir: impl Into<PathBuf>, runtime: Arc<dyn JuliaBayesRuntime>) -> Self {
        Self::from_runtime(app_data_dir.into(), runtime)
    }

    fn issue_handle(
        &self,
        task_id: BayesTaskId,
    ) -> Result<(BayesTaskHandle, u64), BayesWorkerError> {
        let mut current = self.next_generation.load(Ordering::Acquire);
        loop {
            let Some(generation) = NonZeroU64::new(current) else {
                return Err(BayesWorkerError::WorkerUnavailable {
                    phase: BayesWorkerPhase::Start,
                });
            };
            let next = match current.checked_add(1) {
                Some(next) => next,
                None => 0,
            };
            match self.next_generation.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok((
                        BayesTaskHandle::issue_for_worker(task_id, generation),
                        current,
                    ));
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn validate_current<'a>(
        state: &'a AdapterState,
        handle: &BayesTaskHandle,
    ) -> Result<&'a AdapterTaskState, BayesWorkerError> {
        if state.current.get(handle.task_id()) != Some(handle) {
            return Err(BayesWorkerError::StaleTaskHandle {
                task: handle.clone(),
            });
        }
        state
            .tasks
            .get(handle)
            .ok_or_else(|| BayesWorkerError::StaleTaskHandle {
                task: handle.clone(),
            })
    }

    fn store_completion(
        &self,
        handle: &BayesTaskHandle,
        completed: CompletedJuliaTask,
    ) -> Result<BayesTaskResult, BayesWorkerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::AwaitResult,
            })?;
        if state.current.get(handle.task_id()) != Some(handle) {
            return Err(BayesWorkerError::StaleTaskHandle {
                task: handle.clone(),
            });
        }
        if matches!(state.tasks.get(handle), Some(AdapterTaskState::Cancelled)) {
            return Err(BayesWorkerError::Cancelled {
                task: handle.clone(),
            });
        }
        state.tasks.insert(
            handle.clone(),
            AdapterTaskState::Completed {
                result: None,
                artifacts: completed.artifacts,
                _task_directory: completed.task_directory,
            },
        );
        Ok(completed.result)
    }

    fn restore_receiver(
        &self,
        handle: &BayesTaskHandle,
        receiver: mpsc::Receiver<Result<JuliaTaskCompletion, JuliaWorkerError>>,
    ) {
        if let Ok(mut state) = self.state.lock()
            && let Some(AdapterTaskState::Active { completion, .. }) = state.tasks.get_mut(handle)
            && completion.is_none()
        {
            *completion = Some(receiver);
        }
    }
}

impl BayesWorkerPort for JuliaBayesWorkerAdapter {
    fn start(
        &self,
        task: ValidatedBayesTask,
        control: &ExecutionControl,
    ) -> Result<BayesTaskHandle, BayesWorkerError> {
        let task_id = task.task_id().clone();
        if control.is_cancelled() {
            return Err(BayesWorkerError::AdmissionClosed { task: task_id });
        }
        if control.is_expired(Instant::now()) {
            return Err(BayesWorkerError::AcceptanceDeadline { task: task_id });
        }
        let prepared = PreparedJuliaTask::try_from_task(&task).map_err(|_| {
            BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::Start,
            }
        })?;
        if control.is_cancelled() {
            return Err(BayesWorkerError::AdmissionClosed { task: task_id });
        }
        if control.is_expired(Instant::now()) {
            return Err(BayesWorkerError::AcceptanceDeadline { task: task_id });
        }
        let (handle, generation) = self.issue_handle(task_id.clone())?;
        let worker_task_id = format!("bayes-{generation}");
        let app_data_dir = self.app_data_dir.clone();
        let runtime = Arc::clone(&self.runtime);
        let thread_worker_task_id = worker_task_id.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("julia-bayes-worker-adapter".to_owned())
            .spawn(move || {
                let result = runtime.run_task(&app_data_dir, &thread_worker_task_id, &prepared);
                let _ = sender.send(result);
            })
            .map_err(|_| BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::Start,
            })?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::Start,
            })?;
        state.current.insert(task_id, handle.clone());
        state.tasks.insert(
            handle.clone(),
            AdapterTaskState::Active {
                worker_task_id: worker_task_id.into(),
                completion: Some(receiver),
            },
        );
        Ok(handle)
    }

    fn await_result(
        &self,
        handle: &BayesTaskHandle,
        control: &ExecutionControl,
    ) -> Result<BayesTaskResult, BayesWorkerError> {
        let (receiver, expected_worker_task_id) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| BayesWorkerError::WorkerUnavailable {
                    phase: BayesWorkerPhase::AwaitResult,
                })?;
            Self::validate_current(&state, handle)?;
            match state.tasks.get_mut(handle) {
                Some(AdapterTaskState::Active {
                    worker_task_id,
                    completion,
                }) => (
                    completion
                        .take()
                        .ok_or(BayesWorkerError::WorkerUnavailable {
                            phase: BayesWorkerPhase::AwaitResult,
                        })?,
                    worker_task_id.clone(),
                ),
                Some(AdapterTaskState::Completed { result, .. }) => {
                    return result.take().ok_or(BayesWorkerError::WorkerTerminal {
                        task: handle.clone(),
                        terminal: BayesWorkerTerminalCode::Succeeded,
                    });
                }
                Some(AdapterTaskState::Cancelled) => {
                    return Err(BayesWorkerError::Cancelled {
                        task: handle.clone(),
                    });
                }
                Some(AdapterTaskState::Failed) => {
                    return Err(BayesWorkerError::WorkerTerminal {
                        task: handle.clone(),
                        terminal: BayesWorkerTerminalCode::Failed,
                    });
                }
                None => {
                    return Err(BayesWorkerError::StaleTaskHandle {
                        task: handle.clone(),
                    });
                }
            }
        };

        loop {
            if control.is_cancelled() {
                self.restore_receiver(handle, receiver);
                return Err(BayesWorkerError::Cancelled {
                    task: handle.clone(),
                });
            }
            let now = Instant::now();
            let Some(remaining) = control.remaining(now) else {
                self.restore_receiver(handle, receiver);
                return Err(BayesWorkerError::WorkerUnavailable {
                    phase: BayesWorkerPhase::AwaitResult,
                });
            };
            match receiver.recv_timeout(remaining.min(CONTROL_POLL_INTERVAL)) {
                Ok(Ok(completion)) => {
                    let completed = fit::finish_task(handle, &expected_worker_task_id, completion)?;
                    return self.store_completion(handle, completed);
                }
                Ok(Err(error)) => {
                    let mapped = fit::map_worker_error(handle, error);
                    if let Ok(mut state) = self.state.lock() {
                        state.tasks.insert(
                            handle.clone(),
                            if matches!(mapped, BayesWorkerError::Cancelled { .. }) {
                                AdapterTaskState::Cancelled
                            } else {
                                AdapterTaskState::Failed
                            },
                        );
                    }
                    return Err(mapped);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(BayesWorkerError::WorkerUnavailable {
                        phase: BayesWorkerPhase::AwaitResult,
                    });
                }
            }
        }
    }

    fn cancel(
        &self,
        handle: &BayesTaskHandle,
        control: &CancelDeliveryControl,
    ) -> Result<BayesCancelTerminal, BayesWorkerError> {
        let worker_task_id = {
            let state = self
                .state
                .lock()
                .map_err(|_| BayesWorkerError::WorkerUnavailable {
                    phase: BayesWorkerPhase::CancelDelivery,
                })?;
            match Self::validate_current(&state, handle)? {
                AdapterTaskState::Active { worker_task_id, .. } => worker_task_id.clone(),
                AdapterTaskState::Completed { .. } => {
                    return Ok(BayesCancelTerminal::AlreadyTerminal {
                        terminal: BayesWorkerTerminalCode::Succeeded,
                    });
                }
                AdapterTaskState::Cancelled => {
                    return Ok(BayesCancelTerminal::AlreadyTerminal {
                        terminal: BayesWorkerTerminalCode::Cancelled,
                    });
                }
                AdapterTaskState::Failed => {
                    return Ok(BayesCancelTerminal::AlreadyTerminal {
                        terminal: BayesWorkerTerminalCode::Failed,
                    });
                }
            }
        };
        let delivered = self.runtime.cancel(&worker_task_id).map_err(|_| {
            BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::CancelDelivery,
            }
        })?;
        if control.is_expired(Instant::now()) {
            return Err(BayesWorkerError::CancelDeliveryDeadline {
                task: handle.clone(),
            });
        }
        if !delivered {
            return Err(BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::CancelDelivery,
            });
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::CancelDelivery,
            })?;
        Self::validate_current(&state, handle)?;
        match state.tasks.get(handle) {
            Some(AdapterTaskState::Active { .. }) => {
                state
                    .tasks
                    .insert(handle.clone(), AdapterTaskState::Cancelled);
                Ok(BayesCancelTerminal::Cancelled)
            }
            Some(AdapterTaskState::Completed { .. }) => Ok(BayesCancelTerminal::AlreadyTerminal {
                terminal: BayesWorkerTerminalCode::Succeeded,
            }),
            Some(AdapterTaskState::Cancelled) => Ok(BayesCancelTerminal::AlreadyTerminal {
                terminal: BayesWorkerTerminalCode::Cancelled,
            }),
            Some(AdapterTaskState::Failed) => Ok(BayesCancelTerminal::AlreadyTerminal {
                terminal: BayesWorkerTerminalCode::Failed,
            }),
            None => Err(BayesWorkerError::StaleTaskHandle {
                task: handle.clone(),
            }),
        }
    }

    fn read_artifact(
        &self,
        artifact: &BayesArtifactHandle,
        control: &ExecutionControl,
    ) -> Result<BayesArtifact, BayesWorkerError> {
        if control.is_expired(Instant::now()) {
            return Err(BayesWorkerError::ArtifactReadDeadline {
                artifact: artifact.clone(),
            });
        }
        let (path, media_type) = {
            let state = self
                .state
                .lock()
                .map_err(|_| BayesWorkerError::WorkerUnavailable {
                    phase: BayesWorkerPhase::ReadArtifact,
                })?;
            match Self::validate_current(&state, artifact.task())? {
                AdapterTaskState::Completed { artifacts, .. } => {
                    let owned = artifacts.get(artifact).ok_or_else(|| {
                        BayesWorkerError::ArtifactNotOwned {
                            artifact: artifact.clone(),
                        }
                    })?;
                    (owned.path.clone(), owned.media_type)
                }
                AdapterTaskState::Cancelled => {
                    return Err(BayesWorkerError::Cancelled {
                        task: artifact.task().clone(),
                    });
                }
                AdapterTaskState::Active { .. } | AdapterTaskState::Failed => {
                    return Err(BayesWorkerError::ArtifactNotOwned {
                        artifact: artifact.clone(),
                    });
                }
            }
        };
        let bytes = fs::read(path).map_err(|_| BayesWorkerError::ArtifactNotOwned {
            artifact: artifact.clone(),
        })?;
        if control.is_expired(Instant::now()) {
            return Err(BayesWorkerError::ArtifactReadDeadline {
                artifact: artifact.clone(),
            });
        }
        Ok(BayesArtifact::from_worker(
            artifact.clone(),
            media_type,
            Arc::from(bytes),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::time::{Duration, Instant};

    use super::{
        JuliaBayesRuntime, JuliaBayesWorkerAdapter, JuliaTaskCompletion, PreparedJuliaTask,
    };
    use crate::julia::worker::{JuliaWorkerError, JuliaWorkerTaskDirectory};
    use crate::sci::api::bayes::BayesModelSpec;
    use crate::sci::api::bayes::worker::{
        ArtifactId, BayesArtifactHandle, BayesArtifactMediaType, BayesCancelTerminal, BayesTaskId,
        BayesWorkerError, BayesWorkerPort, ValidatedBayesTask,
    };
    use crate::sci::api::computation::{StatisticalInput, StatisticalScalar};
    use crate::sci::api::control::{
        AbsoluteDeadline, CancelDeliveryControl, ExecutionControl, SciCancellationSource,
    };

    struct TemporaryAppRoot(std::path::PathBuf);

    impl TemporaryAppRoot {
        fn new(label: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(1);
            let path = std::env::temp_dir().join(format!(
                "yssbi-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("temporary app root must be created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryAppRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Clone)]
    struct TestGate {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    impl TestGate {
        fn new() -> Self {
            Self {
                entered: Arc::new(Barrier::new(2)),
                release: Arc::new(Barrier::new(2)),
            }
        }

        fn wait_inside(&self) {
            self.entered.wait();
            self.release.wait();
        }

        fn wait_until_entered(&self) {
            self.entered.wait();
        }

        fn release(&self) {
            self.release.wait();
        }
    }

    struct FakeRun {
        artifacts: Vec<(&'static str, &'static [u8])>,
        gate: Option<TestGate>,
    }

    struct FakeRuntime {
        runs: Mutex<VecDeque<FakeRun>>,
        cancel_attempts: AtomicUsize,
    }

    impl FakeRuntime {
        fn new(runs: impl IntoIterator<Item = FakeRun>) -> Self {
            Self {
                runs: Mutex::new(runs.into_iter().collect()),
                cancel_attempts: AtomicUsize::new(0),
            }
        }

        fn cancel_attempts(&self) -> usize {
            self.cancel_attempts.load(Ordering::Acquire)
        }
    }

    impl JuliaBayesRuntime for FakeRuntime {
        fn run_task(
            &self,
            app_data_dir: &Path,
            worker_task_id: &str,
            _task: &PreparedJuliaTask,
        ) -> Result<JuliaTaskCompletion, JuliaWorkerError> {
            let run = self
                .runs
                .lock()
                .expect("fake run queue must be available")
                .pop_front()
                .expect("fake run must be configured");
            if let Some(gate) = &run.gate {
                gate.wait_inside();
            }
            let directory = JuliaWorkerTaskDirectory::create(app_data_dir, worker_task_id)?;
            let mut artifact_records = Vec::new();
            for (name, bytes) in run.artifacts {
                let path = directory.path().join(name);
                fs::write(&path, bytes).expect("fake artifact must be written");
                artifact_records.push(serde_json::json!({ "path": path }));
            }
            let metadata_path = directory.path().join("metadata.json");
            fs::write(
                &metadata_path,
                serde_json::to_vec(&serde_json::json!({
                    "summaries": [],
                    "diagnostics": {
                        "chains": 1,
                        "drawsPerChain": 1,
                        "warmup": 0,
                        "divergences": 0,
                        "maxTreedepthHits": 0,
                        "warnings": []
                    },
                    "artifactManifest": {
                        "taskId": worker_task_id,
                        "artifacts": artifact_records
                    }
                }))
                .expect("fake metadata must serialize"),
            )
            .expect("fake metadata must be written");
            Ok(JuliaTaskCompletion {
                worker_task_id: worker_task_id.into(),
                metadata_path,
                task_directory: directory,
            })
        }

        fn cancel(&self, _worker_task_id: &str) -> Result<bool, JuliaWorkerError> {
            self.cancel_attempts.fetch_add(1, Ordering::AcqRel);
            Ok(true)
        }
    }

    fn validated_task(task_id: &str) -> ValidatedBayesTask {
        let model: BayesModelSpec = serde_json::from_value(serde_json::json!({
            "dataset": { "sourceType": "table", "sourceId": "dataset" },
            "response": {
                "expression": { "type": "data_variable", "name": "y" },
                "dataVariables": { "y": "response" }
            },
            "predictor": {
                "type": "binary",
                "op": "mul",
                "left": { "type": "parameter", "name": "beta" },
                "right": { "type": "data_variable", "name": "x" }
            },
            "dataVariables": { "x": "predictor" },
            "likelihood": {
                "type": "normal",
                "mean": { "source": "predictor" },
                "sigma": { "parameter": "sigma" }
            },
            "parameters": [
                { "name": "beta", "constraint": { "type": "real" }, "prior": { "distribution": "normal", "args": [0.0, 1.0] } },
                { "name": "sigma", "constraint": { "type": "positive" }, "prior": { "distribution": "exponential", "args": [1.0] } }
            ],
            "sampler": { "algorithm": "nuts", "chains": 1, "samples": 10, "warmup": 5, "saveSamples": false },
            "displayFormula": "response ~ beta * predictor"
        }))
        .expect("model fixture must deserialize");
        let input = |name: &str, values: [f64; 2]| {
            StatisticalInput::new(
                name.into(),
                values
                    .into_iter()
                    .map(|value| Some(StatisticalScalar::Numeric(value)))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                None,
            )
        };
        ValidatedBayesTask::try_new(
            BayesTaskId::try_from(task_id).expect("task ID must validate"),
            model,
            Arc::from([
                input("response", [1.0, 2.0]),
                input("predictor", [3.0, 4.0]),
            ]),
        )
        .expect("task fixture must validate")
    }

    fn controls() -> (ExecutionControl, CancelDeliveryControl, Instant) {
        let now = Instant::now();
        let future = now
            .checked_add(Duration::from_secs(30))
            .expect("test deadline must be representable");
        let (_source, token) = SciCancellationSource::new();
        (
            ExecutionControl::new(token, AbsoluteDeadline::at(future)),
            CancelDeliveryControl::new(AbsoluteDeadline::at(future)),
            now,
        )
    }

    #[test]
    fn accepted_result_maps_owned_artifacts_and_rejects_stale_or_foreign_handles() {
        let root = TemporaryAppRoot::new("julia-adapter-result");
        let runtime = Arc::new(FakeRuntime::new([
            FakeRun {
                artifacts: vec![
                    ("summary.json", b"{}"),
                    ("table.csv", b"x\n1\n"),
                    ("plot.png", b"png"),
                    ("draws.arrow", b"arrow"),
                ],
                gate: None,
            },
            FakeRun {
                artifacts: Vec::new(),
                gate: None,
            },
        ]));
        let adapter = JuliaBayesWorkerAdapter::with_runtime(root.path(), runtime);
        let (run_control, _, _) = controls();
        let first = adapter
            .start(validated_task("shared-task"), &run_control)
            .expect("task must be accepted");
        let result = adapter
            .await_result(&first, &run_control)
            .expect("task must complete");
        assert_eq!(result.task(), &first);
        assert_eq!(result.artifacts().len(), 4);
        for handle in result.artifacts() {
            let expected = match handle.artifact_id().as_str() {
                "summary.json" => BayesArtifactMediaType::Json,
                "table.csv" => BayesArtifactMediaType::Csv,
                "plot.png" => BayesArtifactMediaType::Png,
                "draws.arrow" => BayesArtifactMediaType::Binary,
                other => panic!("unexpected artifact fixture: {other}"),
            };
            assert_eq!(
                adapter
                    .read_artifact(handle, &run_control)
                    .expect("owned artifact must be readable")
                    .media_type(),
                expected
            );
        }

        let second = adapter
            .start(validated_task("shared-task"), &run_control)
            .expect("new generation must be accepted");
        assert!(matches!(
            adapter.read_artifact(&result.artifacts()[0], &run_control),
            Err(BayesWorkerError::StaleTaskHandle { task }) if task == first
        ));
        let foreign = BayesArtifactHandle::mint_for_worker(
            second,
            ArtifactId::try_from("foreign.json").expect("artifact ID must validate"),
        );
        assert!(matches!(
            adapter.read_artifact(&foreign, &run_control),
            Err(BayesWorkerError::ArtifactNotOwned { artifact }) if artifact == foreign
        ));
    }

    #[test]
    fn cancellation_deadlines_and_unknown_artifact_formats_are_typed() {
        let root = TemporaryAppRoot::new("julia-adapter-control");
        let gate = TestGate::new();
        let runtime = Arc::new(FakeRuntime::new([
            FakeRun {
                artifacts: Vec::new(),
                gate: Some(gate.clone()),
            },
            FakeRun {
                artifacts: vec![("samples.weird", b"unknown")],
                gate: None,
            },
        ]));
        let adapter = JuliaBayesWorkerAdapter::with_runtime(root.path(), runtime.clone());
        let (run_control, cancel_control, now) = controls();
        let (_source, token) = SciCancellationSource::new();
        let expired_run = ExecutionControl::new(token, AbsoluteDeadline::at(now));
        assert!(matches!(
            adapter.start(validated_task("expired-start"), &expired_run),
            Err(BayesWorkerError::AcceptanceDeadline { task })
                if task.as_str() == "expired-start"
        ));

        let handle = adapter
            .start(validated_task("cancelled-task"), &run_control)
            .expect("active task must be accepted");
        gate.wait_until_entered();
        assert!(matches!(
            adapter.cancel(
                &handle,
                &CancelDeliveryControl::new(AbsoluteDeadline::at(now)),
            ),
            Err(BayesWorkerError::CancelDeliveryDeadline { task }) if task == handle
        ));
        assert_eq!(runtime.cancel_attempts(), 1);
        assert_eq!(
            adapter.cancel(&handle, &cancel_control),
            Ok(BayesCancelTerminal::Cancelled)
        );
        assert!(matches!(
            adapter.await_result(&handle, &run_control),
            Err(BayesWorkerError::Cancelled { task }) if task == handle
        ));
        gate.release();

        let unknown = adapter
            .start(validated_task("unknown-format"), &run_control)
            .expect("unknown-format task must be accepted");
        assert!(matches!(
            adapter.await_result(&unknown, &run_control),
            Err(BayesWorkerError::ArtifactFormatUnsupported { artifact })
                if artifact.task() == &unknown
        ));
    }
}
