use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};

use super::{
    ArtifactId, ArtifactIdValidationError, BayesArtifact, BayesArtifactHandle,
    BayesArtifactMediaType, BayesCancelTerminal, BayesTaskHandle, BayesTaskId,
    BayesTaskIdValidationError, BayesTaskResult, BayesTaskValidationError, BayesWorkerError,
    BayesWorkerPhase, BayesWorkerPort, BayesWorkerTerminalCode, ValidatedBayesTask,
};
use crate::sci::api::bayes::{
    BayesModelSpec, BinaryOp, DatasetRef, DatasetSourceType, Expression, InferenceConfig,
    InferenceDiagnostics, InferenceResult, LikelihoodSpec, ParameterConstraint, ParameterRef,
    ParameterSpec, PredictorSource, PredictorSourceKind, PriorSpec, ResponseSpec,
    ResultArtifactManifest, SamplerAlgorithm,
};
use crate::sci::api::computation::{StatisticalInput, StatisticalScalar};
use crate::sci::api::control::{
    AbsoluteDeadline, CancelDeliveryControl, ExecutionControl, SciCancellationSource,
};

fn valid_model() -> BayesModelSpec {
    BayesModelSpec {
        dataset: DatasetRef {
            source_type: DatasetSourceType::Table,
            source_id: "dataset".to_owned(),
        },
        response: ResponseSpec {
            expression: Expression::DataVariable {
                name: "y".to_owned(),
            },
            data_variables: BTreeMap::from([("y".to_owned(), "response".to_owned())]),
        },
        predictor: Expression::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expression::Parameter {
                name: "beta".to_owned(),
            }),
            right: Box::new(Expression::DataVariable {
                name: "x".to_owned(),
            }),
        },
        data_variables: BTreeMap::from([("x".to_owned(), "predictor".to_owned())]),
        likelihood: LikelihoodSpec::Normal {
            mean: PredictorSource {
                source: PredictorSourceKind::Predictor,
            },
            sigma: ParameterRef {
                parameter: "sigma".to_owned(),
            },
        },
        parameters: vec![
            ParameterSpec {
                name: "beta".to_owned(),
                constraint: ParameterConstraint::Real,
                prior: PriorSpec::Normal([0.0, 1.0]),
            },
            ParameterSpec {
                name: "sigma".to_owned(),
                constraint: ParameterConstraint::Positive,
                prior: PriorSpec::Exponential([1.0]),
            },
        ],
        sampler: InferenceConfig {
            algorithm: SamplerAlgorithm::Nuts,
            chains: 2,
            samples: 100,
            warmup: 50,
            seed: Some(7),
            target_accept: Some(0.8),
            max_tree_depth: Some(10),
            save_samples: false,
        },
        display_formula: "response ~ beta * predictor".to_owned(),
    }
}

fn input(name: &str, values: Vec<Option<StatisticalScalar>>) -> StatisticalInput {
    StatisticalInput::new(name.into(), values.into_boxed_slice(), None)
}

fn valid_inputs() -> Arc<[StatisticalInput]> {
    Arc::from([
        input(
            "response",
            vec![
                Some(StatisticalScalar::Numeric(1.0)),
                Some(StatisticalScalar::Numeric(2.0)),
            ],
        ),
        input(
            "predictor",
            vec![
                Some(StatisticalScalar::Numeric(3.0)),
                Some(StatisticalScalar::Numeric(4.0)),
            ],
        ),
    ])
}

fn task_handle(task_id: &BayesTaskId, generation: u64) -> BayesTaskHandle {
    BayesTaskHandle::issue_for_worker(
        task_id.clone(),
        NonZeroU64::new(generation).expect("test generation must be non-zero"),
    )
}

fn inference_result(task_id: &str) -> InferenceResult {
    InferenceResult::new(
        Vec::new(),
        InferenceDiagnostics {
            chains: 1,
            draws_per_chain: 1,
            warmup: 0,
            divergences: Some(0),
            max_treedepth_hits: Some(0),
            warnings: Vec::new(),
        },
        ResultArtifactManifest {
            task_id: task_id.to_owned(),
            artifacts: Vec::new(),
        },
    )
}

