use crate::node_system::plan::{
    CompiledRelationalPlan, MaterializationBridge, PlannedMaterializationBridge,
    RelationalBackendId, RelationalBridgeInput, RelationalFragmentId, RelationalFragmentRoot,
    RelationalOperator, RelationalOperatorIndex, RelationalPushdownHint, RelationalSubplan,
    RelationalSubplanIndex, infer_relational_pushdown_hints,
};
use crate::node_system::protocol::{InputConsumption, OutputProduction};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A backend-independent relational fragment produced while lowering one graph node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalFragment {
    pub id: RelationalFragmentId,
    pub operators: Box<[RelationalOperator]>,
    pub root: RelationalOperatorIndex,
}

/// A value edge between two relational fragments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalConnection {
    pub producer: RelationalFragmentId,
    pub consumer: RelationalFragmentId,
    /// The consumer-local `RelationalOperator::Input` bound by this edge.
    pub consumer_input: RelationalOperatorIndex,
    pub production: OutputProduction,
    pub consumption: InputConsumption,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalPlanningResult {
    pub subplans: Box<[RelationalSubplan]>,
    pub bridges: Box<[PlannedMaterializationBridge]>,
}

/// Plans relational islands for one configured backend without selecting among backends.
#[derive(Debug, Clone)]
pub struct RelationalPlanner {
    backend: RelationalBackendId,
}

impl RelationalPlanner {
    pub fn new(backend: RelationalBackendId) -> Self {
        Self { backend }
    }

    pub fn plan(
        &self,
        fragments: &[RelationalFragment],
        connections: &[RelationalConnection],
    ) -> Result<RelationalPlanningResult, RelationalPlanningError> {
        let fragments_by_id = validate_inputs(fragments, connections)?;
        let topological_order = topological_order(&fragments_by_id, connections)?;
        let component_by_fragment = connected_components(&fragments_by_id, connections);
        let mut fragments_by_component = BTreeMap::<RelationalFragmentId, Vec<_>>::new();
        let mut component_order = Vec::new();

        for id in topological_order {
            let component = component_by_fragment[&id].clone();
            if !fragments_by_component.contains_key(&component) {
                component_order.push(component.clone());
            }
            fragments_by_component
                .entry(component)
                .or_default()
                .push(id);
        }

        let mut subplan_by_component = BTreeMap::new();
        let mut subplans = Vec::with_capacity(fragments_by_component.len());
        let mut boundary_inputs_by_subplan = Vec::with_capacity(fragments_by_component.len());
        for component in &component_order {
            let index = RelationalSubplanIndex::new(subplans.len() as u32);
            subplan_by_component.insert(component.clone(), index);
            let (compiled_plan, boundary_inputs) = compile_component(
                &fragments_by_component[component],
                &fragments_by_id,
                connections,
                &component_by_fragment,
            )?;
            subplans.push(RelationalSubplan {
                backend: self.backend.clone(),
                compiled_plan,
                materialization_bridges: Box::new([]),
            });
            boundary_inputs_by_subplan.push(boundary_inputs);
        }

        let mut ordered_connections = connections.iter().collect::<Vec<_>>();
        ordered_connections.sort_by(|left, right| {
            (&left.producer, &left.consumer, left.consumer_input).cmp(&(
                &right.producer,
                &right.consumer,
                right.consumer_input,
            ))
        });
        let planned_bridges = ordered_connections
            .into_iter()
            .filter_map(|connection| {
                let producer_component = &component_by_fragment[&connection.producer];
                let consumer_component = &component_by_fragment[&connection.consumer];
                (producer_component != consumer_component).then(|| {
                    (
                        connection,
                        PlannedMaterializationBridge {
                            producer_fragment: connection.producer.clone(),
                            consumer_fragment: connection.consumer.clone(),
                            producer_subplan: subplan_by_component[producer_component],
                            consumer_subplan: subplan_by_component[consumer_component],
                            bridge: materialization_bridge(
                                connection.production,
                                connection.consumption,
                            ),
                        },
                    )
                })
            })
            .collect::<Vec<_>>();
        let bridges = planned_bridges
            .iter()
            .map(|(_, bridge)| bridge.clone())
            .collect::<Vec<_>>();

        let mut requested_outputs_by_producer = vec![BTreeSet::new(); subplans.len()];
        let mut bridges_by_consumer = vec![Vec::new(); subplans.len()];
        let mut input_bridges_by_consumer = vec![Vec::new(); subplans.len()];
        for (connection, bridge) in &planned_bridges {
            requested_outputs_by_producer[bridge.producer_subplan.index()]
                .insert(bridge.producer_fragment.clone());
            bridges_by_consumer[bridge.consumer_subplan.index()].push(bridge.clone());
            let operator = boundary_inputs_by_subplan[bridge.consumer_subplan.index()]
                [&(connection.consumer.clone(), connection.consumer_input)];
            input_bridges_by_consumer[bridge.consumer_subplan.index()].push(
                RelationalBridgeInput {
                    operator,
                    bridge: bridge.clone(),
                },
            );
        }
        for (subplan, requested_outputs) in subplans.iter_mut().zip(requested_outputs_by_producer) {
            subplan.compiled_plan.requested_fragment_outputs =
                requested_outputs.into_iter().collect();
        }
        for ((subplan, bridges), input_bridges) in subplans
            .iter_mut()
            .zip(bridges_by_consumer)
            .zip(input_bridges_by_consumer)
        {
            subplan.materialization_bridges = bridges.into_boxed_slice();
            subplan.compiled_plan.bridge_inputs = input_bridges.into_boxed_slice();
        }

        Ok(RelationalPlanningResult {
            subplans: subplans.into_boxed_slice(),
            bridges: bridges.into_boxed_slice(),
        })
    }
}

