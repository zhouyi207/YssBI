use super::scheduler::{
    PreparedPlanExecution, PreparedPlanExecutor, SchedulerOutput, SchedulerResult,
};
use super::*;
use crate::error::RunPhase;
use crate::identity::ExecutionSessionId;
use crate::package_preparation::PreparedExecutionPlan;
use crate::plan::{
    ExecutionPlan, ExecutionPlanPackage, PlanBasis, PlanExecutionDemand, PlanGraphId, PlanId,
    PlanInputBinding, PlanInputSource, PlanOperation, PlanOutputBinding, PlanOutputRef,
    PlanParameterBundleBuilder, PlanParameterHandle, PlanParameterPayload, PlanParameterSchemaId,
    PlanParameterValue, PlanPortAddress, PlanProjectSessionId, PlanProvenance,
    PlanRegistryFingerprint, PlanResourceId, PlanResourceObservedState, PlanResourceRequirement,
    PlanResourceVersion, PlanSourceIdentity, ResourceAccess, ResourceKind, ValueRef,
};
use crate::resource_preparation::ResourceProviderFactory;
use crate::resource_preparation::{RunResourceBinding, RunResourceBindings};
use crate::result_store::{ResultId, StoredResult};
use crate::run_registry::{RunId, RunState};
use std::collections::BTreeMap;
use std::time::Duration;
use std::{sync::atomic::AtomicBool, time::Instant};
use yss_data_contract::TabularScalar;
use yss_node_kernel::{KernelId, RuntimeValue};

fn prepared_plan(state: &ExecutionRuntimeState) -> PreparedExecutionPlan {
    let resource = PlanResourceId::from_existing("databases/answer".into());
    let version = PlanResourceVersion::from_existing("v1".into());
    let basis = PlanBasis::new(
        PlanProjectSessionId::from_existing("session".into()),
        PlanRegistryFingerprint::from_bytes([4; 32]),
        yss_node_kernel::KernelRegistry::default().fingerprint(),
        BTreeMap::from([(resource.clone(), version.clone())]),
        BTreeMap::from([(resource, PlanResourceObservedState::Present(version))]),
    );
    let parameters = Arc::new(PlanParameterBundleBuilder::new(basis.clone()).freeze());
    let package = ExecutionPlanPackage::new(
        Arc::new(ExecutionPlan::empty()),
        parameters,
        PlanProvenance::new(
            PlanSourceIdentity::new(PlanGraphId::from_existing("events/main".into()), None, None),
            basis,
            PlanId::from_existing(11),
        ),
    );
    state
        .prepare_package(package, RuntimeGeneration::INITIAL)
        .expect("test package is valid")
}

fn bindings() -> RunResourceBindings {
    let requirement = PlanResourceRequirement::new(
        PlanResourceId::from_existing("databases/answer".into()),
        ResourceKind::DataFrame,
        ResourceAccess::Shared,
        false,
    );
    RunResourceBindings::new(
        PlanProjectSessionId::from_existing("session".into()),
        [requirement.clone()],
        [RunResourceBinding::new(
            requirement,
            PlanResourceVersion::from_existing("v1".into()),
            yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(4)),
        )],
    )
}

fn empty_bindings() -> RunResourceBindings {
    RunResourceBindings::new(
        PlanProjectSessionId::from_existing("session".into()),
        Vec::<PlanResourceRequirement>::new(),
        Vec::<RunResourceBinding>::new(),
    )
}

