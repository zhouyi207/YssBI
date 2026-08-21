use super::model::*;
use crate::node_system::protocol::{
    InputConsumption, OutputProduction, TypeCompatibility, type_exprs_compatibility,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[path = "validation/control.rs"]
mod control;
use control::{
    StructuredControlFacts, validate_region, validate_region_availability,
    validate_structured_control_facts, value_source_closure,
};

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
        let mut public_outputs = BTreeSet::new();
        let result_outputs_by_value = self
            .results
            .iter()
            .map(|result| (result.value, &result.output))
            .collect::<BTreeMap<_, _>>();

        for (value, _) in &self.value_contracts {
            check_value(&mut errors, "value contract", *value, value_count);
        }
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
                validate_value_contract(
                    &self.value_contracts,
                    input.value,
                    &input.contract,
                    "operation input",
                    &mut errors,
                );
            }
            for output in &planned.outputs {
                if let Some(public_output) = &output.public_output {
                    if !public_outputs.insert(public_output.clone()) {
                        errors.push(PlanValidationError::DuplicatePublicOutput(
                            public_output.clone(),
                        ));
                    }
                    if public_output.graph_path != self.provenance.graph_path
                        || public_output.port.node_id != planned.source_node_id
                    {
                        errors.push(PlanValidationError::InvalidPublicOutput {
                            operation: OperationIndex::new(operation as u32),
                            output: public_output.clone(),
                        });
                    }
                    if let Some(expected) = result_outputs_by_value.get(&output.value)
                        && public_output != *expected
                    {
                        errors.push(PlanValidationError::PublicOutputResultMismatch {
                            value: output.value,
                            expected: (*expected).clone(),
                            actual: public_output.clone(),
                        });
                    }
                }
                validate_value_contract(
                    &self.value_contracts,
                    output.value,
                    &output.contract,
                    "operation output",
                    &mut errors,
                );
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
        for value in self.bound_values.keys().copied() {
            check_value(&mut errors, "plan bound value", value, value_count);
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
            if let (Some(source), Some(destination)) = (
                self.value_contracts.get(&dependency.source),
                self.value_contracts.get(&dependency.destination),
            ) && !value_contract_is_assignable(source, destination)
            {
                errors.push(PlanValidationError::ValueContractMismatch {
                    context: "value dependency",
                    source: dependency.source,
                    destination: dependency.destination,
                    source_contract: source.clone(),
                    destination_contract: destination.clone(),
                });
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
            .chain(self.bound_values.keys().copied())
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
        let mut region_available = external_inputs
            .iter()
            .copied()
            .chain(self.bound_values.keys().copied())
            .collect();
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

fn value_contract_is_assignable(
    source: &PlannedValueContract,
    destination: &PlannedValueContract,
) -> bool {
    source.kind == destination.kind
        && type_exprs_compatibility(&source.type_expr, &destination.type_expr, &[], &[])
            == TypeCompatibility::Compatible
}

fn validate_value_contract(
    contracts: &BTreeMap<ValueRef, PlannedValueContract>,
    value: ValueRef,
    actual: &PlannedValueContract,
    context: &'static str,
    errors: &mut Vec<PlanValidationError>,
) {
    match contracts.get(&value) {
        Some(expected) if expected != actual => {
            errors.push(PlanValidationError::ValueContractMismatch {
                context,
                source: value,
                destination: value,
                source_contract: expected.clone(),
                destination_contract: actual.clone(),
            })
        }
        None => errors.push(PlanValidationError::MissingValueContract { context, value }),
        Some(_) => {}
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
        let Some((producer, production)) = output_owners.get(&dependency.source) else {
            continue;
        };
        let Some((consumer, consumption)) = input_owners.get(&dependency.destination) else {
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
            && MaterializationAdapterPlan::for_contract(*production, *consumption)
                .adapter
                .is_some()
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
                    let (consumer, consumption) = input_owners.get(&dependency.destination)?;
                    let consumer_operation = &plan.operations[consumer.index()];
                    if !matches!(consumer_operation.kernel, PlannedKernel::Adapter(_)) {
                        return Some(*consumption);
                    }
                    if consumer_operation.outputs.len() != 1 {
                        return None;
                    }
                    let mut adapter_outgoing = plan.value_dependencies.iter().filter(|candidate| {
                        candidate.source == consumer_operation.outputs[0].value
                    });
                    let downstream = adapter_outgoing.next()?;
                    if adapter_outgoing.next().is_some() {
                        return None;
                    }
                    let (downstream_consumer, downstream_consumption) =
                        input_owners.get(&downstream.destination)?;
                    (!matches!(
                        plan.operations[downstream_consumer.index()].kernel,
                        PlannedKernel::Adapter(_)
                    ))
                    .then_some(*downstream_consumption)
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
            let Some(expected_adapter) = expected.adapter else {
                errors.push(PlanValidationError::ExtraMaterializationAdapter {
                    operation: operation_index,
                });
                continue;
            };
            if *actual != expected_adapter {
                errors.push(PlanValidationError::IncompatibleMaterializationAdapter {
                    operation: operation_index,
                    expected: expected_adapter,
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
        let Some(expected_adapter) = expected.adapter else {
            errors.push(PlanValidationError::ExtraMaterializationAdapter {
                operation: operation_index,
            });
            continue;
        };
        if *actual != expected_adapter
            || operation.inputs[0].consumption != expected.input_consumption
            || operation.outputs[0].production != expected.output_production
        {
            errors.push(PlanValidationError::IncompatibleMaterializationAdapter {
                operation: operation_index,
                expected: expected_adapter,
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
    MissingValueContract {
        context: &'static str,
        value: ValueRef,
    },
    ValueContractMismatch {
        context: &'static str,
        source: ValueRef,
        destination: ValueRef,
        source_contract: PlannedValueContract,
        destination_contract: PlannedValueContract,
    },
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
    DuplicatePublicOutput(GraphOutputRef),
    InvalidPublicOutput {
        operation: OperationIndex,
        output: GraphOutputRef,
    },
    PublicOutputResultMismatch {
        value: ValueRef,
        expected: GraphOutputRef,
        actual: GraphOutputRef,
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
