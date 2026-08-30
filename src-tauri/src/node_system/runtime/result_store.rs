use super::{
    ActivationId, ActivationProvenance, ActivationResultGroup, CancellationToken,
    PendingOutputDescriptor, PinResultEntry, ResultFailure, ResultId, ResultProgress,
    ResultProvenance, ResultState, RunDeadline, RunError, RunPhase, StoredResult, StoredValue,
};
#[cfg(test)]
use crate::node_system::runtime::RunId;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct ResultStoreRegistry {
    results: BTreeMap<ResultId, Arc<StoredResult>>,
    groups: BTreeMap<ActivationId, ActivationResultGroup>,
    pin_history: BTreeMap<crate::execution::plan::legacy::GraphOutputRef, Arc<PinHistoryNode>>,
}

struct PinHistoryNode {
    entry: PinResultEntry,
    previous: Option<Arc<PinHistoryNode>>,
    len: usize,
}

static NEXT_RESULT_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_result_range(count: usize) -> Result<u64, ResultStoreError> {
    let count = u64::try_from(count).map_err(|_| ResultStoreError::IdExhausted)?;
    NEXT_RESULT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(count)
        })
        .map_err(|_| ResultStoreError::IdExhausted)
}

struct ResultStoreInner {
    registry: Mutex<ResultStoreRegistry>,
    changed: Arc<Condvar>,
}

/// Project-session authority for logical execution results and Pin history.
#[derive(Clone)]
pub struct ResultStore {
    inner: Arc<ResultStoreInner>,
}

impl fmt::Debug for ResultStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResultStore")
    }
}

impl PartialEq for ResultStore {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for ResultStore {}

impl Default for ResultStore {
    fn default() -> Self {
        Self {
            inner: Arc::new(ResultStoreInner {
                registry: Mutex::new(ResultStoreRegistry::default()),
                changed: Arc::new(Condvar::new()),
            }),
        }
    }
}

impl ResultStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_pending_group(
        &self,
        provenance: ActivationProvenance,
        outputs: &[PendingOutputDescriptor],
    ) -> Result<ActivationResultGroup, ResultStoreError> {
        if outputs.is_empty() {
            return Err(ResultStoreError::EmptyGroup);
        }
        if !matches!(provenance.usage, super::ResultUsage::Produced) {
            return Err(ResultStoreError::InvalidProducedUsage);
        }
        let mut public_outputs = BTreeSet::new();
        let mut values = BTreeSet::new();
        for output in outputs {
            if !values.insert(output.value) {
                return Err(ResultStoreError::DuplicateOutputValue(output.value));
            }
            if let Some(public_output) = &output.output {
                if public_output.graph_path != provenance.graph_path {
                    return Err(ResultStoreError::OutputGraphMismatch(public_output.clone()));
                }
                if public_output.port.node_id != provenance.node_id {
                    return Err(ResultStoreError::OutputNodeMismatch(public_output.clone()));
                }
                if !public_outputs.insert(public_output.clone()) {
                    return Err(ResultStoreError::DuplicatePublicOutput(
                        public_output.clone(),
                    ));
                }
            }
        }
        let first_id = allocate_result_range(outputs.len())?;
        let output_result_ids = (0..outputs.len())
            .map(|offset| ResultId::new(first_id + offset as u64))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let group = ActivationResultGroup {
            activation_id: provenance.activation_id,
            output_result_ids,
        };

        let mut registry = self.registry();
        if registry.groups.contains_key(&provenance.activation_id) {
            return Err(ResultStoreError::DuplicateActivation(
                provenance.activation_id,
            ));
        }
        for (descriptor, result_id) in outputs.iter().zip(&group.output_result_ids) {
            let result = StoredResult {
                id: *result_id,
                provenance: ResultProvenance {
                    run_id: provenance.run_id,
                    activation_id: provenance.activation_id,
                    graph_path: provenance.graph_path.clone(),
                    graph_revision: provenance.graph_revision,
                    node_id: provenance.node_id,
                    output: descriptor.output.clone(),
                    created_at_ms: provenance.created_at_ms,
                },
                value: descriptor.value,
                presentation: descriptor.presentation,
                contract: descriptor.contract.clone(),
                state: ResultState::Pending(ResultProgress::default()),
            };
            registry.results.insert(*result_id, Arc::new(result));
            if let Some(public_output) = &descriptor.output {
                append_pin_history(
                    &mut registry.pin_history,
                    public_output,
                    PinResultEntry {
                        result_id: *result_id,
                        run_id: provenance.run_id,
                        activation_id: provenance.activation_id,
                        graph_revision: provenance.graph_revision,
                        created_at_ms: provenance.created_at_ms,
                        usage: provenance.usage,
                    },
                );
            }
        }
        registry
            .groups
            .insert(provenance.activation_id, group.clone());
        drop(registry);
        self.inner.changed.notify_all();
        Ok(group)
    }

