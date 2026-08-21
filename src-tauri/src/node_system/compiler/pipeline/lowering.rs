#[path = "lowering/control.rs"]
mod control;
use super::function_abi::{allocate_port_values, port_contract};
use super::*;
use control::{collect_control_value_sources, deduplicate_region_operations};

pub(super) enum LowerGraphFailure {
    Cancelled(CompileCancelled),
    Internal(InternalCompilationFailure),
}

impl From<CompileCancelled> for LowerGraphFailure {
    fn from(error: CompileCancelled) -> Self {
        Self::Cancelled(error)
    }
}

impl From<CompilerNodeDiagnostic> for LowerGraphFailure {
    fn from(diagnostic: CompilerNodeDiagnostic) -> Self {
        let node_id = match &diagnostic.primary {
            DiagnosticLocation::Node(node_id) => Some(*node_id),
            DiagnosticLocation::Port(port) => Some(port.node_id),
            DiagnosticLocation::Parameter { node_id, .. } => Some(*node_id),
            DiagnosticLocation::Graph
            | DiagnosticLocation::Connection(_)
            | DiagnosticLocation::Resource(_) => None,
        };
        Self::Internal(InternalCompilationFailure {
            stage: CompilationStage::Lowering,
            code: diagnostic.code.as_str().into(),
            node_id,
        })
    }
}

#[derive(Clone)]
struct PendingOperation {
    stable_id: OperationStableId,
    node_id: NodeId,
    node_type_id: NodeTypeId,
    has_control_or_effect_ports: bool,
    kernel: PendingKernel,
    input_ports: Box<[PortAddress]>,
    inputs: Box<[PlannedInput]>,
    output_ports: Box<[PortAddress]>,
    outputs: Box<[PlannedOutput]>,
    parameters: CompiledParameterHandle,
    resource_dependencies: Box<[ResourceKey]>,
    cache_policy: CachePolicy,
    semantics_version: ExecutionSemanticsVersion,
    workload: WorkloadClass,
    retry: PlannedRetry,
    evaluation: EvaluationPolicy,
    purity: Purity,
    effects: EffectSemantics,
    resources: Box<[CompiledResourceRequirement]>,
}

#[derive(Clone)]
pub(crate) enum PendingKernel {
    Native(KernelHandle),
    Relational,
}

#[derive(Clone)]
struct PendingRelationalFragment {
    backend: RelationalBackendId,
    fragment: RelationalFragment,
    inputs: BTreeMap<PortAddress, crate::node_system::plan::RelationalOperatorIndex>,
}

