use super::*;
use crate::mutations::EditorMutationPlanner;
use yss_graph_editor::projection::{
    ConnectionCandidate, ConnectionCandidates, ConnectionDecision, ConnectionIntent,
};

/// Borrowed graph and validation inputs captured for one connection preview.
pub struct ConnectionCandidatesInput<'a> {
    pub graph: &'a GraphResourcePath,
    pub document: &'a GraphDocument,
    pub source: &'a PortAddress,
    pub intent: ConnectionIntent,
    pub catalog: &'a CatalogMutationValidationSnapshot,
    pub analysis: &'a GraphAnalysis,
}

impl GraphRuntimeState {
    /// Preview the same mutation planner used by commits, including derived-port
    /// claims. Resolve once for the entire gesture's candidate set.
    /// Returns `None` when superseded; cancellation is checked between candidate plans.
    pub fn connection_candidates(
        &self,
        input: ConnectionCandidatesInput<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<ConnectionCandidates>, MutationConflict> {
        if cancelled() {
            return Ok(None);
        }
        let ConnectionCandidatesInput {
            graph,
            document,
            source,
            intent,
            catalog,
            analysis,
        } = input;
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
        let mut candidates =
            Vec::with_capacity(semantics.nodes().iter().map(|node| node.ports.len()).sum());
        for target in semantics.nodes().iter().flat_map(|node| node.ports.iter()) {
            if cancelled() {
                return Ok(None);
            }
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
            candidates.push(ConnectionCandidate {
                port: target.address.clone(),
                decision,
            });
        }
        if cancelled() {
            return Ok(None);
        }
        Ok(Some(ConnectionCandidates {
            semantic_input_hash: *analysis.semantic_input_hash(),
            candidates,
        }))
    }
}
