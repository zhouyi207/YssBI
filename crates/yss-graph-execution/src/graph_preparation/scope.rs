//! Select the graph nodes needed before readiness, resource admission and plan construction.
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_analysis::{GraphResolvedInputSource, GraphSemanticSnapshot};
use yss_graph_document::{GraphResourcePath, NodeId};
use yss_node_protocol::PortDirection;

use super::GraphPlanError;
use crate::plan::PlanExecutionDemand;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphExecutionScope {
    nodes: BTreeSet<NodeId>,
}

impl GraphExecutionScope {
    pub fn all(semantics: &GraphSemanticSnapshot) -> Self {
        Self {
            nodes: semantics.nodes().iter().map(|node| node.node_id).collect(),
        }
    }

    pub fn select(
        graph: &GraphResourcePath,
        semantics: &GraphSemanticSnapshot,
        demand: &PlanExecutionDemand,
    ) -> Result<Self, GraphPlanError> {
        let by_id = semantics
            .nodes()
            .iter()
            .map(|node| (node.node_id, node))
            .collect::<BTreeMap<_, _>>();
        let by_output = semantics
            .nodes()
            .iter()
            .flat_map(|node| {
                node.ports
                    .iter()
                    .filter(|port| port.direction == PortDirection::Output)
                    .map(move |port| (port.address.to_string(), node.node_id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut pending = Vec::new();
        match demand {
            PlanExecutionDemand::Default
            | PlanExecutionDemand::Outputs {
                include_default_results: true,
                ..
            } => return Ok(Self::all(semantics)),
            PlanExecutionDemand::Node { node, .. } => {
                let id = NodeId::from_uuid(
                    uuid::Uuid::parse_str(node.as_str())
                        .map_err(|_| GraphPlanError::InvalidGraph)?,
                );
                if !by_id.contains_key(&id) {
                    return Err(GraphPlanError::InvalidGraph);
                }
                pending.push(id);
            }
            PlanExecutionDemand::Outputs { outputs, .. } => {
                for output in outputs {
                    if output.graph().as_str() != graph.as_str() {
                        return Err(GraphPlanError::InvalidGraph);
                    }
                    pending.push(
                        *by_output
                            .get(output.port().as_str())
                            .ok_or(GraphPlanError::InvalidGraph)?,
                    );
                }
            }
        }
        let mut nodes = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !nodes.insert(id) {
                continue;
            }
            let node = by_id.get(&id).ok_or(GraphPlanError::InvalidGraph)?;
            pending.extend(node.inputs.iter().filter_map(|input| match &input.source {
                GraphResolvedInputSource::Output(source) => Some(source.node_id),
                GraphResolvedInputSource::Literal(_) => None,
            }));
        }
        Ok(Self { nodes })
    }

    pub fn nodes(&self) -> &BTreeSet<NodeId> {
        &self.nodes
    }

    /// Select the earliest data-dependent schemas needed by consumers in this scope.
    /// Readiness and schema provenance still come from the semantic snapshot.
    pub fn schema_frontier(
        &self,
        graph: &GraphResourcePath,
        semantics: &GraphSemanticSnapshot,
        satisfied: impl Fn(&crate::plan::PlanOutputRef) -> bool,
    ) -> Result<Option<(Self, PlanExecutionDemand)>, GraphPlanError> {
        use crate::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
        use yss_graph_analysis::GraphSchemaState;

        let consumed = semantics
            .nodes()
            .iter()
            .filter(|node| self.nodes.contains(&node.node_id))
            .flat_map(|node| &node.inputs)
            .filter_map(|input| match &input.source {
                GraphResolvedInputSource::Output(output) => Some(output),
                GraphResolvedInputSource::Literal(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let candidates = semantics
            .nodes()
            .iter()
            .filter(|node| self.nodes.contains(&node.node_id))
            .flat_map(|node| &node.ports)
            .filter(|port| {
                port.direction == PortDirection::Output
                    && consumed.contains(&port.address)
                    && matches!(
                        port.schema_state,
                        GraphSchemaState::Deferred | GraphSchemaState::Observed { .. }
                    )
            })
            .map(|port| {
                (
                    port.address.node_id,
                    PlanOutputRef::new(
                        PlanGraphId::from_existing(graph.as_str().into()),
                        PlanPortAddress::from_existing(port.address.to_string().into()),
                    ),
                )
            })
            .filter(|(_, output)| !satisfied(output))
            .collect::<Vec<_>>();
        let mut outputs = Vec::new();
        for (node, output) in &candidates {
            let demand = PlanExecutionDemand::Outputs {
                outputs: vec![output.clone()].into_boxed_slice(),
                include_default_results: false,
                reuse_inputs: false,
            };
            let dependencies = Self::select(graph, semantics, &demand)?;
            if semantics.nodes_ready(dependencies.nodes())
                && !candidates
                    .iter()
                    .any(|(other, _)| other != node && dependencies.nodes.contains(other))
            {
                outputs.push(output.clone());
            }
        }
        if outputs.is_empty() {
            return Ok(None);
        }
        let demand = PlanExecutionDemand::Outputs {
            outputs: outputs.into_boxed_slice(),
            include_default_results: false,
            reuse_inputs: false,
        };
        Ok(Some((Self::select(graph, semantics, &demand)?, demand)))
    }
}
