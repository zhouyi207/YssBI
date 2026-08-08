use super::{ArtifactKind, CancellationToken, MaterializedArtifact, RunError, RuntimeValue};
use crate::node_system::analysis::ResourceVersionSet;
use crate::node_system::plan::{
    CallArgumentBinding, CallResultBinding, ExecutionPlan, ExecutionSemanticsVersion,
    FunctionPlanHandle, OperationStableId,
};
use crate::node_system::registry::hash_canonical;
use std::collections::BTreeMap;

use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueFingerprint([u8; 32]);

impl ValueFingerprint {
    pub fn from_runtime_value(value: &RuntimeValue) -> Option<Self> {
        let digest = match value {
            RuntimeValue::Scalar(value) => hash_canonical("yssbi.runtime-value.scalar.v1", value),
            RuntimeValue::Artifact(artifact) => {
                let MaterializedArtifact::InMemory(values) = artifact.materialized() else {
                    return None;
                };
                hash_canonical(
                    "yssbi.runtime-value.artifact.v1",
                    &(artifact_kind_name(artifact.kind()), values),
                )
            }
            RuntimeValue::Stream(_) => return None,
        }
        .expect("runtime values have a canonical JSON representation");
        Some(Self(digest))
    }
}

fn artifact_kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Buffered => "buffered",
        ArtifactKind::Collected => "collected",
        ArtifactKind::Spilled => "spilled",
        ArtifactKind::Replayable => "replayable",
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationMemoKey {
    pub operation: OperationStableId,
    pub input_fingerprints: Box<[ValueFingerprint]>,
    pub resource_versions: ResourceVersionSet,
    pub semantics_version: ExecutionSemanticsVersion,
    pub demand: DemandFingerprint,
}

impl OperationMemoKey {
    pub fn from_inputs(
        operation: OperationStableId,
        inputs: &[RuntimeValue],
        resource_versions: ResourceVersionSet,
        semantics_version: ExecutionSemanticsVersion,
        demand: DemandFingerprint,
    ) -> Option<Self> {
        let input_fingerprints = inputs
            .iter()
            .map(ValueFingerprint::from_runtime_value)
            .collect::<Option<Vec<_>>>()?
            .into_boxed_slice();
        Some(Self {
            operation,
            input_fingerprints,
            resource_versions,
            semantics_version,
            demand,
        })
    }
}

#[derive(Default)]
pub struct RunMemoization {
    owner: Mutex<RunMemoizationState>,
}

#[derive(Default)]
struct RunMemoizationState {
    finalized: bool,
    entries: BTreeMap<OperationMemoKey, Arc<Flight>>,
}

struct Flight {
    state: Mutex<FlightState>,
    ready: Arc<Condvar>,
}

enum FlightState {
    Producing,
    Complete(Box<[RuntimeValue]>),
    Uncacheable,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemoCommitCheckpoint {
    WaiterRegistered,
    BeforeCommit,
    Committed,
}

impl RunMemoization {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn completed(&self, key: &OperationMemoKey) -> Option<Box<[RuntimeValue]>> {
        let owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
        let flight = owner.entries.get(key)?;
        let state = flight
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match &*state {
            FlightState::Complete(outputs) => Some(outputs.clone()),
            _ => None,
        }
    }

    pub(crate) fn commit_completed(&self, key: OperationMemoKey, outputs: &[RuntimeValue]) -> bool {
        let cacheable = outputs.iter().all(|value| match value {
            RuntimeValue::Scalar(_) => true,
            RuntimeValue::Artifact(artifact) => artifact.is_memoization_complete(),
            RuntimeValue::Stream(_) => false,
        });
        if !cacheable {
            return false;
        }
        let mut owner = self.owner.lock().unwrap_or_else(|error| error.into_inner());
        if owner.finalized {
            return false;
        }
        let flight = Arc::new(Flight::producing());
        *flight
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner()) =
            FlightState::Complete(outputs.to_vec().into_boxed_slice());
        owner.entries.insert(key, flight);
        true
    }

    pub fn get_or_produce(
        &self,
        key: OperationMemoKey,
        cancellation: &CancellationToken,
        produce: impl FnOnce() -> Result<Box<[RuntimeValue]>, RunError>,
    ) -> Result<Box<[RuntimeValue]>, RunError> {
        self.get_or_produce_inner(key, cancellation, produce, |_| {})
    }

    #[cfg(test)]
    pub(crate) fn get_or_produce_with_commit_checkpoint(
        &self,
        key: OperationMemoKey,
        cancellation: &CancellationToken,
        produce: impl FnOnce() -> Result<Box<[RuntimeValue]>, RunError>,
        checkpoint: impl Fn(MemoCommitCheckpoint),
    ) -> Result<Box<[RuntimeValue]>, RunError> {
        self.get_or_produce_inner(key, cancellation, produce, checkpoint)
    }

    fn get_or_produce_inner(
        &self,
        key: OperationMemoKey,
        cancellation: &CancellationToken,
        produce: impl FnOnce() -> Result<Box<[RuntimeValue]>, RunError>,
        checkpoint: impl Fn(MemoCommitCheckpoint),
    ) -> Result<Box<[RuntimeValue]>, RunError> {
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
                match wait_for_flight(&flight, cancellation)? {
                    FlightWait::Complete(outputs) => return Ok(outputs),
                    FlightWait::Retry => continue,
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
                    return if guard.publish(FlightState::Failed(error.clone()), false) {
                        Err(error)
                    } else {
                        Err(RunError::Cancelled)
                    };
                }
            };
            let cacheable = outputs.iter().all(|value| match value {
                RuntimeValue::Scalar(_) => true,
                RuntimeValue::Artifact(artifact) => artifact.is_memoization_complete(),
                RuntimeValue::Stream(_) => false,
            });
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
            *state = if cacheable {
                FlightState::Complete(outputs.clone())
            } else {
                FlightState::Uncacheable
            };
            if cacheable {
                checkpoint(MemoCommitCheckpoint::Committed);
            }
            if !cacheable
                && owner
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

impl Drop for RunMemoization {
    fn drop(&mut self) {
        self.finalize();
    }
}

struct ProducerFlightGuard<'a> {
    owner: &'a RunMemoization,
    key: &'a OperationMemoKey,
    flight: &'a Arc<Flight>,
    armed: bool,
}

impl<'a> ProducerFlightGuard<'a> {
    fn new(owner: &'a RunMemoization, key: &'a OperationMemoKey, flight: &'a Arc<Flight>) -> Self {
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

enum FlightWait {
    Complete(Box<[RuntimeValue]>),
    Retry,
}

fn wait_for_flight(
    flight: &Flight,
    cancellation: &CancellationToken,
) -> Result<FlightWait, RunError> {
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
                return Ok(FlightWait::Complete(outputs.clone()));
            }
            FlightState::Uncacheable => return Ok(FlightWait::Retry),
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

    #[test]
    fn per_run_memoization_drop_wakes_owned_flight() {
        let key = OperationMemoKey {
            operation: OperationStableId::new("drop-flight").unwrap(),
            input_fingerprints: Box::new([]),
            resource_versions: ResourceVersionSet::new(),
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            demand: DemandFingerprint::from_bytes([2; 32]),
        };
        let memo = RunMemoization::new();
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
