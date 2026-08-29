use super::*;

#[derive(Default)]
pub(super) struct StructuredControlFacts {
    pub(super) producers: BTreeMap<ValueRef, &'static str>,
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

pub(super) fn validate_structured_control_facts(
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

pub(super) fn validate_region_availability(
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

pub(super) fn value_source_closure(
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

pub(super) fn validate_region(
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
