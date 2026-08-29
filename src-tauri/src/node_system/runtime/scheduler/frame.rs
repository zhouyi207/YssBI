use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct MemoKey {
    pub(super) frame: FrameId,
    pub(super) activation: ActivationId,
    pub(super) operation: OperationIndex,
}

pub(super) struct Frame {
    pub(super) id: FrameId,
    bindings: Vec<Option<ResultId>>,

    pub(super) attempted: BTreeMap<MemoKey, AttemptId>,
    pub(super) completed: BTreeSet<MemoKey>,
    pub(super) completion_counts: BTreeMap<OperationIndex, usize>,
}

impl Frame {
    pub(super) fn new(value_count: u32, frame_ids: &AtomicU64) -> Result<Self, RunError> {
        Ok(Self {
            id: FrameId::allocate(frame_ids)?,
            bindings: vec![None; value_count as usize],

            attempted: BTreeMap::new(),
            completed: BTreeSet::new(),
            completion_counts: BTreeMap::new(),
        })
    }

    pub(super) fn clear_region_values(
        &mut self,
        plan: &ExecutionPlan,
        region: &StructuredControlRegion,
    ) {
        let mut operations = BTreeSet::new();
        collect_region_operations(region, &mut operations);
        let mut cleared = operations
            .into_iter()
            .flat_map(|operation| {
                plan.operations[operation.index()]
                    .outputs
                    .iter()
                    .map(|output| output.value)
            })
            .collect::<BTreeSet<_>>();
        loop {
            let derived = plan
                .value_dependencies
                .iter()
                .filter(|dependency| cleared.contains(&dependency.source))
                .map(|dependency| dependency.destination)
                .filter(|destination| !cleared.contains(destination))
                .collect::<Vec<_>>();
            if derived.is_empty() {
                break;
            }
            cleared.extend(derived);
        }
        for reference in cleared {
            self.clear_result(reference);
        }
    }

    pub(super) fn has(&self, reference: ValueRef) -> bool {
        self.bindings
            .get(reference.index())
            .is_some_and(Option::is_some)
    }

    pub(super) fn result_id(&self, reference: ValueRef) -> Result<ResultId, RunError> {
        self.bindings
            .get(reference.index())
            .and_then(|result| *result)
            .ok_or(RunError::MissingValue(reference))
    }

    pub(super) fn bind_result(
        &mut self,
        reference: ValueRef,
        result_id: ResultId,
    ) -> Result<(), RunError> {
        let slot = self
            .bindings
            .get_mut(reference.index())
            .ok_or(RunError::MissingValue(reference))?;
        *slot = Some(result_id);
        Ok(())
    }

    pub(super) fn copy_result(
        &mut self,
        source: ValueRef,
        destination: ValueRef,
    ) -> Result<(), RunError> {
        self.bind_result(destination, self.result_id(source)?)
    }

    fn clear_result(&mut self, reference: ValueRef) {
        if let Some(slot) = self.bindings.get_mut(reference.index()) {
            *slot = None;
        }
    }

    pub(super) fn completed(&self, activation: ActivationId, operation: OperationIndex) -> bool {
        self.completed.contains(&MemoKey {
            frame: self.id,
            activation,
            operation,
        })
    }

    pub(super) fn completion_count(&self, operation: OperationIndex) -> usize {
        self.completion_counts.get(&operation).copied().unwrap_or(0)
    }
}

pub(super) fn collect_region_operations(
    region: &StructuredControlRegion,
    operations: &mut BTreeSet<OperationIndex>,
) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                match step {
                    ControlStep::Operation(operation) => {
                        operations.insert(*operation);
                    }
                    ControlStep::Region(child) => collect_region_operations(child, operations),
                }
            }
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            collect_region_operations(then_region, operations);
            collect_region_operations(else_region, operations);
        }
        StructuredControlRegion::Loop { body, .. } => {
            collect_region_operations(body, operations);
        }
        StructuredControlRegion::Call { .. } => {}
    }
}

#[cfg(test)]
mod result_id_frame_tests {
    use super::*;
    use crate::execution::plan::legacy::GraphOutputRef;

    #[test]
    fn runtime_id_exhaustion_is_not_classified_as_an_invalid_plan() {
        let allocator = AtomicU64::new(u64::MAX);

        let error = allocate_runtime_id(&allocator).unwrap_err();

        assert_eq!(error, RunError::RuntimeIdExhausted);
        assert_eq!(
            crate::node_system::runtime::RunErrorCode::from(&error),
            crate::node_system::runtime::RunErrorCode::RuntimeIdExhausted
        );
    }

    #[test]
    fn scheduler_uses_current_frame_binding_not_latest_pin_history() {
        let store = ResultStore::new();
        let graph_path =
            crate::graph_document::GraphResourcePath::new("events/test.yssbi-event").unwrap();
        let node_id = crate::graph_document::NodeId::new();
        let output = GraphOutputRef {
            graph_path: graph_path.clone(),
            port: crate::graph_document::PortAddress::declared(
                node_id,
                crate::graph::protocol::PortKey::new("result").unwrap(),
            ),
        };
        let descriptor = PendingOutputDescriptor {
            value: ValueRef::new(0),
            output: Some(output.clone()),
            presentation: ResultPresentation::Inspector,
            contract: PlannedValueContract::opaque(),
        };
        let create_ready = |value| {
            let activation_id = ActivationId::next().unwrap();
            let group = store
                .create_pending_group(
                    ActivationProvenance {
                        run_id: RunId::new(1),
                        activation_id,
                        graph_path: graph_path.clone(),
                        graph_revision: crate::graph_document::GraphRevision::new(1),
                        node_id,
                        created_at_ms: activation_id.get(),
                        usage: ResultUsage::Produced,
                    },
                    std::slice::from_ref(&descriptor),
                )
                .unwrap();
            store
                .complete_group(
                    &group,
                    vec![StoredValue::scalar(Value::Integer(value))].into_boxed_slice(),
                )
                .unwrap();
            group.output_result_ids[0]
        };
        let current = create_ready(1);
        let latest = create_ready(2);
        let frame_ids = AtomicU64::new(1);
        let mut frame = Frame::new(1, &frame_ids).unwrap();
        frame.bind_result(ValueRef::new(0), current).unwrap();

        assert_eq!(frame.result_id(ValueRef::new(0)).unwrap(), current);
        assert_eq!(store.pin_history(&output).last().unwrap().result_id, latest);
        assert_ne!(current, latest);
    }
}