/// Derives the minimum adapter required to satisfy a consumer contract.
pub const fn materialization_bridge(
    production: OutputProduction,
    consumption: InputConsumption,
) -> MaterializationBridge {
    match (production, consumption) {
        (_, InputConsumption::Streaming) => MaterializationBridge::Stream,
        (OutputProduction::Streaming, InputConsumption::SinglePassBatches) => {
            MaterializationBridge::Buffer
        }
        (_, InputConsumption::SinglePassBatches) => MaterializationBridge::Stream,
        (OutputProduction::FullyMaterialized, InputConsumption::RewindableBatches) => {
            MaterializationBridge::Stream
        }
        (_, InputConsumption::RewindableBatches) => MaterializationBridge::Replay,
        (OutputProduction::FullyMaterialized, InputConsumption::RandomAccess) => {
            MaterializationBridge::Stream
        }
        (_, InputConsumption::RandomAccess) => MaterializationBridge::Spill,
        (OutputProduction::FullyMaterialized, InputConsumption::FullyMaterialized) => {
            MaterializationBridge::Stream
        }
        (_, InputConsumption::FullyMaterialized) => MaterializationBridge::Collect,
    }
}

fn validate_inputs<'a>(
    fragments: &'a [RelationalFragment],
    connections: &[RelationalConnection],
) -> Result<BTreeMap<RelationalFragmentId, &'a RelationalFragment>, RelationalPlanningError> {
    let mut by_id = BTreeMap::new();
    for fragment in fragments {
        if by_id.insert(fragment.id.clone(), fragment).is_some() {
            return Err(RelationalPlanningError::DuplicateFragment(
                fragment.id.clone(),
            ));
        }
        validate_fragment(fragment)?;
    }

    let mut bound_inputs = BTreeSet::new();
    for connection in connections {
        if !by_id.contains_key(&connection.producer) {
            return Err(RelationalPlanningError::UnknownFragment(
                connection.producer.clone(),
            ));
        }
        let Some(consumer) = by_id.get(&connection.consumer) else {
            return Err(RelationalPlanningError::UnknownFragment(
                connection.consumer.clone(),
            ));
        };
        let input = connection.consumer_input.index();
        if !matches!(
            consumer.operators.get(input),
            Some(RelationalOperator::Input { .. })
        ) {
            return Err(RelationalPlanningError::InvalidConsumerInput {
                fragment: connection.consumer.clone(),
                input: connection.consumer_input,
            });
        }
        if !bound_inputs.insert((connection.consumer.clone(), connection.consumer_input)) {
            return Err(RelationalPlanningError::DuplicateInputBinding {
                fragment: connection.consumer.clone(),
                input: connection.consumer_input,
            });
        }
    }
    Ok(by_id)
}

fn validate_fragment(fragment: &RelationalFragment) -> Result<(), RelationalPlanningError> {
    if fragment.root.index() >= fragment.operators.len() {
        return Err(RelationalPlanningError::InvalidRoot(fragment.id.clone()));
    }
    for (position, operator) in fragment.operators.iter().enumerate() {
        for input in operator_inputs(operator) {
            if input.index() >= position {
                return Err(RelationalPlanningError::InvalidOperatorInput {
                    fragment: fragment.id.clone(),
                    operator: RelationalOperatorIndex::new(position as u32),
                    input,
                });
            }
        }
    }
    Ok(())
}

