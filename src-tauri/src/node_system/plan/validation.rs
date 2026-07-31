use super::model::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

impl ExecutionPlan {
    pub fn validate(&self) -> Result<(), PlanValidationErrors> {
        let mut errors = Vec::new();
        let operation_count = self.operations.len();
        let relational_count = self.relational_subplans.len();
        let value_count = self.value_count as usize;
        let mut produced_values = BTreeSet::new();
        let mut value_producers = vec![BTreeSet::new(); value_count];
        let mut relational_owners = BTreeMap::new();

        for (operation, planned) in self.operations.iter().enumerate() {
            if let PlannedKernel::Relational(index) = planned.kernel {
                check_index(
                    &mut errors,
                    "operation relational subplan",
                    index.index(),
                    relational_count,
                );
                if index.index() < relational_count {
                    let operation = OperationIndex::new(operation as u32);
                    if let Some(first) = relational_owners.get(&index).copied() {
                        errors.push(PlanValidationError::DuplicateRelationalSubplanOwner {
                            subplan: index,
                            first,
                            duplicate: operation,
                        });
                    } else {
                        relational_owners.insert(index, operation);
                    }
                }
            }
            for input in &planned.inputs {
                check_value(&mut errors, "operation input", input.value, value_count);
            }
            for output in &planned.outputs {
                check_value(&mut errors, "operation output", output.value, value_count);
                if output.value.index() < value_count {
                    let operation = OperationIndex::new(operation as u32);
                    value_producers[output.value.index()].insert(operation);
                    if !produced_values.insert(output.value) {
                        errors.push(PlanValidationError::DuplicateValueProducer {
                            value: output.value,
                            operation,
                        });
                    }
                }
            }
        }

        for (subplan, relational) in self.relational_subplans.iter().enumerate() {
            let subplan = RelationalSubplanIndex::new(subplan as u32);
            let Some(owner) = relational_owners.get(&subplan).copied() else {
                errors.push(PlanValidationError::UnownedRelationalSubplan(subplan));
                continue;
            };
            let output_count = self.operations[owner.index()].outputs.len();
            let root_count = relational.compiled_plan.roots.len();
            if output_count != root_count {
                errors.push(
                    PlanValidationError::RelationalOwnerOutputRootCardinalityMismatch {
                        subplan,
                        owner,
                        output_count,
                        root_count,
                    },
                );
            }
        }

        let mut declared_sources = BTreeSet::new();
        for source in &self.value_sources {
            let value = source.value();
            check_value(&mut errors, "plan value source", value, value_count);
            if value.index() < value_count && !declared_sources.insert(value) {
                errors.push(PlanValidationError::DuplicateValueSource(value));
            }
        }

        for dependency in &self.value_dependencies {
            check_value(
                &mut errors,
                "value dependency source",
                dependency.source,
                value_count,
            );
            check_value(
                &mut errors,
                "value dependency destination",
                dependency.destination,
                value_count,
            );
            if dependency.source == dependency.destination {
                errors.push(PlanValidationError::ValueDependencySelfLoop(
                    dependency.source,
                ));
            }
        }

        if has_directed_cycle(
            value_count,
            self.value_dependencies
                .iter()
                .map(|dependency| (dependency.source.index(), dependency.destination.index())),
        ) {
            errors.push(PlanValidationError::ValueDependencyCycle);
        }

        let sourced_values =
            validate_input_sources(self, &value_producers, &declared_sources, &mut errors);

        for dependency in &self.effect_dependencies {
            check_operation(
                &mut errors,
                "effect dependency before",
                dependency.before,
                operation_count,
            );
            check_operation(
                &mut errors,
                "effect dependency after",
                dependency.after,
                operation_count,
            );
            if dependency.before == dependency.after {
                errors.push(PlanValidationError::EffectDependencySelfLoop(
                    dependency.before,
                ));
            }
        }

        if has_directed_cycle(
            operation_count,
            self.effect_dependencies
                .iter()
                .map(|dependency| (dependency.before.index(), dependency.after.index())),
        ) {
            errors.push(PlanValidationError::EffectDependencyCycle);
        }

        let control_value_sources = self
            .value_sources
            .iter()
            .filter_map(|source| match source {
                PlanValueSource::ControlProduced(value) => Some(*value),
                PlanValueSource::ExternalInput(_) => None,
            })
            .collect::<BTreeSet<_>>();
        validate_region(
            &self.root_region,
            &mut errors,
            operation_count,
            value_count,
            &control_value_sources,
        );
        validate_relational_subplans(self, &mut errors);
        validate_resources(self, &mut errors);

        let mut result_names = BTreeSet::new();
        for result in &self.results {
            check_value(&mut errors, "plan result", result.value, value_count);
            if result.name.is_empty() || result.name.trim() != result.name.as_ref() {
                errors.push(PlanValidationError::InvalidResultName(result.name.clone()));
            } else if !result_names.insert(result.name.clone()) {
                errors.push(PlanValidationError::DuplicateResultName(
                    result.name.clone(),
                ));
            }
            if result.value.index() < value_count && !sourced_values[result.value.index()] {
                errors.push(PlanValidationError::MissingResultSource(result.value));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(PlanValidationErrors(errors.into_boxed_slice()))
        }
    }
}

fn validate_relational_subplans(plan: &ExecutionPlan, errors: &mut Vec<PlanValidationError>) {
    let subplan_count = plan.relational_subplans.len();
    let mut bridges = BTreeSet::new();
    let mut all_fragments = BTreeSet::new();
    for (subplan_index, subplan) in plan.relational_subplans.iter().enumerate() {
        let compiled = &subplan.compiled_plan;
        let operator_count = compiled.operators.len();
        let mut fragments = BTreeSet::new();
        for fragment in &compiled.fragment_order {
            if !fragments.insert(fragment.clone()) || !all_fragments.insert(fragment.clone()) {
                errors.push(PlanValidationError::DuplicateRelationalFragment(
                    fragment.clone(),
                ));
            }
        }
        if compiled.fragment_order.is_empty() || compiled.operators.is_empty() {
            errors.push(PlanValidationError::EmptyRelationalSubplan(
                RelationalSubplanIndex::new(subplan_index as u32),
            ));
        }
        for (operator_index, operator) in compiled.operators.iter().enumerate() {
            for input in relational_operator_inputs(operator) {
                if input.index() >= operator_index {
                    errors.push(PlanValidationError::InvalidRelationalOperatorInput {
                        subplan: RelationalSubplanIndex::new(subplan_index as u32),
                        operator: RelationalOperatorIndex::new(operator_index as u32),
                        input,
                    });
                }
            }
        }
        let mut rooted_fragments = BTreeSet::new();
        for root in &compiled.fragment_roots {
            check_index(
                errors,
                "relational fragment root",
                root.operator.index(),
                operator_count,
            );
            if !fragments.contains(&root.fragment) {
                errors.push(PlanValidationError::RelationalFragmentRootUnexpected(
                    root.fragment.clone(),
                ));
            } else if !rooted_fragments.insert(root.fragment.clone()) {
                errors.push(PlanValidationError::RelationalFragmentRootDuplicate(
                    root.fragment.clone(),
                ));
            }
        }
        for fragment in fragments.difference(&rooted_fragments) {
            errors.push(PlanValidationError::RelationalFragmentRootMissing(
                fragment.clone(),
            ));
        }
        let mut requested_fragment_outputs = BTreeSet::new();
        for fragment in &compiled.requested_fragment_outputs {
            if !fragments.contains(fragment) {
                errors.push(PlanValidationError::RelationalFragmentOutputUnexpected(
                    fragment.clone(),
                ));
            }
            if !requested_fragment_outputs.insert(fragment.clone()) {
                errors.push(PlanValidationError::RelationalFragmentOutputDuplicate(
                    fragment.clone(),
                ));
            }
        }
        for root in &compiled.roots {
            check_index(errors, "relational root", root.index(), operator_count);
        }
        let inferred_pushdown_hints = infer_relational_pushdown_hints(&compiled.operators);
        if compiled.pushdown_hints.as_ref() != inferred_pushdown_hints.as_slice() {
            errors.push(PlanValidationError::RelationalPushdownHintsMismatch {
                subplan: RelationalSubplanIndex::new(subplan_index as u32),
            });
        }
        for bridge in &subplan.materialization_bridges {
            check_index(
                errors,
                "bridge producer subplan",
                bridge.producer_subplan.index(),
                subplan_count,
            );
            check_index(
                errors,
                "bridge consumer subplan",
                bridge.consumer_subplan.index(),
                subplan_count,
            );
            if bridge.consumer_subplan.index() != subplan_index {
                errors.push(PlanValidationError::BridgeStoredOnWrongConsumer {
                    stored_on: RelationalSubplanIndex::new(subplan_index as u32),
                    consumer: bridge.consumer_subplan,
                });
            }
            if bridge.producer_subplan == bridge.consumer_subplan {
                errors.push(PlanValidationError::BridgeWithinSubplan(
                    bridge.consumer_subplan,
                ));
            }
            if bridge.producer_subplan.index() < subplan_count {
                let producer =
                    &plan.relational_subplans[bridge.producer_subplan.index()].compiled_plan;
                if !producer.fragment_order.contains(&bridge.producer_fragment) {
                    errors.push(PlanValidationError::BridgeFragmentMissing(
                        bridge.producer_fragment.clone(),
                    ));
                } else if !producer
                    .requested_fragment_outputs
                    .contains(&bridge.producer_fragment)
                {
                    errors.push(PlanValidationError::BridgeProducerOutputNotRequested {
                        producer_subplan: bridge.producer_subplan,
                        fragment: bridge.producer_fragment.clone(),
                    });
                }
            }
            if !fragments.contains(&bridge.consumer_fragment) {
                errors.push(PlanValidationError::BridgeFragmentMissing(
                    bridge.consumer_fragment.clone(),
                ));
            }
            let key = (
                bridge.producer_fragment.clone(),
                bridge.consumer_fragment.clone(),
                bridge.producer_subplan,
                bridge.consumer_subplan,
            );
            if !bridges.insert(key) {
                errors.push(PlanValidationError::DuplicateMaterializationBridge);
            }
        }

        let subplan_index = RelationalSubplanIndex::new(subplan_index as u32);
        let mut bound_operators = BTreeSet::new();
        let mut bound_bridges = Vec::new();
        for binding in &compiled.bridge_inputs {
            if binding.operator.index() >= operator_count {
                errors.push(
                    PlanValidationError::RelationalBridgeInputOperatorOutOfBounds {
                        subplan: subplan_index,
                        operator: binding.operator,
                        operator_count,
                    },
                );
            } else if !matches!(
                compiled.operators[binding.operator.index()],
                RelationalOperator::Input { .. }
            ) {
                errors.push(PlanValidationError::RelationalBridgeInputOperatorNotInput {
                    subplan: subplan_index,
                    operator: binding.operator,
                });
            }
            if !bound_operators.insert(binding.operator) {
                errors.push(
                    PlanValidationError::DuplicateRelationalBridgeInputOperator {
                        subplan: subplan_index,
                        operator: binding.operator,
                    },
                );
            }
            if !subplan.materialization_bridges.contains(&binding.bridge) {
                errors.push(PlanValidationError::RelationalBridgeInputBridgeUndeclared {
                    subplan: subplan_index,
                    operator: binding.operator,
                    bridge: binding.bridge.clone(),
                });
            }
            if bound_bridges.contains(&&binding.bridge) {
                errors.push(PlanValidationError::DuplicateRelationalBridgeInputBridge {
                    subplan: subplan_index,
                    bridge: binding.bridge.clone(),
                });
            } else {
                bound_bridges.push(&binding.bridge);
            }
        }
        for bridge in &subplan.materialization_bridges {
            if !compiled
                .bridge_inputs
                .iter()
                .any(|binding| binding.bridge == *bridge)
            {
                errors.push(PlanValidationError::RelationalBridgeInputMissing {
                    subplan: subplan_index,
                    bridge: bridge.clone(),
                });
            }
        }
    }
}

fn relational_operator_inputs(operator: &RelationalOperator) -> Vec<RelationalOperatorIndex> {
    match operator {
        RelationalOperator::Input { .. } | RelationalOperator::Source { .. } => Vec::new(),
        RelationalOperator::Project { input, .. }
        | RelationalOperator::Filter { input, .. }
        | RelationalOperator::Rename { input, .. }
        | RelationalOperator::Limit { input, .. } => vec![*input],
        RelationalOperator::Union { inputs, .. } => inputs.to_vec(),
    }
}

fn validate_resources(plan: &ExecutionPlan, errors: &mut Vec<PlanValidationError>) {
    let mut resources = BTreeMap::new();
    for requirement in &plan.resources {
        if let Some(previous) = resources.insert(requirement.resource.clone(), requirement) {
            if previous == requirement {
                errors.push(PlanValidationError::DuplicateResourceRequirement(
                    requirement.resource.clone(),
                ));
            } else {
                errors.push(PlanValidationError::ConflictingResourceRequirement(
                    requirement.resource.clone(),
                ));
            }
        }
    }
}

fn validate_input_sources(
    plan: &ExecutionPlan,
    value_producers: &[BTreeSet<OperationIndex>],
    declared_sources: &BTreeSet<ValueRef>,
    errors: &mut Vec<PlanValidationError>,
) -> Vec<bool> {
    let value_count = value_producers.len();
    let mut has_declared_source = (0..value_count)
        .map(|value| declared_sources.contains(&ValueRef::new(value as u32)))
        .collect::<Vec<_>>();
    let mut propagated_producers = value_producers.to_vec();

    loop {
        let mut changed = false;
        for dependency in &plan.value_dependencies {
            let source = dependency.source.index();
            let destination = dependency.destination.index();
            if source >= value_count || destination >= value_count {
                continue;
            }

            if has_declared_source[source] && !has_declared_source[destination] {
                has_declared_source[destination] = true;
                changed = true;
            }
            let source_producers = propagated_producers[source].clone();
            for producer in source_producers {
                changed |= propagated_producers[destination].insert(producer);
            }
        }
        if !changed {
            break;
        }
    }

    let mut reported = BTreeSet::new();
    for (operation, planned) in plan.operations.iter().enumerate() {
        let operation = OperationIndex::new(operation as u32);
        for input in &planned.inputs {
            let value = input.value.index();
            if value >= value_count {
                continue;
            }
            let has_other_producer = propagated_producers[value]
                .iter()
                .any(|producer| *producer != operation);
            if !has_declared_source[value]
                && !has_other_producer
                && reported.insert((operation, input.value))
            {
                errors.push(PlanValidationError::MissingInputSource {
                    value: input.value,
                    operation,
                });
            }
        }
    }

    has_declared_source
        .into_iter()
        .zip(propagated_producers)
        .map(|(declared, producers)| declared || !producers.is_empty())
        .collect()
}

fn has_directed_cycle(node_count: usize, edges: impl Iterator<Item = (usize, usize)>) -> bool {
    let mut adjacency = vec![BTreeSet::new(); node_count];
    let mut indegree = vec![0usize; node_count];
    for (source, destination) in edges {
        if source >= node_count || destination >= node_count || source == destination {
            continue;
        }
        if adjacency[source].insert(destination) {
            indegree[destination] += 1;
        }
    }

    let mut ready = indegree
        .iter()
        .enumerate()
        .filter_map(|(node, degree)| (*degree == 0).then_some(node))
        .collect::<BTreeSet<_>>();
    let mut visited = 0;
    while let Some(node) = ready.pop_first() {
        visited += 1;
        for &destination in &adjacency[node] {
            indegree[destination] -= 1;
            if indegree[destination] == 0 {
                ready.insert(destination);
            }
        }
    }

    visited != node_count
}

fn validate_region(
    region: &StructuredControlRegion,
    errors: &mut Vec<PlanValidationError>,
    operation_count: usize,
    value_count: usize,
    control_value_sources: &BTreeSet<ValueRef>,
) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                match step {
                    ControlStep::Operation(index) => {
                        check_operation(errors, "control step", *index, operation_count)
                    }
                    ControlStep::Region(region) => validate_region(
                        region,
                        errors,
                        operation_count,
                        value_count,
                        control_value_sources,
                    ),
                }
            }
        }
        StructuredControlRegion::If {
            condition,
            then_region,
            else_region,
            results,
        } => {
            check_value(errors, "if condition", *condition, value_count);
            validate_region(
                then_region,
                errors,
                operation_count,
                value_count,
                control_value_sources,
            );
            validate_region(
                else_region,
                errors,
                operation_count,
                value_count,
                control_value_sources,
            );
            let mut destinations = BTreeSet::new();
            for binding in results {
                check_value(
                    errors,
                    "branch result destination",
                    binding.destination,
                    value_count,
                );
                check_value(
                    errors,
                    "branch then source",
                    binding.then_source,
                    value_count,
                );
                check_value(
                    errors,
                    "branch else source",
                    binding.else_source,
                    value_count,
                );
                if !destinations.insert(binding.destination) {
                    errors.push(PlanValidationError::DuplicateBranchResultDestination(
                        binding.destination,
                    ));
                }
                if binding.destination == binding.then_source
                    || binding.destination == binding.else_source
                    || binding.then_source == binding.else_source
                {
                    errors.push(PlanValidationError::InvalidBranchResultRoles(*binding));
                }
                require_control_value_source(
                    errors,
                    "branch result destination",
                    binding.destination,
                    value_count,
                    control_value_sources,
                );
            }
        }
        StructuredControlRegion::Loop {
            body,
            carried,
            continue_condition,
            max_iterations,
        } => {
            validate_region(
                body,
                errors,
                operation_count,
                value_count,
                control_value_sources,
            );
            check_value(errors, "loop condition", *continue_condition, value_count);
            if *max_iterations == 0 {
                errors.push(PlanValidationError::ZeroLoopIterationLimit);
            }
            if carried.is_empty() {
                errors.push(PlanValidationError::MissingLoopCarriedBinding);
            }
            let mut body_inputs = BTreeSet::new();
            let mut results = BTreeSet::new();
            for binding in carried {
                check_value(errors, "loop body input", binding.body_input, value_count);
                check_value(
                    errors,
                    "loop initial source",
                    binding.initial_source,
                    value_count,
                );
                check_value(errors, "loop next source", binding.next_source, value_count);
                check_value(errors, "loop result", binding.result, value_count);
                if !body_inputs.insert(binding.body_input) {
                    errors.push(PlanValidationError::DuplicateLoopBodyInputDestination(
                        binding.body_input,
                    ));
                }
                if !results.insert(binding.result) {
                    errors.push(PlanValidationError::DuplicateLoopResultDestination(
                        binding.result,
                    ));
                }
                let roles = [
                    binding.body_input,
                    binding.initial_source,
                    binding.next_source,
                    binding.result,
                ];
                if roles.iter().copied().collect::<BTreeSet<_>>().len() != roles.len() {
                    errors.push(PlanValidationError::InvalidLoopCarriedRoles(*binding));
                }
                require_control_value_source(
                    errors,
                    "loop body input destination",
                    binding.body_input,
                    value_count,
                    control_value_sources,
                );
                require_control_value_source(
                    errors,
                    "loop result destination",
                    binding.result,
                    value_count,
                    control_value_sources,
                );
            }
        }
        StructuredControlRegion::Call {
            arguments, results, ..
        } => {
            for binding in arguments {
                validate_binding(errors, "call argument", binding, value_count);
            }
            for binding in results {
                validate_binding(errors, "call result", binding, value_count);
            }
        }
    }
}

