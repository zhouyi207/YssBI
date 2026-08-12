mod finalization;

use super::relational::{RelationalConnection, RelationalFragment, RelationalPlanner};
use crate::node_system::analysis::CompileProvenance;
use crate::node_system::document::{NodeId, PortAddress};
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, ControlStep,
    EXECUTION_SEMANTICS_SCHEMA_VERSION, EffectDependency, ExecutionDemand, ExecutionPlan,
    ExecutionSemanticsVersion, GraphOutputRef, KernelHandle, OperationIndex, OperationStableId,
    PlanResult, PlanValueSource, PlannedInput, PlannedKernel, PlannedOperation, PlannedOutput,
    PlannedPublication, PlannedRetry, RelationalBackendId, RelationalFragmentId,
    RelationalOperatorIndex, RelationalSubplanIndex, ResourceId, StructuredControlRegion,
    ValueDependency, ValueRef, WorkloadClass,
};
use crate::node_system::protocol::{
    CachePolicy, EffectSemantics, EvaluationPolicy, InputConsumption, OutputProduction,
    PortDirection, PortKind, Purity,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NormalizedExecutionDemand {
    Default,
    Outputs {
        outputs: Box<[GraphOutputRef]>,
        include_default_results: bool,
    },
    PinPreview {
        output: GraphOutputRef,
        generation: u64,
    },
}

impl NormalizedExecutionDemand {
    pub fn digest(&self) -> Result<[u8; 32], crate::node_system::registry::CanonicalEncodingError> {
        crate::node_system::registry::hash_canonical("yssbi.execution-demand.v1", self)
    }
}

fn preview_generation(demand: &NormalizedExecutionDemand) -> Option<u64> {
    match demand {
        NormalizedExecutionDemand::PinPreview { generation, .. } => Some(*generation),
        NormalizedExecutionDemand::Default | NormalizedExecutionDemand::Outputs { .. } => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemandPlanError {
    GraphPathMismatch(GraphOutputRef),
    MissingNode(GraphOutputRef),
    MissingPort(GraphOutputRef),
    StalePortInstance(GraphOutputRef),
    InputPort(GraphOutputRef),
    ControlPort(GraphOutputRef),
    EffectPort(GraphOutputRef),
    UnboundInput(PortAddress),
    InvalidDerivedPlan(Box<str>),
    CanonicalEncoding(crate::node_system::registry::CanonicalEncodingError),
}

impl fmt::Display for DemandPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GraphPathMismatch(output) => write!(
                formatter,
                "requested output belongs to '{}' instead of the compiled graph",
                output.graph_path.0
            ),
            Self::MissingNode(output) => write!(
                formatter,
                "requested output node is missing: {}",
                output.port
            ),
            Self::MissingPort(output) => write!(
                formatter,
                "requested output port is missing: {}",
                output.port
            ),
            Self::StalePortInstance(output) => write!(
                formatter,
                "requested output instance is stale: {}",
                output.port
            ),
            Self::InputPort(output) => {
                write!(formatter, "requested port is an input: {}", output.port)
            }
            Self::ControlPort(output) => {
                write!(formatter, "requested port is control: {}", output.port)
            }
            Self::EffectPort(output) => {
                write!(formatter, "requested port is effect: {}", output.port)
            }
            Self::UnboundInput(port) => write!(formatter, "required input is unbound: {port}"),
            Self::InvalidDerivedPlan(message) => {
                write!(formatter, "derived execution plan is invalid: {message}")
            }
            Self::CanonicalEncoding(error) => error.fmt(formatter),
        }
    }
}

impl From<crate::node_system::registry::CanonicalEncodingError> for DemandPlanError {
    fn from(error: crate::node_system::registry::CanonicalEncodingError) -> Self {
        Self::CanonicalEncoding(error)
    }
}