pub(crate) fn effective_cache_policy(
    requested: CachePolicy,
    determinism: Determinism,
    purity: Purity,
    effects: EffectSemantics,
) -> CachePolicy {
    if determinism == Determinism::Deterministic
        && purity == Purity::Pure
        && effects == EffectSemantics::None
    {
        requested
    } else {
        CachePolicy::Disabled
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn effective_retry_policy(
    idempotent: bool,
    policy: Option<RetryPolicy>,
    determinism: Determinism,
    purity: Purity,
    effects: EffectSemantics,
    has_control_or_effect_ports: bool,
    kernel: &PendingKernel,
    resources: &[CompiledResourceRequirement],
) -> PlannedRetry {
    let compiler_approved = idempotent
        && policy.is_some()
        && determinism == Determinism::Deterministic
        && purity == Purity::Pure
        && effects == EffectSemantics::None
        && !has_control_or_effect_ports
        && matches!(kernel, PendingKernel::Native(_))
        && resources.is_empty();
    if compiler_approved {
        PlannedRetry { idempotent, policy }
    } else {
        PlannedRetry::default()
    }
}

fn fragment_metadata_identity(metadata: &FragmentMetadata) -> serde_json::Value {
    serde_json::json!({
        "effect": &metadata.effect,
        "resources": &metadata.resources,
        "results": &metadata.results.iter().map(|result| serde_json::json!({
            "name": &result.name,
            "output": &result.output,
        })).collect::<Vec<_>>(),
    })
}

fn lowered_kernel_identity(kernel: &LoweredKernel) -> serde_json::Value {
    match kernel {
        LoweredKernel::Native(handle) => serde_json::json!({
            "kind": "native",
            "handle": handle,
        }),
        LoweredKernel::Scalar(fragment) => serde_json::json!({
            "kind": "scalar",
            "handle": &fragment.kernel,
            "metadata": fragment_metadata_identity(&fragment.metadata),
        }),
        LoweredKernel::Kernel(fragment) => serde_json::json!({
            "kind": "kernel",
            "handle": &fragment.kernel,
            "metadata": fragment_metadata_identity(&fragment.metadata),
        }),
        LoweredKernel::Relational(fragment) => serde_json::json!({
            "kind": "relational",
            "backend": &fragment.backend,
            "fragment": {
                "id": &fragment.fragment.id,
                "operators": &fragment.fragment.operators,
                "root": fragment.fragment.root,
            },
            "inputs": fragment.inputs.iter().map(|input| serde_json::json!({
                "port": &input.port,
                "operator": input.operator,
            })).collect::<Vec<_>>(),
            "metadata": fragment_metadata_identity(&fragment.metadata),
        }),
    }
}

fn lowering_identity_failure(node_id: NodeId) -> LowerGraphFailure {
    LowerGraphFailure::Internal(InternalCompilationFailure {
        stage: CompilationStage::Lowering,
        code: CompilerDiagnostic::LoweringExecutionIdentity {}
            .definition()
            .code
            .into(),
        node_id: Some(node_id),
    })
}

fn effective_workload_class(
    kernel: &PendingKernel,
    purity: Purity,
    effects: EffectSemantics,
    resources: &BTreeMap<ResourceId, CompiledResourceRequirement>,
) -> WorkloadClass {
    if purity == Purity::Effectful
        || effects != EffectSemantics::None
        || resources
            .values()
            .any(|requirement| requirement.access == ResourceAccess::Exclusive)
    {
        WorkloadClass::Exclusive
    } else if matches!(kernel, PendingKernel::Relational) || !resources.is_empty() {
        WorkloadClass::Io
    } else {
        WorkloadClass::Cpu
    }
}

pub(super) fn structural_role_name(role: StructuralNodeRole) -> &'static str {
    match role {
        StructuralNodeRole::EventBegin => "event_begin",
        StructuralNodeRole::FunctionEntry => "function_entry",
        StructuralNodeRole::FunctionReturn => "function_return",
        StructuralNodeRole::Branch => "branch",
        StructuralNodeRole::Loop => "loop",
        StructuralNodeRole::Sequence => "sequence",
        StructuralNodeRole::Call => "call",
    }
}

pub(super) fn call_member_role(template: &str) -> &'static str {
    match template {
        "arguments" => "argument",
        "results" => "result",
        _ => "member",
    }
}

pub(super) fn protocol_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Integer(value) => serde_json::Value::Number((*value).into()),
        Value::Unsigned(value) => serde_json::Value::Number((*value).into()),
        Value::Decimal(value) => serde_json::Value::String(value.as_str().to_owned()),
        Value::String(value) => serde_json::Value::String(value.as_ref().to_owned()),
        Value::Bytes(value) => serde_json::Value::Array(
            value
                .iter()
                .map(|value| serde_json::Value::Number(u64::from(*value).into()))
                .collect(),
        ),
        Value::List(values) => {
            serde_json::Value::Array(values.iter().map(protocol_value_to_json).collect())
        }
        Value::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.to_string(), protocol_value_to_json(value)))
                .collect(),
        ),
    }
}