fn validated_task(task_id: &str) -> ValidatedBayesTask {
    ValidatedBayesTask::try_new(
        BayesTaskId::try_from(task_id).expect("test task ID must validate"),
        valid_model(),
        valid_inputs(),
    )
    .expect("test task must validate")
}

fn execution_control(deadline: Instant) -> ExecutionControl {
    let (_source, token) = SciCancellationSource::new();
    ExecutionControl::new(token, AbsoluteDeadline::at(deadline))
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

#[derive(Clone, Copy)]
enum FakeTaskState {
    Active,
    Terminal(BayesWorkerTerminalCode),
}

struct FakeWorkerState {
    current_tasks: BTreeMap<BayesTaskId, BayesTaskHandle>,
    tasks: BTreeMap<BayesTaskHandle, FakeTaskState>,
    artifacts: BTreeMap<BayesArtifactHandle, (BayesArtifactMediaType, Arc<[u8]>)>,
    cancel_attempts: usize,
}

struct FakeWorker {
    state: Mutex<FakeWorkerState>,
    next_generation: AtomicU64,
    now: Mutex<Instant>,
    start_gate: Option<TestGate>,
    read_gate: Option<TestGate>,
}

impl FakeWorker {
    fn new(now: Instant) -> Self {
        Self::with_gates(now, None, None)
    }

    fn with_gates(now: Instant, start_gate: Option<TestGate>, read_gate: Option<TestGate>) -> Self {
        Self {
            state: Mutex::new(FakeWorkerState {
                current_tasks: BTreeMap::new(),
                tasks: BTreeMap::new(),
                artifacts: BTreeMap::new(),
                cancel_attempts: 0,
            }),
            next_generation: AtomicU64::new(1),
            now: Mutex::new(now),
            start_gate,
            read_gate,
        }
    }

    fn now(&self) -> Instant {
        *self.now.lock().expect("test clock lock must be available")
    }

    fn set_now(&self, now: Instant) {
        *self.now.lock().expect("test clock lock must be available") = now;
    }

    fn tracked_task_count(&self) -> usize {
        self.state
            .lock()
            .expect("test state lock must be available")
            .tasks
            .len()
    }

    fn cancel_attempts(&self) -> usize {
        self.state
            .lock()
            .expect("test state lock must be available")
            .cancel_attempts
    }

    fn complete(&self, handle: &BayesTaskHandle, terminal: BayesWorkerTerminalCode) {
        let mut state = self
            .state
            .lock()
            .expect("test state lock must be available");
        assert_eq!(state.current_tasks.get(handle.task_id()), Some(handle));
        state
            .tasks
            .insert(handle.clone(), FakeTaskState::Terminal(terminal));
    }

    fn add_artifact(
        &self,
        task: BayesTaskHandle,
        artifact_id: ArtifactId,
        bytes: &'static [u8],
    ) -> BayesArtifactHandle {
        let handle = BayesArtifactHandle::mint_for_worker(task, artifact_id);
        self.state
            .lock()
            .expect("test state lock must be available")
            .artifacts
            .insert(
                handle.clone(),
                (BayesArtifactMediaType::Json, Arc::from(bytes)),
            );
        handle
    }

    fn validate_current(
        state: &FakeWorkerState,
        handle: &BayesTaskHandle,
    ) -> Result<FakeTaskState, BayesWorkerError> {
        if state.current_tasks.get(handle.task_id()) != Some(handle) {
            return Err(BayesWorkerError::StaleTaskHandle {
                task: handle.clone(),
            });
        }
        state
            .tasks
            .get(handle)
            .copied()
            .ok_or_else(|| BayesWorkerError::StaleTaskHandle {
                task: handle.clone(),
            })
    }
}

impl BayesWorkerPort for FakeWorker {
    fn start(
        &self,
        task: ValidatedBayesTask,
        control: &ExecutionControl,
    ) -> Result<BayesTaskHandle, BayesWorkerError> {
        let task_id = task.task_id().clone();
        if control.is_cancelled() {
            return Err(BayesWorkerError::AdmissionClosed { task: task_id });
        }
        if control.is_expired(self.now()) {
            return Err(BayesWorkerError::AcceptanceDeadline { task: task_id });
        }
        if let Some(gate) = &self.start_gate {
            gate.wait_inside();
        }
        if control.is_cancelled() {
            return Err(BayesWorkerError::AdmissionClosed { task: task_id });
        }
        if control.is_expired(self.now()) {
            return Err(BayesWorkerError::AcceptanceDeadline { task: task_id });
        }

        let generation = NonZeroU64::new(self.next_generation.fetch_add(1, Ordering::Relaxed))
            .expect("test generation must stay non-zero");
        let handle = BayesTaskHandle::issue_for_worker(task_id.clone(), generation);
        let mut state = self
            .state
            .lock()
            .expect("test state lock must be available");
        state.current_tasks.insert(task_id, handle.clone());
        state.tasks.insert(handle.clone(), FakeTaskState::Active);
        Ok(handle)
    }

    fn await_result(
        &self,
        handle: &BayesTaskHandle,
        _control: &ExecutionControl,
    ) -> Result<BayesTaskResult, BayesWorkerError> {
        let task_state = {
            let state = self
                .state
                .lock()
                .expect("test state lock must be available");
            Self::validate_current(&state, handle)?
        };
        match task_state {
            FakeTaskState::Active => Err(BayesWorkerError::WorkerUnavailable {
                phase: BayesWorkerPhase::AwaitResult,
            }),
            FakeTaskState::Terminal(BayesWorkerTerminalCode::Succeeded) => {
                BayesTaskResult::validated_worker_result(
                    handle,
                    handle.clone(),
                    inference_result(handle.task_id().as_str()),
                    Arc::from([]),
                )
            }
            FakeTaskState::Terminal(BayesWorkerTerminalCode::Failed) => {
                Err(BayesWorkerError::WorkerTerminal {
                    task: handle.clone(),
                    terminal: BayesWorkerTerminalCode::Failed,
                })
            }
            FakeTaskState::Terminal(BayesWorkerTerminalCode::Cancelled) => {
                Err(BayesWorkerError::Cancelled {
                    task: handle.clone(),
                })
            }
        }
    }

    fn cancel(
        &self,
        handle: &BayesTaskHandle,
        control: &CancelDeliveryControl,
    ) -> Result<BayesCancelTerminal, BayesWorkerError> {
        {
            let mut state = self
                .state
                .lock()
                .expect("test state lock must be available");
            Self::validate_current(&state, handle)?;
            state.cancel_attempts += 1;
        }
        if control.is_expired(self.now()) {
            return Err(BayesWorkerError::CancelDeliveryDeadline {
                task: handle.clone(),
            });
        }

        let mut state = self
            .state
            .lock()
            .expect("test state lock must be available");
        match Self::validate_current(&state, handle)? {
            FakeTaskState::Active => {
                state.tasks.insert(
                    handle.clone(),
                    FakeTaskState::Terminal(BayesWorkerTerminalCode::Cancelled),
                );
                Ok(BayesCancelTerminal::Cancelled)
            }
            FakeTaskState::Terminal(terminal) => {
                Ok(BayesCancelTerminal::AlreadyTerminal { terminal })
            }
        }
    }

    fn read_artifact(
        &self,
        artifact: &BayesArtifactHandle,
        control: &ExecutionControl,
    ) -> Result<BayesArtifact, BayesWorkerError> {
        if control.is_expired(self.now()) {
            return Err(BayesWorkerError::ArtifactReadDeadline {
                artifact: artifact.clone(),
            });
        }
        let (media_type, bytes) = {
            let state = self
                .state
                .lock()
                .expect("test state lock must be available");
            Self::validate_current(&state, artifact.task())?;
            state.artifacts.get(artifact).cloned().ok_or_else(|| {
                BayesWorkerError::ArtifactNotOwned {
                    artifact: artifact.clone(),
                }
            })?
        };
        if let Some(gate) = &self.read_gate {
            gate.wait_inside();
        }
        if control.is_expired(self.now()) {
            return Err(BayesWorkerError::ArtifactReadDeadline {
                artifact: artifact.clone(),
            });
        }
        Ok(BayesArtifact::from_worker(
            artifact.clone(),
            media_type,
            bytes,
        ))
    }
}

#[test]
fn task_and_artifact_ids_reject_non_portable_or_traversal_names() {
    let valid = "Az09._-";
    assert_eq!(
        BayesTaskId::try_from(valid)
            .expect("portable task ID must validate")
            .as_str(),
        valid
    );
    assert_eq!(
        ArtifactId::try_from(valid)
            .expect("portable artifact ID must validate")
            .as_str(),
        valid
    );

    let long = "a".repeat(129);
    let task_cases = [
        ("", BayesTaskIdValidationError::Empty),
        (
            long.as_str(),
            BayesTaskIdValidationError::TooLong { max: 128 },
        ),
        (
            "a/b",
            BayesTaskIdValidationError::InvalidCharacter { index: 1 },
        ),
        (
            "a\\b",
            BayesTaskIdValidationError::InvalidCharacter { index: 1 },
        ),
        (
            "a\0b",
            BayesTaskIdValidationError::InvalidCharacter { index: 1 },
        ),
        (
            "a..b",
            BayesTaskIdValidationError::ReservedSequence { index: 1 },
        ),
    ];
    for (value, expected) in task_cases {
        assert_eq!(BayesTaskId::try_from(value), Err(expected));
    }

    let artifact_cases = [
        ("", ArtifactIdValidationError::Empty),
        (
            long.as_str(),
            ArtifactIdValidationError::TooLong { max: 128 },
        ),
        (
            "a/b",
            ArtifactIdValidationError::InvalidCharacter { index: 1 },
        ),
        (
            "a\\b",
            ArtifactIdValidationError::InvalidCharacter { index: 1 },
        ),
        (
            "a\0b",
            ArtifactIdValidationError::InvalidCharacter { index: 1 },
        ),
        (
            "a..b",
            ArtifactIdValidationError::ReservedSequence { index: 1 },
        ),
    ];
    for (value, expected) in artifact_cases {
        assert_eq!(ArtifactId::try_from(value), Err(expected));
    }
}

#[test]
fn validated_task_reruns_model_and_indexed_input_invariants() {
    let task_id = BayesTaskId::try_from("task-valid").expect("valid task ID");
    let task = ValidatedBayesTask::try_new(task_id.clone(), valid_model(), valid_inputs())
        .expect("valid model and inputs must produce a task");
    assert_eq!(task.task_id(), &task_id);
    assert!(matches!(
        task.model().predictor(),
        Expression::Binary { .. }
    ));
    assert_eq!(
        task.inputs()
            .iter()
            .map(StatisticalInput::name)
            .collect::<Vec<_>>(),
        ["response", "predictor"]
    );

    let mut invalid_model = valid_model();
    invalid_model.sampler.samples = 0;
    assert!(matches!(
        ValidatedBayesTask::try_new(task_id.clone(), invalid_model, valid_inputs()),
        Err(BayesTaskValidationError::InvalidModel)
    ));

    let invalid_inputs = Arc::from([
        input(
            "response",
            vec![
                Some(StatisticalScalar::Numeric(1.0)),
                Some(StatisticalScalar::Numeric(2.0)),
            ],
        ),
        input(
            "predictor",
            vec![Some(StatisticalScalar::Numeric(3.0)), None],
        ),
    ]);
    assert!(matches!(
        ValidatedBayesTask::try_new(task_id, valid_model(), invalid_inputs),
        Err(BayesTaskValidationError::InvalidInput { index: 1 })
    ));
}

#[test]
fn result_and_artifact_builders_require_the_full_awaited_handle() {
    let task_id = BayesTaskId::try_from("task-authority").expect("valid task ID");
    let awaited = task_handle(&task_id, 1);
    let other_generation = task_handle(&task_id, 2);
    let artifact_id = ArtifactId::try_from("summary.json").expect("valid artifact ID");
    let matching_artifact =
        BayesArtifactHandle::mint_for_worker(awaited.clone(), artifact_id.clone());

    let result = BayesTaskResult::validated_worker_result(
        &awaited,
        awaited.clone(),
        inference_result(task_id.as_str()),
        Arc::from([matching_artifact.clone()]),
    )
    .expect("matching full handles must produce a result");
    assert_eq!(result.task(), &awaited);
    assert_eq!(
        result.inference().artifact_manifest().task_id,
        task_id.as_str()
    );
    assert_eq!(result.artifacts(), [matching_artifact.clone()]);

    assert!(matches!(
        BayesTaskResult::validated_worker_result(
            &awaited,
            other_generation.clone(),
            inference_result(task_id.as_str()),
            Arc::from([]),
        ),
        Err(BayesWorkerError::StaleTaskHandle { task }) if task == other_generation
    ));

    let foreign_artifact = BayesArtifactHandle::mint_for_worker(other_generation, artifact_id);
    assert!(matches!(
        BayesTaskResult::validated_worker_result(
            &awaited,
            awaited.clone(),
            inference_result(task_id.as_str()),
            Arc::from([foreign_artifact.clone()]),
        ),
        Err(BayesWorkerError::ArtifactNotOwned { artifact }) if artifact == foreign_artifact
    ));

    let artifact = BayesArtifact::from_worker(
        matching_artifact.clone(),
        BayesArtifactMediaType::Json,
        Arc::from(&b"{}"[..]),
    );
    assert_eq!(artifact.handle(), &matching_artifact);
    assert_eq!(artifact.media_type(), BayesArtifactMediaType::Json);
    assert_eq!(artifact.bytes(), b"{}");
}

#[test]
fn bayes_worker_handle_cancel_completion_and_artifact_deadlines_are_typed() {
    let now = Instant::now();
    let future = now
        .checked_add(Duration::from_secs(5))
        .expect("short test deadline must be representable");

    let start_gate = TestGate::new();
    let gated_worker = Arc::new(FakeWorker::with_gates(now, Some(start_gate.clone()), None));
    let (cancel_source, cancel_token) = SciCancellationSource::new();
    let start_control = ExecutionControl::new(cancel_token, AbsoluteDeadline::at(future));
    let start_worker = Arc::clone(&gated_worker);
    let start_thread = std::thread::spawn(move || {
        start_worker.start(validated_task("pre-publication"), &start_control)
    });
    start_gate.wait_until_entered();
    cancel_source.cancel();
    start_gate.release();
    assert!(matches!(
        start_thread.join().expect("test start thread must finish"),
        Err(BayesWorkerError::AdmissionClosed { task })
            if task.as_str() == "pre-publication"
    ));
    assert_eq!(gated_worker.tracked_task_count(), 0);

    let worker = FakeWorker::new(now);
    let run_control = execution_control(future);
    let cancel_first = worker
        .start(validated_task("cancel-first"), &run_control)
        .expect("start must publish a handle");
    assert!(matches!(
        worker.await_result(&cancel_first, &run_control),
        Err(BayesWorkerError::WorkerUnavailable {
            phase: BayesWorkerPhase::AwaitResult
        })
    ));
    let cancel_control = CancelDeliveryControl::new(AbsoluteDeadline::at(future));
    assert_eq!(
        worker.cancel(&cancel_first, &cancel_control),
        Ok(BayesCancelTerminal::Cancelled)
    );
    assert_eq!(
        worker.cancel(&cancel_first, &cancel_control),
        Ok(BayesCancelTerminal::AlreadyTerminal {
            terminal: BayesWorkerTerminalCode::Cancelled
        })
    );
    assert!(matches!(
        worker.await_result(&cancel_first, &run_control),
        Err(BayesWorkerError::Cancelled { task }) if task == cancel_first
    ));

    let completion_first = worker
        .start(validated_task("completion-first"), &run_control)
        .expect("start must publish a handle");
    worker.complete(&completion_first, BayesWorkerTerminalCode::Succeeded);
    assert_eq!(
        worker.cancel(&completion_first, &cancel_control),
        Ok(BayesCancelTerminal::AlreadyTerminal {
            terminal: BayesWorkerTerminalCode::Succeeded
        })
    );
    assert_eq!(
        worker
            .await_result(&completion_first, &run_control)
            .expect("completed task must return its result")
            .task(),
        &completion_first
    );

    let failed = worker
        .start(validated_task("failed"), &run_control)
        .expect("start must publish a handle");
    worker.complete(&failed, BayesWorkerTerminalCode::Failed);
    assert!(matches!(
        worker.await_result(&failed, &run_control),
        Err(BayesWorkerError::WorkerTerminal {
            task,
            terminal: BayesWorkerTerminalCode::Failed
        }) if task == failed
    ));

    let independent_cancel = worker
        .start(validated_task("independent-cancel"), &run_control)
        .expect("start must publish a handle");
    let (run_cancel_source, run_cancel_token) = SciCancellationSource::new();
    run_cancel_source.cancel();
    let cancelled_run = ExecutionControl::new(run_cancel_token, AbsoluteDeadline::at(future));
    assert!(cancelled_run.is_cancelled());
    let attempts_before = worker.cancel_attempts();
    assert!(matches!(
        worker.cancel(
            &independent_cancel,
            &CancelDeliveryControl::new(AbsoluteDeadline::at(now)),
        ),
        Err(BayesWorkerError::CancelDeliveryDeadline { task })
            if task == independent_cancel
    ));
    assert_eq!(worker.cancel_attempts(), attempts_before + 1);
    assert_eq!(
        worker.cancel(&independent_cancel, &cancel_control),
        Ok(BayesCancelTerminal::Cancelled)
    );

    let artifact_id = ArtifactId::try_from("shared.json").expect("valid artifact ID");
    let first_generation = worker
        .start(validated_task("artifact-task"), &run_control)
        .expect("first generation must start");
    let first_artifact =
        worker.add_artifact(first_generation.clone(), artifact_id.clone(), b"first");
    assert_eq!(
        worker
            .read_artifact(&first_artifact, &run_control)
            .expect("current artifact must be readable")
            .bytes(),
        b"first"
    );

    let second_generation = worker
        .start(validated_task("artifact-task"), &run_control)
        .expect("second generation must start");
    let second_artifact =
        worker.add_artifact(second_generation.clone(), artifact_id.clone(), b"second");
    assert_eq!(
        worker
            .read_artifact(&second_artifact, &run_control)
            .expect("new generation artifact must be readable")
            .bytes(),
        b"second"
    );
    assert!(matches!(
        worker.read_artifact(&first_artifact, &run_control),
        Err(BayesWorkerError::StaleTaskHandle { task }) if task == first_generation
    ));

    let other_task = worker
        .start(validated_task("other-task"), &run_control)
        .expect("other task must start");
    let foreign_lookup = BayesArtifactHandle::mint_for_worker(other_task, artifact_id.clone());
    assert!(matches!(
        worker.read_artifact(&foreign_lookup, &run_control),
        Err(BayesWorkerError::ArtifactNotOwned { artifact })
            if artifact == foreign_lookup
    ));

    let read_gate = TestGate::new();
    let deadline_worker = Arc::new(FakeWorker::with_gates(now, None, Some(read_gate.clone())));
    let deadline_control = execution_control(future);
    let deadline_task = deadline_worker
        .start(validated_task("deadline-task"), &deadline_control)
        .expect("deadline task must start");
    let deadline_artifact =
        deadline_worker.add_artifact(deadline_task, artifact_id, b"never-returned");
    let read_worker = Arc::clone(&deadline_worker);
    let expected_artifact = deadline_artifact.clone();
    let read_thread = std::thread::spawn(move || {
        read_worker.read_artifact(&deadline_artifact, &deadline_control)
    });
    read_gate.wait_until_entered();
    deadline_worker.set_now(future);
    read_gate.release();
    assert!(matches!(
        read_thread.join().expect("test read thread must finish"),
        Err(BayesWorkerError::ArtifactReadDeadline { artifact })
            if artifact == expected_artifact
    ));
}
