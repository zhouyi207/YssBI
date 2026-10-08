use super::*;
use crate::mutations::EditorMutationPlanner;
use yss_graph_editor::projection::{
    ConnectionCandidate, ConnectionCandidates, ConnectionDecision, ConnectionIntent,
};

impl GraphRuntimeState {
    /// Preview the same mutation planner used by commits, including derived-port
    /// claims. Resolve once for the entire gesture's candidate set.
    pub fn connection_candidates(
        &self,
        graph: &GraphResourcePath,
        document: &GraphDocument,
        source: &PortAddress,
        intent: ConnectionIntent,
        catalog: &CatalogMutationValidationSnapshot,
        analysis: &GraphAnalysis,
    ) -> Result<ConnectionCandidates, MutationConflict> {
        let semantics = analysis.semantic_snapshot();
        let source_port = semantics.concrete_interface().port(source).ok_or_else(|| {
            MutationConflict::Editor(yss_graph_editor::EditorMutationError {
                code: yss_graph_editor::EditorMutationErrorCode::GraphPortNotFound,
                detail: "connection source is absent from the graph".into(),
            })
        })?;
        let moving = if intent == ConnectionIntent::MoveConnections {
            document
                .connections
                .values()
                .filter(|connection| &connection.output == source || &connection.input == source)
                .map(|connection| connection.id)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut planner =
            EditorMutationPlanner::new(graph, document, self.registry(), catalog, Some(analysis));
        let candidates = semantics
            .nodes()
            .iter()
            .flat_map(|node| node.ports.iter())
            .map(|target| {
                let mutation = match intent {
                    ConnectionIntent::Connect => {
                        let (output, input) = if source_port.direction == PortDirection::Output {
                            (source.clone(), target.address.clone())
                        } else {
                            (target.address.clone(), source.clone())
                        };
                        EditorGraphMutation::Connect {
                            output,
                            input,
                            order: None,
                        }
                    }
                    ConnectionIntent::MoveConnections => EditorGraphMutation::MoveConnections {
                        source: source.clone(),
                        target: target.address.clone(),
                    },
                };
                let decision = match planner.plan(mutation) {
                    Ok(patch) => ConnectionDecision::from_patch(&patch, &moving),
                    Err(error) => ConnectionDecision::Invalid {
                        reason: error.code(),
                    },
                };
                ConnectionCandidate {
                    port: target.address.clone(),
                    decision,
                }
            })
            .collect();
        Ok(ConnectionCandidates {
            semantic_input_hash: *analysis.semantic_input_hash(),
            candidates,
        })
    }
}