impl std::error::Error for DemandPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CanonicalEncoding(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DemandPortFact {
    pub kind: PortKind,
    pub direction: PortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IntermediateKernel {
    Native(KernelHandle),
    Relational {
        backend: RelationalBackendId,
        fragment: RelationalFragment,
        input_bindings: BTreeMap<PortAddress, RelationalOperatorIndex>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntermediateOperation {
    pub stable_id: OperationStableId,
    pub source_node_id: NodeId,
    pub source_node_type_id: crate::node_system::protocol::NodeTypeId,
    pub has_control_or_effect_ports: bool,
    pub kernel: IntermediateKernel,
    pub input_ports: Box<[PortAddress]>,
    pub inputs: Box<[PlannedInput]>,
    pub output_ports: Box<[PortAddress]>,
    pub outputs: Box<[PlannedOutput]>,
    pub params: CompiledParameterHandle,
    pub resource_dependencies: Box<[crate::node_system::analysis::ResourceKey]>,
    pub cache_policy: CachePolicy,
    pub semantics_version: ExecutionSemanticsVersion,
    pub workload: WorkloadClass,
    pub retry: PlannedRetry,
    pub evaluation: EvaluationPolicy,
    pub purity: Purity,
    pub effects: EffectSemantics,
    pub resources: Box<[CompiledResourceRequirement]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlanBasis {
    pub(crate) provenance: CompileProvenance,
    pub(crate) value_count: u32,
    pub(crate) operations: Box<[IntermediateOperation]>,
    pub(crate) value_contracts: BTreeMap<ValueRef, crate::node_system::plan::PlannedValueContract>,
    pub(crate) value_sources: Box<[PlanValueSource]>,
    pub(crate) value_dependencies: Box<[ValueDependency]>,
    pub(crate) effect_dependencies: Box<[(usize, usize)]>,
    pub(crate) root_region: StructuredControlRegion,
    pub(crate) relational_connections: Box<[RelationalConnection]>,
    pub(crate) port_facts: BTreeMap<PortAddress, DemandPortFact>,
    pub(crate) unbound_inputs: BTreeMap<ValueRef, PortAddress>,
    pub(crate) bound_values: BTreeMap<ValueRef, crate::node_system::protocol::Value>,
    pub(crate) nodes: BTreeSet<NodeId>,
    pub(crate) output_results: BTreeMap<GraphOutputRef, PlanResult>,
    pub(crate) default_outputs: BTreeSet<GraphOutputRef>,
}

impl ExecutionPlanBasis {
    pub fn normalize_demand(
        &self,
        demand: &ExecutionDemand,
    ) -> Result<NormalizedExecutionDemand, DemandPlanError> {
        match demand {
            ExecutionDemand::Default => Ok(NormalizedExecutionDemand::Default),
            ExecutionDemand::Outputs {
                outputs,
                include_default_results,
            } => {
                let outputs = outputs.iter().cloned().collect::<BTreeSet<_>>();
                for output in &outputs {
                    self.validate_output(output)?;
                }
                Ok(NormalizedExecutionDemand::Outputs {
                    outputs: outputs.into_iter().collect(),
                    include_default_results: *include_default_results,
                })
            }
            ExecutionDemand::PinPreview { output, generation } => {
                self.validate_output(output)?;
                Ok(NormalizedExecutionDemand::PinPreview {
                    output: output.clone(),
                    generation: *generation,
                })
            }
        }
    }

    pub fn derive_plan(&self, demand: &ExecutionDemand) -> Result<ExecutionPlan, DemandPlanError> {
        let normalized = self.normalize_demand(demand)?;
        let selected_outputs = self.selected_outputs(&normalized);
        let retained = self.retained_plan(&selected_outputs);
        if let Some(port) = retained
            .required_values
            .iter()
            .find_map(|value| self.unbound_inputs.get(value))
        {
            return Err(DemandPlanError::UnboundInput(port.clone()));
        }
        self.finalize(
            &retained.operations,
            &selected_outputs,
            preview_generation(&normalized),
            &retained.root_region,
            Some(&retained.required_values),
        )
    }

    pub(crate) fn derive_full_plan(&self) -> Result<ExecutionPlan, DemandPlanError> {
        let retained = (0..self.operations.len()).collect::<BTreeSet<_>>();
        self.finalize(
            &retained,
            &self.default_outputs,
            None,
            &self.root_region,
            None,
        )
    }

    fn selected_outputs(&self, demand: &NormalizedExecutionDemand) -> BTreeSet<GraphOutputRef> {
        match demand {
            NormalizedExecutionDemand::Default => self.default_outputs.clone(),
            NormalizedExecutionDemand::Outputs {
                outputs,
                include_default_results,
            } => {
                let mut selected = outputs.iter().cloned().collect::<BTreeSet<_>>();
                if *include_default_results {
                    selected.extend(self.default_outputs.iter().cloned());
                }
                selected
            }
            NormalizedExecutionDemand::PinPreview { output, .. } => {
                [output.clone()].into_iter().collect()
            }
        }
    }

    fn retained_plan(&self, selected_outputs: &BTreeSet<GraphOutputRef>) -> RetainedPlan {
        let producers = self.value_producers();
        let effect_predecessors = self.effect_predecessors();
        let mut retained = BTreeSet::new();
        let mut pending_operations = VecDeque::new();
        let mut required_values = BTreeSet::new();
        let mut pending_values = VecDeque::new();

        for output in selected_outputs {
            require_value(
                self.output_results[output].value,
                &mut required_values,
                &mut pending_values,
            );
        }
        for (index, operation) in self.operations.iter().enumerate() {
            if operation.evaluation == EvaluationPolicy::EagerWhenRegionEntered {
                pending_operations.push_back(index);
            }
        }

        loop {
            while let Some(value) = pending_values.pop_front() {
                if let Some(owner) = producers.get(&value) {
                    pending_operations.push_back(*owner);
                }
                for source in self.sources_for(value) {
                    require_value(source, &mut required_values, &mut pending_values);
                }
            }
            while let Some(index) = pending_operations.pop_front() {
                if !retained.insert(index) {
                    continue;
                }
                for input in &self.operations[index].inputs {
                    require_value(input.value, &mut required_values, &mut pending_values);
                }
                pending_operations.extend(
                    effect_predecessors
                        .get(&index)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }

            let previous_required = required_values.len();
            collect_region_requirements(
                &self.root_region,
                &retained,
                &mut required_values,
                &mut pending_values,
            );
            if pending_values.is_empty() && previous_required == required_values.len() {
                break;
            }
        }

        let root_region = project_region(&self.root_region, &retained, &required_values)
            .unwrap_or_else(empty_region);
        RetainedPlan {
            operations: retained,
            required_values,
            root_region,
        }
    }

    fn validate_output(&self, output: &GraphOutputRef) -> Result<(), DemandPlanError> {
        if output.graph_path != self.provenance.graph_path {
            return Err(DemandPlanError::GraphPathMismatch(output.clone()));
        }
        if !self.nodes.contains(&output.port.node_id) {
            return Err(DemandPlanError::MissingNode(output.clone()));
        }
        let Some(fact) = self.port_facts.get(&output.port) else {
            return Err(if output.port.is_instance() {
                DemandPlanError::StalePortInstance(output.clone())
            } else {
                DemandPlanError::MissingPort(output.clone())
            });
        };
        match (fact.kind, fact.direction) {
            (PortKind::Data, PortDirection::Output) => Ok(()),
            (PortKind::Data, PortDirection::Input) => {
                Err(DemandPlanError::InputPort(output.clone()))
            }
            (PortKind::Control, _) => Err(DemandPlanError::ControlPort(output.clone())),
            (PortKind::Effect, _) => Err(DemandPlanError::EffectPort(output.clone())),
        }
    }

    fn value_producers(&self) -> BTreeMap<ValueRef, usize> {
        self.operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .outputs
                    .iter()
                    .map(move |output| (output.value, index))
            })
            .collect()
    }

    fn value_ownership(&self) -> ValueOwnership {
        let mut ownership = ValueOwnership::default();
        for (index, operation) in self.operations.iter().enumerate() {
            for input in &operation.inputs {
                ownership.consumer.insert(input.value, index);
            }
            for output in &operation.outputs {
                ownership.producer.insert(output.value, index);
            }
        }
        ownership
    }

    fn sources_for(&self, destination: ValueRef) -> impl Iterator<Item = ValueRef> + '_ {
        self.value_dependencies
            .iter()
            .filter(move |dependency| dependency.destination == destination)
            .map(|dependency| dependency.source)
    }

    fn effect_predecessors(&self) -> BTreeMap<usize, Vec<usize>> {
        let mut predecessors = BTreeMap::<usize, Vec<usize>>::new();
        for (before, after) in &self.effect_dependencies {
            predecessors.entry(*after).or_default().push(*before);
        }
        predecessors
    }
}

struct RetainedPlan {
    operations: BTreeSet<usize>,
    required_values: BTreeSet<ValueRef>,
    root_region: StructuredControlRegion,
}

fn require_value(
    value: ValueRef,
    required: &mut BTreeSet<ValueRef>,
    pending: &mut VecDeque<ValueRef>,
) {
    if required.insert(value) {
        pending.push_back(value);
    }
}

fn collect_region_requirements(
    region: &StructuredControlRegion,
    retained: &BTreeSet<usize>,
    required: &mut BTreeSet<ValueRef>,
    pending: &mut VecDeque<ValueRef>,
) -> bool {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            let mut active = false;
            for step in steps {
                active |= match step {
                    ControlStep::Operation(operation) => retained.contains(&operation.index()),
                    ControlStep::Region(child) => {
                        collect_region_requirements(child, retained, required, pending)
                    }
                };
            }
            active
        }
        StructuredControlRegion::If {
            condition,
            then_region,
            else_region,
            results,
        } => {
            let then_retained =
                collect_region_requirements(then_region, retained, required, pending);
            let else_retained =
                collect_region_requirements(else_region, retained, required, pending);
            let retained_results = results
                .iter()
                .filter(|binding| required.contains(&binding.destination))
                .copied()
                .collect::<Vec<_>>();
            let active = then_retained || else_retained || !retained_results.is_empty();
            if active {
                require_value(*condition, required, pending);
                for binding in retained_results {
                    require_value(binding.then_source, required, pending);
                    require_value(binding.else_source, required, pending);
                }
            }
            active
        }
        StructuredControlRegion::Loop {
            body,
            carried,
            continue_condition,
            ..
        } => {
            let body_retained = collect_region_requirements(body, retained, required, pending);
            let result_retained = carried
                .iter()
                .any(|binding| required.contains(&binding.result));
            let active = body_retained || result_retained;
            if active {
                require_value(*continue_condition, required, pending);
                for binding in carried {
                    require_value(binding.body_input, required, pending);
                    require_value(binding.initial_source, required, pending);
                    require_value(binding.next_source, required, pending);
                    require_value(binding.result, required, pending);
                }
            }
            active
        }
        StructuredControlRegion::Call {
            arguments,
            results,
            mandatory,
            ..
        } => {
            let active = *mandatory
                || results
                    .iter()
                    .any(|binding| required.contains(&binding.caller_destination));
            if active {
                for binding in arguments {
                    require_value(binding.caller_source, required, pending);
                }
            }
            active
        }
    }
}

fn project_region(
    region: &StructuredControlRegion,
    retained: &BTreeSet<usize>,
    required: &BTreeSet<ValueRef>,
) -> Option<StructuredControlRegion> {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            let steps = steps
                .iter()
                .filter_map(|step| match step {
                    ControlStep::Operation(operation) if retained.contains(&operation.index()) => {
                        Some(ControlStep::Operation(*operation))
                    }
                    ControlStep::Operation(_) => None,
                    ControlStep::Region(child) => project_region(child, retained, required)
                        .map(|region| ControlStep::Region(Box::new(region))),
                })
                .collect::<Vec<_>>();
            (!steps.is_empty()).then(|| StructuredControlRegion::Sequence(steps.into_boxed_slice()))
        }
        StructuredControlRegion::If {
            condition,
            then_region,
            else_region,
            results,
        } => {
            let then_region = project_region(then_region, retained, required);
            let else_region = project_region(else_region, retained, required);
            let results = results
                .iter()
                .filter(|binding| required.contains(&binding.destination))
                .copied()
                .collect::<Vec<_>>();
            (then_region.is_some() || else_region.is_some() || !results.is_empty()).then(|| {
                StructuredControlRegion::If {
                    condition: *condition,
                    then_region: Box::new(then_region.unwrap_or_else(empty_region)),
                    else_region: Box::new(else_region.unwrap_or_else(empty_region)),
                    results: results.into_boxed_slice(),
                }
            })
        }
        StructuredControlRegion::Loop {
            body,
            carried,
            continue_condition,
            max_iterations,
        } => {
            let body = project_region(body, retained, required);
            let active = body.is_some()
                || carried
                    .iter()
                    .any(|binding| required.contains(&binding.result));
            active.then(|| StructuredControlRegion::Loop {
                body: Box::new(body.unwrap_or_else(empty_region)),
                carried: carried.clone(),
                continue_condition: *continue_condition,
                max_iterations: *max_iterations,
            })
        }
        StructuredControlRegion::Call {
            target,
            arguments,
            results,
            mandatory,
        } => (*mandatory
            || results
                .iter()
                .any(|binding| required.contains(&binding.caller_destination)))
        .then(|| StructuredControlRegion::Call {
            target: target.clone(),
            arguments: arguments.clone(),
            results: results.clone(),
            mandatory: *mandatory,
        }),
    }
}