fn prepared_operation_plan(
    state: &ExecutionRuntimeState,
    operations: impl IntoIterator<Item = PlanOperation>,
    parameter_entries: impl IntoIterator<Item = (PlanParameterHandle, PlanParameterPayload)>,
) -> PreparedExecutionPlan {
    let basis = PlanBasis::new(
        PlanProjectSessionId::from_existing("session".into()),
        PlanRegistryFingerprint::from_bytes([4; 32]),
        yss_node_kernel::KernelRegistry::default().fingerprint(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let mut parameters = PlanParameterBundleBuilder::new(basis.clone());
    for (handle, payload) in parameter_entries {
        parameters
            .insert(handle, payload)
            .expect("test parameter handles are unique");
    }
    let package = ExecutionPlanPackage::new(
        Arc::new(ExecutionPlan::new(
            operations
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )),
        Arc::new(parameters.freeze()),
        PlanProvenance::new(
            PlanSourceIdentity::new(PlanGraphId::from_existing("events/main".into()), None, None),
            basis,
            PlanId::from_existing(12),
        ),
    );
    state
        .prepare_package(package, RuntimeGeneration::INITIAL)
        .expect("test package is valid")
}

fn operation_source(node: &str) -> PlanSourceIdentity {
    PlanSourceIdentity::new(
        PlanGraphId::from_existing("events/main".into()),
        Some(crate::plan::PlanNodeId::from_existing(node.into())),
        None,
    )
}

fn operation_output(node: &str, value: ValueRef) -> PlanOutputBinding {
    PlanOutputBinding::new(
        PlanOutputRef::new(
            PlanGraphId::from_existing("events/main".into()),
            PlanPortAddress::from_existing(format!("{node}:result").into_boxed_str()),
        ),
        value,
        crate::plan::PlanOutputContract {
            data_type: yss_data_contract::ValueType::Scalar(
                yss_data_contract::SemanticType::Numeric,
            ),
            schema: None,
            category: crate::plan::ResultCategory::Value,
            source: PlanSourceIdentity::new(
                PlanGraphId::from_existing("events/main".into()),
                Some(crate::plan::PlanNodeId::from_existing(node.into())),
                Some(PlanPortAddress::from_existing(
                    format!("{node}:result").into_boxed_str(),
                )),
            ),
        },
    )
}

fn operation_specialization(kind: &str, node: &str) -> crate::plan::PlanKernelSpecialization {
    crate::plan::PlanKernelSpecialization::new(
        KernelId::from_existing(kind.into()),
        Box::new([]),
        Box::new([crate::plan::PlanTypeBinding::new(
            PlanPortAddress::from_existing(format!("{node}:result").into_boxed_str()),
            yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        )]),
        Box::new([]),
    )
}

struct TestExecutor;

impl PreparedPlanExecutor for TestExecutor {
    fn execute(
        &self,
        execution: PreparedPlanExecution<'_>,
    ) -> Result<SchedulerOutput, OperationExecutionError> {
        let resources = execution.resources;
        assert_eq!(
            resources.value(&PlanResourceId::from_existing("databases/answer".into())),
            Some(&yss_node_kernel::RuntimeValue::Scalar(
                TabularScalar::Integer(4)
            ))
        );
        Ok(SchedulerOutput::new(
            vec![SchedulerResult {
                value: StoredResult::new(yss_node_kernel::RuntimeValue::Scalar(
                    TabularScalar::Integer(5),
                )),
                category: crate::plan::ResultCategory::Value,
                output: operation_output("test-executor", ValueRef::new(0))
                    .output()
                    .clone(),
            }]
            .into_boxed_slice(),
            Box::new([]),
        ))
    }
}

fn state() -> ExecutionRuntimeState {
    ExecutionRuntimeState::new(
        ExecutionSessionId::new(uuid::Uuid::nil()),
        crate::identity::RuntimeGeneration::INITIAL,
        yss_node_kernel::KernelRegistry::default().into(),
        crate::test_relations(),
    )
}

#[test]
fn closed_session_drains_an_active_lease_and_rejects_new_work() {
    let state = state();
    let lease = state.admit().expect("test admission must open");
    assert_eq!(
        state.cancel_and_drain(&ExecutionDrainControl::new(Instant::now())),
        ExecutionDrainOutcome::TimedOut {
            outstanding: ExecutionOutstandingWork { active: 1 },
        }
    );
    assert!(matches!(
        state.admit(),
        Err(ExecutionAdmissionError::Closed)
    ));

    drop(lease);
    assert_eq!(
        state.drain(&ExecutionDrainControl::new(
            Instant::now() + Duration::from_secs(1),
        )),
        ExecutionDrainOutcome::Drained {
            outstanding: ExecutionOutstandingWork { active: 0 },
        }
    );
}

fn numeric_chain_plan(state: &ExecutionRuntimeState) -> PreparedExecutionPlan {
    let parameter_handle = PlanParameterHandle::from_existing("constant/value".into());
    let consumer = PlanOperation::new(
        operation_source("consumer"),
        crate::plan::PlanNodeTypeId::from_existing("yssbi.numeric.square".into()),
        BTreeMap::new(),
        Box::new([PlanInputBinding::new(
            PlanPortAddress::from_existing("consumer:value".into()),
            PlanInputSource::Value(ValueRef::new(1)),
            crate::plan::PlanInputContract {
                key: "input".into(),
                group: None,
                expected_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Numeric,
                ),
                coercions: Box::new([]),
            },
        )]),
        Box::new([]),
        Box::new([operation_output("consumer", ValueRef::new(0))]),
        crate::plan::PlanKernelSpecialization::new(
            KernelId::from_existing("yssbi.numeric.square".into()),
            Box::new([crate::plan::PlanTypeBinding::new(
                PlanPortAddress::from_existing("consumer:value".into()),
                yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            )]),
            operation_specialization("yssbi.numeric.square", "consumer")
                .output_types()
                .into(),
            Box::new([]),
        ),
    );
    let producer = PlanOperation::new(
        operation_source("producer"),
        crate::plan::PlanNodeTypeId::from_existing("yssbi.constant.get".into()),
        BTreeMap::from([(
            yss_node_kernel::KernelParameterKey::from_existing("value".into()),
            parameter_handle.clone(),
        )]),
        Box::new([]),
        Box::new([]),
        Box::new([operation_output("producer", ValueRef::new(1))]),
        operation_specialization("yssbi.constant.get", "producer"),
    );
    prepared_operation_plan(
        state,
        [consumer, producer],
        [(
            parameter_handle,
            PlanParameterPayload::new(
                PlanParameterSchemaId::from_existing("graph.constant".into()),
                PlanParameterValue::Literal(std::sync::Arc::new(
                    yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(7)),
                )),
            ),
        )],
    )
}

