use super::{CancellationToken, ResultId, ResultState, ResultStore, RunError, StoredValue};
use crate::execution::plan::legacy::{
    CallArgumentBinding, CallResultBinding, ExecutionPlan, ExecutionSemanticsVersion,
    FunctionPlanHandle, OperationStableId,
};
use crate::graph::analysis::contracts::ResourceVersionSet;
use std::collections::BTreeMap;
use yss_canonical_hash::hash_canonical;

use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueFingerprint([u8; 32]);

impl ValueFingerprint {
    pub fn from_stored_value(value: &StoredValue) -> Self {
        Self(value.logical_digest())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DemandFingerprint([u8; 32]);

impl DemandFingerprint {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn for_root(plan: &ExecutionPlan, normalized_selection: Option<[u8; 32]>) -> Self {
        let identity = match normalized_selection {
            Some(digest) => serde_json::json!({
                "kind": "normalized-root-demand",
                "digest": digest,
            }),
            None => serde_json::json!({
                "kind": "root-plan-fallback",
                "plan": plan_demand_identity(plan),
            }),
        };
        Self(
            hash_canonical("yssbi.runtime-demand.root.v2", &identity)
                .expect("root demand identity has a canonical JSON representation"),
        )
    }

    pub(crate) fn for_callee(
        plan: &ExecutionPlan,
        target: &FunctionPlanHandle,
        arguments: &[CallArgumentBinding],
        results: &[CallResultBinding],
    ) -> Self {
        Self(
            hash_canonical(
                "yssbi.runtime-demand.callee-frame.v2",
                &serde_json::json!({
                    "plan": plan_demand_identity(plan),
                    "target": target,
                    "arguments": arguments,
                    "results": results,
                }),
            )
            .expect("callee demand identity has a canonical JSON representation"),
        )
    }
}

fn plan_demand_identity(plan: &ExecutionPlan) -> serde_json::Value {
    serde_json::json!({
        "graphPath": &plan.provenance.graph_path,
        "operations": plan.operations.iter().map(|operation| serde_json::json!({
            "stableId": &operation.stable_id,
            "semantics": operation.semantics_version,
        })).collect::<Vec<_>>(),
        "publications": &plan.publications,
        "results": &plan.results,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComputationSettingsFingerprint([u8; 32]);

impl ComputationSettingsFingerprint {
    pub fn new(settings: super::EffectiveComputationSettings) -> Self {
        Self(
            hash_canonical("yssbi.computation-settings.v1", &settings)
                .expect("effective computation settings are canonical"),
        )
    }

    #[cfg(test)]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationMemoKey {
    pub operation: OperationStableId,
    pub input_fingerprints: Box<[ValueFingerprint]>,
    pub resource_versions: ResourceVersionSet,
    pub semantics_version: ExecutionSemanticsVersion,
    pub computation_settings: ComputationSettingsFingerprint,
    pub demand: DemandFingerprint,
}

impl OperationMemoKey {
    pub fn from_inputs(
        operation: OperationStableId,
        input_result_ids: &[ResultId],
        results: &ResultStore,
        resource_versions: ResourceVersionSet,
        semantics_version: ExecutionSemanticsVersion,
        computation_settings: super::EffectiveComputationSettings,
        demand: DemandFingerprint,
    ) -> Option<Self> {
        let input_fingerprints = input_result_ids
            .iter()
            .map(|result_id| {
                let result = results.result(*result_id)?;
                let ResultState::Ready(value) = &result.state else {
                    return None;
                };
                Some(ValueFingerprint::from_stored_value(value))
            })
            .collect::<Option<Vec<_>>>()?
            .into_boxed_slice();
        Some(Self {
            operation,
            input_fingerprints,
            resource_versions,
            semantics_version,
            computation_settings: ComputationSettingsFingerprint::new(computation_settings),
            demand,
        })
    }
}

#[derive(Default)]
pub struct SessionMemoization {
    owner: Mutex<SessionMemoizationState>,
}

#[derive(Default)]
struct SessionMemoizationState {
    finalized: bool,
    entries: BTreeMap<OperationMemoKey, Arc<Flight>>,
}

struct Flight {
    state: Mutex<FlightState>,
    ready: Arc<Condvar>,
}

enum FlightState {
    Producing,
    Complete(Box<[ResultId]>),
    RetryableAborted,
    Failed(RunError),
    Panicked,
    Finalized,
}

impl Flight {
    fn producing() -> Self {
        Self {
            state: Mutex::new(FlightState::Producing),
            ready: Arc::new(Condvar::new()),
        }
    }
}

pub(crate) enum MemoReservation {
    Complete(Box<[ResultId]>),
    Producer,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemoCommitCheckpoint {
    WaiterRegistered,
    BeforeCommit,
    Committed,
}

impl SessionMemoization {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn completed(
        &self,
        key: &OperationMemoKey,
        results: &ResultStore,
    ) -> Option<Box<[ResultId]>> {
        let owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
        let flight = owner.entries.get(key)?;
        let state = flight
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let outputs = match &*state {
            FlightState::Complete(outputs) => outputs.clone(),
            _ => return None,
        };
        if results.ready_group_activation(&outputs).is_some() {
            return Some(outputs);
        }
        drop(state);
        drop(owner);
        self.invalidate(key);
        None
    }

    pub(crate) fn reserve(
        &self,
        key: &OperationMemoKey,
        results: &ResultStore,
    ) -> Result<MemoReservation, RunError> {
        let mut owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
        if owner.finalized {
            return Err(RunError::Cancelled);
        }
        let Some(flight) = owner.entries.get(key) else {
            owner
                .entries
                .insert(key.clone(), Arc::new(Flight::producing()));
            return Ok(MemoReservation::Producer);
        };
        let state = flight
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let reservation = match &*state {
            FlightState::Complete(outputs) if results.ready_group_activation(outputs).is_some() => {
                MemoReservation::Complete(outputs.clone())
            }
            FlightState::Complete(_) | FlightState::RetryableAborted => {
                drop(state);
                owner.entries.remove(key);
                owner
                    .entries
                    .insert(key.clone(), Arc::new(Flight::producing()));
                MemoReservation::Producer
            }
            FlightState::Producing => MemoReservation::Running,
            FlightState::Failed(_) | FlightState::Panicked | FlightState::Finalized => {
                MemoReservation::Running
            }
        };
        Ok(reservation)
    }

    pub(crate) fn wait_completed(
        &self,
        key: &OperationMemoKey,
        cancellation: &CancellationToken,
    ) -> Result<Box<[ResultId]>, RunError> {
        let flight =
            {
                let owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
                if owner.finalized {
                    return Err(RunError::Cancelled);
                }
                Arc::clone(owner.entries.get(key).ok_or_else(|| {
                    RunError::InvalidPlan("memoization flight disappeared".into())
                })?)
            };
        wait_for_flight(&flight, cancellation)
    }

    pub(crate) fn commit_completed(
        &self,
        key: OperationMemoKey,
        outputs: &[ResultId],
        results: &ResultStore,
    ) -> bool {
        let owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
        if owner.finalized || results.ready_group_activation(outputs).is_none() {
            drop(owner);
            self.invalidate(&key);
            return false;
        }
        let Some(flight) = owner.entries.get(&key).cloned() else {
            return false;
        };
        let mut state = flight
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !matches!(*state, FlightState::Producing) {
            return false;
        }
        *state = FlightState::Complete(outputs.to_vec().into_boxed_slice());
        drop(state);
        drop(owner);
        flight.ready.notify_all();
        true
    }

    pub(crate) fn abort(&self, key: &OperationMemoKey, error: RunError, retryable: bool) {
        let flight = {
            let mut owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
            let Some(flight) = owner.entries.remove(key) else {
                return;
            };
            *flight
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = if retryable {
                FlightState::RetryableAborted
            } else {
                FlightState::Failed(error)
            };
            flight
        };
        flight.ready.notify_all();
    }

    pub(crate) fn invalidate(&self, key: &OperationMemoKey) {
        let flight = self
            .owner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .remove(key);
        if let Some(flight) = flight {
            *flight
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = FlightState::RetryableAborted;
            flight.ready.notify_all();
        }
    }

    pub fn get_or_produce(
        &self,
        key: OperationMemoKey,
        cancellation: &CancellationToken,
        produce: impl FnOnce() -> Result<Box<[ResultId]>, RunError>,
    ) -> Result<Box<[ResultId]>, RunError> {
        self.get_or_produce_inner(key, cancellation, produce, |_| {})
    }

    #[cfg(test)]
    pub(crate) fn get_or_produce_with_commit_checkpoint(
        &self,
        key: OperationMemoKey,
        cancellation: &CancellationToken,
        produce: impl FnOnce() -> Result<Box<[ResultId]>, RunError>,
        checkpoint: impl Fn(MemoCommitCheckpoint),
    ) -> Result<Box<[ResultId]>, RunError> {
        self.get_or_produce_inner(key, cancellation, produce, checkpoint)
    }

    fn get_or_produce_inner(
        &self,
        key: OperationMemoKey,
        cancellation: &CancellationToken,
        produce: impl FnOnce() -> Result<Box<[ResultId]>, RunError>,
        checkpoint: impl Fn(MemoCommitCheckpoint),
    ) -> Result<Box<[ResultId]>, RunError> {
        let mut produce = Some(produce);
        loop {
            cancellation.check()?;
            let (flight, producer) = {
                let mut owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
                if owner.finalized {
                    return Err(RunError::Cancelled);
                }
                if let Some(flight) = owner.entries.get(&key) {
                    (Arc::clone(flight), false)
                } else {
                    let flight = Arc::new(Flight::producing());
                    owner.entries.insert(key.clone(), Arc::clone(&flight));
                    (flight, true)
                }
            };

            if !producer {
                checkpoint(MemoCommitCheckpoint::WaiterRegistered);
                match wait_for_flight(&flight, cancellation) {
                    Err(RunError::MemoizationRetry) => continue,
                    result => return result,
                }
            }

            let mut guard = ProducerFlightGuard::new(self, &key, &flight);
            let produced = produce
                .take()
                .expect("only the selected producer consumes its closure")(
            );
            let outputs = match produced {
                Ok(outputs) => outputs,
                Err(error) => {
                    let state = if matches!(
                        error,
                        RunError::Cancelled | RunError::DeadlineExceeded { .. }
                    ) {
                        FlightState::RetryableAborted
                    } else {
                        FlightState::Failed(error.clone())
                    };
                    return if guard.publish(state, false) {
                        Err(error)
                    } else {
                        Err(RunError::Cancelled)
                    };
                }
            };

            let mut owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
            if owner.finalized {
                guard.disarm();
                return Err(RunError::Cancelled);
            }
            let mut state = flight
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if matches!(*state, FlightState::Finalized) {
                guard.disarm();
                return Err(RunError::Cancelled);
            }
            checkpoint(MemoCommitCheckpoint::BeforeCommit);
            if cancellation.is_cancelled() {
                *state = FlightState::Failed(RunError::Cancelled);
                if owner
                    .entries
                    .get(&key)
                    .is_some_and(|current| Arc::ptr_eq(current, &flight))
                {
                    owner.entries.remove(&key);
                }
                drop(state);
                drop(owner);
                flight.ready.notify_all();
                guard.disarm();
                return Err(RunError::Cancelled);
            }
            *state = FlightState::Complete(outputs.clone());
            checkpoint(MemoCommitCheckpoint::Committed);
            drop(state);
            drop(owner);
            flight.ready.notify_all();
            guard.disarm();
            return Ok(outputs);
        }
    }

    pub(crate) fn finalize(&self) {
        self.finalize_inner(|| {});
    }

    #[cfg(test)]
    pub(crate) fn finalize_with_checkpoint(&self, checkpoint: impl FnOnce()) {
        self.finalize_inner(checkpoint);
    }

    fn finalize_inner(&self, checkpoint: impl FnOnce()) {
        let flights = {
            let mut owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
            owner.finalized = true;
            checkpoint();
            let flights = std::mem::take(&mut owner.entries)
                .into_values()
                .collect::<Vec<_>>();
            for flight in &flights {
                *flight
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = FlightState::Finalized;
            }
            flights
        };
        for flight in flights {
            flight.ready.notify_all();
        }
    }

    fn publish_transition(
        &self,
        key: &OperationMemoKey,
        flight: &Arc<Flight>,
        state: FlightState,
        retain: bool,
    ) -> bool {
        let mut owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
        if owner.finalized {
            return false;
        }
        *flight
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = state;
        if !retain
            && owner
                .entries
                .get(key)
                .is_some_and(|current| Arc::ptr_eq(current, flight))
        {
            owner.entries.remove(key);
        }
        drop(owner);
        flight.ready.notify_all();
        true
    }
}

impl Drop for SessionMemoization {
    fn drop(&mut self) {
        self.finalize();
    }
}

struct ProducerFlightGuard<'a> {
    owner: &'a SessionMemoization,
    key: &'a OperationMemoKey,
    flight: &'a Arc<Flight>,
    armed: bool,
}

impl<'a> ProducerFlightGuard<'a> {
    fn new(
        owner: &'a SessionMemoization,
        key: &'a OperationMemoKey,
        flight: &'a Arc<Flight>,
    ) -> Self {
        Self {
            owner,
            key,
            flight,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn publish(&mut self, state: FlightState, retain: bool) -> bool {
        let published = self
            .owner
            .publish_transition(self.key, self.flight, state, retain);
        self.armed = false;
        published
    }
}

impl Drop for ProducerFlightGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.publish(FlightState::Panicked, false);
        }
    }
}

fn memoization_panic_error() -> RunError {
    RunError::InvalidPlan("memoization producer panicked".into())
}

fn wait_for_flight(
    flight: &Flight,
    cancellation: &CancellationToken,
) -> Result<Box<[ResultId]>, RunError> {
    cancellation.register_waiter(&flight.ready);
    let mut state = flight
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    loop {
        cancellation.check()?;
        match &*state {
            FlightState::Producing => {
                state = flight
                    .ready
                    .wait(state)
                    .unwrap_or_else(|error| error.into_inner());
            }
            FlightState::Complete(outputs) => {
                return Ok(outputs.clone());
            }
            FlightState::RetryableAborted => return Err(RunError::MemoizationRetry),
            FlightState::Failed(error) => return Err(error.clone()),
            FlightState::Panicked => return Err(memoization_panic_error()),
            FlightState::Finalized => return Err(RunError::Cancelled),
        }
    }
}

#[cfg(test)]
mod owner_drop_tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    fn test_key(name: &str) -> OperationMemoKey {
        OperationMemoKey {
            operation: OperationStableId::new(name).unwrap(),
            input_fingerprints: Box::new([]),
            resource_versions: ResourceVersionSet::new(),
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            computation_settings: ComputationSettingsFingerprint::from_bytes([3; 32]),
            demand: DemandFingerprint::from_bytes([2; 32]),
        }
    }

    #[test]
    fn retryable_producer_cancellation_or_deadline_wakes_waiter_to_produce() {
        for (index, error) in [
            RunError::Cancelled,
            RunError::DeadlineExceeded {
                phase: super::super::RunPhase::Kernel,
            },
        ]
        .into_iter()
        .enumerate()
        {
            let memo = Arc::new(SessionMemoization::new());
            let key = test_key(&format!("retryable-flight-{index}"));
            assert!(matches!(
                memo.reserve(&key, &ResultStore::new()).unwrap(),
                MemoReservation::Producer
            ));
            let waiter_registered = Arc::new(Barrier::new(2));
            let produced = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let waiter = {
                let memo = Arc::clone(&memo);
                let key = key.clone();
                let waiter_registered = Arc::clone(&waiter_registered);
                let produced = Arc::clone(&produced);
                thread::spawn(move || {
                    memo.get_or_produce_with_commit_checkpoint(
                        key,
                        &CancellationToken::new(),
                        || {
                            produced.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            Ok(vec![ResultId::new(17)].into_boxed_slice())
                        },
                        |checkpoint| {
                            if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                                waiter_registered.wait();
                            }
                        },
                    )
                })
            };
            waiter_registered.wait();

            memo.abort(&key, error, true);

            assert_eq!(
                waiter.join().unwrap().unwrap().as_ref(),
                &[ResultId::new(17)]
            );
            assert_eq!(produced.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn direct_cancelled_producer_wakes_waiter_to_retry_election() {
        let memo = Arc::new(SessionMemoization::new());
        let key = test_key("direct-cancelled-producer");
        let producer_started = Arc::new(Barrier::new(2));
        let release_producer = Arc::new(Barrier::new(2));
        let producer = {
            let memo = Arc::clone(&memo);
            let key = key.clone();
            let producer_started = Arc::clone(&producer_started);
            let release_producer = Arc::clone(&release_producer);
            thread::spawn(move || {
                memo.get_or_produce(key, &CancellationToken::new(), || {
                    producer_started.wait();
                    release_producer.wait();
                    Err(RunError::Cancelled)
                })
            })
        };
        producer_started.wait();
        let waiter_registered = Arc::new(Barrier::new(2));
        let waiter = {
            let memo = Arc::clone(&memo);
            let key = key.clone();
            let waiter_registered = Arc::clone(&waiter_registered);
            thread::spawn(move || {
                memo.get_or_produce_with_commit_checkpoint(
                    key,
                    &CancellationToken::new(),
                    || Ok(vec![ResultId::new(18)].into_boxed_slice()),
                    |checkpoint| {
                        if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                            waiter_registered.wait();
                        }
                    },
                )
            })
        };
        waiter_registered.wait();
        release_producer.wait();

        assert_eq!(producer.join().unwrap(), Err(RunError::Cancelled));
        assert_eq!(
            waiter.join().unwrap().unwrap().as_ref(),
            &[ResultId::new(18)]
        );
    }

    #[test]
    fn session_memoization_drop_wakes_owned_flight() {
        let key = test_key("drop-flight");
        let memo = SessionMemoization::new();
        let flight = Arc::new(Flight::producing());
        memo.owner
            .lock()
            .unwrap()
            .entries
            .insert(key, Arc::clone(&flight));
        let started = Arc::new(Barrier::new(2));
        let waiter = {
            let flight = Arc::clone(&flight);
            let started = Arc::clone(&started);
            thread::spawn(move || {
                started.wait();
                wait_for_flight(&flight, &CancellationToken::new())
            })
        };
        started.wait();

        drop(memo);

        assert!(matches!(waiter.join().unwrap(), Err(RunError::Cancelled)));
    }
}
