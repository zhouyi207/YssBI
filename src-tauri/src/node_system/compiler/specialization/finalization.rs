use super::*;
use crate::node_system::plan::infer_relational_pushdown_hints;

impl ExecutionPlanBasis {
    pub(super) fn finalize(
        &self,
        retained: &BTreeSet<usize>,
        selected_outputs: &BTreeSet<GraphOutputRef>,
        retained_region: &StructuredControlRegion,
        required_values: Option<&BTreeSet<ValueRef>>,
    ) -> Result<ExecutionPlan, DemandPlanError> {
        let selected_results = selected_outputs
            .iter()
            .map(|output| self.output_results[output].clone())
            .collect::<Vec<_>>();
        let result_values = selected_results
            .iter()
            .map(|result| result.value)
            .collect::<BTreeSet<_>>();
        let relational = self.plan_relational(retained)?;
        let ownership = self.value_ownership();
        let mut internal_inputs = BTreeSet::new();
        let mut internal_outputs = BTreeSet::new();
        let mut external_outputs = BTreeSet::new();
        let mut internal_dependencies = BTreeSet::new();
        for dependency in &self.value_dependencies {
            let producer = ownership.producer.get(&dependency.source).copied();
            let consumer = ownership.consumer.get(&dependency.destination).copied();
            let producer_subplan =
                producer.and_then(|index| relational.subplan_by_operation.get(&index));
            let consumer_subplan =
                consumer.and_then(|index| relational.subplan_by_operation.get(&index));
            if producer_subplan.is_some() && producer_subplan == consumer_subplan {
                internal_inputs.insert(dependency.destination);
                internal_outputs.insert(dependency.source);
                internal_dependencies.insert((dependency.source, dependency.destination));
            } else if producer_subplan.is_some() {
                external_outputs.insert(dependency.source);
            }
        }

        let mut groups = BTreeMap::<RelationalSubplanIndex, Vec<usize>>::new();
        let mut group_order = Vec::new();
        for index in retained {
            match &self.operations[*index].kernel {
                IntermediateKernel::Native(_) => group_order.push(OperationGroup::Native(*index)),
                IntermediateKernel::Relational { fragment, .. } => {
                    let subplan = relational.subplan_by_fragment[&fragment.id];
                    if !groups.contains_key(&subplan) {
                        group_order.push(OperationGroup::Relational(subplan));
                    }
                    groups.entry(subplan).or_default().push(*index);
                }
            }
        }

        let mut relational_subplans = relational.subplans;
        let mut operations = Vec::with_capacity(group_order.len());
        let mut dense_by_intermediate = BTreeMap::new();
        for group in group_order {
            let dense = OperationIndex::new(operations.len() as u32);
            match group {
                OperationGroup::Native(index) => {
                    let operation = &self.operations[index];
                    let IntermediateKernel::Native(handle) = &operation.kernel else {
                        unreachable!("native operation group")
                    };
                    dense_by_intermediate.insert(index, dense);
                    operations.push(PlannedOperation {
                        source_node_id: operation.source_node_id,
                        source_node_type_id: operation.source_node_type_id.clone(),
                        kernel: PlannedKernel::Native(handle.clone()),
                        inputs: operation.inputs.clone(),
                        outputs: operation.outputs.clone(),
                        params: operation.params.clone(),
                    });
                }
                OperationGroup::Relational(subplan) => {
                    let mut members = groups
                        .remove(&subplan)
                        .expect("registered relational group");
                    let fragment_order = &relational_subplans[subplan.index()]
                        .compiled_plan
                        .fragment_order;
                    members.sort_by_key(|index| {
                        let IntermediateKernel::Relational { fragment, .. } =
                            &self.operations[*index].kernel
                        else {
                            unreachable!("relational operation group")
                        };
                        fragment_order
                            .iter()
                            .position(|candidate| candidate == &fragment.id)
                            .expect("planned fragment belongs to subplan")
                    });
                    if members.len() > 1
                        && members
                            .iter()
                            .any(|index| self.operations[*index].has_control_or_effect_ports)
                    {
                        return Err(DemandPlanError::InvalidDerivedPlan(
                            "a relational island cannot merge multiple nodes with control or effect ports"
                                .into(),
                        ));
                    }
                    let representative = &self.operations[members[0]];
                    let mut inputs = Vec::new();
                    let mut outputs_by_fragment = BTreeMap::new();
                    for index in &members {
                        dense_by_intermediate.insert(*index, dense);
                        let operation = &self.operations[*index];
                        for input in &operation.inputs {
                            if !internal_inputs.contains(&input.value) {
                                inputs.push(input.clone());
                            }
                        }
                        let IntermediateKernel::Relational { fragment, .. } = &operation.kernel
                        else {
                            unreachable!("relational operation group")
                        };
                        let outputs = operation
                            .outputs
                            .iter()
                            .filter(|output| {
                                !internal_outputs.contains(&output.value)
                                    || external_outputs.contains(&output.value)
                                    || result_values.contains(&output.value)
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        if !outputs.is_empty() {
                            outputs_by_fragment.insert(fragment.id.clone(), outputs);
                        }
                    }
                    let fragment_roots = relational_subplans[subplan.index()]
                        .compiled_plan
                        .fragment_roots
                        .iter()
                        .map(|root| (root.fragment.clone(), root.operator))
                        .collect::<BTreeMap<_, _>>();
                    let mut outputs = Vec::new();
                    let mut roots = Vec::new();
                    for fragment in fragment_order {
                        if let Some(fragment_outputs) = outputs_by_fragment.remove(fragment) {
                            let root = fragment_roots[fragment];
                            for output in fragment_outputs {
                                outputs.push(output);
                                roots.push(root);
                            }
                        }
                    }
                    let compiled_plan = &mut relational_subplans[subplan.index()].compiled_plan;
                    compiled_plan.roots = roots.into_boxed_slice();
                    let mut lineage_roots = compiled_plan.roots.to_vec();
                    lineage_roots.extend(
                        compiled_plan
                            .requested_fragment_outputs
                            .iter()
                            .filter_map(|fragment| {
                                compiled_plan
                                    .fragment_roots
                                    .iter()
                                    .find(|root| &root.fragment == fragment)
                                    .map(|root| root.operator)
                            }),
                    );
                    compiled_plan.pushdown_hints =
                        infer_relational_pushdown_hints(&compiled_plan.operators, &lineage_roots)
                            .into_boxed_slice();
                    operations.push(PlannedOperation {
                        source_node_id: representative.source_node_id,
                        source_node_type_id: representative.source_node_type_id.clone(),
                        kernel: PlannedKernel::Relational(subplan),
                        inputs: inputs.into_boxed_slice(),
                        outputs: outputs.into_boxed_slice(),
                        params: representative.params.clone(),
                    });
                }
            }
        }

        let retained_input_values = retained
            .iter()
            .flat_map(|index| {
                self.operations[*index]
                    .inputs
                    .iter()
                    .map(|input| input.value)
            })
            .collect::<BTreeSet<_>>();
        let value_dependencies = self
            .value_dependencies
            .iter()
            .filter(|dependency| {
                required_values.is_none_or(|required| {
                    required.contains(&dependency.destination)
                        || retained_input_values.contains(&dependency.destination)
                }) && !internal_dependencies.contains(&(dependency.source, dependency.destination))
            })
            .copied()
            .collect::<Vec<_>>();
        let effect_dependencies = self
            .effect_dependencies
            .iter()
            .filter_map(|(before, after)| {
                Some((
                    *dense_by_intermediate.get(before)?,
                    *dense_by_intermediate.get(after)?,
                ))
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|(before, after)| EffectDependency { before, after })
            .collect::<Vec<_>>();
        let mut resources = BTreeMap::<ResourceId, CompiledResourceRequirement>::new();
        for index in retained {
            for requirement in &self.operations[*index].resources {
                resources.insert(requirement.resource.clone(), requirement.clone());
            }
        }
        let mut root_region = remap_region(retained_region, &dense_by_intermediate);
        deduplicate_region_operations(&mut root_region);
        let mut value_sources = self
            .value_sources
            .iter()
            .filter_map(|source| match source {
                PlanValueSource::ExternalInput(value)
                    if required_values.is_none_or(|required| required.contains(value)) =>
                {
                    Some(source.clone())
                }
                PlanValueSource::ExternalInput(_) | PlanValueSource::ControlProduced(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let mut control_produced = BTreeSet::new();
        collect_control_produced_values(&root_region, &mut control_produced);
        value_sources.extend(
            control_produced
                .into_iter()
                .map(PlanValueSource::ControlProduced),
        );
        let plan = ExecutionPlan {
            provenance: self.provenance.clone(),
            value_count: self.value_count,
            operations: operations.into_boxed_slice(),
            value_sources: value_sources
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            value_dependencies: value_dependencies.into_boxed_slice(),
            root_region,
            effect_dependencies: effect_dependencies.into_boxed_slice(),
            relational_subplans,
            resources: resources
                .into_values()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            results: selected_results.into_boxed_slice(),
        };
        plan.validate()
            .map_err(|error| DemandPlanError::InvalidDerivedPlan(error.to_string().into()))?;
        Ok(plan)
    }

    fn plan_relational(
        &self,
        retained: &BTreeSet<usize>,
    ) -> Result<RelationalFinalization, DemandPlanError> {
        let mut backend = None;
        let mut fragments = Vec::new();
        let mut retained_fragment_ids = BTreeSet::new();
        let mut operation_by_fragment = BTreeMap::new();
        for index in retained {
            let IntermediateKernel::Relational {
                backend: operation_backend,
                fragment,
                input_bindings,
            } = &self.operations[*index].kernel
            else {
                continue;
            };
            let _ = input_bindings;
            if backend
                .as_ref()
                .is_some_and(|backend| backend != operation_backend)
            {
                return Err(DemandPlanError::InvalidDerivedPlan(
                    "relational operations use multiple backends".into(),
                ));
            }
            backend.get_or_insert_with(|| operation_backend.clone());
            retained_fragment_ids.insert(fragment.id.clone());
            operation_by_fragment.insert(fragment.id.clone(), *index);
            fragments.push(fragment.clone());
        }
        let Some(backend) = backend else {
            return Ok(RelationalFinalization::default());
        };
        let connections = self
            .relational_connections
            .iter()
            .filter(|connection| {
                retained_fragment_ids.contains(&connection.producer)
                    && retained_fragment_ids.contains(&connection.consumer)
            })
            .cloned()
            .collect::<Vec<_>>();
        let planning = RelationalPlanner::new(backend)
            .plan(&fragments, &connections)
            .map_err(|error| DemandPlanError::InvalidDerivedPlan(error.to_string().into()))?;
        let subplan_by_fragment = planning
            .subplans
            .iter()
            .enumerate()
            .flat_map(|(index, subplan)| {
                subplan
                    .compiled_plan
                    .fragment_order
                    .iter()
                    .cloned()
                    .map(move |fragment| (fragment, RelationalSubplanIndex::new(index as u32)))
            })
            .collect::<BTreeMap<_, _>>();
        let subplan_by_operation = operation_by_fragment
            .into_iter()
            .map(|(fragment, operation)| (operation, subplan_by_fragment[&fragment]))
            .collect();
        Ok(RelationalFinalization {
            subplans: planning.subplans,
            subplan_by_fragment,
            subplan_by_operation,
        })
    }
}