fn operator_inputs(operator: &RelationalOperator) -> Vec<RelationalOperatorIndex> {
    match operator {
        RelationalOperator::Input { .. } | RelationalOperator::Source { .. } => Vec::new(),
        RelationalOperator::Project { input, .. }
        | RelationalOperator::Filter { input, .. }
        | RelationalOperator::Rename { input, .. }
        | RelationalOperator::Limit { input, .. } => vec![*input],
        RelationalOperator::Union { inputs, .. } => inputs.to_vec(),
    }
}

fn topological_order(
    fragments: &BTreeMap<RelationalFragmentId, &RelationalFragment>,
    connections: &[RelationalConnection],
) -> Result<Vec<RelationalFragmentId>, RelationalPlanningError> {
    let mut indegree = fragments
        .keys()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<RelationalFragmentId, Vec<RelationalFragmentId>>::new();
    for connection in connections {
        *indegree
            .get_mut(&connection.consumer)
            .expect("validated consumer") += 1;
        outgoing
            .entry(connection.producer.clone())
            .or_default()
            .push(connection.consumer.clone());
    }
    for consumers in outgoing.values_mut() {
        consumers.sort();
    }

    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(fragments.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.clone());
        if let Some(consumers) = outgoing.get(&id) {
            for consumer in consumers {
                let degree = indegree.get_mut(consumer).expect("validated consumer");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(consumer.clone());
                }
            }
        }
    }

    if order.len() != fragments.len() {
        return Err(RelationalPlanningError::CyclicConnections);
    }
    Ok(order)
}

fn connected_components(
    fragments: &BTreeMap<RelationalFragmentId, &RelationalFragment>,
    connections: &[RelationalConnection],
) -> BTreeMap<RelationalFragmentId, RelationalFragmentId> {
    let ids = fragments.keys().cloned().collect::<Vec<_>>();
    let index_by_id = ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut sets = DisjointSets::new(ids.len());
    for connection in connections {
        if materialization_bridge(connection.production, connection.consumption)
            == MaterializationBridge::Stream
        {
            sets.union(
                index_by_id[&connection.producer],
                index_by_id[&connection.consumer],
            );
        }
    }

    let mut minimum_by_root = BTreeMap::<usize, RelationalFragmentId>::new();
    for (index, id) in ids.iter().enumerate() {
        let root = sets.find(index);
        minimum_by_root
            .entry(root)
            .and_modify(|minimum| *minimum = (*minimum).clone().min(id.clone()))
            .or_insert_with(|| id.clone());
    }
    ids.into_iter()
        .enumerate()
        .map(|(index, id)| {
            let component = minimum_by_root[&sets.find(index)].clone();
            (id, component)
        })
        .collect()
}

fn compile_component(
    fragment_ids: &[RelationalFragmentId],
    fragments: &BTreeMap<RelationalFragmentId, &RelationalFragment>,
    connections: &[RelationalConnection],
    components: &BTreeMap<RelationalFragmentId, RelationalFragmentId>,
) -> Result<
    (
        CompiledRelationalPlan,
        BTreeMap<(RelationalFragmentId, RelationalOperatorIndex), RelationalOperatorIndex>,
    ),
    RelationalPlanningError,
