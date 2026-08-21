use super::*;

impl<'a, R: CompilerRegistry, S: ResourceSnapshot> GraphCompiler<'a, R, S> {
    pub(super) fn function_abis_for_calls(
        &self,
        graph: &CompilerSemanticGraph,
        compile_id: CompileId,
        resources: &mut dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
    ) -> Result<CallClosureAnalysis, CompileCancelled> {
        let mut closure = CallClosureAnalysis::default();
        let root_targets = self.call_targets(graph, &mut closure.diagnostics);
        let mut pending = root_targets.keys().cloned().collect::<Vec<_>>();
        let mut dependencies = BTreeMap::new();
        let mut direct_resolution_failures = BTreeSet::new();

        while let Some(target) = pending.pop() {
            cancellation.checkpoint()?;
            if dependencies.contains_key(&target) {
                continue;
            }
            let root_sites = root_targets.get(&target).map(Vec::as_slice);
            let (node, resolution_failed) = self.analyze_call_dependency(
                &target,
                root_sites,
                compile_id,
                resources,
                cancellation,
                &mut closure.diagnostics,
            )?;
            if resolution_failed {
                direct_resolution_failures.insert(target.clone());
            }
            pending.extend(node.dependencies.iter().cloned());
            dependencies.insert(target, node);
        }

        let initially_invalid = invalid_call_targets(&dependencies);
        self.finalize_call_dependency_abis(
            &mut dependencies,
            &initially_invalid,
            resources,
            cancellation,
        )?;
        let invalid = invalid_call_targets(&dependencies);
        for (target, node) in dependencies {
            if !invalid.contains(&target)
                && let Some(abi) = node.abi
            {
                closure.abis.insert(target, abi);
            }
        }
        for (target, sites) in root_targets {
            if invalid.contains(&target) && !direct_resolution_failures.contains(&target) {
                self.push_call_abi_invalid(&target, &sites, &mut closure.diagnostics);
            }
        }
        Ok(closure)
    }

