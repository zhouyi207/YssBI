use std::sync::Arc;

use crate::graph::error::GraphMutationError;
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use yss_graph_document::{GraphDocument, GraphRevision};

#[derive(Clone, Debug)]
pub enum GraphMutation {
    ReplaceDocument { candidate: GraphDocument },
}

#[must_use]
pub(crate) struct PlannedGraphMutation {
    candidate_document: Arc<GraphDocument>,
}

impl PlannedGraphMutation {
    fn from_validated_candidate(candidate_document: Arc<GraphDocument>) -> Self {
        Self { candidate_document }
    }

    pub(crate) fn into_candidate_document(self) -> Arc<GraphDocument> {
        self.candidate_document
    }
}

pub(crate) fn plan_graph_mutation(
    document: &GraphDocument,
    basis_revision: GraphRevision,
    request: GraphMutation,
    _catalog: &ResourceCatalogSnapshot,
) -> Result<PlannedGraphMutation, GraphMutationError> {
    if document.revision != basis_revision {
        return Err(GraphMutationError::InvalidMutation {
            code: crate::graph::error::GraphMutationErrorCode::GraphConnectionTypeUnavailable,
        });
    }
    let GraphMutation::ReplaceDocument { candidate } = request;
    if candidate.revision != basis_revision {
        return Err(GraphMutationError::InvalidMutation {
            code: crate::graph::error::GraphMutationErrorCode::GraphConnectionTypeUnavailable,
        });
    }
    Ok(PlannedGraphMutation::from_validated_candidate(Arc::new(
        candidate,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn planned_mutation_returns_only_a_consuming_candidate() {
        let document = GraphDocument::default();
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            crate::graph::resource_catalog::ResourceCatalogFingerprint::from_bytes([1; 32]),
        );
        let planned = plan_graph_mutation(
            &document,
            GraphRevision::INITIAL,
            GraphMutation::ReplaceDocument {
                candidate: document.clone(),
            },
            &catalog,
        )
        .unwrap();
        assert_eq!(planned.into_candidate_document().as_ref(), &document);
    }
}