> {
    let component = &components[&fragment_ids[0]];
    let bindings = connections
        .iter()
        .map(|connection| {
            (
                (connection.consumer.clone(), connection.consumer_input),
                connection,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut operators = Vec::new();
    let mut roots = BTreeMap::<RelationalFragmentId, RelationalOperatorIndex>::new();
    let mut boundary_inputs = BTreeMap::new();

    for id in fragment_ids {
        let fragment = fragments[id];
        let mut remapped = Vec::<RelationalOperatorIndex>::with_capacity(fragment.operators.len());
        for (position, operator) in fragment.operators.iter().enumerate() {
            let local_index = RelationalOperatorIndex::new(position as u32);
            if matches!(operator, RelationalOperator::Input { .. }) {
                if let Some(connection) = bindings.get(&(id.clone(), local_index)) {
                    if &components[&connection.producer] == component {
                        let producer_root = roots
                            .get(&connection.producer)
                            .copied()
                            .ok_or_else(|| RelationalPlanningError::CyclicConnections)?;
                        remapped.push(producer_root);
                        continue;
                    }
                }
            }
            let compiled = remap_operator(operator, &remapped);
            let index = RelationalOperatorIndex::new(operators.len() as u32);
            operators.push(compiled);
            remapped.push(index);
            if matches!(operator, RelationalOperator::Input { .. })
                && bindings
                    .get(&(id.clone(), local_index))
                    .is_some_and(|connection| &components[&connection.producer] != component)
            {
                boundary_inputs.insert((id.clone(), local_index), index);
            }
        }
        roots.insert(id.clone(), remapped[fragment.root.index()]);
    }

    let exposed_roots = fragment_ids
        .iter()
        .filter(|id| {
            !connections.iter().any(|connection| {
                &connection.producer == *id && &components[&connection.consumer] == component
            }) || connections.iter().any(|connection| {
                &connection.producer == *id && &components[&connection.consumer] != component
            })
        })
        .map(|id| roots[id])
        .collect::<Vec<_>>();
    let pushdown_hints = infer_pushdown_hints(&operators, &exposed_roots);

    let fragment_roots = fragment_ids
        .iter()
        .map(|fragment| RelationalFragmentRoot {
            fragment: fragment.clone(),
            operator: roots[fragment],
        })
        .collect::<Vec<_>>();
    Ok((
        CompiledRelationalPlan {
            fragment_order: fragment_ids.to_vec().into_boxed_slice(),
            operators: operators.into_boxed_slice(),
            fragment_roots: fragment_roots.into_boxed_slice(),
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
            roots: exposed_roots.into_boxed_slice(),
            pushdown_hints: pushdown_hints.into_boxed_slice(),
        },
        boundary_inputs,
    ))
}

fn remap_operator(
    operator: &RelationalOperator,
    remapped: &[RelationalOperatorIndex],
) -> RelationalOperator {
    let map = |index: RelationalOperatorIndex| remapped[index.index()];
    match operator {
        RelationalOperator::Input { name } => RelationalOperator::Input { name: name.clone() },
        RelationalOperator::Source { resource, relation } => RelationalOperator::Source {
            resource: resource.clone(),
            relation: relation.clone(),
        },
        RelationalOperator::Project { input, columns } => RelationalOperator::Project {
            input: map(*input),
            columns: columns.clone(),
        },
        RelationalOperator::Filter { input, predicate } => RelationalOperator::Filter {
            input: map(*input),
            predicate: predicate.clone(),
        },
        RelationalOperator::Rename { input, columns } => RelationalOperator::Rename {
            input: map(*input),
            columns: columns.clone(),
        },
        RelationalOperator::Limit { input, rows } => RelationalOperator::Limit {
            input: map(*input),
            rows: *rows,
        },
        RelationalOperator::Union { inputs, all } => RelationalOperator::Union {
            inputs: inputs.iter().map(|input| map(*input)).collect(),
            all: *all,
        },
    }
}

fn infer_pushdown_hints(
    operators: &[RelationalOperator],
    roots: &[RelationalOperatorIndex],
) -> Vec<RelationalPushdownHint> {
    infer_relational_pushdown_hints(operators, roots)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationalPlanningError {
    DuplicateFragment(RelationalFragmentId),
    UnknownFragment(RelationalFragmentId),
    InvalidRoot(RelationalFragmentId),
    InvalidOperatorInput {
        fragment: RelationalFragmentId,
        operator: RelationalOperatorIndex,
        input: RelationalOperatorIndex,
    },
    InvalidConsumerInput {
        fragment: RelationalFragmentId,
        input: RelationalOperatorIndex,
    },
    DuplicateInputBinding {
        fragment: RelationalFragmentId,
        input: RelationalOperatorIndex,
    },
    CyclicConnections,
}

impl fmt::Display for RelationalPlanningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFragment(id) => {
                write!(formatter, "duplicate relational fragment {}", id.as_str())
            }
            Self::UnknownFragment(id) => {
                write!(formatter, "unknown relational fragment {}", id.as_str())
            }
            Self::InvalidRoot(id) => write!(
                formatter,
                "invalid root for relational fragment {}",
                id.as_str()
            ),
            Self::InvalidOperatorInput { fragment, .. } => write!(
                formatter,
                "relational fragment {} has a non-prior operator input",
                fragment.as_str()
            ),
            Self::InvalidConsumerInput { fragment, .. } => write!(
                formatter,
                "connection target in {} is not an input operator",
                fragment.as_str()
            ),
            Self::DuplicateInputBinding { fragment, .. } => write!(
                formatter,
                "relational input in {} has multiple producers",
                fragment.as_str()
            ),
            Self::CyclicConnections => {
                formatter.write_str("relational fragment connections are cyclic")
            }
        }
    }
}

impl std::error::Error for RelationalPlanningError {}

struct DisjointSets {
    parents: Vec<usize>,
}

impl DisjointSets {
    fn new(len: usize) -> Self {
        Self {
            parents: (0..len).collect(),
        }
    }

    fn find(&mut self, index: usize) -> usize {
        if self.parents[index] != index {
            self.parents[index] = self.find(self.parents[index]);
        }
        self.parents[index]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left != right {
            self.parents[right] = left;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::plan::{
        RelationalExpression, RelationalLiteral, RelationalProjection, ResourceId,
    };

    fn fragment_id(value: &str) -> RelationalFragmentId {
        RelationalFragmentId::new(value).unwrap()
    }

    fn backend() -> RelationalBackendId {
        RelationalBackendId::new("relational.default").unwrap()
    }

    fn source(id: &str) -> RelationalFragment {
        RelationalFragment {
            id: fragment_id(id),
            operators: Box::new([RelationalOperator::Source {
                resource: ResourceId::new("database.main").unwrap(),
                relation: id.into(),
            }]),
            root: RelationalOperatorIndex::new(0),
        }
    }

    fn input_limit(id: &str) -> RelationalFragment {
        RelationalFragment {
            id: fragment_id(id),
            operators: Box::new([
                RelationalOperator::Input {
                    name: "input".into(),
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows: 10,
                },
            ]),
            root: RelationalOperatorIndex::new(1),
        }
    }

    fn connection(producer: &str, consumer: &str) -> RelationalConnection {
        RelationalConnection {
            producer: fragment_id(producer),
            consumer: fragment_id(consumer),
            consumer_input: RelationalOperatorIndex::new(0),
            production: OutputProduction::Streaming,
            consumption: InputConsumption::Streaming,
        }
    }

    #[test]
    fn builds_maximal_relational_islands() {
        let fragments = [input_limit("c"), source("a"), source("d"), input_limit("b")];
        let connections = [connection("a", "b"), connection("b", "c")];

        let result = RelationalPlanner::new(backend())
            .plan(&fragments, &connections)
            .unwrap();

        assert_eq!(result.subplans.len(), 2);
        assert_eq!(
            result.subplans[0].compiled_plan.fragment_order.as_ref(),
            &[fragment_id("a"), fragment_id("b"), fragment_id("c")]
        );
        assert_eq!(result.subplans[0].compiled_plan.operators.len(), 3);
        assert!(result.bridges.is_empty());
    }

    #[test]
    fn produces_deterministic_subplan_and_fragment_order() {
        let first = RelationalPlanner::new(backend())
            .plan(&[source("z"), source("a"), source("m")], &[])
            .unwrap();
        let second = RelationalPlanner::new(backend())
            .plan(&[source("m"), source("z"), source("a")], &[])
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(
            first.subplans[0].compiled_plan.fragment_order[0],
            fragment_id("a")
        );
    }

    #[test]
    fn materialization_contract_prevents_unsafe_merge() {
        let fragments = [source("producer"), input_limit("consumer")];
        let mut edge = connection("producer", "consumer");
        edge.consumption = InputConsumption::FullyMaterialized;

        let result = RelationalPlanner::new(backend())
            .plan(&fragments, &[edge])
            .unwrap();

        assert_eq!(result.subplans.len(), 2);
        assert_eq!(result.bridges.len(), 1);
        assert_eq!(result.bridges[0].bridge, MaterializationBridge::Collect);
        assert!(matches!(
            result.subplans[1].compiled_plan.operators[0],
            RelationalOperator::Input { .. }
        ));
    }

    #[test]
    fn requests_cross_island_producer_fragment_outputs_once() {
        let fragments = [
            source("producer"),
            input_limit("consumer-a"),
            input_limit("consumer-b"),
        ];
        let mut first = connection("producer", "consumer-a");
        first.consumption = InputConsumption::FullyMaterialized;
        let mut second = connection("producer", "consumer-b");
        second.consumption = InputConsumption::FullyMaterialized;

        let result = RelationalPlanner::new(backend())
            .plan(&fragments, &[second, first])
            .unwrap();

        assert_eq!(
            result.subplans[0]
                .compiled_plan
                .requested_fragment_outputs
                .as_ref(),
            &[fragment_id("producer")]
        );
        assert!(
            result.subplans[1]
                .compiled_plan
                .requested_fragment_outputs
                .is_empty()
        );
        assert!(
            result.subplans[2]
                .compiled_plan
                .requested_fragment_outputs
                .is_empty()
        );
    }

    #[test]
    fn derives_bridges_from_consumer_contracts() {
        assert_eq!(
            materialization_bridge(OutputProduction::Streaming, InputConsumption::Streaming),
            MaterializationBridge::Stream
        );
        assert_eq!(
            materialization_bridge(
                OutputProduction::Streaming,
                InputConsumption::SinglePassBatches
            ),
            MaterializationBridge::Buffer
        );
        assert_eq!(
            materialization_bridge(
                OutputProduction::Batches,
                InputConsumption::RewindableBatches
            ),
            MaterializationBridge::Replay
        );
        assert_eq!(
            materialization_bridge(OutputProduction::Batches, InputConsumption::RandomAccess),
            MaterializationBridge::Spill
        );
        assert_eq!(
            materialization_bridge(
                OutputProduction::Streaming,
                InputConsumption::FullyMaterialized
            ),
            MaterializationBridge::Collect
        );
        assert_eq!(
            materialization_bridge(
                OutputProduction::FullyMaterialized,
                InputConsumption::RandomAccess
            ),
            MaterializationBridge::Stream
        );
    }

    #[test]
    fn emits_limit_pushdown_hint_for_direct_source() {
        let fragment = RelationalFragment {
            id: fragment_id("limit"),
            operators: Box::new([
                RelationalOperator::Source {
                    resource: ResourceId::new("database.main").unwrap(),
                    relation: "orders".into(),
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows: 25,
                },
            ]),
            root: RelationalOperatorIndex::new(1),
        };

        let result = RelationalPlanner::new(backend())
            .plan(&[fragment], &[])
            .unwrap();
        assert_eq!(
            result.subplans[0].compiled_plan.pushdown_hints.as_ref(),
            &[RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 25,
            }]
        );
    }

    fn input_operator(operator: RelationalOperator) -> Box<[RelationalOperator]> {
        Box::new([
            RelationalOperator::Input {
                name: "source".into(),
            },
            operator,
        ])
    }

    fn direct_projection(name: &str) -> RelationalProjection {
        RelationalProjection {
            name: name.into(),
            expression: RelationalExpression::Column(name.into()),
        }
    }

    #[test]
    fn full_relational_chain_is_one_deterministic_zero_bridge_island() {
        let predicate = RelationalExpression::GreaterThan(
            Box::new(RelationalExpression::Column("amount".into())),
            Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(
                10,
            ))),
        );
        let fragments = [
            source("source"),
            RelationalFragment {
                id: fragment_id("filter"),
                operators: input_operator(RelationalOperator::Filter {
                    input: RelationalOperatorIndex::new(0),
                    predicate: predicate.clone(),
                }),
                root: RelationalOperatorIndex::new(1),
            },
            RelationalFragment {
                id: fragment_id("project"),
                operators: input_operator(RelationalOperator::Project {
                    input: RelationalOperatorIndex::new(0),
                    columns: Box::new([direct_projection("amount")]),
                }),
                root: RelationalOperatorIndex::new(1),
            },
            RelationalFragment {
                id: fragment_id("rename"),
                operators: input_operator(RelationalOperator::Rename {
                    input: RelationalOperatorIndex::new(0),
                    columns: Box::new([crate::node_system::plan::RelationalRename {
                        from: "amount".into(),
                        to: "total".into(),
                    }]),
                }),
                root: RelationalOperatorIndex::new(1),
            },
            RelationalFragment {
                id: fragment_id("limit"),
                operators: input_operator(RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows: 25,
                }),
                root: RelationalOperatorIndex::new(1),
            },
        ];
        let connections = [
            connection("source", "filter"),
            connection("filter", "project"),
            connection("project", "rename"),
            connection("rename", "limit"),
        ];

        let first = RelationalPlanner::new(backend())
            .plan(&fragments, &connections)
            .unwrap();
        let reversed = RelationalPlanner::new(backend())
            .plan(
                &fragments.iter().cloned().rev().collect::<Vec<_>>(),
                &connections.iter().cloned().rev().collect::<Vec<_>>(),
            )
            .unwrap();

        assert_eq!(first, reversed);
        assert_eq!(first.subplans.len(), 1);
        assert!(first.bridges.is_empty());
        let plan = &first.subplans[0].compiled_plan;
        assert_eq!(plan.operators.len(), 5);
        assert_eq!(
            plan.fragment_order.as_ref(),
            ["source", "filter", "project", "rename", "limit"]
                .map(fragment_id)
                .as_slice()
        );
        assert!(
            plan.operators
                .iter()
                .all(|operator| !matches!(operator, RelationalOperator::Input { .. }))
        );
        assert_eq!(plan.roots.as_ref(), [RelationalOperatorIndex::new(4)]);
        for (index, operator) in plan.operators.iter().enumerate().skip(1) {
            assert_eq!(
                operator_inputs(operator),
                vec![RelationalOperatorIndex::new(index as u32 - 1)]
            );
        }
    }

    #[test]
    fn projection_lineage_preserves_declared_column_order() {
        let fragment = RelationalFragment {
            id: fragment_id("ordered-project"),
            operators: Box::new([
                RelationalOperator::Source {
                    resource: ResourceId::new("database.main").unwrap(),
                    relation: "orders".into(),
                },
                RelationalOperator::Project {
                    input: RelationalOperatorIndex::new(0),
                    columns: Box::new([direct_projection("b"), direct_projection("a")]),
                },
            ]),
            root: RelationalOperatorIndex::new(1),
        };

        let result = RelationalPlanner::new(backend())
            .plan(&[fragment], &[])
            .unwrap();

        assert_eq!(
            result.subplans[0].compiled_plan.pushdown_hints.as_ref(),
            [RelationalPushdownHint::Projection {
                source: RelationalOperatorIndex::new(0),
                columns: Box::new(["b".into(), "a".into()]),
            }]
        );
    }

    #[test]
    fn infers_exact_projection_and_predicate_lineage_through_rename() {
        let predicate = RelationalExpression::Equal(
            Box::new(RelationalExpression::Column("visible_status".into())),
            Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                "paid".into(),
            ))),
        );
        let fragment = RelationalFragment {
            id: fragment_id("lineage"),
            operators: Box::new([
                RelationalOperator::Source {
                    resource: ResourceId::new("database.main").unwrap(),
                    relation: "orders".into(),
                },
                RelationalOperator::Rename {
                    input: RelationalOperatorIndex::new(0),
                    columns: Box::new([crate::node_system::plan::RelationalRename {
                        from: "status".into(),
                        to: "visible_status".into(),
                    }]),
                },
                RelationalOperator::Filter {
                    input: RelationalOperatorIndex::new(1),
                    predicate,
                },
                RelationalOperator::Project {
                    input: RelationalOperatorIndex::new(2),
                    columns: Box::new([direct_projection("amount")]),
                },
            ]),
            root: RelationalOperatorIndex::new(3),
        };

        let result = RelationalPlanner::new(backend())
            .plan(&[fragment], &[])
            .unwrap();

        assert_eq!(
            result.subplans[0].compiled_plan.pushdown_hints.as_ref(),
            [
                RelationalPushdownHint::Projection {
                    source: RelationalOperatorIndex::new(0),
                    columns: Box::new(["amount".into(), "status".into()]),
                },
                RelationalPushdownHint::Predicate {
                    source: RelationalOperatorIndex::new(0),
                    predicate: RelationalExpression::Equal(
                        Box::new(RelationalExpression::Column("status".into())),
                        Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                            "paid".into(),
                        ))),
                    ),
                },
            ]
        );
    }

    #[test]
    fn multiple_roots_union_lineage_and_boundaries_stop_it() {
        let operators = Box::new([
            RelationalOperator::Source {
                resource: ResourceId::new("database.main").unwrap(),
                relation: "orders".into(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([direct_projection("amount")]),
            },
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::IsNull(Box::new(RelationalExpression::Column(
                    "status".into(),
                ))),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(2),
                columns: Box::new([direct_projection("id")]),
            },
        ]);
        let fragments = [
            RelationalFragment {
                id: fragment_id("project-root"),
                operators: operators.clone(),
                root: RelationalOperatorIndex::new(1),
            },
            RelationalFragment {
                id: fragment_id("filter-root"),
                operators,
                root: RelationalOperatorIndex::new(3),
            },
        ];
        let inferred = infer_relational_pushdown_hints(
            &fragments[0].operators,
            &[
                RelationalOperatorIndex::new(3),
                RelationalOperatorIndex::new(1),
            ],
        );
        assert_eq!(
            inferred,
            vec![
                RelationalPushdownHint::Projection {
                    source: RelationalOperatorIndex::new(0),
                    columns: Box::new(["amount".into(), "id".into(), "status".into()]),
                },
                RelationalPushdownHint::Predicate {
                    source: RelationalOperatorIndex::new(0),
                    predicate: RelationalExpression::IsNull(Box::new(
                        RelationalExpression::Column("status".into()),
                    )),
                },
            ]
        );

        let input_boundary = [
            RelationalOperator::Input {
                name: "bridge".into(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([direct_projection("amount")]),
            },
        ];
        assert!(
            infer_relational_pushdown_hints(&input_boundary, &[RelationalOperatorIndex::new(1)])
                .is_empty()
        );

        let union_boundary = [
            RelationalOperator::Source {
                resource: ResourceId::new("database.a").unwrap(),
                relation: "a".into(),
            },
            RelationalOperator::Source {
                resource: ResourceId::new("database.b").unwrap(),
                relation: "b".into(),
            },
            RelationalOperator::Union {
                inputs: Box::new([
                    RelationalOperatorIndex::new(0),
                    RelationalOperatorIndex::new(1),
                ]),
                all: true,
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(2),
                columns: Box::new([direct_projection("amount")]),
            },
        ];
        assert!(
            infer_relational_pushdown_hints(&union_boundary, &[RelationalOperatorIndex::new(3)])
                .is_empty()
        );
    }

    #[test]
    fn derived_project_makes_shared_source_unhintable_in_any_root_order() {
        let operators = [
            RelationalOperator::Source {
                resource: ResourceId::new("database.main").unwrap(),
                relation: "orders".into(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([RelationalProjection {
                    name: "derived".into(),
                    expression: RelationalExpression::Literal(RelationalLiteral::Integer(1)),
                }]),
            },
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::IsNull(Box::new(RelationalExpression::Column(
                    "status".into(),
                ))),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(2),
                columns: Box::new([direct_projection("amount")]),
            },
        ];

        for roots in [
            [
                RelationalOperatorIndex::new(1),
                RelationalOperatorIndex::new(3),
            ],
            [
                RelationalOperatorIndex::new(3),
                RelationalOperatorIndex::new(1),
            ],
        ] {
            assert!(infer_relational_pushdown_hints(&operators, &roots).is_empty());
        }
    }

    #[test]
    fn opaque_metadata_lineage_preserves_direct_source_limit_hint() {
        let operators = [
            RelationalOperator::Source {
                resource: ResourceId::new("database.main").unwrap(),
                relation: "orders".into(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([RelationalProjection {
                    name: "derived".into(),
                    expression: RelationalExpression::Literal(RelationalLiteral::Integer(1)),
                }]),
            },
            RelationalOperator::Limit {
                input: RelationalOperatorIndex::new(0),
                rows: 25,
            },
        ];

        assert_eq!(
            infer_relational_pushdown_hints(
                &operators,
                &[
                    RelationalOperatorIndex::new(1),
                    RelationalOperatorIndex::new(2)
                ],
            ),
            vec![RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 25,
            }]
        );
    }

    #[test]
    fn union_makes_shared_descendant_source_unhintable_in_any_root_order() {
        let operators = [
            RelationalOperator::Source {
                resource: ResourceId::new("database.shared").unwrap(),
                relation: "shared".into(),
            },
            RelationalOperator::Source {
                resource: ResourceId::new("database.other").unwrap(),
                relation: "other".into(),
            },
            RelationalOperator::Union {
                inputs: Box::new([
                    RelationalOperatorIndex::new(0),
                    RelationalOperatorIndex::new(1),
                ]),
                all: true,
            },
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::IsNull(Box::new(RelationalExpression::Column(
                    "status".into(),
                ))),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(3),
                columns: Box::new([direct_projection("amount")]),
            },
        ];

        for roots in [
            [
                RelationalOperatorIndex::new(2),
                RelationalOperatorIndex::new(4),
            ],
            [
                RelationalOperatorIndex::new(4),
                RelationalOperatorIndex::new(2),
            ],
        ] {
            assert!(infer_relational_pushdown_hints(&operators, &roots).is_empty());
        }
    }

    #[test]
    fn emits_only_safe_projection_pushdown_hints() {
        let fragment = RelationalFragment {
            id: fragment_id("project"),
            operators: Box::new([
                RelationalOperator::Source {
                    resource: ResourceId::new("database.main").unwrap(),
                    relation: "orders".into(),
                },
                RelationalOperator::Project {
                    input: RelationalOperatorIndex::new(0),
                    columns: Box::new([RelationalProjection {
                        name: "id".into(),
                        expression: RelationalExpression::Column("order_id".into()),
                    }]),
                },
                RelationalOperator::Filter {
                    input: RelationalOperatorIndex::new(1),
                    predicate: RelationalExpression::Equal(
                        Box::new(RelationalExpression::Column("id".into())),
                        Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(1))),
                    ),
                },
            ]),
            root: RelationalOperatorIndex::new(2),
        };

        let result = RelationalPlanner::new(backend())
            .plan(&[fragment], &[])
            .unwrap();
        assert_eq!(
            result.subplans[0].compiled_plan.pushdown_hints.as_ref(),
            &[
                RelationalPushdownHint::Projection {
                    source: RelationalOperatorIndex::new(0),
                    columns: Box::new([Box::<str>::from("order_id")]),
                },
                RelationalPushdownHint::Predicate {
                    source: RelationalOperatorIndex::new(0),
                    predicate: RelationalExpression::Equal(
                        Box::new(RelationalExpression::Column("order_id".into())),
                        Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(1))),
                    ),
                },
            ]
        );
    }
}