fn collect_control_produced_values(
    region: &StructuredControlRegion,
    values: &mut BTreeSet<ValueRef>,
) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                if let ControlStep::Region(region) = step {
                    collect_control_produced_values(region, values);
                }
            }
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            results,
            ..
        } => {
            values.extend(results.iter().map(|binding| binding.destination));
            collect_control_produced_values(then_region, values);
            collect_control_produced_values(else_region, values);
        }
        StructuredControlRegion::Loop { body, carried, .. } => {
            for binding in carried {
                values.insert(binding.body_input);
                values.insert(binding.result);
            }
            collect_control_produced_values(body, values);
        }
        StructuredControlRegion::Call { results, .. } => {
            values.extend(results.iter().map(|binding| binding.caller_destination));
        }
    }
}

fn empty_region() -> StructuredControlRegion {
    StructuredControlRegion::Sequence(Box::new([]))
}

#[derive(Default)]
struct ValueOwnership {
    producer: BTreeMap<ValueRef, usize>,
    consumer: BTreeMap<ValueRef, usize>,
}

#[derive(Default)]
struct RelationalFinalization {
    subplans: Box<[crate::node_system::plan::RelationalSubplan]>,
    subplan_by_fragment: BTreeMap<RelationalFragmentId, RelationalSubplanIndex>,
    subplan_by_operation: BTreeMap<usize, RelationalSubplanIndex>,
}