    pub fn complete_group(
        &self,
        group: &ActivationResultGroup,
        values: Box<[StoredValue]>,
    ) -> Result<(), ResultStoreError> {
        if values.len() != group.output_result_ids.len() {
            return Err(ResultStoreError::OutputCount {
                expected: group.output_result_ids.len(),
                actual: values.len(),
            });
        }
        self.transition_group(group, |index, _| {
            Ok(ResultState::Ready(values[index].clone()))
        })
    }

    pub fn fail_group(
        &self,
        group: &ActivationResultGroup,
        failure: Arc<ResultFailure>,
    ) -> Result<(), ResultStoreError> {
        self.transition_group(group, |_, _| Ok(ResultState::Failed(Arc::clone(&failure))))
    }

    pub fn cancel_group(&self, group: &ActivationResultGroup) -> Result<(), ResultStoreError> {
        self.transition_group(group, |_, _| Ok(ResultState::Cancelled))
    }

    pub fn result(&self, id: ResultId) -> Option<Arc<StoredResult>> {
        self.registry().results.get(&id).cloned()
    }

    pub(crate) fn ready_group_activation(&self, result_ids: &[ResultId]) -> Option<ActivationId> {
        if result_ids.is_empty() {
            return None;
        }
        let registry = self.registry();
        let first = registry.results.get(&result_ids[0])?;
        let activation_id = first.provenance.activation_id;
        let group = registry.groups.get(&activation_id)?;
        if group.output_result_ids.as_ref() != result_ids {
            return None;
        }
        result_ids
            .iter()
            .all(|result_id| {
                registry.results.get(result_id).is_some_and(|result| {
                    result.provenance.activation_id == activation_id
                        && matches!(result.state, ResultState::Ready(_))
                })
            })
            .then_some(activation_id)
    }

