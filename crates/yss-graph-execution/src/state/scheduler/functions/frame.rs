//! A function reuses plan construction and DAG evaluation, with private values across schema stages.
use super::*;
use crate::graph_preparation::{GraphExecutionScope, build_template};
use yss_graph_analysis::{
    GraphFunctionSemanticFact, GraphResolvedInputSource, GraphSchemaObservation,
    GraphSchemaObservations, GraphSemanticCache,
};
use yss_graph_document::FunctionParameterId;

pub(super) struct FunctionFrame<'a> {
    pub path: &'a GraphResourcePath,
    pub definition: &'a GraphFunctionSemanticFact,
    pub arguments: &'a BTreeMap<FunctionParameterId, GraphFunctionArgument>,
    pub entry_values: &'a BTreeMap<PlanOutputRef, RuntimeValue>,
}

pub(super) fn execute(
    executor: &NeutralPlanExecutor,
    parent: &ExecutionPlanPackage,
    resources: &PreparedRunResources,
    control: &KernelControl,
    frame: FunctionFrame<'_>,
    active: &mut BTreeSet<GraphResourcePath>,
) -> Result<Vec<RuntimeValue>, OperationExecutionError> {
    let FunctionFrame {
        path,
        definition,
        arguments,
        entry_values,
    } = frame;
    let library = parent
        .functions
        .as_ref()
        .ok_or(OperationExecutionError::Failed)?;
    let mut cache = GraphSemanticCache::default();
    let mut observations = GraphSchemaObservations::new();
    let mut values: BTreeMap<PlanOutputRef, RuntimeValue> = BTreeMap::new();
    loop {
        control.check()?;
        let bound = yss_graph_analysis::specialize_function(
            path,
            definition,
            &library.registry,
            &library.resources,
            arguments,
            &observations,
            &mut cache,
        )
        .map_err(|_| KernelError::InvalidParameter)?;
        let semantics = bound
            .semantics
            .with_execution_kernel_support(&|id| executor.kernels.supports(id));
        let demand = match &bound.abi.result {
            Some(result) => PlanExecutionDemand::Node {
                node: PlanNodeId::from_existing(result.return_input.node_id.to_string().into()),
                mode: NodeExecutionMode::Dependencies,
            },
            None => PlanExecutionDemand::Default,
        };
        let scope = GraphExecutionScope::select(path, &semantics, &demand)
            .map_err(|_| not_ready(path, &semantics))?;
        let frontier = scope
            .schema_frontier(path, &semantics, |output| values.contains_key(output))
            .map_err(|_| not_ready(path, &semantics))?;
        let (stage_scope, boundary_outputs, last) = match frontier {
            Some((scope, PlanExecutionDemand::Outputs { outputs, .. })) => (scope, outputs, false),
            Some(_) => return Err(OperationExecutionError::Failed),
            None => (scope, Box::new([]) as Box<[_]>, true),
        };
        if !semantics.nodes_ready(stage_scope.nodes()) {
            return Err(not_ready(path, &semantics));
        }
        let mut package = build_template(
            path,
            parent.provenance().plan_id(),
            &semantics,
            &stage_scope,
        )
        .and_then(|template| template.package(parent.provenance().basis().clone()))
        .map_err(|_| not_ready(path, &semantics))?;
        package.functions = parent.functions.clone();
        let producers = super::super::execution_producers(&package)?;
        let required = package
            .plan()
            .operations()
            .iter()
            .map(|operation| {
                operation.outputs().is_empty()
                    || !operation
                        .outputs()
                        .iter()
                        .all(|output| values.contains_key(output.output()))
            })
            .collect::<Vec<_>>();
        let seeded = package
            .plan()
            .operations()
            .iter()
            .flat_map(|operation| operation.outputs())
            .filter_map(|output| {
                values
                    .get(output.output())
                    .map(|value| (output.value().index() as usize, value.clone()))
            })
            .collect();
        let boundaries = package
            .plan()
            .operations()
            .iter()
            .flat_map(|operation| operation.outputs())
            .filter(|output| boundary_outputs.contains(output.output()))
            .map(|output| output.value())
            .collect();
        let results = executor.execute_dag(
            super::super::dag::DagExecution {
                package: &package,
                resources,
                control,
                producers: &producers,
                required: &required,
                boundaries: &boundaries,
                seeded,
                arguments: entry_values,
            },
            active,
        )?;
        let before = values.len();
        for result in results {
            values.insert(result.output, result.value.value().clone());
        }
        if last {
            let Some(result) = &bound.abi.result else {
                return Ok(Vec::new());
            };
            let input = semantics
                .node(result.return_input.node_id)
                .and_then(|node| {
                    node.inputs
                        .iter()
                        .find(|input| input.address == result.return_input)
                })
                .ok_or(OperationExecutionError::Failed)?;
            let value = match &input.source {
                GraphResolvedInputSource::Output(source) => values
                    .get(&output_ref(path, source))
                    .cloned()
                    .ok_or(OperationExecutionError::Failed)?,
                GraphResolvedInputSource::Literal(value) => RuntimeValue::try_from(&value.value)
                    .map_err(|_| OperationExecutionError::Failed)?,
            };
            let expected = yss_graph_type_mapping::data_type_from_resolved_type(&result.value_type)
                .ok_or(OperationExecutionError::Failed)?;
            if !value.matches_carrier(&expected) {
                return Err(KernelError::OutputContractMismatch.into());
            }
            return Ok(vec![value]);
        }
        if values.len() == before {
            return Err(OperationExecutionError::Failed);
        }
        for port in semantics.nodes().iter().flat_map(|node| &node.ports) {
            let output = output_ref(path, &port.address);
            if !boundary_outputs.contains(&output) {
                continue;
            }
            let fields = values
                .get(&output)
                .and_then(schema_fields)
                .ok_or(OperationExecutionError::Failed)?;
            observations.insert(
                port.address.clone(),
                GraphSchemaObservation { version: 1, fields },
            );
        }
    }
}

fn not_ready(
    path: &GraphResourcePath,
    semantics: &yss_graph_analysis::GraphSemanticSnapshot,
) -> OperationExecutionError {
    use yss_graph_analysis::GraphDiagnosticLocation;
    let location = semantics
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.blocking)
        // Prefer the node that discovered an actual invalid schema to its downstream symptoms.
        .min_by_key(|diagnostic| match &diagnostic.primary {
            GraphDiagnosticLocation::Parameter { .. } => 0,
            GraphDiagnosticLocation::Port(address) => semantics
                .concrete_interface()
                .port(address)
                .map_or(4, |port| {
                    if matches!(port.schema_state, GraphSchemaState::Conflict(_)) {
                        if port.direction == yss_node_protocol::PortDirection::Output {
                            1
                        } else {
                            2
                        }
                    } else {
                        4
                    }
                }),
            _ => 3,
        })
        .map(|diagnostic| &diagnostic.primary);
    let (node, port) = match location {
        Some(GraphDiagnosticLocation::Port(address)) => (
            Some(address.node_id),
            Some(PlanPortAddress::from_existing(address.to_string().into())),
        ),
        Some(
            GraphDiagnosticLocation::Node(node)
            | GraphDiagnosticLocation::Parameter { node_id: node, .. },
        ) => (Some(*node), None),
        _ => (None, None),
    };
    OperationExecutionError::FunctionNotReady {
        location: PlanSourceIdentity::new(
            PlanGraphId::from_existing(path.as_str().into()),
            node.map(|node| PlanNodeId::from_existing(node.to_string().into())),
            port,
        ),
    }
}
