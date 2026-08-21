use super::*;

pub(super) fn allocate_port_values<R: CompilerRegistry>(
    registry: &R,
    graph: &CompilerSemanticGraph,
) -> Result<(u32, BTreeMap<PortAddress, ValueRef>), CompilerNodeDiagnostic> {
    let mut next_value = 0u32;
    let mut values = BTreeMap::new();
    for node in graph.nodes.iter() {
        let resolved = resolve_for_lowering(registry, node)?;
        for port in node.ports.iter() {
            if protocol_port(resolved.protocol, &port.address).kind == PortKind::Data {
                values.insert(port.address.clone(), ValueRef::new(next_value));
                next_value += 1;
            }
        }
    }
    Ok((next_value, values))
}

#[derive(Debug)]
pub(crate) enum FinalizeFunctionAbiError {
    InvalidPlan,
    MissingResultProduction,
}

pub(crate) fn finalize_function_abi_productions(
    plan: &ExecutionPlan,
    abi: &mut FunctionPlanAbi,
) -> Result<(), FinalizeFunctionAbiError> {
    let source_facts = plan
        .validate_with_source_facts()
        .map_err(|_| FinalizeFunctionAbiError::InvalidPlan)?;
    let mut productions = BTreeMap::new();
    for (result, value) in &abi.results {
        let production = source_facts
            .production(*value)
            .ok_or(FinalizeFunctionAbiError::MissingResultProduction)?;
        productions.insert(result.clone(), production);
    }
    abi.result_productions = productions;
    Ok(())
}

fn planned_value_contract(type_expr: &TypeExpr) -> Option<PlannedValueContract> {
    if type_expr_contains_unresolved(type_expr) {
        return None;
    }
    let kind = match type_expr {
        TypeExpr::Applied { constructor, .. }
            if constructor.as_str() == crate::node_system::protocol::DATA_SERIES_CONSTRUCTOR_ID =>
        {
            PlannedValueKind::DataSeries
        }
        TypeExpr::Concrete(id) if id.as_str() == "tabular.dataframe" => PlannedValueKind::DataFrame,
        TypeExpr::Concrete(id)
            if matches!(
                id.as_str(),
                "core.bool"
                    | "core.int64"
                    | "core.float64"
                    | "core.string"
                    | "core.date"
                    | "core.datetime"
                    | "core.time"
                    | "core.categorical"
            ) =>
        {
            PlannedValueKind::Scalar
        }
        TypeExpr::Union(members) => {
            let mut kinds = members
                .iter()
                .map(planned_value_contract)
                .map(|contract| contract.map(|contract| contract.kind));
            let first = kinds.next().flatten()?;
            if kinds.all(|kind| kind == Some(first)) {
                first
            } else {
                return None;
            }
        }
        TypeExpr::Concrete(_) | TypeExpr::Applied { .. } => PlannedValueKind::Opaque,
        TypeExpr::Generic(_) | TypeExpr::Unknown => return None,
    };
    Some(PlannedValueContract {
        kind,
        type_expr: type_expr.clone(),
    })
}

fn type_expr_contains_unresolved(type_expr: &TypeExpr) -> bool {
    match type_expr {
        TypeExpr::Generic(_) | TypeExpr::Unknown => true,
        TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => {
            arguments.iter().any(type_expr_contains_unresolved)
        }
        TypeExpr::Concrete(_) => false,
    }
}

pub(super) fn port_contract(
    port: &ValidatedSemanticPort<PortAddress, TypeExpr, crate::node_system::protocol::SchemaExpr>,
    declared: &TypeExpr,
) -> Result<PlannedValueContract, CompilerNodeDiagnostic> {
    let type_expr = port.resolved_type.as_ref().unwrap_or(declared);
    planned_value_contract(type_expr).ok_or_else(|| {
        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Port(port.address.clone()))
    })
}

pub(super) fn derive_function_abi<R: CompilerRegistry>(
    registry: &R,
    graph: &CompilerSemanticGraph,
    projection: &ValidatedInterfaceProjection,
    provenance: &CompileProvenance,
) -> Result<Option<FunctionPlanAbi>, CompilerNodeDiagnostic> {
    if !provenance.graph_path.0.starts_with("functions/") {
        return Ok(None);
    }
    let (_, values) = allocate_port_values(registry, graph)?;
    let mut parameters = BTreeMap::new();
    let mut parameter_contracts = BTreeMap::new();
    let mut results = BTreeMap::new();
    let mut result_productions = BTreeMap::new();
    let mut result_contracts = BTreeMap::new();
    for node in graph.nodes.iter() {
        let resolved = resolve_for_lowering(registry, node)?;
        let destination = match resolved.structural_role() {
            Some(StructuralNodeRole::FunctionEntry) => &mut parameters,
            Some(StructuralNodeRole::FunctionReturn) => &mut results,
            _ => continue,
        };
        let Some(node_projection) = projection.nodes.get(&node.node_id) else {
            continue;
        };
        for port in node.ports.iter() {
            let Some(ProjectedDynamicPortBinding::Resolved { origin, .. }) =
                node_projection.projected_bindings.get(&port.address)
            else {
                continue;
            };
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                continue;
            };
            if function != &provenance.graph_path {
                return Err(CompilerDiagnostic::FunctionAbiTargetMismatch {
                    function_path: function.0.clone(),
                }
                .into_node(DiagnosticLocation::Port(port.address.clone())));
            }
            let value = values[&port.address];
            let contract = port_contract(
                port,
                &protocol_port(resolved.protocol, &port.address).value_type,
            )?;
            if resolved.structural_role() == Some(StructuralNodeRole::FunctionReturn) {
                let production = graph
                    .dependencies
                    .iter()
                    .find_map(|dependency| match dependency {
                        SemanticDependency::Value(edge) if edge.target == port.address => {
                            let source_node = graph
                                .nodes
                                .iter()
                                .find(|candidate| candidate.node_id == edge.source.node_id)?;
                            let source = resolve_for_lowering(registry, source_node).ok()?;
                            protocol_port(source.protocol, &edge.source).production
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}
                            .into_node(DiagnosticLocation::Port(port.address.clone()))
                    })?;
                result_productions.insert(parameter.clone(), production);
                result_contracts.insert(parameter.clone(), contract);
            } else {
                parameter_contracts.insert(parameter.clone(), contract);
            }
            if destination.insert(parameter.clone(), value).is_some() {
                return Err(CompilerDiagnostic::FunctionAbiMemberDuplicate {
                    field_name: parameter.0.clone(),
                }
                .into_node(DiagnosticLocation::Port(port.address.clone())));
            }
        }
    }
    Ok(Some(FunctionPlanAbi {
        provenance: provenance.clone(),
        parameters,
        parameter_contracts,
        results,
        result_productions,
        result_contracts,
    }))
}
