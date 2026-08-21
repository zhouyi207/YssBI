//! Function-plan publication and compiled node-parameter binding for execution.

use super::*;

fn function_targets(
    region: &crate::node_system::plan::StructuredControlRegion,
) -> std::collections::BTreeSet<crate::node_system::document::GraphResourcePath> {
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let mut targets = std::collections::BTreeSet::new();
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                if let ControlStep::Region(region) = step {
                    targets.extend(function_targets(region));
                }
            }
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            targets.extend(function_targets(then_region));
            targets.extend(function_targets(else_region));
        }
        StructuredControlRegion::Loop { body, .. } => {
            targets.extend(function_targets(body));
        }
        StructuredControlRegion::Call { target, .. } => {
            targets.insert(crate::node_system::document::GraphResourcePath(
                target.as_str().into(),
            ));
        }
    }
    targets
}

pub(in crate::project) fn publish_function_plans(
    registry: &crate::node_system::registry::NodeRegistry,
    store: &crate::node_system::runtime::FunctionPlanStore,
    resources: &CompileResourceSnapshot,
    root_plan: Option<&crate::node_system::plan::ExecutionPlan>,
    session_id: crate::node_system::analysis::ProjectSessionId,
    trace_sink: &dyn crate::node_system::analysis::TraceSink,
    cancellation: &crate::node_system::compiler::CompileCancellationToken,
    computation_settings: &crate::project::ProjectComputationSettings,
    parameters: &mut crate::node_system::runtime::CompiledParameterStore,
) -> Result<crate::node_system::runtime::FunctionPlanGeneration, ProjectExecutionError> {
    let compiler = GraphCompiler::with_resolvers(
        registry,
        resources,
        resources.schema_resolvers(),
        crate::node_system::compiler::build_builtin_interface_resolvers(),
    )
    .with_observability(session_id, trace_sink);
    let compile_id = root_plan
        .map(|plan| plan.provenance.compile_id)
        .unwrap_or_else(|| crate::node_system::analysis::CompileId::new(0));
    let mut entries = Vec::with_capacity(resources.function_graphs.len());
    let mut pending = root_plan
        .map(|plan| function_targets(&plan.root_region))
        .unwrap_or_else(|| resources.function_graphs.keys().cloned().collect());
    let mut published = std::collections::BTreeSet::new();
    while let Some(document_path) = pending.pop_first() {
        cancellation
            .checkpoint()
            .map_err(|error| error.to_string())?;
        if !published.insert(document_path.clone()) {
            continue;
        }
        let document = resources
            .function_graphs
            .get(&document_path)
            .ok_or_else(|| format!("required function '{}' is unavailable", document_path.0))?;
        let snapshot =
            compiler.snapshot_with_compile_id(compile_id, document_path.clone(), document);
        let products = compiler
            .compile_snapshot(&snapshot, cancellation)
            .map_err(|error| error.to_string())?;
        match &products.outcome {
            crate::node_system::compiler::CompilationOutcome::Succeeded => {}
            crate::node_system::compiler::CompilationOutcome::AnalysisBlocked => {
                let diagnostics = products
                    .analysis
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "function '{}' has blocking diagnostics and cannot be published: {}",
                    document_path.0, diagnostics
                )
                .into());
            }
            crate::node_system::compiler::CompilationOutcome::InternalFailure(failure) => {
                return Err(ProjectExecutionError::internal_compilation(failure.clone()));
            }
        }
        let abi = products.function_abi.clone().ok_or_else(|| {
            format!(
                "function '{}' did not produce an Entry/Return ABI",
                document_path.0
            )
        })?;
        let plan = products.plan.ok_or_else(|| {
            ProjectExecutionError::internal_compilation(
                crate::node_system::compiler::InternalCompilationFailure {
                    stage: crate::node_system::compiler::CompilationStage::Lowering,
                    code: "project.execution.function_plan_missing".into(),
                    node_id: None,
                },
            )
        })?;
        pending.extend(function_targets(&plan.root_region));
        build_run_parameters(parameters, document, &plan, computation_settings)?;
        let resource_key = crate::node_system::analysis::ResourceKey::new(document_path.0.as_ref());
        let version = resources
            .versions
            .get(&resource_key)
            .cloned()
            .ok_or_else(|| format!("function '{}' has no resource version", document_path.0))?;
        entries.push((
            document_path.clone(),
            version,
            Arc::new(plan),
            Arc::new(abi),
        ));
    }
    store
        .generation(
            registry.fingerprint().clone(),
            resources.versions(),
            entries,
        )
        .map_err(|error| ProjectExecutionError::internal(error.to_string()))
}

