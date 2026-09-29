use yss_graph_document::{ConnectionId, GraphDocumentPatch, PortAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionIntent {
    Connect,
    MoveConnections,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionDecision {
    Append,
    Replace {
        displaced_connection_ids: Vec<ConnectionId>,
    },
    Invalid {
        reason: &'static str,
    },
}

impl ConnectionDecision {
    /// A move removes its own links as part of changing endpoints. Only other
    /// removed links are displaced and should appear in the replacement preview.
    pub fn from_patch(patch: &GraphDocumentPatch, moving: &[ConnectionId]) -> Self {
        let displaced_connection_ids = patch
            .operations
            .iter()
            .filter_map(|operation| match operation {
                yss_graph_document::GraphDocumentOperation::RemoveConnection { connection }
                    if !moving.contains(&connection.id) =>
                {
                    Some(connection.id)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if displaced_connection_ids.is_empty() {
            Self::Append
        } else {
            Self::Replace {
                displaced_connection_ids,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionCandidate {
    pub port: PortAddress,
    pub decision: ConnectionDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionCandidates {
    pub semantic_input_hash: [u8; 32],
    pub candidates: Vec<ConnectionCandidate>,
}
