//! Bind semantic facts to the captured registry and resource analysis basis.
use crate::GraphSemanticSnapshot;
use yss_graph_analysis_contract::GraphAnalysisBasis;

#[derive(Clone, Debug, PartialEq)]
pub struct GraphAnalysis {
    registry_fingerprint: [u8; 32],
    kernel_fingerprint: [u8; 32],
    resource_observations: yss_graph_analysis_contract::ResourceObservationSet,
    semantic_snapshot: std::sync::Arc<GraphSemanticSnapshot>,
    semantic_input_hash: [u8; 32],
    definition_input_hash: [u8; 32],
}

impl GraphAnalysis {
    pub const fn kernel_fingerprint(&self) -> &[u8; 32] {
        &self.kernel_fingerprint
    }
    pub fn resource_observations(&self) -> &yss_graph_analysis_contract::ResourceObservationSet {
        &self.resource_observations
    }
    pub const fn semantic_input_hash(&self) -> &[u8; 32] {
        &self.semantic_input_hash
    }

    pub fn with_semantic_input_hash(mut self, hash: [u8; 32]) -> Self {
        self.semantic_input_hash = hash;
        self.definition_input_hash = hash;
        self
    }

    pub fn definition_input_hash(&self) -> &[u8; 32] {
        &self.definition_input_hash
    }

    pub fn with_observed_semantic_input_hash(mut self, hash: [u8; 32]) -> Self {
        self.semantic_input_hash = hash;
        self
    }
    pub fn registry_fingerprint(&self) -> &[u8; 32] {
        &self.registry_fingerprint
    }

    pub fn semantic_snapshot(&self) -> &GraphSemanticSnapshot {
        &self.semantic_snapshot
    }

    pub fn with_semantic_snapshot(mut self, snapshot: GraphSemanticSnapshot) -> Self {
        self.semantic_snapshot = std::sync::Arc::new(snapshot);
        self
    }

    pub fn map_semantic_snapshot(
        mut self,
        transform: impl FnOnce(GraphSemanticSnapshot) -> GraphSemanticSnapshot,
    ) -> Self {
        self.semantic_snapshot = std::sync::Arc::new(transform(std::sync::Arc::unwrap_or_clone(
            self.semantic_snapshot,
        )));
        self
    }
}

pub fn analyze(
    basis: &GraphAnalysisBasis,
    semantic_snapshot: GraphSemanticSnapshot,
) -> GraphAnalysis {
    GraphAnalysis {
        registry_fingerprint: *basis.registry_fingerprint.as_bytes(),
        kernel_fingerprint: basis.kernel_fingerprint,
        resource_observations: basis.resource_observations.clone(),
        semantic_snapshot: std::sync::Arc::new(semantic_snapshot),
        semantic_input_hash: [0; 32],
        definition_input_hash: [0; 32],
    }
}