#[test]
fn neutral_executor_waits_for_an_upstream_value_later_in_the_plan() {
    let state = state();
    let plan = numeric_chain_plan(&state);

    let candidate = state
        .execute_prepared(
            &plan,
            empty_bindings(),
            &ResourceProviderFactory::new("session".into()),
            &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
        )
        .expect("the executor must wait for the producer instead of reading by plan order");

    assert_eq!(candidate.results().len(), 2);
    assert!(candidate.results().iter().all(|result| {
        result.value().value()
            == &if result.output().port().as_str().starts_with("consumer:") {
                RuntimeValue::float64(49.0).unwrap()
            } else {
                RuntimeValue::Scalar(TabularScalar::Integer(7))
            }
    }));
    let outputs = candidate
        .results()
        .iter()
        .map(|result| result.output().clone())
        .collect::<Vec<_>>();
    let handoff = candidate.into_finalization_handoff();
    state.publish_committed_results(&handoff);
    assert!(
        outputs
            .iter()
            .all(|output| { state.query_pin_result(output).is_some() })
    );
    drop(handoff);
    for _ in 0..100 {
        let previous = outputs
            .iter()
            .map(|output| state.query_pin_result(output).unwrap())
            .collect::<Vec<_>>();
        let weak = previous
            .iter()
            .map(|result| Arc::downgrade(result.value()))
            .collect::<Vec<_>>();
        let ids = previous
            .iter()
            .map(|result| result.provenance().result_id())
            .collect::<Vec<_>>();
        drop(previous);
        let candidate = state
            .execute_prepared(
                &plan,
                empty_bindings(),
                &ResourceProviderFactory::new("session".into()),
                &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
            )
            .unwrap();
        assert!(weak.iter().all(|result| result.upgrade().is_none()));
        assert!(ids.iter().all(|id| state.query_result(*id).is_none()));
        assert!(state.publish_committed_results(&candidate.into_finalization_handoff()));
    }
    let failed = state.execute_prepared(
        &plan,
        empty_bindings(),
        &ResourceProviderFactory::new("another-session".into()),
        &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
    );
    assert!(failed.is_err());
    assert!(
        outputs
            .iter()
            .all(|output| state.query_pin_result(output).is_none())
    );
}