    fn finalize_call_dependency_abis(
        &self,
        dependencies: &mut BTreeMap<GraphResourcePath, CallDependencyNode>,
        initially_invalid: &BTreeSet<GraphResourcePath>,
        resources: &dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
    ) -> Result<(), CompileCancelled> {
        let final_versions = resources.reads().clone();
        let final_observations = resources.observations().clone();
        for (target, node) in dependencies.iter_mut() {
            if initially_invalid.contains(target) {
                continue;
            }
            if let Some(abi) = node.abi.as_mut() {
                abi.provenance.basis.resource_versions = final_versions.clone();
                abi.provenance.basis.resource_observations = final_observations.clone();
            }
            if let Some(lowering) = node.lowering.as_mut() {
                lowering.provenance.basis.resource_versions = final_versions.clone();
                lowering.provenance.basis.resource_observations = final_observations.clone();
                lowering.semantic.basis.resource_versions = final_versions.clone();
                lowering.semantic.basis.resource_observations = final_observations.clone();
            }
        }

        let components = strongly_connected_call_components(dependencies);
        for component in components.into_iter().rev() {
            cancellation.checkpoint()?;
            let invalid = invalid_call_targets(dependencies);
            if component.iter().any(|target| invalid.contains(target)) {
                continue;
            }
            match self.finalize_call_component(dependencies, &component, cancellation) {
                Ok(()) => {}
                Err(CallAbiFinalizationFailure::Cancelled(error)) => return Err(error),
                Err(CallAbiFinalizationFailure::Invalid) => {
                    for target in component {
                        if let Some(node) = dependencies.get_mut(&target) {
                            node.abi = None;
                            node.locally_valid = false;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn finalize_call_component(
        &self,
        dependencies: &mut BTreeMap<GraphResourcePath, CallDependencyNode>,
        component: &[GraphResourcePath],
        cancellation: &CompileCancellationToken,
    ) -> Result<(), CallAbiFinalizationFailure> {
        const MAX_ITERATIONS: usize = 32;
        let mut seen = BTreeSet::new();
        for _ in 0..MAX_ITERATIONS {
            cancellation
                .checkpoint()
                .map_err(CallAbiFinalizationFailure::Cancelled)?;
            let state = component
                .iter()
                .map(|target| {
                    (
                        target.clone(),
                        dependencies[target]
                            .abi
                            .as_ref()
                            .map(|abi| abi.result_productions.clone()),
                    )
                })
                .collect::<Vec<_>>();
            if !seen.insert(state) {
                return Err(CallAbiFinalizationFailure::Invalid);
            }
            let call_abis = dependencies
                .iter()
                .filter_map(|(target, node)| {
                    node.abi.as_ref().map(|abi| (target.clone(), abi.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            let mut finalized = Vec::with_capacity(component.len());
            for target in component {
                let node = &dependencies[target];
                let lowering = node
                    .lowering
                    .as_ref()
                    .ok_or(CallAbiFinalizationFailure::Invalid)?;
                let (_, plan) = lower_graph(
                    self.registry,
                    &lowering.document,
                    &lowering.semantic,
                    &lowering.prepared_configs,
                    &lowering.decoded_literals,
                    &lowering.interface_projection,
                    &call_abis,
                    lowering.provenance.clone(),
                    cancellation,
                )
                .map_err(|error| match error {
                    LowerGraphFailure::Cancelled(error) => {
                        CallAbiFinalizationFailure::Cancelled(error)
                    }
                    LowerGraphFailure::Internal(_) => CallAbiFinalizationFailure::Invalid,
                })?;
                let plan = plan.ok_or(CallAbiFinalizationFailure::Invalid)?;
                let mut abi = node
                    .abi
                    .clone()
                    .ok_or(CallAbiFinalizationFailure::Invalid)?;
                finalize_function_abi_productions(&plan, &mut abi)
                    .map_err(|_| CallAbiFinalizationFailure::Invalid)?;
                finalized.push((target.clone(), abi));
            }
            let stable = finalized.iter().all(|(target, abi)| {
                dependencies[target]
                    .abi
                    .as_ref()
                    .is_some_and(|current| current.result_productions == abi.result_productions)
            });
            for (target, abi) in finalized {
                dependencies
                    .get_mut(&target)
                    .expect("call component target exists")
                    .abi = Some(abi);
            }
            if stable {
                return Ok(());
            }
        }
        Err(CallAbiFinalizationFailure::Invalid)
    }

    fn analyze_call_dependency(
        &self,
        target: &GraphResourcePath,
        root_sites: Option<&[NodeId]>,
        compile_id: CompileId,
        resources: &mut dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
        diagnostics: &mut Vec<CompilerNodeDiagnostic>,
    ) -> Result<(CallDependencyNode, bool), CompileCancelled> {
        cancellation.checkpoint()?;
        let resolved = match resources.resolve_function(target) {
            Ok(resolved) => resolved,
            Err(error) => {
                if let Some(sites) = root_sites {
                    for &node_id in sites {
                        diagnostics.push(
                            CompilerDiagnostic::resource_resolution_failed(
                                error.key().as_str(),
                                error.reason(),
                            )
                            .into_node(DiagnosticLocation::Node(node_id)),
                        );
                    }
                }
                return Ok((CallDependencyNode::invalid(), true));
            }
        };
        let document = resolved.value.graph.clone();
        let provenance = CompileProvenance {
            project_session_id: self.project_session_id.clone(),
            graph_path: target.clone(),
            basis: CompilationBasis {
                graph_revision: document.revision,
                registry_fingerprint: self.registry.fingerprint().clone(),
                resource_versions: ResourceVersionSet::new(),
                resource_observations: ResourceObservationSet::new(),
            },
            compile_id,
        };
        let mut state = AnalysisState::new(&document, target.clone(), provenance.basis.clone());
        state.analyze(
            self.registry,
            &self.schema_resolvers,
            &self.interface_resolvers,
            resources,
            cancellation,
        )?;
        let prepared_configs = state.prepared_configs();
        let decoded_literals = state.decoded_literals.clone();
        state.basis.resource_versions = resources.reads().clone();
        state.basis.resource_observations = resources.observations().clone();
        let provisional_semantic = state.semantic_graph();
        let mut nested_diagnostics = Vec::new();
        let dependencies = self
            .call_targets(&provisional_semantic, &mut nested_diagnostics)
            .into_keys()
            .collect();
        let analysis = state.snapshot();
        if analysis.has_blocking_errors() {
            return Ok((
                CallDependencyNode {
                    abi: None,
                    dependencies,
                    locally_valid: false,
                    lowering: None,
                },
                false,
            ));
        }
        let interface_projection = state.interface_projection();
        let semantic = match analysis.validated(provisional_semantic) {
            Ok(semantic) => semantic_for_lowering(self.registry, semantic),
            Err(_) => {
                return Ok((
                    CallDependencyNode {
                        abi: None,
                        dependencies,
                        locally_valid: false,
                        lowering: None,
                    },
                    false,
                ));
            }
        };
        let abi =
            match derive_function_abi(self.registry, &semantic, &interface_projection, &provenance)
            {
                Ok(Some(abi)) => abi,
                Ok(None) | Err(_) => return Ok((CallDependencyNode::invalid(), false)),
            };
        Ok((
            CallDependencyNode {
                abi: Some(abi),
                dependencies,
                locally_valid: nested_diagnostics.is_empty(),
                lowering: Some(CallDependencyLowering {
                    document,
                    semantic,
                    prepared_configs,
                    decoded_literals,
                    interface_projection,
                    provenance,
                }),
            },
            false,
        ))
    }

    fn call_targets(
        &self,
        graph: &CompilerSemanticGraph,
        diagnostics: &mut Vec<CompilerNodeDiagnostic>,
    ) -> BTreeMap<GraphResourcePath, Vec<NodeId>> {
        let mut targets = BTreeMap::<GraphResourcePath, Vec<NodeId>>::new();
        for node in graph.nodes.iter() {
            let resolved = match resolve_for_lowering(self.registry, node) {
                Ok(resolved) => resolved,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if resolved.structural_role() != Some(StructuralNodeRole::Call) {
                continue;
            }
            let Some(target) = function_target(&node.normalized_parameters) else {
                continue;
            };
            targets
                .entry(GraphResourcePath(target.into()))
                .or_default()
                .push(node.node_id);
        }
        targets
    }

    fn push_call_abi_invalid(
        &self,
        target: &GraphResourcePath,
        sites: &[NodeId],
        diagnostics: &mut Vec<CompilerNodeDiagnostic>,
    ) {
        diagnostics.extend(sites.iter().map(|&node_id| {
            CompilerDiagnostic::ControlCallAbiInvalid {
                function_path: target.0.clone(),
            }
            .into_node(DiagnosticLocation::Node(node_id))
        }));
    }
}

#[derive(Default)]
pub(super) struct CallClosureAnalysis {
    pub(super) abis: BTreeMap<GraphResourcePath, FunctionPlanAbi>,
    pub(super) diagnostics: Vec<CompilerNodeDiagnostic>,
}

enum CallAbiFinalizationFailure {
    Cancelled(CompileCancelled),
    Invalid,
}

struct CallDependencyNode {
    abi: Option<FunctionPlanAbi>,
    dependencies: BTreeSet<GraphResourcePath>,
    locally_valid: bool,
    lowering: Option<CallDependencyLowering>,
}

struct CallDependencyLowering {
    document: GraphDocument,
    semantic: CompilerSemanticGraph,
    prepared_configs: BTreeMap<NodeId, ValidatedNodeConfig>,
    decoded_literals: BTreeMap<PortAddress, crate::node_system::protocol::TypedValue>,
    interface_projection: ValidatedInterfaceProjection,
    provenance: CompileProvenance,
}

impl CallDependencyNode {
    fn invalid() -> Self {
        Self {
            abi: None,
            dependencies: BTreeSet::new(),
            locally_valid: false,
            lowering: None,
        }
    }
}

fn invalid_call_targets(
    graph: &BTreeMap<GraphResourcePath, CallDependencyNode>,
) -> BTreeSet<GraphResourcePath> {
    let components = strongly_connected_call_components(graph);
    let component_by_target = components
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.iter().cloned().map(move |target| (target, index)))
        .collect::<BTreeMap<_, _>>();
    let mut invalid_components = components
        .iter()
        .map(|component| {
            component.iter().any(|target| {
                graph
                    .get(target)
                    .is_none_or(|node| !node.locally_valid || node.abi.is_none())
            })
        })
        .collect::<Vec<_>>();

    loop {
        let newly_invalid = graph
            .iter()
            .filter_map(|(target, node)| {
                let component = component_by_target[target];
                (!invalid_components[component]
                    && node.dependencies.iter().any(|dependency| {
                        component_by_target
                            .get(dependency)
                            .is_none_or(|dependency| invalid_components[*dependency])
                    }))
                .then_some(component)
            })
            .collect::<BTreeSet<_>>();
        if newly_invalid.is_empty() {
            break;
        }
        for component in newly_invalid {
            invalid_components[component] = true;
        }
    }

    components
        .into_iter()
        .enumerate()
        .filter(|(index, _)| invalid_components[*index])
        .flat_map(|(_, component)| component)
        .collect()
}

fn strongly_connected_call_components(
    graph: &BTreeMap<GraphResourcePath, CallDependencyNode>,
) -> Vec<Vec<GraphResourcePath>> {
    fn visit(
        target: &GraphResourcePath,
        graph: &BTreeMap<GraphResourcePath, CallDependencyNode>,
        visited: &mut BTreeSet<GraphResourcePath>,
        order: &mut Vec<GraphResourcePath>,
    ) {
        if !visited.insert(target.clone()) {
            return;
        }
        if let Some(node) = graph.get(target) {
            for dependency in &node.dependencies {
                if graph.contains_key(dependency) {
                    visit(dependency, graph, visited, order);
                }
            }
        }
        order.push(target.clone());
    }

    fn visit_reverse(
        target: &GraphResourcePath,
        reverse: &BTreeMap<GraphResourcePath, BTreeSet<GraphResourcePath>>,
        visited: &mut BTreeSet<GraphResourcePath>,
        component: &mut Vec<GraphResourcePath>,
    ) {
        if !visited.insert(target.clone()) {
            return;
        }
        component.push(target.clone());
        if let Some(dependents) = reverse.get(target) {
            for dependent in dependents {
                visit_reverse(dependent, reverse, visited, component);
            }
        }
    }

    let mut order = Vec::with_capacity(graph.len());
    let mut visited = BTreeSet::new();
    for target in graph.keys() {
        visit(target, graph, &mut visited, &mut order);
    }
    let mut reverse = graph
        .keys()
        .cloned()
        .map(|target| (target, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (target, node) in graph {
        for dependency in &node.dependencies {
            if let Some(dependents) = reverse.get_mut(dependency) {
                dependents.insert(target.clone());
            }
        }
    }
    visited.clear();
    let mut components = Vec::new();
    while let Some(target) = order.pop() {
        if visited.contains(&target) {
            continue;
        }
        let mut component = Vec::new();
        visit_reverse(&target, &reverse, &mut visited, &mut component);
        components.push(component);
    }
    components
}