    pub fn wait_terminal(
        &self,
        id: ResultId,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<Arc<StoredResult>, ResultStoreError> {
        cancellation.register_waiter(&self.inner.changed);
        let mut registry = self.registry();
        loop {
            let result = registry
                .results
                .get(&id)
                .ok_or(ResultStoreError::UnknownResult(id))?;
            if result.state.is_terminal() {
                return Ok(Arc::clone(result));
            }
            if cancellation.is_cancelled() {
                return Err(ResultStoreError::WaitCancelled);
            }
            let poll = std::time::Duration::from_millis(25);
            let wait = if let Some(deadline) = deadline {
                deadline
                    .remaining(cancellation, RunPhase::ResultPublication)
                    .map_err(ResultStoreError::from_wait_error)?
                    .min(poll)
            } else {
                poll
            };
            let (next_registry, timeout) = self
                .inner
                .changed
                .wait_timeout(registry, wait)
                .unwrap_or_else(|error| error.into_inner());
            registry = next_registry;
            if timeout.timed_out() {
                if cancellation.is_cancelled() {
                    return Err(ResultStoreError::WaitCancelled);
                }
                if deadline.is_some_and(|deadline| {
                    deadline
                        .remaining(cancellation, RunPhase::ResultPublication)
                        .is_err()
                }) {
                    return Err(ResultStoreError::WaitDeadlineExceeded);
                }
            }
        }
    }

    pub fn pin_history(
        &self,
        output: &crate::execution::plan::legacy::GraphOutputRef,
    ) -> Box<[PinResultEntry]> {
        let tail = self.registry().pin_history.get(output).cloned();
        collect_pin_history(tail)
    }

    pub fn record_reused_group(
        &self,
        provenance: ActivationProvenance,
        outputs: &[PendingOutputDescriptor],
        result_ids: &[ResultId],
    ) -> Result<ActivationResultGroup, ResultStoreError> {
        if outputs.is_empty() {
            return Err(ResultStoreError::EmptyGroup);
        }
        if outputs.len() != result_ids.len() {
            return Err(ResultStoreError::OutputCount {
                expected: outputs.len(),
                actual: result_ids.len(),
            });
        }
        let super::ResultUsage::Reused {
            original_activation_id,
        } = provenance.usage
        else {
            return Err(ResultStoreError::InvalidReusedUsage);
        };
        let group = ActivationResultGroup {
            activation_id: provenance.activation_id,
            output_result_ids: result_ids.to_vec().into_boxed_slice(),
        };
        let mut registry = self.registry();
        if registry.groups.contains_key(&provenance.activation_id) {
            return Err(ResultStoreError::DuplicateActivation(
                provenance.activation_id,
            ));
        }
        let original_group = registry
            .groups
            .get(&original_activation_id)
            .ok_or(ResultStoreError::UnknownActivation(original_activation_id))?;
        if original_group.output_result_ids.as_ref() != result_ids {
            let index = original_group
                .output_result_ids
                .iter()
                .zip(result_ids)
                .position(|(expected, actual)| expected != actual)
                .unwrap_or(0);
            return Err(ResultStoreError::ReusedOutputMismatch {
                index,
                result_id: result_ids[index],
            });
        }
        for (index, (descriptor, result_id)) in outputs.iter().zip(result_ids).enumerate() {
            let result = registry
                .results
                .get(result_id)
                .ok_or(ResultStoreError::UnknownResult(*result_id))?;
            if !matches!(result.state, ResultState::Ready(_)) {
                return Err(ResultStoreError::ReusedResultNotReady(*result_id));
            }
            if result.provenance.activation_id != original_activation_id
                || result.provenance.graph_path != provenance.graph_path
                || result.provenance.node_id != provenance.node_id
                || result.provenance.output != descriptor.output
                || result.value != descriptor.value
                || result.presentation != descriptor.presentation
                || result.contract != descriptor.contract
            {
                return Err(ResultStoreError::ReusedOutputMismatch {
                    index,
                    result_id: *result_id,
                });
            }
        }
        for (descriptor, result_id) in outputs.iter().zip(result_ids) {
            if let Some(public_output) = &descriptor.output {
                append_pin_history(
                    &mut registry.pin_history,
                    public_output,
                    PinResultEntry {
                        result_id: *result_id,
                        run_id: provenance.run_id,
                        activation_id: provenance.activation_id,
                        graph_revision: provenance.graph_revision,
                        created_at_ms: provenance.created_at_ms,
                        usage: provenance.usage,
                    },
                );
            }
        }
        registry
            .groups
            .insert(provenance.activation_id, group.clone());
        drop(registry);
        self.inner.changed.notify_all();
        Ok(group)
    }

    fn transition_group(
        &self,
        group: &ActivationResultGroup,
        mut transition: impl FnMut(usize, &StoredResult) -> Result<ResultState, ResultStoreError>,
    ) -> Result<(), ResultStoreError> {
        let mut registry = self.registry();
        let registered = registry
            .groups
            .get(&group.activation_id)
            .ok_or(ResultStoreError::UnknownActivation(group.activation_id))?;
        if registered != group {
            return Err(ResultStoreError::GroupMismatch(group.activation_id));
        }

        let mut next_states = Vec::with_capacity(group.output_result_ids.len());
        for (index, result_id) in group.output_result_ids.iter().enumerate() {
            let result = registry
                .results
                .get(result_id)
                .ok_or(ResultStoreError::UnknownResult(*result_id))?;
            if result.provenance.activation_id != group.activation_id {
                return Err(ResultStoreError::GroupMismatch(group.activation_id));
            }
            if !result.state.is_pending() {
                return Err(ResultStoreError::TerminalResult(*result_id));
            }
            next_states.push(transition(index, result)?);
        }
        for (result_id, state) in group.output_result_ids.iter().zip(next_states) {
            Arc::make_mut(
                registry
                    .results
                    .get_mut(result_id)
                    .expect("validated result remains registered"),
            )
            .state = state;
        }
        drop(registry);
        self.inner.changed.notify_all();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn authoritative_result_count_for_test(&self) -> usize {
        self.registry().results.len()
    }

    #[cfg(test)]
    pub(crate) fn group_count_for_test(&self) -> usize {
        self.registry().groups.len()
    }

    #[cfg(test)]
    pub(crate) fn results_for_run(&self, run_id: RunId) -> Box<[Arc<StoredResult>]> {
        self.registry()
            .results
            .values()
            .filter(|result| result.provenance.run_id == run_id)
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, ResultStoreRegistry> {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

fn append_pin_history(
    histories: &mut BTreeMap<crate::execution::plan::legacy::GraphOutputRef, Arc<PinHistoryNode>>,
    output: &crate::execution::plan::legacy::GraphOutputRef,
    entry: PinResultEntry,
) {
    let previous = histories.get(output).cloned();
    let len = previous.as_ref().map_or(1, |node| node.len + 1);
    histories.insert(
        output.clone(),
        Arc::new(PinHistoryNode {
            entry,
            previous,
            len,
        }),
    );
}

fn collect_pin_history(tail: Option<Arc<PinHistoryNode>>) -> Box<[PinResultEntry]> {
    let Some(tail) = tail else {
        return Box::default();
    };
    let mut entries = Vec::with_capacity(tail.len);
    let mut current = Some(tail);
    while let Some(node) = current {
        entries.push(node.entry.clone());
        current = node.previous.clone();
    }
    entries.reverse();
    entries.into_boxed_slice()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultStoreError {
    IdExhausted,
    EmptyGroup,
    InvalidProducedUsage,
    InvalidReusedUsage,
    DuplicateActivation(ActivationId),
    DuplicateOutputValue(crate::execution::plan::legacy::ValueRef),
    DuplicatePublicOutput(crate::execution::plan::legacy::GraphOutputRef),
    OutputGraphMismatch(crate::execution::plan::legacy::GraphOutputRef),
    OutputNodeMismatch(crate::execution::plan::legacy::GraphOutputRef),
    UnknownActivation(ActivationId),
    UnknownResult(ResultId),
    GroupMismatch(ActivationId),
    TerminalResult(ResultId),
    ReusedResultNotReady(ResultId),
    ReusedOutputMismatch { index: usize, result_id: ResultId },
    WaitCancelled,
    WaitDeadlineExceeded,
    OutputCount { expected: usize, actual: usize },
}

impl ResultStoreError {
    fn from_wait_error(error: RunError) -> Self {
        match error {
            RunError::Cancelled => Self::WaitCancelled,
            RunError::DeadlineExceeded { .. } => Self::WaitDeadlineExceeded,
            _ => unreachable!("terminal wait only checks cancellation and deadline"),
        }
    }
}

impl fmt::Display for ResultStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdExhausted => formatter.write_str("result ID space is exhausted"),
            Self::EmptyGroup => formatter.write_str(
                "result groups require at least one data output; control-only operations skip them",
            ),
            Self::InvalidProducedUsage => {
                formatter.write_str("pending result groups must describe produced results")
            }
            Self::InvalidReusedUsage => {
                formatter.write_str("reused result groups require original activation provenance")
            }
            Self::DuplicateActivation(id) => {
                write!(
                    formatter,
                    "activation {} already has a result group",
                    id.get()
                )
            }
            Self::DuplicateOutputValue(value) => {
                write!(formatter, "result group repeats value {}", value.index())
            }
            Self::DuplicatePublicOutput(output) => {
                write!(
                    formatter,
                    "result group repeats public output {}",
                    output.port
                )
            }
            Self::OutputGraphMismatch(output) => write!(
                formatter,
                "public output graph '{}' does not match activation provenance",
                output.graph_path.as_str()
            ),
            Self::OutputNodeMismatch(output) => write!(
                formatter,
                "public output node '{}' does not match activation provenance",
                output.port.node_id
            ),
            Self::UnknownActivation(id) => {
                write!(formatter, "activation {} has no result group", id.get())
            }
            Self::UnknownResult(id) => write!(formatter, "result {} is not registered", id.get()),
            Self::GroupMismatch(id) => {
                write!(
                    formatter,
                    "result group does not match activation {}",
                    id.get()
                )
            }
            Self::TerminalResult(id) => write!(formatter, "result {} is terminal", id.get()),
            Self::ReusedResultNotReady(id) => {
                write!(formatter, "reused result {} is not ready", id.get())
            }
            Self::ReusedOutputMismatch { index, result_id } => write!(
                formatter,
                "reused output {index} is incompatible with result {}",
                result_id.get()
            ),
            Self::WaitCancelled => formatter.write_str("result wait was cancelled"),
            Self::WaitDeadlineExceeded => formatter.write_str("result wait deadline exceeded"),
            Self::OutputCount { expected, actual } => {
                write!(
                    formatter,
                    "result group has {actual} values; expected {expected}"
                )
            }
        }
    }
}

impl std::error::Error for ResultStoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::legacy::{
        GraphOutputRef, PlannedValueContract, ResultPresentation, ValueRef,
    };
    use crate::graph_document::{GraphResourcePath, GraphRevision, NodeId, PortAddress};
    use crate::node_system::runtime::{ArtifactValueKind, ResultUsage, StoredValueKind};
    use yss_graph_protocol::{PortKey, Value};

    fn test_output(port_key: &str, value: u32) -> PendingOutputDescriptor {
        PendingOutputDescriptor {
            value: ValueRef::new(value),
            output: Some(GraphOutputRef {
                graph_path: GraphResourcePath::new("events/test.yssbi-event").unwrap(),
                port: PortAddress::declared(
                    NodeId::from_uuid(uuid::Uuid::nil()),
                    PortKey::new(port_key).unwrap(),
                ),
            }),
            presentation: ResultPresentation::Inspector,
            contract: PlannedValueContract::opaque(),
        }
    }

    fn test_outputs<const N: usize>(port_keys: [&str; N]) -> Vec<PendingOutputDescriptor> {
        port_keys
            .into_iter()
            .enumerate()
            .map(|(index, port_key)| test_output(port_key, index as u32))
            .collect()
    }

    fn test_provenance(run: u64) -> ActivationProvenance {
        ActivationProvenance {
            run_id: RunId::new(run),
            activation_id: ActivationId::next().unwrap(),
            graph_path: GraphResourcePath::new("events/test.yssbi-event").unwrap(),
            graph_revision: GraphRevision::new(8),
            node_id: NodeId::from_uuid(uuid::Uuid::nil()),
            created_at_ms: run,
            usage: ResultUsage::Produced,
        }
    }

    fn create_ready_test_group(store: &ResultStore, run: u64) -> ActivationResultGroup {
        let group = store
            .create_pending_group(test_provenance(run), &test_outputs(["result"]))
            .unwrap();
        store
            .complete_group(
                &group,
                vec![StoredValue::scalar(Value::Integer(run as i64))].into_boxed_slice(),
            )
            .unwrap();
        group
    }

    #[test]
    fn pending_group_rejects_empty_and_mismatched_public_outputs() {
        let store = ResultStore::new();
        assert!(matches!(
            store.create_pending_group(test_provenance(5), &[]),
            Err(ResultStoreError::EmptyGroup)
        ));

        let mut wrong_graph = test_outputs(["result"]);
        wrong_graph[0].output.as_mut().unwrap().graph_path =
            GraphResourcePath::new("events/other.yssbi-event").unwrap();
        assert!(matches!(
            store.create_pending_group(test_provenance(6), &wrong_graph),
            Err(ResultStoreError::OutputGraphMismatch(_))
        ));

        let mut wrong_node = test_outputs(["result"]);
        wrong_node[0].output.as_mut().unwrap().port.node_id = NodeId::new();
        assert!(matches!(
            store.create_pending_group(test_provenance(6), &wrong_node),
            Err(ResultStoreError::OutputNodeMismatch(_))
        ));
        assert_eq!(store.authoritative_result_count_for_test(), 0);
    }

    #[test]
    fn pending_group_allocates_ordered_results_and_pin_history() {
        let store = ResultStore::new();
        let outputs = test_outputs(["z_result", "a_report"]);
        let group = store
            .create_pending_group(test_provenance(7), &outputs)
            .unwrap();

        assert_eq!(group.output_result_ids.len(), 2);
        assert_ne!(group.output_result_ids[0], group.output_result_ids[1]);
        assert!(matches!(
            store.result(group.output_result_ids[0]).unwrap().state,
            ResultState::Pending(_)
        ));
        assert_eq!(
            store.pin_history(outputs[0].output.as_ref().unwrap())[0].result_id,
            group.output_result_ids[0]
        );
    }

    #[test]
    fn pending_group_setup_rolls_back_on_invalid_descriptors() {
        let store = ResultStore::new();
        let output = test_output("result", 0);
        let outputs = vec![output.clone(), output];

        assert!(
            store
                .create_pending_group(test_provenance(8), &outputs)
                .is_err()
        );
        assert_eq!(store.authoritative_result_count_for_test(), 0);
        assert!(
            store
                .pin_history(outputs[0].output.as_ref().unwrap())
                .is_empty()
        );
    }

    #[test]
    fn complete_group_is_all_or_nothing_and_terminal() {
        let store = ResultStore::new();
        let group = store
            .create_pending_group(test_provenance(9), &test_outputs(["left", "right"]))
            .unwrap();

        assert!(
            store
                .complete_group(
                    &group,
                    vec![StoredValue::scalar(Value::Null)].into_boxed_slice()
                )
                .is_err()
        );
        assert!(
            group
                .output_result_ids
                .iter()
                .all(|id| matches!(store.result(*id).unwrap().state, ResultState::Pending(_)))
        );

        store
            .complete_group(
                &group,
                vec![
                    StoredValue::scalar(Value::Null),
                    StoredValue::scalar(Value::Null),
                ]
                .into_boxed_slice(),
            )
            .unwrap();
        assert!(
            group
                .output_result_ids
                .iter()
                .all(|id| matches!(store.result(*id).unwrap().state, ResultState::Ready(_)))
        );
        assert!(store.cancel_group(&group).is_err());
    }

    #[test]
    fn fail_and_cancel_transition_the_whole_group_and_preserve_history() {
        let store = ResultStore::new();
        let failed_outputs = test_outputs(["failed_left", "failed_right"]);
        let failed = store
            .create_pending_group(test_provenance(10), &failed_outputs)
            .unwrap();
        store
            .fail_group(&failed, Arc::new(ResultFailure::new("kernel failed")))
            .unwrap();
        assert!(
            failed
                .output_result_ids
                .iter()
                .all(|id| matches!(store.result(*id).unwrap().state, ResultState::Failed(_)))
        );

        let cancelled_outputs = test_outputs(["cancelled_left", "cancelled_right"]);
        let cancelled = store
            .create_pending_group(test_provenance(11), &cancelled_outputs)
            .unwrap();
        store.cancel_group(&cancelled).unwrap();
        assert!(
            cancelled
                .output_result_ids
                .iter()
                .all(|id| matches!(store.result(*id).unwrap().state, ResultState::Cancelled))
        );
        assert_eq!(
            store.pin_history(cancelled_outputs[1].output.as_ref().unwrap())[0].result_id,
            cancelled.output_result_ids[1]
        );
    }

    #[test]
    fn concurrent_duplicate_activation_never_mutates_results_or_history() {
        let store = Arc::new(ResultStore::new());
        let provenance = test_provenance(12);
        let output = test_outputs(["result"]);
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let attempts = (0..2)
            .map(|_| {
                let store = Arc::clone(&store);
                let provenance = provenance.clone();
                let output = output.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store.create_pending_group(provenance, &output)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let outcomes = attempts
            .into_iter()
            .map(|attempt| attempt.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(store.authoritative_result_count_for_test(), 1);
        assert_eq!(
            store.pin_history(output[0].output.as_ref().unwrap()).len(),
            1
        );
        assert!(outcomes.iter().any(|outcome| matches!(
            outcome,
            Err(ResultStoreError::DuplicateActivation(id)) if *id == provenance.activation_id
        )));
        // The losing pre-lock allocation may leave an opaque ResultId gap by design.
    }

    #[test]
    fn completion_and_cancellation_race_has_one_atomic_winner() {
        let store = Arc::new(ResultStore::new());
        let group = store
            .create_pending_group(test_provenance(12), &test_outputs(["left", "right"]))
            .unwrap();
        let complete_store = Arc::clone(&store);
        let complete_group = group.clone();
        let complete = std::thread::spawn(move || {
            complete_store.complete_group(
                &complete_group,
                vec![
                    StoredValue::scalar(Value::Null),
                    StoredValue::scalar(Value::Null),
                ]
                .into_boxed_slice(),
            )
        });
        let cancel_store = Arc::clone(&store);
        let cancel_group = group.clone();
        let cancel = std::thread::spawn(move || cancel_store.cancel_group(&cancel_group));

        let outcomes = [complete.join().unwrap(), cancel.join().unwrap()];
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        let states = group
            .output_result_ids
            .iter()
            .map(|id| store.result(*id).unwrap().state.clone())
            .collect::<Vec<_>>();
        assert!(
            states
                .iter()
                .all(|state| matches!(state, ResultState::Ready(_)))
                || states
                    .iter()
                    .all(|state| matches!(state, ResultState::Cancelled))
        );
    }

    #[test]
    fn wait_terminal_observes_ready_failed_and_cancelled_without_lost_wakeup() {
        for terminal in ["ready", "failed", "cancelled"] {
            let store = Arc::new(ResultStore::new());
            let group = store
                .create_pending_group(test_provenance(14), &test_outputs([terminal]))
                .unwrap();
            let result_id = group.output_result_ids[0];
            let waiter_store = Arc::clone(&store);
            let (waiting_tx, waiting_rx) = std::sync::mpsc::channel();
            let waiter = std::thread::spawn(move || {
                waiting_tx.send(()).unwrap();
                waiter_store.wait_terminal(result_id, &CancellationToken::new(), None)
            });
            waiting_rx.recv().unwrap();
            match terminal {
                "ready" => store
                    .complete_group(
                        &group,
                        vec![StoredValue::scalar(Value::Null)].into_boxed_slice(),
                    )
                    .unwrap(),
                "failed" => store
                    .fail_group(&group, Arc::new(ResultFailure::new("failed")))
                    .unwrap(),
                "cancelled" => store.cancel_group(&group).unwrap(),
                _ => unreachable!(),
            }
            let result = waiter.join().unwrap().unwrap();
            assert!(result.state.is_terminal());
        }

        let store = ResultStore::new();
        let ready = create_ready_test_group(&store, 15);
        assert!(matches!(
            store
                .wait_terminal(ready.output_result_ids[0], &CancellationToken::new(), None)
                .unwrap()
                .state,
            ResultState::Ready(_)
        ));
    }

    #[test]
    fn wait_terminal_reports_missing_cancellation_and_deadline() {
        let store = ResultStore::new();
        assert!(matches!(
            store.wait_terminal(ResultId::new(u64::MAX), &CancellationToken::new(), None),
            Err(ResultStoreError::UnknownResult(_))
        ));

        let cancelled = CancellationToken::new();
        let cancelled_group = store
            .create_pending_group(test_provenance(16), &test_outputs(["cancelled_wait"]))
            .unwrap();
        cancelled.cancel();
        assert!(matches!(
            store.wait_terminal(cancelled_group.output_result_ids[0], &cancelled, None),
            Err(ResultStoreError::WaitCancelled)
        ));

        let deadline_group = store
            .create_pending_group(test_provenance(17), &test_outputs(["deadline_wait"]))
            .unwrap();
        assert!(matches!(
            store.wait_terminal(
                deadline_group.output_result_ids[0],
                &CancellationToken::new(),
                Some(RunDeadline::after(std::time::Duration::from_millis(1)))
            ),
            Err(ResultStoreError::WaitDeadlineExceeded)
        ));
    }

    #[test]
    fn reused_group_records_occurrences_without_rewriting_producers() {
        let store = ResultStore::new();
        let outputs = test_outputs(["left", "right"]);
        let produced = store
            .create_pending_group(test_provenance(18), &outputs)
            .unwrap();
        store
            .complete_group(
                &produced,
                vec![
                    StoredValue::scalar(Value::Null),
                    StoredValue::scalar(Value::Null),
                ]
                .into_boxed_slice(),
            )
            .unwrap();
        let producer = store
            .result(produced.output_result_ids[0])
            .unwrap()
            .provenance
            .clone();
        let result_count = store.authoritative_result_count_for_test();
        let mut reuse = test_provenance(19);
        reuse.usage = ResultUsage::Reused {
            original_activation_id: produced.activation_id,
        };

        let reused = store
            .record_reused_group(reuse.clone(), &outputs, &produced.output_result_ids)
            .unwrap();

        assert_eq!(reused.output_result_ids, produced.output_result_ids);
        assert_eq!(store.authoritative_result_count_for_test(), result_count);
        assert_eq!(
            store
                .result(produced.output_result_ids[0])
                .unwrap()
                .provenance,
            producer
        );
        let history = store.pin_history(outputs[0].output.as_ref().unwrap());
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].activation_id, reuse.activation_id);
        assert_eq!(
            history[1].usage,
            ResultUsage::Reused {
                original_activation_id: produced.activation_id
            }
        );
    }

    #[test]
    fn reused_group_validates_count_order_compatibility_and_ready_state() {
        let store = ResultStore::new();
        let outputs = test_outputs(["left", "right"]);
        let ready = store
            .create_pending_group(test_provenance(20), &outputs)
            .unwrap();
        store
            .complete_group(
                &ready,
                vec![
                    StoredValue::scalar(Value::Null),
                    StoredValue::scalar(Value::Null),
                ]
                .into_boxed_slice(),
            )
            .unwrap();
        let reuse = |original_activation_id| {
            let mut provenance = test_provenance(21);
            provenance.usage = ResultUsage::Reused {
                original_activation_id,
            };
            provenance
        };

        assert!(matches!(
            store.record_reused_group(
                reuse(ready.activation_id),
                &outputs,
                &ready.output_result_ids[..1]
            ),
            Err(ResultStoreError::OutputCount { .. })
        ));
        assert!(matches!(
            store.record_reused_group(
                reuse(ready.activation_id),
                &outputs,
                &[ready.output_result_ids[1], ready.output_result_ids[0]]
            ),
            Err(ResultStoreError::ReusedOutputMismatch { .. })
        ));
        let mut incompatible = outputs.clone();
        incompatible[0].presentation = ResultPresentation::default();
        incompatible[0].value = ValueRef::new(99);
        assert!(matches!(
            store.record_reused_group(
                reuse(ready.activation_id),
                &incompatible,
                &ready.output_result_ids
            ),
            Err(ResultStoreError::ReusedOutputMismatch { .. })
        ));

        let foreign_store = ResultStore::new();
        let foreign_outputs = test_outputs(["foreign"]);
        let foreign = create_ready_test_group(&foreign_store, 22);
        assert!(matches!(
            store.record_reused_group(
                reuse(foreign.activation_id),
                &foreign_outputs,
                &foreign.output_result_ids
            ),
            Err(ResultStoreError::UnknownActivation(_))
        ));

        let pending_outputs = test_outputs(["pending"]);
        let pending = store
            .create_pending_group(test_provenance(22), &pending_outputs)
            .unwrap();
        assert!(matches!(
            store.record_reused_group(
                reuse(pending.activation_id),
                &pending_outputs,
                &pending.output_result_ids
            ),
            Err(ResultStoreError::ReusedResultNotReady(_))
        ));
        assert_eq!(
            store.pin_history(outputs[0].output.as_ref().unwrap()).len(),
            1
        );
    }

    #[test]
    fn result_ids_are_process_global_across_session_stores() {
        let first_store = ResultStore::new();
        let second_store = ResultStore::new();
        let first = first_store
            .create_pending_group(test_provenance(20), &test_outputs(["first"]))
            .unwrap();
        let second = second_store
            .create_pending_group(test_provenance(21), &test_outputs(["second"]))
            .unwrap();

        assert!(second.output_result_ids[0].get() > first.output_result_ids[0].get());
    }

    #[test]
    fn result_store_never_evicts_within_the_session() {
        let store = ResultStore::new();
        let first = create_ready_test_group(&store, 1);
        for activation in 2..=5000 {
            create_ready_test_group(&store, activation);
        }
        assert!(store.result(first.output_result_ids[0]).is_some());
    }

    #[test]
    fn stored_value_pages_and_opens_independent_readers() {
        let value = StoredValue::sequence(
            vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)].into_boxed_slice(),
        );
        assert_eq!(value.kind(), StoredValueKind::Sequence);
        assert_eq!(value.len(), 3);
        assert_eq!(value.page(1, 1).unwrap().as_ref(), &[Value::Integer(2)]);
        let mut first = value.open_reader().unwrap();
        let mut second = value.open_reader().unwrap();
        assert_eq!(first.next().unwrap().unwrap(), Value::Integer(1));
        assert_eq!(second.next().unwrap().unwrap(), Value::Integer(1));
    }

    #[test]
    fn dropping_the_session_store_releases_spill_backing() {
        let transient = std::env::temp_dir().join(format!(
            "yssbi-result-store-test-{}.jsonf",
            uuid::Uuid::new_v4()
        ));
        let metadata = super::super::spill::write_spill(
            &transient,
            vec![Ok(Value::Integer(1))].into_iter(),
            &CancellationToken::new(),
            |_| Ok(()),
        )
        .unwrap();
        let spill = Arc::new(super::super::spill::SpillStorage::new(
            transient,
            metadata,
            ArtifactValueKind::Sequence,
            None,
            [0; 32],
            None,
        ));
        spill.promote(&CancellationToken::new(), None).unwrap();
        let durable = spill.path_for_test();
        assert!(durable.exists());

        {
            let store = ResultStore::new();
            let group = store
                .create_pending_group(test_provenance(13), &test_outputs(["result"]))
                .unwrap();
            store
                .complete_group(
                    &group,
                    vec![StoredValue::spill_backed(spill)].into_boxed_slice(),
                )
                .unwrap();
        }

        assert!(!durable.exists());
    }
}
