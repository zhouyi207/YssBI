use super::model::*;
use crate::node_system::protocol::{InputConsumption, OutputProduction};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone)]
pub(crate) struct PlanSourceFacts {
    external_inputs: BTreeSet<ValueRef>,
    statically_sourced: Box<[bool]>,
    productions: BTreeMap<ValueRef, OutputProduction>,
}

impl PlanSourceFacts {
    pub(crate) fn is_external_input(&self, value: ValueRef) -> bool {
        self.external_inputs.contains(&value)
    }

    pub(crate) fn is_statically_sourced(&self, value: ValueRef) -> bool {
        self.statically_sourced
            .get(value.index())
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn production(&self, value: ValueRef) -> Option<OutputProduction> {
        self.productions.get(&value).copied()
    }
}

impl ExecutionPlan {
    pub fn validate(&self) -> Result<(), PlanValidationErrors> {
        self.validate_with_source_facts().map(|_| ())
    }

    pub(crate) fn validate_with_source_facts(
        &self,
    ) -> Result<PlanSourceFacts, PlanValidationErrors> {
        let mut errors = Vec::new();
        let operation_count = self.operations.len();
        let relational_count = self.relational_subplans.len();
        let value_count = self.value_count as usize;
        let mut produced_values = BTreeSet::new();
        let mut value_producers = vec![BTreeSet::new(); value_count];
        let mut relational_owners = BTreeMap::new();
        let mut stable_operation_ids = BTreeMap::new();

        for (operation, planned) in self.operations.iter().enumerate() {
            let operation = OperationIndex::new(operation as u32);
            if let Some(first) = stable_operation_ids.insert(planned.stable_id.clone(), operation) {
                errors.push(PlanValidationError::DuplicateOperationStableId {
                    stable_id: planned.stable_id.clone(),
                    first,
                    duplicate: operation,
                });
            }
            let retry_policy = planned.retry.policy.as_ref();
            let retry_has_effect_edge = self
                .effect_dependencies
                .iter()
                .any(|edge| edge.before == operation || edge.after == operation);
            if planned.retry.idempotent != retry_policy.is_some()
                || retry_policy.is_some_and(|policy| policy.validate().is_err())
                || retry_policy.is_some()
                    && (!matches!(planned.kernel, PlannedKernel::Native(_))
                        || planned.workload != WorkloadClass::Cpu
                        || !planned.resource_dependencies.is_empty()
                        || retry_has_effect_edge)
            {
                errors.push(PlanValidationError::InvalidRetryPolicy { operation });
            }
            let operation = operation.index();
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
        let mut external_inputs = BTreeSet::new();
        let mut control_value_sources = BTreeSet::new();
        for source in &self.value_sources {
            let value = source.value();
            check_value(&mut errors, "plan value source", value, value_count);
            if value.index() < value_count && !declared_sources.insert(value) {
                errors.push(PlanValidationError::DuplicateValueSource(value));
            }
            match source {
                PlanValueSource::ExternalInput(value, _) => {
                    external_inputs.insert(*value);
                }
                PlanValueSource::ControlProduced(value, _) => {
                    control_value_sources.insert(*value);
                }
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
        validate_materialization_adapters(self, &mut errors);

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

        let declared_control_productions = self
            .value_sources
            .iter()
            .filter_map(|source| match source {
                PlanValueSource::ControlProduced(value, production) => Some((*value, *production)),
                PlanValueSource::ExternalInput(_, _) => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut structured_productions = self
            .operations
            .iter()
            .flat_map(|operation| {
                operation
                    .outputs
                    .iter()
                    .map(|output| (output.value, output.production))
            })
            .chain(self.value_sources.iter().filter_map(|source| match source {
                PlanValueSource::ExternalInput(value, production) => Some((*value, *production)),
                PlanValueSource::ControlProduced(_, _) => None,
            }))
            .collect::<BTreeMap<_, _>>();
        let mut aliases_by_destination = BTreeMap::<ValueRef, Vec<ValueRef>>::new();
        for dependency in &self.value_dependencies {
            aliases_by_destination
                .entry(dependency.destination)
                .or_default()
                .push(dependency.source);
        }
        for (destination, sources) in &mut aliases_by_destination {
            sources.sort_unstable();
            if sources.len() > 1 {
                errors.push(PlanValidationError::DuplicateValueDependencyAlias {
                    destination: *destination,
                    sources: sources.clone().into_boxed_slice(),
                });
            }
        }
        let mut reported_alias_conflicts = BTreeSet::new();
        propagate_alias_productions(
            &aliases_by_destination,
            &mut structured_productions,
            &mut reported_alias_conflicts,
            &mut errors,
        );
        let mut structured = StructuredControlFacts::default();
        validate_region(
            &self.root_region,
            &mut errors,
            operation_count,
            value_count,
            &mut structured,
            &mut structured_productions,
            &declared_control_productions,
        );
        propagate_alias_productions(
            &aliases_by_destination,
            &mut structured_productions,
            &mut reported_alias_conflicts,
            &mut errors,
        );
        let source_roots = produced_values
            .iter()
            .chain(&external_inputs)
            .copied()
            .chain(structured.producers.keys().copied())
            .collect::<BTreeSet<_>>();
        let source_facts = PlanSourceFacts {
            external_inputs: external_inputs.clone(),
            statically_sourced: value_source_closure(
                value_count,
                &source_roots,
                &self.value_dependencies,
            )
            .into_boxed_slice(),
            productions: structured_productions.clone(),
        };
        validate_structured_control_facts(
            &structured,
            &produced_values,
            &external_inputs,
            &control_value_sources,
            &source_facts,
            &mut errors,
        );
        let mut region_available = external_inputs.clone();
        validate_region_availability(&self.root_region, self, &mut region_available, &mut errors);
        validate_relational_subplans(self, &mut errors);
        validate_resources(self, &mut errors);

        let mut result_names = BTreeSet::new();
        let mut result_outputs = BTreeSet::new();
        for result in &self.results {
            check_value(&mut errors, "plan result", result.value, value_count);
            if !result_outputs.insert(result.output.clone()) {
                errors.push(PlanValidationError::DuplicateResultOutput(
                    result.output.clone(),
                ));
            }
            if result.name.is_empty() || result.name.trim() != result.name.as_ref() {
                errors.push(PlanValidationError::InvalidResultName(result.name.clone()));
            } else if !result_names.insert(result.name.clone()) {
                errors.push(PlanValidationError::DuplicateResultName(
                    result.name.clone(),
                ));
            }
            if result.value.index() < value_count
                && (!sourced_values[result.value.index()]
                    || !region_available.contains(&result.value))
            {
                errors.push(PlanValidationError::MissingResultSource(result.value));
            }
        }
        validate_publications(
            self,
            value_count,
            &sourced_values,
            &region_available,
            &mut errors,
        );

        if errors.is_empty() {
            Ok(source_facts)
        } else {
            Err(PlanValidationErrors(errors.into_boxed_slice()))
        }
    }
}

fn propagate_alias_productions(
    aliases_by_destination: &BTreeMap<ValueRef, Vec<ValueRef>>,
    productions: &mut BTreeMap<ValueRef, OutputProduction>,
    reported_conflicts: &mut BTreeSet<ValueRef>,
    errors: &mut Vec<PlanValidationError>,
) {
    loop {
        let mut changed = false;
        for (destination, sources) in aliases_by_destination {
            let mut actual = sources
                .iter()
                .filter_map(|source| productions.get(source).copied())
                .collect::<BTreeSet<_>>();
            if let Some(production) = productions.get(destination).copied() {
                actual.insert(production);
            }
            if actual.len() > 1 {
                if reported_conflicts.insert(*destination) {
                    errors.push(PlanValidationError::ConflictingAliasProductions {
                        destination: *destination,
                        productions: actual.into_iter().collect(),
                    });
                }
                continue;
            }
            let Some(production) = actual.first().copied() else {
                continue;
            };
            if !productions.contains_key(destination) {
                productions.insert(*destination, production);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn validate_relational_subplans(plan: &ExecutionPlan, errors: &mut Vec<PlanValidationError>) {
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
        for root in &compiled.roots {
            check_index(errors, "relational root", root.index(), operator_count);
        }
        let inferred_pushdown_hints =
            infer_relational_pushdown_hints(&compiled.operators, &compiled.roots);
        if compiled.pushdown_hints.as_ref() != inferred_pushdown_hints.as_slice() {
            errors.push(PlanValidationError::RelationalPushdownHintsMismatch {
                subplan: RelationalSubplanIndex::new(subplan_index as u32),
            });
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

fn validate_materialization_adapters(plan: &ExecutionPlan, errors: &mut Vec<PlanValidationError>) {
    let mut output_owners = plan
        .operations
        .iter()
        .enumerate()
        .flat_map(|(index, operation)| {
            operation.outputs.iter().map(move |output| {
                (
                    output.value,
                    (Some(OperationIndex::new(index as u32)), output.production),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    for source in &plan.value_sources {
        output_owners.insert(source.value(), (None, source.production()));
    }
    let input_owners = plan
        .operations
        .iter()
        .enumerate()
        .flat_map(|(index, operation)| {
            operation.inputs.iter().map(move |input| {
                (
                    input.value,
                    (OperationIndex::new(index as u32), input.consumption),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();

    for dependency in &plan.value_dependencies {
        let Some((producer, _)) = output_owners.get(&dependency.source) else {
            continue;
        };
        let Some((consumer, _)) = input_owners.get(&dependency.destination) else {
            continue;
        };
        let producer_is_adapter = producer.is_some_and(|producer| {
            matches!(
                plan.operations[producer.index()].kernel,
                PlannedKernel::Adapter(_)
            )
        });
        if !producer_is_adapter
            && !matches!(
                plan.operations[consumer.index()].kernel,
                PlannedKernel::Adapter(_)
            )
        {
            errors.push(PlanValidationError::MissingMaterializationAdapter {
                source: dependency.source,
                destination: dependency.destination,
            });
        }
    }

    for (index, operation) in plan.operations.iter().enumerate() {
        let PlannedKernel::Adapter(actual) = &operation.kernel else {
            continue;
        };
        let operation_index = OperationIndex::new(index as u32);
        if operation.inputs.len() != 1 || operation.outputs.len() != 1 {
            errors.push(PlanValidationError::ExtraMaterializationAdapter {
                operation: operation_index,
            });
            continue;
        }
        if operation.workload != WorkloadClass::AdapterIo
            || operation.cache_policy != crate::node_system::protocol::CachePolicy::Disabled
            || operation.retry != PlannedRetry::default()
            || !operation.resource_dependencies.is_empty()
        {
            errors.push(
                PlanValidationError::InvalidMaterializationAdapterSemantics {
                    operation: operation_index,
                },
            );
        }
        let incoming = plan
            .value_dependencies
            .iter()
            .filter(|dependency| dependency.destination == operation.inputs[0].value)
            .collect::<Vec<_>>();
        let outgoing = plan
            .value_dependencies
            .iter()
            .filter(|dependency| dependency.source == operation.outputs[0].value)
            .collect::<Vec<_>>();
        if incoming.len() != 1 || outgoing.is_empty() {
            errors.push(PlanValidationError::ExtraMaterializationAdapter {
                operation: operation_index,
            });
            continue;
        }
        let Some((producer, production)) = output_owners.get(&incoming[0].source) else {
            errors.push(PlanValidationError::ExtraMaterializationAdapter {
                operation: operation_index,
            });
            continue;
        };
        if outgoing.len() > 1 {
            let downstream_consumptions = outgoing
                .iter()
                .filter_map(|dependency| {
                    let (adapter, _) = input_owners.get(&dependency.destination)?;
                    let adapter_operation = &plan.operations[adapter.index()];
                    if !matches!(adapter_operation.kernel, PlannedKernel::Adapter(_))
                        || adapter_operation.outputs.len() != 1
                    {
                        return None;
                    }
                    let mut adapter_outgoing = plan
                        .value_dependencies
                        .iter()
                        .filter(|candidate| candidate.source == adapter_operation.outputs[0].value);
                    let downstream = adapter_outgoing.next()?;
                    if adapter_outgoing.next().is_some() {
                        return None;
                    }
                    let (consumer, consumption) = input_owners.get(&downstream.destination)?;
                    (!matches!(
                        plan.operations[consumer.index()].kernel,
                        PlannedKernel::Adapter(_)
                    ))
                    .then_some(*consumption)
                })
                .collect::<Vec<_>>();
            if downstream_consumptions.len() != outgoing.len() {
                errors.push(PlanValidationError::ExtraMaterializationAdapter {
                    operation: operation_index,
                });
                continue;
            }
            let shared_consumption = if downstream_consumptions.iter().any(|consumption| {
                matches!(
                    consumption,
                    InputConsumption::RandomAccess | InputConsumption::FullyMaterialized
                )
            }) {
                InputConsumption::FullyMaterialized
            } else {
                InputConsumption::RewindableBatches
            };
            let expected =
                MaterializationAdapterPlan::for_contract(*production, shared_consumption);
            if *actual != expected.adapter {
                errors.push(PlanValidationError::IncompatibleMaterializationAdapter {
                    operation: operation_index,
                    expected: expected.adapter,
                    actual: actual.clone(),
                });
            }
            if operation.inputs[0].consumption != expected.input_consumption
                || operation.outputs[0].production != expected.output_production
            {
                errors.push(
                    PlanValidationError::InvalidMaterializationAdapterSemantics {
                        operation: operation_index,
                    },
                );
            }
            continue;
        }
        let Some((consumer, consumption)) = input_owners.get(&outgoing[0].destination) else {
            errors.push(PlanValidationError::ExtraMaterializationAdapter {
                operation: operation_index,
            });
            continue;
        };
        if matches!(
            plan.operations[consumer.index()].kernel,
            PlannedKernel::Adapter(_)
        ) {
            errors.push(PlanValidationError::ExtraMaterializationAdapter {
                operation: operation_index,
            });
            continue;
        }
        if producer.is_some_and(|producer| {
            matches!(
                plan.operations[producer.index()].kernel,
                PlannedKernel::Adapter(_)
            )
        }) {
            let producer_outgoing = plan
                .value_dependencies
                .iter()
                .filter(|dependency| dependency.source == incoming[0].source)
                .count();
            if producer_outgoing < 2 {
                errors.push(PlanValidationError::ExtraMaterializationAdapter {
                    operation: operation_index,
                });
                continue;
            }
        }
        let expected = MaterializationAdapterPlan::for_contract(*production, *consumption);
        if *actual != expected.adapter
            || operation.inputs[0].consumption != expected.input_consumption
            || operation.outputs[0].production != expected.output_production
        {
            errors.push(PlanValidationError::IncompatibleMaterializationAdapter {
                operation: operation_index,
                expected: expected.adapter,
                actual: actual.clone(),
            });
        }
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
            if input.bound_value.is_some() {
                continue;
            }
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

#[derive(Default)]
struct StructuredControlFacts {
    producers: BTreeMap<ValueRef, &'static str>,
    source_requirements: Vec<(&'static str, ValueRef)>,
}

impl StructuredControlFacts {
    fn producer(
        &mut self,
        errors: &mut Vec<PlanValidationError>,
        value: ValueRef,
        value_count: usize,
        kind: &'static str,
    ) {
        if value.index() >= value_count {
            return;
        }
        if let Some(first) = self.producers.get(&value).copied() {
            errors.push(PlanValidationError::DuplicateStructuredControlProducer {
                value,
                first,
                duplicate: kind,
            });
        } else {
            self.producers.insert(value, kind);
        }
    }

    fn source(&mut self, context: &'static str, value: ValueRef) {
        self.source_requirements.push((context, value));
    }
}

fn validate_structured_control_facts(
    facts: &StructuredControlFacts,
    operation_outputs: &BTreeSet<ValueRef>,
    external_inputs: &BTreeSet<ValueRef>,
    control_declarations: &BTreeSet<ValueRef>,
    source_facts: &PlanSourceFacts,
    errors: &mut Vec<PlanValidationError>,
) {
    let value_count = source_facts.statically_sourced.len();
    for &value in control_declarations {
        if value.index() >= value_count {
            continue;
        }
        if operation_outputs.contains(&value) {
            errors.push(PlanValidationError::ControlProducedConflictsWithOperationOutput(value));
        }
        if external_inputs.contains(&value) {
            errors.push(PlanValidationError::ControlProducedConflictsWithExternalInput(value));
        }
        if !facts.producers.contains_key(&value) {
            errors.push(PlanValidationError::OrphanControlProduced(value));
        }
    }
    for (&value, &producer) in &facts.producers {
        if !control_declarations.contains(&value) {
            errors.push(PlanValidationError::MissingControlProducedDeclaration { value, producer });
        }
    }

    for &(context, value) in &facts.source_requirements {
        if value.index() < value_count && !source_facts.is_statically_sourced(value) {
            errors.push(PlanValidationError::MissingStructuredBindingSource { context, value });
        }
    }
}

fn validate_region_availability(
    region: &StructuredControlRegion,
    plan: &ExecutionPlan,
    available: &mut BTreeSet<ValueRef>,
    errors: &mut Vec<PlanValidationError>,
) {
    extend_available_dependencies(plan, available);
    match region {
        StructuredControlRegion::Sequence(steps) => {
            let mut pending = Vec::new();
            for step in steps {
                match step {
                    ControlStep::Operation(operation) => pending.push(*operation),
                    ControlStep::Region(child) => {
                        validate_operation_block(&pending, plan, available, errors);
                        pending.clear();
                        validate_region_availability(child, plan, available, errors);
                    }
                }
            }
            validate_operation_block(&pending, plan, available, errors);
        }
        StructuredControlRegion::If {
            condition,
            then_region,
            else_region,
            results,
        } => {
            require_available("If condition", *condition, available, errors);
            let incoming = available.clone();
            let mut then_available = incoming.clone();
            let mut else_available = incoming;
            validate_region_availability(then_region, plan, &mut then_available, errors);
            validate_region_availability(else_region, plan, &mut else_available, errors);
            available
                .retain(|value| then_available.contains(value) && else_available.contains(value));
            for binding in results {
                let then_ready = require_available(
                    "If then result",
                    binding.then_source,
                    &then_available,
                    errors,
                );
                let else_ready = require_available(
                    "If else result",
                    binding.else_source,
                    &else_available,
                    errors,
                );
                if then_ready && else_ready {
                    available.insert(binding.destination);
                }
            }
            extend_available_dependencies(plan, available);
        }
        StructuredControlRegion::Loop {
            body,
            carried,
            continue_condition,
            ..
        } => {
            let mut body_available = available.clone();
            for binding in carried {
                if require_available(
                    "Loop initial source",
                    binding.initial_source,
                    available,
                    errors,
                ) {
                    body_available.insert(binding.body_input);
                }
            }
            validate_region_availability(body, plan, &mut body_available, errors);
            require_available(
                "Loop continue condition",
                *continue_condition,
                &body_available,
                errors,
            );
            for binding in carried {
                if require_available(
                    "Loop next source",
                    binding.next_source,
                    &body_available,
                    errors,
                ) {
                    available.insert(binding.result);
                }
            }
            extend_available_dependencies(plan, available);
        }
        StructuredControlRegion::Call {
            arguments, results, ..
        } => {
            for binding in arguments {
                require_available(
                    "Call caller argument",
                    binding.caller_source,
                    available,
                    errors,
                );
            }
            available.extend(results.iter().map(|binding| binding.caller_destination));
            extend_available_dependencies(plan, available);
        }
    }
}

fn validate_operation_block(
    operations: &[OperationIndex],
    plan: &ExecutionPlan,
    available: &mut BTreeSet<ValueRef>,
    errors: &mut Vec<PlanValidationError>,
) {
    let mut pending = operations
        .iter()
        .copied()
        .filter(|operation| operation.index() < plan.operations.len())
        .collect::<BTreeSet<_>>();
    loop {
        extend_available_dependencies(plan, available);
        let ready =
            pending
                .iter()
                .copied()
                .filter(|operation| {
                    let operation = &plan.operations[operation.index()];
                    operation.inputs.iter().all(|input| {
                        input.bound_value.is_some() || available.contains(&input.value)
                    }) && operation.outputs.iter().all(|output| {
                        plan.value_dependencies
                            .iter()
                            .filter(|dependency| dependency.destination == output.value)
                            .all(|dependency| available.contains(&dependency.source))
                    })
                })
                .collect::<Vec<_>>();
        if ready.is_empty() {
            break;
        }
        for operation in ready {
            pending.remove(&operation);
            available.extend(
                plan.operations[operation.index()]
                    .outputs
                    .iter()
                    .map(|output| output.value),
            );
        }
    }
    for operation in pending {
        let operation = &plan.operations[operation.index()];
        for input in &operation.inputs {
            if input.bound_value.is_some() || available.contains(&input.value) {
                continue;
            }
            if matches!(operation.kernel, PlannedKernel::Adapter(_)) {
                errors.push(
                    PlanValidationError::MaterializationAdapterSourceUnavailable {
                        operation: operation.stable_id.clone(),
                        value: input.value,
                    },
                );
            } else {
                require_available("operation input", input.value, available, errors);
            }
        }
        for output in &operation.outputs {
            for dependency in plan
                .value_dependencies
                .iter()
                .filter(|dependency| dependency.destination == output.value)
            {
                require_available(
                    "operation value dependency",
                    dependency.source,
                    available,
                    errors,
                );
            }
        }
    }
}

fn extend_available_dependencies(plan: &ExecutionPlan, available: &mut BTreeSet<ValueRef>) {
    let operation_outputs = plan
        .operations
        .iter()
        .flat_map(|operation| operation.outputs.iter().map(|output| output.value))
        .collect::<BTreeSet<_>>();
    loop {
        let derived = plan
            .value_dependencies
            .iter()
            .filter(|dependency| {
                available.contains(&dependency.source)
                    && !operation_outputs.contains(&dependency.destination)
                    && !available.contains(&dependency.destination)
            })
            .map(|dependency| dependency.destination)
            .collect::<Vec<_>>();
        if derived.is_empty() {
            break;
        }
        available.extend(derived);
    }
}

fn require_available(
    context: &'static str,
    value: ValueRef,
    available: &BTreeSet<ValueRef>,
    errors: &mut Vec<PlanValidationError>,
) -> bool {
    if available.contains(&value) {
        true
    } else {
        errors.push(PlanValidationError::MissingStructuredBindingSource { context, value });
        false
    }
}

fn value_source_closure(
    value_count: usize,
    roots: &BTreeSet<ValueRef>,
    dependencies: &[ValueDependency],
) -> Vec<bool> {
    let mut sourced = (0..value_count)
        .map(|value| roots.contains(&ValueRef::new(value as u32)))
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for dependency in dependencies {
            let source = dependency.source.index();
            let destination = dependency.destination.index();
            if source < value_count
                && destination < value_count
                && sourced[source]
                && !sourced[destination]
            {
                sourced[destination] = true;
                changed = true;
            }
        }
        if !changed {
            return sourced;
        }
    }
}

fn validate_region(
    region: &StructuredControlRegion,
    errors: &mut Vec<PlanValidationError>,
    operation_count: usize,
    value_count: usize,
    facts: &mut StructuredControlFacts,
    productions: &mut BTreeMap<ValueRef, OutputProduction>,
    declared_control_productions: &BTreeMap<ValueRef, OutputProduction>,
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
                        facts,
                        productions,
                        declared_control_productions,
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
                facts,
                productions,
                declared_control_productions,
            );
            validate_region(
                else_region,
                errors,
                operation_count,
                value_count,
                facts,
                productions,
                declared_control_productions,
            );
            facts.source("branch condition", *condition);
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
                validate_merged_structured_production(
                    errors,
                    productions,
                    declared_control_productions,
                    "branch result",
                    binding.destination,
                    binding.production,
                    [binding.then_source, binding.else_source],
                );
                facts.producer(errors, binding.destination, value_count, "branch result");
                facts.source("branch then source", binding.then_source);
                facts.source("branch else source", binding.else_source);
            }
        }
        StructuredControlRegion::Loop {
            body,
            carried,
            continue_condition,
            max_iterations,
        } => {
            for binding in carried {
                validate_declared_structured_production(
                    errors,
                    productions,
                    declared_control_productions,
                    "loop body input",
                    binding.body_input,
                    binding.production,
                    binding.initial_source,
                );
            }
            validate_region(
                body,
                errors,
                operation_count,
                value_count,
                facts,
                productions,
                declared_control_productions,
            );
            check_value(errors, "loop condition", *continue_condition, value_count);
            facts.source("loop condition", *continue_condition);
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
                validate_merged_structured_production(
                    errors,
                    productions,
                    declared_control_productions,
                    "loop result",
                    binding.result,
                    binding.production,
                    [binding.initial_source, binding.next_source],
                );
                facts.producer(errors, binding.body_input, value_count, "loop body input");
                facts.producer(errors, binding.result, value_count, "loop result");
                facts.source("loop initial source", binding.initial_source);
                facts.source("loop next source", binding.next_source);
            }
        }
        StructuredControlRegion::Call {
            arguments, results, ..
        } => {
            for binding in arguments {
                check_value(
                    errors,
                    "call argument source",
                    binding.caller_source,
                    value_count,
                );
                facts.source("call argument source", binding.caller_source);
            }
            for binding in results {
                check_value(
                    errors,
                    "call result destination",
                    binding.caller_destination,
                    value_count,
                );
                validate_call_structured_production(
                    errors,
                    productions,
                    declared_control_productions,
                    binding,
                );
                facts.producer(
                    errors,
                    binding.caller_destination,
                    value_count,
                    "call result",
                );
            }
        }
    }
}

fn validate_declared_structured_production(
    errors: &mut Vec<PlanValidationError>,
    productions: &mut BTreeMap<ValueRef, OutputProduction>,
    declarations: &BTreeMap<ValueRef, OutputProduction>,
    producer: &'static str,
    destination: ValueRef,
    declared: Option<OutputProduction>,
    source: ValueRef,
) {
    let Some(declared) = declared else {
        errors.push(PlanValidationError::MissingStructuredProductionFact {
            producer,
            value: destination,
        });
        return;
    };
    let Some(actual) = productions.get(&source).copied() else {
        errors.push(PlanValidationError::MissingStructuredProductionFact {
            producer,
            value: source,
        });
        return;
    };
    if !declarations.contains_key(&destination) {
        errors.push(PlanValidationError::MissingStructuredProductionFact {
            producer,
            value: destination,
        });
        return;
    }
    if actual != declared || declarations.get(&destination).copied() != Some(actual) {
        errors.push(PlanValidationError::StructuredProductionMismatch {
            producer,
            value: destination,
            expected: actual,
            actual: declared,
        });
        return;
    }
    productions.insert(destination, actual);
}

fn validate_merged_structured_production<const N: usize>(
    errors: &mut Vec<PlanValidationError>,
    productions: &mut BTreeMap<ValueRef, OutputProduction>,
    declarations: &BTreeMap<ValueRef, OutputProduction>,
    producer: &'static str,
    destination: ValueRef,
    declared: Option<OutputProduction>,
    sources: [ValueRef; N],
) {
    let Some(declared) = declared else {
        errors.push(PlanValidationError::MissingStructuredProductionFact {
            producer,
            value: destination,
        });
        return;
    };
    let actual = sources
        .iter()
        .filter_map(|source| productions.get(source).copied())
        .collect::<BTreeSet<_>>();
    if let Some(value) = sources
        .iter()
        .copied()
        .find(|source| !productions.contains_key(source))
    {
        errors.push(PlanValidationError::MissingStructuredProductionFact { producer, value });
        return;
    }
    if actual.len() != 1 {
        errors.push(PlanValidationError::ConflictingStructuredProductions {
            producer,
            value: destination,
            productions: actual.into_iter().collect(),
        });
        return;
    }
    let actual = *actual.first().expect("one production");
    if !declarations.contains_key(&destination) {
        errors.push(PlanValidationError::MissingStructuredProductionFact {
            producer,
            value: destination,
        });
        return;
    }
    if actual != declared || declarations.get(&destination).copied() != Some(actual) {
        errors.push(PlanValidationError::StructuredProductionMismatch {
            producer,
            value: destination,
            expected: actual,
            actual: declared,
        });
        return;
    }
    productions.insert(destination, actual);
}

fn validate_call_structured_production(
    errors: &mut Vec<PlanValidationError>,
    productions: &mut BTreeMap<ValueRef, OutputProduction>,
    declarations: &BTreeMap<ValueRef, OutputProduction>,
    binding: &CallResultBinding,
) {
    let Some(actual) = binding.production else {
        errors.push(PlanValidationError::MissingStructuredProductionFact {
            producer: "call result",
            value: binding.caller_destination,
        });
        return;
    };
    let Some(expected) = declarations.get(&binding.caller_destination).copied() else {
        errors.push(PlanValidationError::MissingStructuredProductionFact {
            producer: "call result",
            value: binding.caller_destination,
        });
        return;
    };
    if expected != actual {
        errors.push(PlanValidationError::StructuredProductionMismatch {
            producer: "call result",
            value: binding.caller_destination,
            expected,
            actual,
        });
        return;
    }
    productions.insert(binding.caller_destination, actual);
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

fn validate_publications(
    plan: &ExecutionPlan,
    value_count: usize,
    sourced_values: &[bool],
    region_available: &BTreeSet<ValueRef>,
    errors: &mut Vec<PlanValidationError>,
) {
    if plan.publications.is_empty() {
        if !plan.results.is_empty() {
            errors.push(PlanValidationError::GraphPublicationCountMismatch {
                publications: 0,
                results: plan.results.len(),
            });
        }
        return;
    }

    let mut graph_results = 0;
    let mut previews = 0;
    let mut published_outputs = BTreeSet::new();
    let mut published_results = BTreeSet::new();

    for publication in &plan.publications {
        let value = publication.value();
        check_value(errors, "plan publication", value, value_count);
        if value.index() < value_count
            && (!sourced_values[value.index()] || !region_available.contains(&value))
        {
            errors.push(PlanValidationError::MissingPublicationSource(value));
        }

        let (output, matching_result) = match publication {
            PlannedPublication::GraphResult {
                name,
                output,
                value,
            } => {
                graph_results += 1;
                let matching = plan.results.iter().position(|result| {
                    result.name == *name && result.output == *output && result.value == *value
                });
                (output, matching)
            }
            PlannedPublication::PinPreview {
                output,
                generation,
                value,
            } => {
                previews += 1;
                if *generation > MAX_SAFE_PREVIEW_GENERATION {
                    errors.push(PlanValidationError::PreviewGenerationOutOfRange(
                        *generation,
                    ));
                }
                let matching = plan
                    .results
                    .iter()
                    .position(|result| result.output == *output && result.value == *value);
                (output, matching)
            }
        };

        if !published_outputs.insert(output.clone()) {
            errors.push(PlanValidationError::DuplicatePublicationOutput(
                output.clone(),
            ));
        }
        match matching_result {
            Some(index) if published_results.insert(index) => {}
            Some(index) => errors.push(PlanValidationError::DuplicatePublicationResult(index)),
            None => errors.push(PlanValidationError::PublicationResultMismatch),
        }
    }

    if graph_results > 0 && previews > 0 {
        errors.push(PlanValidationError::MixedPublicationModes);
    }
    if graph_results > 0 && graph_results != plan.results.len() {
        errors.push(PlanValidationError::GraphPublicationCountMismatch {
            publications: graph_results,
            results: plan.results.len(),
        });
    }
    if previews > 0 && (previews != 1 || plan.results.len() != 1) {
        errors.push(PlanValidationError::InvalidPreviewPublicationMode {
            publications: previews,
            results: plan.results.len(),
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
            "execution plan has {} structural error(s): {:?}",
            self.0.len(),
            self.0
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
    DuplicateOperationStableId {
        stable_id: OperationStableId,
        first: OperationIndex,
        duplicate: OperationIndex,
    },
    InvalidRetryPolicy {
        operation: OperationIndex,
    },
    ValueDependencySelfLoop(ValueRef),
    ValueDependencyCycle,
    DuplicateValueDependencyAlias {
        destination: ValueRef,
        sources: Box<[ValueRef]>,
    },
    ConflictingAliasProductions {
        destination: ValueRef,
        productions: Box<[OutputProduction]>,
    },
    MissingMaterializationAdapter {
        source: ValueRef,
        destination: ValueRef,
    },
    ExtraMaterializationAdapter {
        operation: OperationIndex,
    },
    IncompatibleMaterializationAdapter {
        operation: OperationIndex,
        expected: PlannedAdapter,
        actual: PlannedAdapter,
    },
    InvalidMaterializationAdapterSemantics {
        operation: OperationIndex,
    },
    MaterializationAdapterSourceUnavailable {
        operation: OperationStableId,
        value: ValueRef,
    },
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
    DuplicateStructuredControlProducer {
        value: ValueRef,
        first: &'static str,
        duplicate: &'static str,
    },
    OrphanControlProduced(ValueRef),
    MissingControlProducedDeclaration {
        value: ValueRef,
        producer: &'static str,
    },
    ControlProducedConflictsWithOperationOutput(ValueRef),
    ControlProducedConflictsWithExternalInput(ValueRef),
    MissingStructuredBindingSource {
        context: &'static str,
        value: ValueRef,
    },
    MissingStructuredProductionFact {
        producer: &'static str,
        value: ValueRef,
    },
    ConflictingStructuredProductions {
        producer: &'static str,
        value: ValueRef,
        productions: Box<[OutputProduction]>,
    },
    StructuredProductionMismatch {
        producer: &'static str,
        value: ValueRef,
        expected: OutputProduction,
        actual: OutputProduction,
    },
    InvalidResultName(Box<str>),
    DuplicateResultName(Box<str>),
    DuplicateResultOutput(GraphOutputRef),
    MissingResultSource(ValueRef),
    MissingPublicationSource(ValueRef),
    PublicationResultMismatch,
    DuplicatePublicationOutput(GraphOutputRef),
    DuplicatePublicationResult(usize),
    MixedPublicationModes,
    GraphPublicationCountMismatch {
        publications: usize,
        results: usize,
    },
    InvalidPreviewPublicationMode {
        publications: usize,
        results: usize,
    },
    PreviewGenerationOutOfRange(u64),
    DuplicateResourceRequirement(ResourceId),
    ConflictingResourceRequirement(ResourceId),
    EmptyRelationalSubplan(RelationalSubplanIndex),
    DuplicateRelationalFragment(RelationalFragmentId),
    RelationalFragmentRootMissing(RelationalFragmentId),
    RelationalFragmentRootUnexpected(RelationalFragmentId),
    RelationalFragmentRootDuplicate(RelationalFragmentId),
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
}