fn require_control_value_source(
    errors: &mut Vec<PlanValidationError>,
    context: &'static str,
    value: ValueRef,
    value_count: usize,
    control_value_sources: &BTreeSet<ValueRef>,
) {
    if value.index() < value_count && !control_value_sources.contains(&value) {
        errors.push(PlanValidationError::MissingStructuredControlValueSource { context, value });
    }
}

fn validate_binding(
    errors: &mut Vec<PlanValidationError>,
    context: &'static str,
    binding: &RegionValueBinding,
    value_count: usize,
) {
    check_value(errors, context, binding.destination, value_count);
    check_value(errors, context, binding.source, value_count);
}

fn check_operation(
    errors: &mut Vec<PlanValidationError>,
    context: &'static str,
    index: OperationIndex,
    len: usize,
) {
    check_index(errors, context, index.index(), len);
}

fn check_value(
    errors: &mut Vec<PlanValidationError>,
    context: &'static str,
    value: ValueRef,
    len: usize,
) {
    if value.index() >= len {
        errors.push(PlanValidationError::ValueOutOfBounds {
            context,
            value,
            value_count: len,
        });
    }
}

fn check_index(
    errors: &mut Vec<PlanValidationError>,
    context: &'static str,
    index: usize,
    len: usize,
) {
    if index >= len {
        errors.push(PlanValidationError::IndexOutOfBounds {
            context,
            index,
            len,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanValidationErrors(pub Box<[PlanValidationError]>);

impl fmt::Display for PlanValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "execution plan has {} structural error(s)",
            self.0.len()
        )
    }
}

impl std::error::Error for PlanValidationErrors {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanValidationError {
    IndexOutOfBounds {
        context: &'static str,
        index: usize,
        len: usize,
    },
    ValueOutOfBounds {
        context: &'static str,
        value: ValueRef,
        value_count: usize,
    },
    ValueDependencySelfLoop(ValueRef),
    ValueDependencyCycle,
    DuplicateValueSource(ValueRef),
    EffectDependencySelfLoop(OperationIndex),
    EffectDependencyCycle,
    MissingInputSource {
        value: ValueRef,
        operation: OperationIndex,
    },
    DuplicateValueProducer {
        value: ValueRef,
        operation: OperationIndex,
    },
    ZeroLoopIterationLimit,
    DuplicateBranchResultDestination(ValueRef),
    InvalidBranchResultRoles(BranchResultBinding),
    MissingLoopCarriedBinding,
    DuplicateLoopBodyInputDestination(ValueRef),
    DuplicateLoopResultDestination(ValueRef),
    InvalidLoopCarriedRoles(LoopCarriedBinding),
    MissingStructuredControlValueSource {
        context: &'static str,
        value: ValueRef,
    },
    InvalidResultName(Box<str>),
    DuplicateResultName(Box<str>),
    MissingResultSource(ValueRef),
    DuplicateResourceRequirement(ResourceId),
    ConflictingResourceRequirement(ResourceId),
    EmptyRelationalSubplan(RelationalSubplanIndex),
    DuplicateRelationalFragment(RelationalFragmentId),
    RelationalFragmentRootMissing(RelationalFragmentId),
    RelationalFragmentRootUnexpected(RelationalFragmentId),
    RelationalFragmentRootDuplicate(RelationalFragmentId),
    RelationalFragmentOutputUnexpected(RelationalFragmentId),
    RelationalFragmentOutputDuplicate(RelationalFragmentId),
    DuplicateRelationalSubplanOwner {
        subplan: RelationalSubplanIndex,
        first: OperationIndex,
        duplicate: OperationIndex,
    },
    UnownedRelationalSubplan(RelationalSubplanIndex),
    RelationalOwnerOutputRootCardinalityMismatch {
        subplan: RelationalSubplanIndex,
        owner: OperationIndex,
        output_count: usize,
        root_count: usize,
    },
    RelationalPushdownHintsMismatch {
        subplan: RelationalSubplanIndex,
    },
    InvalidRelationalOperatorInput {
        subplan: RelationalSubplanIndex,
        operator: RelationalOperatorIndex,
        input: RelationalOperatorIndex,
    },
    BridgeStoredOnWrongConsumer {
        stored_on: RelationalSubplanIndex,
        consumer: RelationalSubplanIndex,
    },
    BridgeWithinSubplan(RelationalSubplanIndex),
    BridgeFragmentMissing(RelationalFragmentId),
    BridgeProducerOutputNotRequested {
        producer_subplan: RelationalSubplanIndex,
        fragment: RelationalFragmentId,
    },
    RelationalBridgeInputOperatorOutOfBounds {
        subplan: RelationalSubplanIndex,
        operator: RelationalOperatorIndex,
        operator_count: usize,
    },
    RelationalBridgeInputOperatorNotInput {
        subplan: RelationalSubplanIndex,
        operator: RelationalOperatorIndex,
    },
    DuplicateRelationalBridgeInputOperator {
        subplan: RelationalSubplanIndex,
        operator: RelationalOperatorIndex,
    },
    RelationalBridgeInputBridgeUndeclared {
        subplan: RelationalSubplanIndex,
        operator: RelationalOperatorIndex,
        bridge: PlannedMaterializationBridge,
    },
    DuplicateRelationalBridgeInputBridge {
        subplan: RelationalSubplanIndex,
        bridge: PlannedMaterializationBridge,
    },
    RelationalBridgeInputMissing {
        subplan: RelationalSubplanIndex,
        bridge: PlannedMaterializationBridge,
    },
    DuplicateMaterializationBridge,
}
