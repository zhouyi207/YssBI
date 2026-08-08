use super::*;
use crate::node_system::plan::infer_relational_pushdown_hints;

fn insert_materialization_adapters(
    provenance: &crate::node_system::analysis::CompileProvenance,
    value_count: &mut u32,
    operations: &mut Vec<PlannedOperation>,
    value_sources: &[PlanValueSource],
    dependencies: &mut Vec<ValueDependency>,
    root_region: &mut StructuredControlRegion,
) -> Result<(), DemandPlanError> {
    let mut outputs = operations
        .iter()
        .enumerate()
        .flat_map(|(index, operation)| {
            operation.outputs.iter().map(move |output| {
                (
                    output.value,
                    (Some(index), operation.stable_id.clone(), output.production),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    for source in value_sources {
        outputs.insert(
            source.value(),
            (
                None,
                OperationStableId::from_digest(crate::node_system::registry::hash_canonical(
                    "yssbi.operation-stable-id.plan-value-source.v1",
                    &serde_json::json!({
                        "graphPath": &provenance.graph_path,
                        "source": source,
                    }),
                )?),
                source.production(),
            ),
        );
    }
    let inputs = operations
        .iter()
        .enumerate()
        .flat_map(|(index, operation)| {
            operation
                .inputs
                .iter()
                .map(move |input| (input.value, (index, input.consumption)))
        })
        .collect::<BTreeMap<_, _>>();
    let mut boundaries = dependencies
        .iter()
        .filter_map(|dependency| {
            let (producer, producer_stable_id, production) = outputs.get(&dependency.source)?;
            let (consumer, consumption) = inputs.get(&dependency.destination)?;
            Some((
                producer_stable_id.clone(),
                operations[*consumer].stable_id.clone(),
                dependency.source,
                dependency.destination,
                *producer,
                *consumer,
                *production,
                *consumption,
            ))
        })
        .collect::<Vec<_>>();
    boundaries.sort_by(|left, right| {
        (&left.0, &left.1, left.2, left.3).cmp(&(&right.0, &right.1, right.2, right.3))
    });
    let replaced = boundaries
        .iter()
        .map(|boundary| (boundary.2, boundary.3))
        .collect::<BTreeSet<_>>();
    dependencies
        .retain(|dependency| !replaced.contains(&(dependency.source, dependency.destination)));

    let mut adapters_by_consumer = BTreeMap::<OperationIndex, Vec<OperationIndex>>::new();
    let mut fanout_by_producer = BTreeMap::<OperationIndex, Vec<OperationIndex>>::new();
    let mut boundary_indices_by_source = BTreeMap::<ValueRef, Vec<usize>>::new();
    for (index, boundary) in boundaries.iter().enumerate() {
        boundary_indices_by_source
            .entry(boundary.2)
            .or_default()
            .push(index);
    }
    for indices in boundary_indices_by_source.into_values() {
        if indices.len() < 2 || boundaries[indices[0]].6 == OutputProduction::FullyMaterialized {
            continue;
        }
        let production = boundaries[indices[0]].6;
        let collect = indices.iter().any(|index| {
            matches!(
                boundaries[*index].7,
                InputConsumption::RandomAccess | InputConsumption::FullyMaterialized
            )
        });
        let adapter = if collect {
            PlannedAdapter::Collect {
                limits: crate::node_system::plan::MaterializationLimits {
                    max_values: 1_000_000,
                    max_bytes: 64 * 1024 * 1024,
                },
            }
        } else {
            PlannedAdapter::Replay
        };
        let output_production = if collect {
            OutputProduction::FullyMaterialized
        } else {
            OutputProduction::Batches
        };
        let input_consumption = match production {
            OutputProduction::Streaming => InputConsumption::Streaming,
            OutputProduction::Batches => InputConsumption::SinglePassBatches,
            OutputProduction::FullyMaterialized => unreachable!(),
        };
        let source = boundaries[indices[0]].2;
        let producer = boundaries[indices[0]].4;
        let producer_stable_id = boundaries[indices[0]].0.clone();
        let consumers = indices
            .iter()
            .map(|index| (&boundaries[*index].1, boundaries[*index].7))
            .collect::<Vec<_>>();
        let stable_id =
            OperationStableId::from_digest(crate::node_system::registry::hash_canonical(
                "yssbi.operation-stable-id.materialization-fanout.v1",
                &serde_json::json!({
                    "graphPath": &provenance.graph_path,
                    "producer": &producer_stable_id,
                    "production": production,
                    "consumers": consumers,
                    "adapter": &adapter,
                }),
            )?);
        let semantics_version =
            ExecutionSemanticsVersion::from_bytes(crate::node_system::registry::hash_canonical(
                "yssbi.execution-semantics.materialization-fanout.v1",
                &serde_json::json!({
                    "schemaVersion": EXECUTION_SEMANTICS_SCHEMA_VERSION,
                    "adapter": &adapter,
                    "inputConsumption": input_consumption,
                    "outputProduction": output_production,
                }),
            )?);
        let adapter_input = ValueRef::new(*value_count);
        *value_count += 1;
        let adapter_output = ValueRef::new(*value_count);
        *value_count += 1;
        let operation_index = OperationIndex::new(operations.len() as u32);
        let source_node_id = producer
            .map(|producer| operations[producer].source_node_id)
            .unwrap_or_else(|| operations[boundaries[indices[0]].5].source_node_id);
        operations.push(PlannedOperation {
            stable_id: stable_id.clone(),
            source_node_id,
            source_node_type_id: crate::node_system::protocol::NodeTypeId::new(
                "yssbi.compiler.materialization_fanout",
            )
            .expect("static fanout node type is valid"),
            kernel: PlannedKernel::Adapter(adapter),
            inputs: Box::new([PlannedInput {
                value: adapter_input,
                consumption: input_consumption,
                bound_value: None,
            }]),
            outputs: Box::new([PlannedOutput {
                value: adapter_output,
                production: output_production,
            }]),
            params: CompiledParameterHandle::new("adapter.fanout")
                .expect("static fanout parameter handle is valid"),
            resource_dependencies: Box::new([]),
            cache_policy: CachePolicy::Disabled,
            semantics_version,
            workload: WorkloadClass::AdapterIo,
            retry: PlannedRetry::default(),
        });
        dependencies.push(ValueDependency {
            source,
            destination: adapter_input,
        });
        if let Some(producer) = producer {
            fanout_by_producer
                .entry(OperationIndex::new(producer as u32))
                .or_default()
                .push(operation_index);
        } else {
            let first_consumer = indices
                .iter()
                .map(|index| boundaries[*index].5)
                .min()
                .expect("fanout has consumers");
            adapters_by_consumer
                .entry(OperationIndex::new(first_consumer as u32))
                .or_default()
                .push(operation_index);
        }
        for index in indices {
            boundaries[index].0 = stable_id.clone();
            boundaries[index].2 = adapter_output;
            boundaries[index].4 = Some(operation_index.index());
            boundaries[index].6 = output_production;
        }
    }
    for (
        producer_stable_id,
        consumer_stable_id,
        source,
        destination,
        _,
        consumer,
        production,
        consumption,
    ) in boundaries
    {
        let adapter_plan = crate::node_system::plan::MaterializationAdapterPlan::for_contract(
            production,
            consumption,
        );
        let adapter = adapter_plan.adapter.clone();
        let adapter_input = ValueRef::new(*value_count);
        *value_count += 1;
        let adapter_output = ValueRef::new(*value_count);
        *value_count += 1;
        let stable_id =
            OperationStableId::from_digest(crate::node_system::registry::hash_canonical(
                "yssbi.operation-stable-id.materialization-adapter.v1",
                &serde_json::json!({
                    "graphPath": &provenance.graph_path,
                    "producer": &producer_stable_id,
                    "consumer": &consumer_stable_id,
                    "source": source,
                    "destination": destination,
                    "production": production,
                    "consumption": consumption,
                    "adapter": &adapter,
                }),
            )?);
        let semantics_version =
            ExecutionSemanticsVersion::from_bytes(crate::node_system::registry::hash_canonical(
                "yssbi.execution-semantics.materialization-adapter.v1",
                &serde_json::json!({
                    "schemaVersion": EXECUTION_SEMANTICS_SCHEMA_VERSION,
                    "adapter": &adapter,
                    "production": production,
                    "consumption": consumption,
                }),
            )?);
        let operation_index = OperationIndex::new(operations.len() as u32);
        let consumer_operation = &operations[consumer];
        operations.push(PlannedOperation {
            stable_id,
            source_node_id: consumer_operation.source_node_id,
            source_node_type_id: crate::node_system::protocol::NodeTypeId::new(
                "yssbi.compiler.materialization_adapter",
            )
            .expect("static adapter node type is valid"),
            kernel: PlannedKernel::Adapter(adapter),
            inputs: Box::new([PlannedInput {
                value: adapter_input,
                consumption: adapter_plan.input_consumption,
                bound_value: None,
            }]),
            outputs: Box::new([PlannedOutput {
                value: adapter_output,
                production: adapter_plan.output_production,
            }]),
            params: CompiledParameterHandle::new("adapter.none")
                .expect("static adapter parameter handle is valid"),
            resource_dependencies: Box::new([]),
            cache_policy: CachePolicy::Disabled,
            semantics_version,
            workload: WorkloadClass::AdapterIo,
            retry: PlannedRetry::default(),
        });
        dependencies.push(ValueDependency {
            source,
            destination: adapter_input,
        });
        dependencies.push(ValueDependency {
            source: adapter_output,
            destination,
        });
        adapters_by_consumer
            .entry(OperationIndex::new(consumer as u32))
            .or_default()
            .push(operation_index);
    }
    dependencies.sort_by_key(|dependency| (dependency.source, dependency.destination));
    inject_fanout_steps(root_region, &fanout_by_producer);
    inject_adapter_steps(root_region, &adapters_by_consumer);
    Ok(())
}

fn inject_fanout_steps(
    region: &mut StructuredControlRegion,
    fanout_by_producer: &BTreeMap<OperationIndex, Vec<OperationIndex>>,
) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            let mut expanded = Vec::new();
            for mut step in std::mem::take(steps).into_vec() {
                if let ControlStep::Region(child) = &mut step {
                    inject_fanout_steps(child, fanout_by_producer);
                }
                let producer = match &step {
                    ControlStep::Operation(operation) => Some(*operation),
                    ControlStep::Region(_) => None,
                };
                expanded.push(step);
                if let Some(producer) = producer
                    && let Some(fanouts) = fanout_by_producer.get(&producer)
                {
                    expanded.extend(fanouts.iter().copied().map(ControlStep::Operation));
                }
            }
            *steps = expanded.into_boxed_slice();
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            inject_fanout_steps(then_region, fanout_by_producer);
            inject_fanout_steps(else_region, fanout_by_producer);
        }
        StructuredControlRegion::Loop { body, .. } => {
            inject_fanout_steps(body, fanout_by_producer);
        }
        StructuredControlRegion::Call { .. } => {}
    }
}

fn inject_adapter_steps(
    region: &mut StructuredControlRegion,
    adapters_by_consumer: &BTreeMap<OperationIndex, Vec<OperationIndex>>,
) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            let mut expanded = Vec::new();
            for mut step in std::mem::take(steps).into_vec() {
                match &mut step {
                    ControlStep::Operation(operation) => {
                        if let Some(adapters) = adapters_by_consumer.get(operation) {
                            expanded.extend(adapters.iter().copied().map(ControlStep::Operation));
                        }
                    }
                    ControlStep::Region(child) => {
                        inject_adapter_steps(child, adapters_by_consumer);
                    }
                }
                expanded.push(step);
            }
            *steps = expanded.into_boxed_slice();
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            inject_adapter_steps(then_region, adapters_by_consumer);
            inject_adapter_steps(else_region, adapters_by_consumer);
        }
        StructuredControlRegion::Loop { body, .. } => {
            inject_adapter_steps(body, adapters_by_consumer);
        }
        StructuredControlRegion::Call { .. } => {}
    }
}