#[test]
fn output_extension_reuses_valid_inputs_and_rejects_replaced_input_results() {
    use crate::result::OutputResultInputs;
    let state = state();
    let plan = numeric_chain_plan(&state);
    let source = operation_output("producer", ValueRef::new(1))
        .output()
        .clone();
    let target = operation_output("consumer", ValueRef::new(0))
        .output()
        .clone();
    let source_inputs = OutputResultInputs {
        fingerprint: [1; 32],
        bindings: BTreeMap::new(),
        resources: BTreeMap::new(),
        available: true,
    };
    let inputs = GraphResultInputs {
        semantic_input_hash: [1; 32],
        observers: BTreeMap::new(),
        outputs: BTreeMap::from([
            (source.clone(), source_inputs.clone()),
            (
                target.clone(),
                OutputResultInputs {
                    bindings: BTreeMap::from([(
                        PlanPortAddress::from_existing("consumer:value".into()),
                        vec![source.clone()].into_boxed_slice(),
                    )]),
                    ..source_inputs
                },
            ),
        ]),
    };
    let basis = state
        .capture_result_run_basis("events/main", inputs.clone())
        .unwrap();
    let run = |demand: &PlanExecutionDemand, basis: &ResultRunBasis| {
        state
            .execute_prepared_handoff(
                &plan,
                empty_bindings(),
                &ResourceProviderFactory::new("session".into()),
                &RunExecutionControl::new(Instant::now() + Duration::from_secs(10)),
                ExecutionResultRequest {
                    demand,
                    basis: Some(basis),
                },
                |_| {},
            )
            .unwrap()
    };
    let initial = run(&PlanExecutionDemand::Default, &basis);
    assert!(state.publish_committed_results(initial.handoff()));
    state.finalize_run_success(initial.run_id()).unwrap();
    let source_id = state
        .query_pin_result(&source)
        .unwrap()
        .provenance()
        .result_id();
    let demand = PlanExecutionDemand::Outputs {
        outputs: vec![target.clone()].into_boxed_slice(),
        include_default_results: false,
        reuse_inputs: true,
    };
    let extension = run(&demand, &basis);
    assert_eq!(extension.handoff().results().len(), 1);
    assert_eq!(extension.handoff().results()[0].output(), &target);
    assert!(state.publish_committed_results(extension.handoff()));
    state.finalize_run_success(extension.run_id()).unwrap();
    assert_eq!(
        state
            .query_pin_result(&source)
            .unwrap()
            .provenance()
            .result_id(),
        source_id
    );
    assert!(state.query_pin_result(&target).is_some());

    let delayed = run(&demand, &basis);
    let replacement = run(
        &PlanExecutionDemand::Outputs {
            outputs: vec![source.clone()].into_boxed_slice(),
            include_default_results: false,
            reuse_inputs: false,
        },
        &basis,
    );
    assert!(state.publish_committed_results(replacement.handoff()));
    state.finalize_run_success(replacement.run_id()).unwrap();
    assert!(!state.publish_committed_results(delayed.handoff()));
    state.finalize_run_cancelled(delayed.run_id()).unwrap();

    let mut changed = inputs;
    changed.semantic_input_hash = [2; 32];
    changed.outputs.get_mut(&source).unwrap().fingerprint = [2; 32];
    state.observe_graph_result_inputs("events/main", changed.clone());
    let basis = state
        .capture_result_run_basis("events/main", changed)
        .unwrap();
    let recomputed = run(&demand, &basis);
    assert_eq!(recomputed.handoff().results().len(), 2);
    assert!(state.publish_committed_results(recomputed.handoff()));
    state.finalize_run_success(recomputed.run_id()).unwrap();
}

