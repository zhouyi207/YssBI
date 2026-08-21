use super::*;

pub(super) fn deduplicate_region_operations(region: &mut StructuredControlRegion) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            let mut seen = BTreeSet::new();
            let mut deduplicated = Vec::with_capacity(steps.len());
            for mut step in std::mem::take(steps).into_vec() {
                match &mut step {
                    ControlStep::Operation(operation) if !seen.insert(*operation) => continue,
                    ControlStep::Region(child) => deduplicate_region_operations(child),
                    ControlStep::Operation(_) => {}
                }
                deduplicated.push(step);
            }
            *steps = deduplicated.into_boxed_slice();
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            deduplicate_region_operations(then_region);
            deduplicate_region_operations(else_region);
        }
        StructuredControlRegion::Loop { body, .. } => deduplicate_region_operations(body),
        StructuredControlRegion::Call { .. } => {}
    }
}

pub(super) fn collect_control_value_sources(
    region: &mut StructuredControlRegion,
    sources: &mut BTreeSet<PlanValueSource>,
    productions: &mut BTreeMap<ValueRef, OutputProduction>,
    contracts: &mut BTreeMap<ValueRef, PlannedValueContract>,
    function_abis: &BTreeMap<GraphResourcePath, FunctionPlanAbi>,
) -> Result<(), LowerGraphFailure> {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                if let crate::node_system::plan::ControlStep::Region(region) = step {
                    collect_control_value_sources(
                        region,
                        sources,
                        productions,
                        contracts,
                        function_abis,
                    )?;
                }
            }
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            results,
            ..
        } => {
            collect_control_value_sources(
                then_region,
                sources,
                productions,
                contracts,
                function_abis,
            )?;
            collect_control_value_sources(
                else_region,
                sources,
                productions,
                contracts,
                function_abis,
            )?;
            for binding in results {
                let then_production =
                    productions
                        .get(&binding.then_source)
                        .copied()
                        .ok_or_else(|| {
                            CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                        })?;
                let else_production =
                    productions
                        .get(&binding.else_source)
                        .copied()
                        .ok_or_else(|| {
                            CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                        })?;
                if then_production != else_production {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                let then_contract =
                    contracts
                        .get(&binding.then_source)
                        .cloned()
                        .ok_or_else(|| {
                            CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                        })?;
                if contracts.get(&binding.else_source) != Some(&then_contract) {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                binding.production = Some(then_production);
                productions.insert(binding.destination, then_production);
                contracts.insert(binding.destination, then_contract);
                sources.insert(PlanValueSource::ControlProduced(
                    binding.destination,
                    then_production,
                ));
            }
        }
        StructuredControlRegion::Loop { body, carried, .. } => {
            for binding in carried.iter_mut() {
                let initial = productions
                    .get(&binding.initial_source)
                    .copied()
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                let contract =
                    contracts
                        .get(&binding.initial_source)
                        .cloned()
                        .ok_or_else(|| {
                            CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                        })?;
                binding.production = Some(initial);
                productions.insert(binding.body_input, initial);
                contracts.insert(binding.body_input, contract);
                sources.insert(PlanValueSource::ControlProduced(
                    binding.body_input,
                    initial,
                ));
            }
            collect_control_value_sources(body, sources, productions, contracts, function_abis)?;
            for binding in carried {
                let initial = binding.production.ok_or_else(|| {
                    CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                })?;
                let next = productions
                    .get(&binding.next_source)
                    .copied()
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                if initial != next {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                let initial_contract =
                    contracts
                        .get(&binding.initial_source)
                        .cloned()
                        .ok_or_else(|| {
                            CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                        })?;
                if contracts.get(&binding.next_source) != Some(&initial_contract) {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                productions.insert(binding.result, initial);
                contracts.insert(binding.result, initial_contract);
                sources.insert(PlanValueSource::ControlProduced(binding.result, initial));
            }
        }
        StructuredControlRegion::Call {
            target,
            arguments,
            results,
            ..
        } => {
            let path = GraphResourcePath(target.as_str().into());
            let abi = function_abis.get(&path).ok_or_else(|| {
                CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
            })?;
            for binding in arguments {
                let expected = abi
                    .parameters
                    .iter()
                    .find_map(|(parameter, value)| {
                        (*value == binding.callee_destination)
                            .then(|| abi.parameter_contracts.get(parameter))
                            .flatten()
                    })
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                if contracts.get(&binding.caller_source) != Some(expected) {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
            }
            for binding in results {
                let production = abi
                    .results
                    .iter()
                    .find_map(|(parameter, value)| {
                        (*value == binding.callee_source)
                            .then(|| abi.result_productions.get(parameter).copied())
                            .flatten()
                    })
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                if Some(production) != binding.production {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                let contract = abi
                    .results
                    .iter()
                    .find_map(|(parameter, value)| {
                        (*value == binding.callee_source)
                            .then(|| abi.result_contracts.get(parameter).cloned())
                            .flatten()
                    })
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                productions.insert(binding.caller_destination, production);
                contracts.insert(binding.caller_destination, contract);
                sources.insert(PlanValueSource::ControlProduced(
                    binding.caller_destination,
                    production,
                ));
            }
        }
    }
    Ok(())
}