pub(super) fn function_target(
    parameters: &BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
) -> Option<&str> {
    ["target", "function_plan", "function"]
        .into_iter()
        .find_map(|name| {
            parameters
                .iter()
                .find(|(key, _)| key.as_str() == name)
                .and_then(|(_, value)| value.as_str())
        })
        .filter(|target| !target.is_empty() && target.trim() == *target)
}

pub(super) fn lower_graph<R: CompilerRegistry>(
    registry: &R,
    document: &GraphDocument,
    graph: &CompilerSemanticGraph,
    prepared_configs: &BTreeMap<NodeId, ValidatedNodeConfig>,
    decoded_literals: &BTreeMap<PortAddress, crate::node_system::protocol::TypedValue>,
    interface_projection: &ValidatedInterfaceProjection,
    function_abis: &BTreeMap<GraphResourcePath, FunctionPlanAbi>,
    provenance: CompileProvenance,
    cancellation: &CompileCancellationToken,
) -> Result<(ExecutionPlanBasis, Option<ExecutionPlan>), LowerGraphFailure> {
    cancellation.checkpoint()?;
    let (next_value, port_values) = allocate_port_values(registry, graph)?;
    let mut production_by_port = BTreeMap::new();
    let mut consumption_by_port = BTreeMap::new();
    let mut value_contracts = BTreeMap::new();
    let mut structural_inputs = BTreeSet::new();
    let mut structural_outputs = BTreeSet::new();
    let mut value_sources = BTreeSet::new();
    let mut unbound_inputs = BTreeMap::new();
    let mut bound_values = BTreeMap::new();
    for node in graph.nodes.iter() {
        cancellation.checkpoint()?;
        let resolved = resolve_for_lowering(registry, node)?;
        let structural_role = resolved.structural_role();
        for port in node.ports.iter() {
            let spec = protocol_port(resolved.protocol, &port.address);
            if spec.kind == PortKind::Data {
                let value = port_values[&port.address];
                value_contracts.insert(value, port_contract(port, &spec.value_type)?);
                if let Some(role) = structural_role {
                    match spec.direction {
                        PortDirection::Input => {
                            structural_inputs.insert(port.address.clone());
                        }
                        PortDirection::Output => {
                            structural_outputs.insert(port.address.clone());
                            if matches!(
                                role,
                                StructuralNodeRole::EventBegin | StructuralNodeRole::FunctionEntry
                            ) {
                                value_sources.insert(PlanValueSource::ExternalInput(
                                    value,
                                    spec.production
                                        .unwrap_or(OutputProduction::FullyMaterialized),
                                ));
                            }
                        }
                    }
                }
                match spec.direction {
                    PortDirection::Output => {
                        production_by_port.insert(
                            port.address.clone(),
                            spec.production
                                .unwrap_or(OutputProduction::FullyMaterialized),
                        );
                    }
                    PortDirection::Input => {
                        consumption_by_port.insert(
                            port.address.clone(),
                            spec.consumption
                                .unwrap_or(InputConsumption::FullyMaterialized),
                        );
                        let has_connection = document
                            .connections
                            .values()
                            .any(|connection| connection.input == port.address);
                        let bound_value = if has_connection {
                            None
                        } else if let Some(literal) = decoded_literals.get(&port.address) {
                            Some(literal.value.clone())
                        } else {
                            spec.input_binding
                                .as_ref()
                                .and_then(|binding| binding.default_value.as_ref())
                                .map(|default| default.value.clone())
                        };
                        if let Some(bound_value) = bound_value {
                            if structural_role.is_some() {
                                bound_values.insert(value, bound_value);
                            }
                        } else if !has_connection {
                            unbound_inputs.insert(value, port.address.clone());
                        }
                    }
                }
            }
        }
    }

    let mut pending_operations = Vec::new();
    let mut operation_by_node = BTreeMap::new();
    let mut operation_inputs = BTreeMap::new();
    let mut operation_outputs = BTreeMap::new();
    let mut relational_by_node = BTreeMap::new();
    let mut resources = BTreeMap::<_, CompiledResourceRequirement>::new();
    let mut results = BTreeMap::<Box<str>, PlanResult>::new();
    for node in graph.nodes.iter() {
        cancellation.checkpoint()?;
        let resolved = resolve_for_lowering(registry, node)?;
        if resolved.structural_role().is_some() {
            continue;
        }
        let implementation = resolved.implementation().ok_or_else(|| {
            CompilerDiagnostic::LoweringImplementationMissing {
                node_type: node.node_type_id.to_string().into(),
            }
            .into_node(DiagnosticLocation::Node(node.node_id))
        })?;
        let prepared_config = prepared_configs.get(&node.node_id).ok_or_else(|| {
            CompilerDiagnostic::LoweringInternalInvariant {
                node_type: node.node_type_id.to_string().into(),
            }
            .into_node(DiagnosticLocation::Node(node.node_id))
        })?;
        let mut inputs = Vec::new();
        let mut planned_inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut planned_outputs = Vec::new();
        for port in node.ports.iter() {
            let spec = protocol_port(resolved.protocol, &port.address);
            if spec.kind != PortKind::Data {
                continue;
            }
            let value = port_values[&port.address];
            match spec.direction {
                PortDirection::Output => {
                    outputs.push((port.address.clone(), value));
                    planned_outputs.push(PlannedOutput {
                        value,
                        contract: value_contracts[&value].clone(),
                        production: spec
                            .production
                            .unwrap_or(OutputProduction::FullyMaterialized),
                        public_output: Some(GraphOutputRef {
                            graph_path: provenance.graph_path.clone(),
                            port: port.address.clone(),
                        }),
                        presentation: crate::node_system::plan::presentation_for_output(
                            node.node_type_id.as_str(),
                            spec.key.as_str(),
                        ),
                    });
                }
                PortDirection::Input => {
                    inputs.push((port.address.clone(), value));
                    let has_connection = document
                        .connections
                        .values()
                        .any(|connection| connection.input == port.address);
                    let bound_value = if has_connection {
                        None
                    } else if let Some(literal) = decoded_literals.get(&port.address) {
                        Some(literal.value.clone())
                    } else {
                        spec.input_binding
                            .as_ref()
                            .and_then(|binding| binding.default_value.as_ref())
                            .map(|default| default.value.clone())
                    };
                    planned_inputs.push(PlannedInput {
                        value,
                        contract: value_contracts[&value].clone(),
                        consumption: spec
                            .consumption
                            .unwrap_or(InputConsumption::FullyMaterialized),
                        bound_value,
                    });
                }
            }
        }
        let context = LoweringContext {
            cancellation,
            node_id: node.node_id,
            protocol: resolved.protocol,
            parameters: prepared_config,
            inputs: &inputs,
            outputs: &outputs,
        };
        let lowered = match implementation.lowerer.lower(&context) {
            Ok(lowered) => lowered,
            Err(LoweringError::Cancelled(error)) => {
                return Err(LowerGraphFailure::Cancelled(error));
            }
            Err(error) => {
                let node_type = node.node_type_id.to_string().into();
                let diagnostic = match error {
                    LoweringError::InternalInvariant(_) => {
                        CompilerDiagnostic::LoweringInternalInvariant { node_type }
                    }
                    LoweringError::DeadlineExceeded => {
                        CompilerDiagnostic::LoweringDeadlineExceeded { node_type }
                    }
                    LoweringError::ResourceExhausted => {
                        CompilerDiagnostic::LoweringResourceExhausted { node_type }
                    }
                    LoweringError::Cancelled(_) => unreachable!("handled above"),
                };
                return Err(diagnostic
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
            }
        };
        let mut owned_resources = BTreeMap::<ResourceId, CompiledResourceRequirement>::new();
        if let Some(metadata) = lowered.kernel.metadata() {
            if metadata.effect != resolved.protocol.execution.effects {
                return Err(CompilerDiagnostic::LoweringEffectContract {}
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
            }
            for requirement in &metadata.resources {
                owned_resources.insert(requirement.resource.clone(), requirement.clone());
                if let Some(previous) =
                    resources.insert(requirement.resource.clone(), requirement.clone())
                {
                    if previous != *requirement {
                        return Err(CompilerDiagnostic::LoweringResourceConflict {
                            resource_id: requirement.resource.as_str().into(),
                        }
                        .into_node(DiagnosticLocation::Node(node.node_id))
                        .into());
                    }
                }
            }
            for result in &metadata.results {
                let Some(&value) = port_values.get(&result.output) else {
                    return Err(CompilerDiagnostic::LoweringResultPort {
                        port: result.output.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                };
                if !outputs.iter().any(|(address, _)| address == &result.output) {
                    return Err(CompilerDiagnostic::LoweringResultPort {
                        port: result.output.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                }
                if results
                    .insert(
                        result.name.clone(),
                        PlanResult {
                            name: result.name.clone(),
                            output: GraphOutputRef {
                                graph_path: provenance.graph_path.clone(),
                                port: result.output.clone(),
                            },
                            value,
                        },
                    )
                    .is_some()
                {
                    return Err(CompilerDiagnostic::LoweringResultDuplicate {
                        result_name: result.name.clone(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                }
            }
        }

        for parameter in resolved.protocol.parameters.parameters.iter() {
            if !matches!(parameter.editor, ParameterEditorSpec::Resource { .. }) {
                continue;
            }
            let Some(resource) = prepared_config.resource(&parameter.key).cloned() else {
                if node.normalized_parameters.contains_key(&parameter.key) {
                    return Err(CompilerDiagnostic::LoweringInternalInvariant {
                        node_type: node.node_type_id.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                }
                continue;
            };
            let kind = match parameter.key.as_str() {
                "dataframe" => ResourceKind::DatabaseConnection,
                "variable" => ResourceKind::ExternalArtifact,
                "function" => continue,
                _ => continue,
            };
            let access = if node.node_type_id.as_str() == "yssbi.project.variable.set" {
                ResourceAccess::Exclusive
            } else {
                ResourceAccess::Shared
            };
            let requirement = CompiledResourceRequirement {
                resource: resource.clone(),
                kind,
                access,
                optional: false,
            };
            owned_resources.insert(resource.clone(), requirement.clone());
            if let Some(previous) = resources.insert(resource, requirement.clone()) {
                if previous != requirement {
                    return Err(CompilerDiagnostic::LoweringResourceConflict {
                        resource_id: requirement.resource.as_str().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                }
            }
        }

        let operation = OperationIndex::new(pending_operations.len() as u32);
        operation_by_node.insert(node.node_id, operation);
        for (address, _) in &inputs {
            operation_inputs.insert(address.clone(), operation);
        }
        for (address, _) in &outputs {
            operation_outputs.insert(address.clone(), operation);
        }
        let stable_id = OperationStableId::from_digest(
            hash_canonical(
                "yssbi.operation-stable-id.node.v2",
                &serde_json::json!({
                    "graphPath": &provenance.graph_path,
                    "nodeId": node.node_id,
                }),
            )
            .map_err(|_| lowering_identity_failure(node.node_id))?,
        );
        let semantics_version = ExecutionSemanticsVersion::from_bytes(
            hash_canonical(
                "yssbi.execution-semantics.native.v2",
                &serde_json::json!({
                    "schemaVersion": EXECUTION_SEMANTICS_SCHEMA_VERSION,
                    "registryFingerprint": &provenance.basis.registry_fingerprint,
                    "protocolFingerprint": &node.protocol_fingerprint,
                    "nodeTypeId": &node.node_type_id,
                    "execution": &resolved.protocol.execution,
                    "kernel": lowered_kernel_identity(&lowered.kernel),
                    "compiledParameters": &lowered.parameters,
                    "normalizedParameters": &node.normalized_parameters,
                    "inputPorts": &inputs,
                    "inputs": &planned_inputs,
                    "outputPorts": &outputs,
                    "outputs": &planned_outputs,
                }),
            )
            .map_err(|_| lowering_identity_failure(node.node_id))?,
        );
        let kernel = match lowered.kernel {
            LoweredKernel::Native(handle) => PendingKernel::Native(handle),
            LoweredKernel::Scalar(fragment) => PendingKernel::Native(fragment.kernel),
            LoweredKernel::Kernel(fragment) => PendingKernel::Native(fragment.kernel),
            LoweredKernel::Relational(fragment) => {
                if relational_by_node
                    .insert(
                        node.node_id,
                        PendingRelationalFragment {
                            backend: fragment.backend,
                            fragment: fragment.fragment,
                            inputs: fragment
                                .inputs
                                .into_vec()
                                .into_iter()
                                .map(|binding| (binding.port, binding.operator))
                                .collect(),
                        },
                    )
                    .is_some()
                {
                    unreachable!("one lowering result per semantic node");
                }
                PendingKernel::Relational
            }
        };
        let execution = resolved.protocol.execution;
        let cache_policy = effective_cache_policy(
            execution.cache,
            execution.determinism,
            execution.purity,
            execution.effects,
        );
        let workload = effective_workload_class(
            &kernel,
            execution.purity,
            execution.effects,
            &owned_resources,
        );
        let has_control_or_effect_ports = resolved
            .protocol
            .interface
            .ports
            .iter()
            .any(|port| port.kind != PortKind::Data);
        let retry = effective_retry_policy(
            execution.idempotent,
            execution.retry,
            execution.determinism,
            execution.purity,
            execution.effects,
            has_control_or_effect_ports,
            &kernel,
            owned_resources
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let resource_dependencies = owned_resources
            .keys()
            .map(|resource| ResourceKey::new(resource.as_str()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        pending_operations.push(PendingOperation {
            stable_id,
            node_id: node.node_id,
            node_type_id: node.node_type_id.clone(),
            has_control_or_effect_ports,
            kernel,
            input_ports: inputs
                .into_iter()
                .map(|(address, _)| address)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            inputs: planned_inputs.into_boxed_slice(),
            output_ports: outputs
                .into_iter()
                .map(|(address, _)| address)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            outputs: planned_outputs.into_boxed_slice(),
            parameters: lowered.parameters,
            resource_dependencies,
            cache_policy,
            semantics_version,
            workload,
            retry,
            evaluation: execution.evaluation,
            purity: execution.purity,
            effects: execution.effects,
            resources: owned_resources
                .into_values()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        });
    }

    cancellation.checkpoint()?;
    let mut value_dependencies = Vec::new();
    let mut effect_dependencies = Vec::new();
    let mut relational_connections = Vec::new();
    for dependency in graph.dependencies.iter() {
        match dependency {
            SemanticDependency::Value(edge) => {
                let Some(&source) = port_values.get(&edge.source) else {
                    return Err(CompilerDiagnostic::PlanValueProducerMissing {
                        port: edge.source.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
                    .into());
                };
                let Some(&destination) = port_values.get(&edge.target) else {
                    return Err(CompilerDiagnostic::PlanValueConsumerMissing {
                        port: edge.target.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
                    .into());
                };
                if !operation_outputs.contains_key(&edge.source)
                    && !structural_outputs.contains(&edge.source)
                {
                    return Err(CompilerDiagnostic::PlanValueProducerMissing {
                        port: edge.source.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
                    .into());
                }
                if !operation_inputs.contains_key(&edge.target)
                    && !structural_inputs.contains(&edge.target)
                {
                    return Err(CompilerDiagnostic::PlanValueConsumerMissing {
                        port: edge.target.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
                    .into());
                }
                value_dependencies.push(ValueDependency {
                    source,
                    destination,
                });

                if let (Some(producer), Some(consumer)) = (
                    relational_by_node.get(&edge.source.node_id),
                    relational_by_node.get(&edge.target.node_id),
                ) {
                    let Some(&consumer_input) = consumer.inputs.get(&edge.target) else {
                        return Err(CompilerDiagnostic::RelationalInputBindingMissing {
                            port: edge.target.to_string().into(),
                        }
                        .into_node(DiagnosticLocation::Connection(edge.connection_id))
                        .into());
                    };
                    relational_connections.push(RelationalConnection {
                        producer: producer.fragment.id.clone(),
                        consumer: consumer.fragment.id.clone(),
                        consumer_input,
                        production: production_by_port[&edge.source],
                        consumption: consumption_by_port[&edge.target],
                    });
                }
            }
            SemanticDependency::Effect(edge) => {
                let Some(&before) = operation_by_node.get(&edge.predecessor) else {
                    return Err(CompilerDiagnostic::PlanEffectProducerMissing {
                        port: edge.effect_key.clone(),
                    }
                    .into_node(DiagnosticLocation::Node(edge.predecessor))
                    .into());
                };
                let Some(&after) = operation_by_node.get(&edge.successor) else {
                    return Err(CompilerDiagnostic::PlanEffectConsumerMissing {
                        port: edge.effect_key.clone(),
                    }
                    .into_node(DiagnosticLocation::Node(edge.successor))
                    .into());
                };
                effect_dependencies.push(PlannedEffectDependency { before, after });
            }
            SemanticDependency::Control(_) => {}
        }
    }
    value_dependencies.sort_by_key(|dependency| (dependency.source, dependency.destination));
    value_dependencies.dedup();
    effect_dependencies.sort_by_key(|dependency| (dependency.before, dependency.after));
    effect_dependencies.dedup();

    cancellation.checkpoint()?;
    let mut port_facts = BTreeMap::new();
    let mut nodes = BTreeSet::new();
    let mut output_results = BTreeMap::new();
    for node in graph.nodes.iter() {
        nodes.insert(node.node_id);
        let resolved = resolve_for_lowering(registry, node)?;
        for port in node.ports.iter() {
            let spec = protocol_port(resolved.protocol, &port.address);
            port_facts.insert(
                port.address.clone(),
                DemandPortFact {
                    kind: spec.kind,
                    direction: spec.direction,
                },
            );
            if spec.kind == PortKind::Data && spec.direction == PortDirection::Output {
                let output = GraphOutputRef {
                    graph_path: provenance.graph_path.clone(),
                    port: port.address.clone(),
                };
                output_results.insert(
                    output.clone(),
                    PlanResult {
                        name: format!("requested.{}", port.address).into(),
                        output,
                        value: port_values[&port.address],
                    },
                );
            }
        }
    }
    let default_outputs = results
        .values()
        .map(|result| result.output.clone())
        .collect::<BTreeSet<_>>();
    for result in results.values() {
        output_results.insert(result.output.clone(), result.clone());
    }
    let control_nodes = graph
        .nodes
        .iter()
        .map(|node| {
            let resolved = resolve_for_lowering(registry, node)?;
            Ok((
                node.node_id,
                ControlNode {
                    node_id: node.node_id,
                    role: resolved.structural_role(),
                    protocol: resolved.protocol,
                    parameters: prepared_configs.get(&node.node_id).ok_or_else(|| {
                        CompilerDiagnostic::LoweringInternalInvariant {
                            node_type: node.node_type_id.to_string().into(),
                        }
                        .into_node(DiagnosticLocation::Node(node.node_id))
                    })?,
                    ports: node
                        .ports
                        .iter()
                        .map(|port| port.address.clone())
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    values: node
                        .ports
                        .iter()
                        .filter_map(|port| {
                            port_values
                                .get(&port.address)
                                .copied()
                                .map(|value| (port.address.clone(), value))
                        })
                        .collect(),
                    dynamic_members: interface_projection
                        .nodes
                        .get(&node.node_id)
                        .into_iter()
                        .flat_map(|projection| projection.projected_bindings.iter())
                        .filter_map(|(address, binding)| match binding {
                            ProjectedDynamicPortBinding::Resolved { origin, .. } => {
                                Some((address.clone(), origin.clone()))
                            }
                            ProjectedDynamicPortBinding::Orphan { .. } => None,
                        })
                        .collect(),
                    operation: operation_by_node.get(&node.node_id).copied(),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, NodeDiagnostic<_, _, _, _>>>()?;
    let control_edges = graph
        .dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            SemanticDependency::Control(edge) => Some(RegionControlEdge {
                source: edge.source_port.clone(),
                target: edge.target_port.clone(),
            }),
            _ => None,
        })
        .collect();
    cancellation.checkpoint()?;
    let mut root_region = build_control_region(control_nodes, control_edges, function_abis)
        .map_err(|issue| {
            issue.diagnostic.into_node(
                issue
                    .node_id
                    .map(DiagnosticLocation::Node)
                    .unwrap_or(DiagnosticLocation::Graph),
            )
        })?;
    deduplicate_region_operations(&mut root_region);
    let mut production_by_value = production_by_port
        .iter()
        .map(|(port, production)| (port_values[port], *production))
        .collect::<BTreeMap<_, _>>();
    for dependency in &value_dependencies {
        if let Some(production) = production_by_value.get(&dependency.source).copied() {
            production_by_value.insert(dependency.destination, production);
        }
    }
    collect_control_value_sources(
        &mut root_region,
        &mut value_sources,
        &mut production_by_value,
        &mut value_contracts,
        function_abis,
    )?;
    debug_assert_eq!(provenance.basis, graph.basis);
    let operations = pending_operations
        .into_iter()
        .map(|pending| {
            let kernel = match pending.kernel {
                PendingKernel::Native(handle) => IntermediateKernel::Native(handle),
                PendingKernel::Relational => {
                    let relational = relational_by_node
                        .remove(&pending.node_id)
                        .expect("relational lowering fact belongs to its operation");
                    IntermediateKernel::Relational {
                        backend: relational.backend,
                        fragment: relational.fragment,
                        input_bindings: relational.inputs,
                    }
                }
            };
            IntermediateOperation {
                stable_id: pending.stable_id,
                source_node_id: pending.node_id,
                source_node_type_id: pending.node_type_id,
                has_control_or_effect_ports: pending.has_control_or_effect_ports,
                kernel,
                input_ports: pending.input_ports,
                inputs: pending.inputs,
                output_ports: pending.output_ports,
                outputs: pending.outputs,
                params: pending.parameters,
                resource_dependencies: pending.resource_dependencies,
                cache_policy: pending.cache_policy,
                semantics_version: pending.semantics_version,
                workload: pending.workload,
                retry: pending.retry,
                evaluation: pending.evaluation,
                purity: pending.purity,
                effects: pending.effects,
                resources: pending.resources,
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let basis = ExecutionPlanBasis {
        provenance,
        value_count: next_value,
        operations,
        value_contracts,
        value_sources: value_sources
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        value_dependencies: value_dependencies.into_boxed_slice(),
        effect_dependencies: effect_dependencies
            .into_iter()
            .map(|dependency| (dependency.before.index(), dependency.after.index()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        root_region,
        relational_connections: relational_connections.into_boxed_slice(),
        port_facts,
        unbound_inputs,
        bound_values,
        nodes,
        output_results,
        default_outputs,
    };
    cancellation.checkpoint()?;
    let plan =
        if basis.unbound_inputs.is_empty() {
            Some(basis.derive_full_plan().map_err(|_| {
                CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
            })?)
        } else {
            None
        };
    Ok((basis, plan))
}