enum OperationGroup {
    Native(usize),
    Relational(RelationalSubplanIndex),
}

fn remap_region(
    region: &StructuredControlRegion,
    remap: &BTreeMap<usize, OperationIndex>,
) -> StructuredControlRegion {
    match region {
        StructuredControlRegion::Sequence(steps) => StructuredControlRegion::Sequence(
            steps
                .iter()
                .filter_map(|step| match step {
                    ControlStep::Operation(operation) => remap
                        .get(&operation.index())
                        .copied()
                        .map(ControlStep::Operation),
                    ControlStep::Region(child) => {
                        Some(ControlStep::Region(Box::new(remap_region(child, remap))))
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        StructuredControlRegion::If {
            condition,
            then_region,
            else_region,
            results,
        } => StructuredControlRegion::If {
            condition: *condition,
            then_region: Box::new(remap_region(then_region, remap)),
            else_region: Box::new(remap_region(else_region, remap)),
            results: results.clone(),
        },
        StructuredControlRegion::Loop {
            body,
            carried,
            continue_condition,
            max_iterations,
        } => StructuredControlRegion::Loop {
            body: Box::new(remap_region(body, remap)),
            carried: carried.clone(),
            continue_condition: *continue_condition,
            max_iterations: *max_iterations,
        },
        StructuredControlRegion::Call {
            target,
            arguments,
            results,
            mandatory,
        } => StructuredControlRegion::Call {
            target: target.clone(),
            arguments: arguments.clone(),
            results: results.clone(),
            mandatory: *mandatory,
        },
    }
}

fn deduplicate_region_operations(region: &mut StructuredControlRegion) {
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