pub(in crate::project) fn build_run_parameters(
    parameters: &mut crate::node_system::runtime::CompiledParameterStore,
    document: &crate::node_system::document::GraphDocument,
    plan: &crate::node_system::plan::ExecutionPlan,
    computation_settings: &crate::project::ProjectComputationSettings,
) -> Result<(), String> {
    for operation in &plan.operations {
        let node_type = operation.source_node_type_id.as_str();
        let node = document
            .nodes
            .get(&operation.source_node_id)
            .ok_or_else(|| format!("plan source node '{}' is missing", operation.source_node_id))?;
        if matches!(
            node_type,
            "yssbi.project.variable.get" | "yssbi.project.variable.set"
        ) {
            let resource = node
                .parameters
                .iter()
                .find(|(key, _)| key.as_str() == "variable")
                .and_then(|(_, value)| value.as_str())
                .ok_or_else(|| format!("variable node '{}' has no binding", node.id))?;
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::BuiltinVariableParameters::new(
                        crate::node_system::plan::ResourceId::new(resource)
                            .map_err(|error| error.to_string())?,
                    ),
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if node_type.starts_with("yssbi.statistics.") {
            let parameter = |name: &str| {
                node.parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == name)
                    .map(|(_, value)| value)
            };
            let positive_integer = |name: &str| {
                parameter(name)
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as usize)
            };
            let convergence_override = parameter("convergence_tolerance")
                .map(|value| {
                    value
                        .as_f64()
                        .filter(|value| value.is_finite() && *value > 0.0)
                        .ok_or_else(|| {
                            "statistics convergence tolerance must be finite and greater than zero"
                                .to_string()
                        })
                })
                .transpose()?;
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::StatisticsKernelParameters {
                        data_series_input_indices: Some(
                            operation
                                .inputs
                                .iter()
                                .enumerate()
                                .filter_map(|(index, input)| {
                                    (input.contract.kind
                                        == crate::node_system::plan::PlannedValueKind::DataSeries)
                                        .then_some(index)
                                })
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        ),
                        lags: positive_integer("lags"),
                        max_lags: positive_integer("max_lags"),
                        rank: positive_integer("rank"),
                        regression: parameter("regression")
                            .and_then(serde_json::Value::as_str)
                            .map(Into::into),
                        trend: parameter("trend")
                            .and_then(serde_json::Value::as_str)
                            .map(Into::into),
                        convergence_tolerance: convergence_override
                            .unwrap_or(computation_settings.numeric.tolerance.absolute),
                        convergence_tolerance_source: if parameter("convergence_tolerance")
                            .is_some()
                        {
                            crate::sci::models::regression::StatisticalSettingSource::Node
                        } else {
                            crate::sci::models::regression::StatisticalSettingSource::Project
                        },
                        missing_value_policy: match parameter("missing_value_policy")
                            .and_then(serde_json::Value::as_str)
                        {
                            Some("Reject") => crate::project::StatisticalMissingValuePolicy::Reject,
                            Some("Listwise") => {
                                crate::project::StatisticalMissingValuePolicy::Listwise
                            }
                            Some(other) => return Err(format!(
                                "statistics missing-value policy must be Listwise or Reject, got '{other}'"
                            )),
                            None => computation_settings.missing_values.statistics,
                        },
                        missing_value_policy_source: if parameter("missing_value_policy").is_some()
                        {
                            crate::sci::models::regression::StatisticalSettingSource::Node
                        } else {
                            crate::sci::models::regression::StatisticalSettingSource::Project
                        },
                    },
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if node_type.starts_with("yssbi.dataframe.") {
            let parameter = |name: &str| {
                node.parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == name)
                    .map(|(_, value)| value)
            };
            let resource = parameter("dataframe")
                .and_then(serde_json::Value::as_str)
                .map(crate::node_system::plan::ResourceId::new)
                .transpose()
                .map_err(|error| error.to_string())?;
            let column = parameter("column")
                .and_then(serde_json::Value::as_str)
                .map(Into::into);
            let order = parameter("order")
                .or_else(|| parameter("window"))
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as usize);
            let columns = if node_type == "yssbi.dataframe.decompose" {
                let mut columns = document
                    .port_bindings
                    .iter()
                    .filter_map(|(address, binding)| {
                        if address.node_id != operation.source_node_id {
                            return None;
                        }
                        let crate::node_system::document::PortRef::Instance { template, .. } =
                            &address.port
                        else {
                            return None;
                        };
                        if template.as_str() != "columns" {
                            return None;
                        }
                        match binding {
                            crate::node_system::document::DynamicPortBinding::Resolved {
                                origin:
                                    crate::node_system::document::DynamicMemberLocator::SchemaField {
                                        field,
                                        ..
                                    },
                                order,
                                ..
                            }
                            | crate::node_system::document::DynamicPortBinding::Orphan {
                                origin:
                                    crate::node_system::document::DynamicMemberLocator::SchemaField {
                                        field,
                                        ..
                                    },
                                order,
                                ..
                            } => Some((order.clone(), field.0.clone())),
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                columns.sort_by(|left, right| left.0.cmp(&right.0));
                let columns = columns
                    .into_iter()
                    .map(|(_, column)| column)
                    .collect::<Vec<_>>();
                if columns.len() != operation.outputs.len() {
                    return Err(format!(
                        "dataframe decompose node '{}' has {} compiled outputs but {} bound columns",
                        operation.source_node_id,
                        operation.outputs.len(),
                        columns.len()
                    ));
                }
                Some(columns.into_boxed_slice())
            } else {
                None
            };
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::DataframeKernelParameters {
                        resource,
                        column,
                        columns,
                        order,
                    },
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if !node_type.starts_with("yssbi.constant.") {
            continue;
        }
        let node = document
            .nodes
            .get(&operation.source_node_id)
            .ok_or_else(|| format!("plan source node '{}' is missing", operation.source_node_id))?;
        let value = node
            .parameters
            .iter()
            .find(|(key, _)| key.as_str() == "value")
            .map(|(_, value)| json_to_protocol_value(value))
            .transpose()?
            .unwrap_or(crate::node_system::protocol::Value::Null);
        parameters
            .insert(
                operation.params.clone(),
                crate::node_system::runtime::BuiltinConstantParameters::new(value),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(in crate::project) fn json_to_protocol_value(
    value: &serde_json::Value,
) -> Result<crate::node_system::protocol::Value, String> {
    use crate::node_system::protocol::{CanonicalDecimal, Value};
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::Number(value) if value.is_i64() => {
            Value::Integer(value.as_i64().expect("checked i64"))
        }
        serde_json::Value::Number(value) if value.is_u64() => {
            Value::Unsigned(value.as_u64().expect("checked u64"))
        }
        serde_json::Value::Number(value) => Value::Decimal(
            CanonicalDecimal::new(value.to_string()).map_err(|error| error.to_string())?,
        ),
        serde_json::Value::String(value) => Value::String(value.as_str().into()),
        serde_json::Value::Array(values) => Value::List(
            values
                .iter()
                .map(json_to_protocol_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        serde_json::Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.as_str().into(), json_to_protocol_value(value)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}