impl ExecutionPlanBasis {
    pub(super) fn finalize(
        &self,
        retained: &BTreeSet<usize>,
        selected_outputs: &BTreeSet<GraphOutputRef>,
        preview_generation: Option<u64>,
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
        let publications = selected_results
            .iter()
            .map(|result| match preview_generation {
                Some(generation) => PlannedPublication::PinPreview {
                    output: result.output.clone(),
                    generation,
                    value: result.value,
                },
                None => PlannedPublication::GraphResult {
                    name: result.name.clone(),
                    output: result.output.clone(),
                    value: result.value,
                },
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
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
                        stable_id: operation.stable_id.clone(),
                        source_node_id: operation.source_node_id,
                        source_node_type_id: operation.source_node_type_id.clone(),
                        kernel: PlannedKernel::Native(handle.clone()),
                        inputs: operation.inputs.clone(),
                        outputs: operation.outputs.clone(),
                        params: operation.params.clone(),
                        resource_dependencies: operation.resource_dependencies.clone(),
                        cache_policy: operation.cache_policy,
                        semantics_version: operation.semantics_version,
                        workload: operation.workload,
                        retry: operation.retry.clone(),
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
                        if let Some(planned_outputs) = outputs_by_fragment.remove(fragment) {
                            let root = fragment_roots[fragment];
                            for output in planned_outputs {
                                outputs.push(output);
                                roots.push(root);
                            }
                        }
                    }
                    let compiled_plan = &mut relational_subplans[subplan.index()].compiled_plan;
                    compiled_plan.roots = roots.into_boxed_slice();
                    compiled_plan.pushdown_hints = infer_relational_pushdown_hints(
                        &compiled_plan.operators,
                        &compiled_plan.roots,
                    )
                    .into_boxed_slice();
                    let cache_policy = if members
                        .iter()
                        .all(|index| self.operations[*index].cache_policy == CachePolicy::PerRun)
                    {
                        CachePolicy::PerRun
                    } else {
                        CachePolicy::Disabled
                    };
                    let workload = members
                        .iter()
                        .map(|index| self.operations[*index].workload)
                        .max_by_key(|workload| match workload {
                            WorkloadClass::Cpu => 0,
                            WorkloadClass::Io => 1,
                            WorkloadClass::AdapterIo => 2,
                            WorkloadClass::Exclusive => 3,
                        })
                        .expect("relational operation group has members");
                    let retry = members
                        .first()
                        .map(|index| self.operations[*index].retry.clone())
                        .filter(|candidate| {
                            candidate.idempotent
                                && members
                                    .iter()
                                    .all(|index| self.operations[*index].retry == *candidate)
                        })
                        .unwrap_or_default();
                    let resource_dependencies = members
                        .iter()
                        .flat_map(|index| {
                            self.operations[*index]
                                .resource_dependencies
                                .iter()
                                .cloned()
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .into_boxed_slice();
                    let member_stable_ids = members
                        .iter()
                        .map(|index| self.operations[*index].stable_id.clone())
                        .collect::<Vec<_>>();
                    let member_semantics_versions = members
                        .iter()
                        .map(|index| self.operations[*index].semantics_version)
                        .collect::<Vec<_>>();
                    let fused_subplan = &relational_subplans[subplan.index()];
                    let composite_semantics = serde_json::json!({
                        "backend": &fused_subplan.backend,
                        "compiledPlan": &fused_subplan.compiled_plan,
                        "inputs": &inputs,
                        "outputs": &outputs,
                    });
                    let stable_id = OperationStableId::from_digest(
                        crate::node_system::registry::hash_canonical(
                            "yssbi.operation-stable-id.relational.v2",
                            &serde_json::json!({
                                "graphPath": &self.provenance.graph_path,
                                "members": &member_stable_ids,
                                "fusedSubplan": &composite_semantics,
                            }),
                        )?,
                    );
                    let semantics_version = ExecutionSemanticsVersion::from_bytes(
                        crate::node_system::registry::hash_canonical(
                            "yssbi.execution-semantics.relational.v2",
                            &serde_json::json!({
                                "schemaVersion": EXECUTION_SEMANTICS_SCHEMA_VERSION,
                                "registryFingerprint": &self.provenance.basis.registry_fingerprint,
                                "memberVersions": &member_semantics_versions,
                                "fusedSubplan": &composite_semantics,
                                "cachePolicy": cache_policy,
                                "workload": workload,
                                "retry": &retry,
                            }),
                        )?,
                    );
                    operations.push(PlannedOperation {
                        stable_id,
                        source_node_id: representative.source_node_id,
                        source_node_type_id: representative.source_node_type_id.clone(),
                        kernel: PlannedKernel::Relational(subplan),
                        inputs: inputs.into_boxed_slice(),
                        outputs: outputs.into_boxed_slice(),
                        params: representative.params.clone(),
                        resource_dependencies,
                        cache_policy,
                        semantics_version,
                        workload,
                        retry,
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
        let mut value_dependencies = self
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
        let mut value_count = self.value_count;
        insert_materialization_adapters(
            &self.provenance,
            &mut value_count,
            &mut operations,
            &self.value_sources,
            &mut value_dependencies,
            &mut root_region,
        )?;
        let mut control_produced = BTreeSet::new();
        collect_control_produced_values(&root_region, &mut control_produced);
        let value_sources = self
            .value_sources
            .iter()
            .filter(|source| match source {
                PlanValueSource::ExternalInput(value, _) => {
                    required_values.is_none_or(|required| required.contains(value))
                }
                PlanValueSource::ControlProduced(value, _) => control_produced.contains(value),
            })
            .copied()
            .collect::<BTreeSet<_>>();
        let plan = ExecutionPlan {
            provenance: self.provenance.clone(),
            value_count,
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
            publications,
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

#[cfg(test)]
mod materialization_tests {
    use super::*;
    use crate::node_system::analysis::{
        CompilationBasis, CompileId, ProjectSessionId, ResourceVersionSet,
    };
    use crate::node_system::document::{GraphResourcePath, GraphRevision};
    use crate::node_system::plan::PlannedAdapter;
    use crate::node_system::protocol::{InputConsumption, OutputProduction};
    use crate::node_system::registry::RegistryFingerprint;

    fn operation(
        stable: &str,
        node: u128,
        kernel: PlannedKernel,
        input: Option<ValueRef>,
        output: ValueRef,
    ) -> PlannedOperation {
        PlannedOperation {
            stable_id: OperationStableId::new(stable).unwrap(),
            source_node_id: NodeId::from_uuid(uuid::Uuid::from_u128(node)),
            source_node_type_id: crate::node_system::protocol::NodeTypeId::new(
                "yssbi.test.adapter",
            )
            .unwrap(),
            kernel,
            inputs: match input {
                Some(value) => vec![PlannedInput {
                    value,
                    consumption: InputConsumption::FullyMaterialized,
                    bound_value: None,
                }]
                .into_boxed_slice(),
                None => Box::new([]),
            },
            outputs: Box::new([PlannedOutput {
                value: output,
                production: OutputProduction::Streaming,
            }]),
            params: CompiledParameterHandle::new("test.params").unwrap(),
            resource_dependencies: Box::new([]),
            cache_policy: CachePolicy::Disabled,
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            workload: WorkloadClass::Cpu,
            retry: PlannedRetry::default(),
        }
    }

    fn adapter_identity(
        producer_relational: bool,
        consumer_relational: bool,
        reversed: bool,
        node_offset: u128,
    ) -> (
        OperationStableId,
        ExecutionSemanticsVersion,
        Vec<ValueDependency>,
    ) {
        let native =
            |name: &str| PlannedKernel::Native(KernelHandle::new(name.to_owned()).unwrap());
        let producer_kernel = if producer_relational {
            PlannedKernel::Relational(RelationalSubplanIndex::new(0))
        } else {
            native("test.producer")
        };
        let consumer_kernel = if consumer_relational {
            PlannedKernel::Relational(RelationalSubplanIndex::new(1))
        } else {
            native("test.consumer")
        };
        let producer = operation(
            "stable.producer",
            node_offset + 1,
            producer_kernel,
            None,
            ValueRef::new(0),
        );
        let consumer = operation(
            "stable.consumer",
            node_offset + 2,
            consumer_kernel,
            Some(ValueRef::new(1)),
            ValueRef::new(2),
        );
        let (mut operations, producer_index, consumer_index) = if reversed {
            (vec![consumer, producer], 1, 0)
        } else {
            (vec![producer, consumer], 0, 1)
        };
        let mut dependencies = vec![ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        }];
        let mut region = StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(producer_index)),
            ControlStep::Operation(OperationIndex::new(consumer_index)),
        ]));
        let provenance = crate::node_system::analysis::CompileProvenance {
            project_session_id: ProjectSessionId::new("test-session"),
            graph_path: GraphResourcePath("events/materialization".into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(1),
                registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                resource_versions: ResourceVersionSet::new(),
                resource_observations: Default::default(),
            },
            compile_id: CompileId::new(1),
        };
        let mut value_count = 3;
        insert_materialization_adapters(
            &provenance,
            &mut value_count,
            &mut operations,
            &[],
            &mut dependencies,
            &mut region,
        )
        .unwrap();
        let adapter = operations
            .iter()
            .find(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .unwrap();
        assert!(matches!(
            adapter.kernel,
            PlannedKernel::Adapter(PlannedAdapter::Collect { .. })
        ));
        (
            adapter.stable_id.clone(),
            adapter.semantics_version,
            dependencies,
        )
    }

    #[test]
    fn adapter_insertion_covers_all_kernel_pairs_independent_of_order_and_uuid() {
        for (producer_relational, consumer_relational) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let forward = adapter_identity(producer_relational, consumer_relational, false, 0);
            let permuted = adapter_identity(producer_relational, consumer_relational, true, 100);
            assert_eq!(forward, permuted);
        }
    }
}