#[test]
fn explicit_output_demand_skips_unrelated_graph_components() {
    let state = state();
    let parameter_handle = PlanParameterHandle::from_existing("constant/value".into());
    let selected_output = operation_output("selected", ValueRef::new(0));
    let requested = selected_output.output().clone();
    let selected = PlanOperation::new(
        operation_source("selected"),
        crate::plan::PlanNodeTypeId::from_existing("yssbi.constant.get".into()),
        BTreeMap::from([(
            yss_node_kernel::KernelParameterKey::from_existing("value".into()),
            parameter_handle.clone(),
        )]),
        Box::new([]),
        Box::new([]),
        Box::new([selected_output]),
        operation_specialization("yssbi.constant.get", "selected"),
    );
    let unrelated = PlanOperation::new(
        operation_source("unrelated"),
        crate::plan::PlanNodeTypeId::from_existing("yssbi.constant.get".into()),
        BTreeMap::new(),
        Box::new([]),
        Box::new([]),
        Box::new([operation_output("unrelated", ValueRef::new(1))]),
        operation_specialization("yssbi.unsupported", "unrelated"),
    );
    let plan = prepared_operation_plan(
        &state,
        [selected, unrelated],
        [(
            parameter_handle,
            PlanParameterPayload::new(
                PlanParameterSchemaId::from_existing("graph.constant".into()),
                PlanParameterValue::Literal(std::sync::Arc::new(
                    yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(7)),
                )),
            ),
        )],
    );

    let executed = state
        .execute_prepared_handoff(
            &plan,
            empty_bindings(),
            &ResourceProviderFactory::new("session".into()),
            &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
            ExecutionResultRequest {
                demand: &PlanExecutionDemand::Outputs {
                    outputs: vec![requested].into_boxed_slice(),
                    include_default_results: false,
                    reuse_inputs: false,
                },
                basis: None,
            },
            |_| {},
        )
        .expect("an unrelated unsupported component must not be scheduled");

    assert_eq!(
        executed.handoff().results()[0].value().value(),
        &yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(7))
    );
}

#[test]
fn execute_prepared_success_uses_the_candidate_to_create_the_only_handoff() {
    let state = state();
    let plan = prepared_plan(&state);
    let candidate = state
        .execute_prepared_with_executor(
            &plan,
            bindings(),
            &ResourceProviderFactory::new("session".into()),
            &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
            &TestExecutor,
        )
        .expect("test executor produces a neutral scheduler output");
    let handoff = candidate.into_finalization_handoff();

    assert_eq!(handoff.results().len(), 1);
    assert_eq!(handoff.results()[0].result_id(), ResultId::from_existing(1));
    assert_eq!(
        handoff.results()[0].value().value(),
        &yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(5))
    );
    assert_eq!(
        handoff.results()[0].category(),
        crate::plan::ResultCategory::Value
    );
    assert_eq!(
        state.runs().state(RunId::from_existing(1)),
        Some(RunState::Succeeded)
    );
}

#[test]
fn aborted_run_notification_releases_control_and_admission() {
    let state = state();
    let plan = prepared_plan(&state);
    let cancellation = Arc::new(AtomicBool::new(false));
    let retained_control = Arc::downgrade(&cancellation);
    let control = RunExecutionControl::with_cancellation(
        cancellation,
        Instant::now() + Duration::from_secs(1),
    );
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state.execute_prepared_handoff(
            &plan,
            bindings(),
            &ResourceProviderFactory::new("session".into()),
            &control,
            ExecutionResultRequest {
                demand: &PlanExecutionDemand::Default,
                basis: None,
            },
            |_| panic!("notification aborted"),
        )
    }));
    assert!(outcome.is_err());
    assert_eq!(
        state.runs().state(RunId::from_existing(1)),
        Some(RunState::Failed)
    );
    assert!(matches!(
        state.drain(&ExecutionDrainControl::new(Instant::now())),
        ExecutionDrainOutcome::Drained { .. }
    ));
    drop(control);
    assert!(retained_control.upgrade().is_none());
}

#[test]
fn session_cancel_and_drain_signals_an_active_run() {
    let state = state();
    let plan = prepared_plan(&state);
    let control = RunExecutionControl::new(Instant::now() + Duration::from_secs(1));
    let result = state.execute_prepared_handoff(
        &plan,
        bindings(),
        &ResourceProviderFactory::new("session".into()),
        &control,
        ExecutionResultRequest {
            demand: &PlanExecutionDemand::Default,
            basis: None,
        },
        |_| {
            assert!(matches!(
                state.cancel_and_drain(&ExecutionDrainControl::new(Instant::now())),
                ExecutionDrainOutcome::TimedOut { .. }
            ));
        },
    );
    assert!(matches!(
        result,
        Err(ExecutePreparedError::Cancelled { .. })
    ));
    assert_eq!(
        state.runs().state(RunId::from_existing(1)),
        Some(RunState::Cancelled)
    );
    assert!(matches!(
        state.admit(),
        Err(ExecutionAdmissionError::Closed)
    ));
    assert!(matches!(
        state.drain(&ExecutionDrainControl::new(Instant::now())),
        ExecutionDrainOutcome::Drained { .. }
    ));
}

#[test]
fn finalizing_run_remains_cancellable_until_terminal_transition() {
    let state = state();
    let plan = prepared_plan(&state);
    let control = RunExecutionControl::new(Instant::now() + Duration::from_secs(1));
    let retained_control = Arc::downgrade(&control.cancellation);
    let executed = state
        .execute_prepared_handoff(
            &plan,
            bindings(),
            &ResourceProviderFactory::new("session".into()),
            &control,
            ExecutionResultRequest {
                demand: &PlanExecutionDemand::Default,
                basis: None,
            },
            |_| {},
        )
        .unwrap();
    assert_eq!(
        state.runs().state(executed.run_id()),
        Some(RunState::Finalizing)
    );
    assert_eq!(
        state.cancel_run(executed.run_id()),
        ExecutionCancelOutcome::Requested
    );
    assert!(control.cancellation.load(Ordering::Acquire));
    state.finalize_run_cancelled(executed.run_id()).unwrap();
    drop(control);
    assert!(retained_control.upgrade().is_none());
    assert_eq!(
        state.cancel_run(executed.run_id()),
        ExecutionCancelOutcome::AlreadyCancelled
    );
}

#[test]
fn execute_prepared_cancellation_happens_before_run_registration() {
    let state = state();
    let plan = prepared_plan(&state);
    let cancellation = Arc::new(AtomicBool::new(true));
    let result = state.execute_prepared(
        &plan,
        bindings(),
        &ResourceProviderFactory::new("session".into()),
        &RunExecutionControl::with_cancellation(
            cancellation,
            Instant::now() + Duration::from_secs(1),
        ),
    );

    assert!(matches!(
        result,
        Err(ExecutePreparedError::Cancelled {
            phase: RunPhase::Admission
        })
    ));
    assert_eq!(state.runs().state(RunId::from_existing(1)), None);
}
