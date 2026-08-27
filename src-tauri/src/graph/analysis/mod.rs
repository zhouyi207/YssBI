use crate::execution::plan::PlanCompilationBasis;
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::graph::settings::GraphCompileSettings;
use crate::graph_document::GraphDocument;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphNodeFact {
    pub node_id: crate::graph_document::NodeId,
    pub node_type: crate::node_system::protocol::NodeTypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphAnalysis {
    nodes: Box<[GraphNodeFact]>,
    registry_fingerprint: [u8; 32],
}

impl GraphAnalysis {
    pub fn nodes(&self) -> &[GraphNodeFact] {
        &self.nodes
    }

    pub fn registry_fingerprint(&self) -> &[u8; 32] {
        &self.registry_fingerprint
    }
}

pub struct GraphAnalysisInput<'a> {
    pub document: &'a GraphDocument,
    pub catalog: &'a ResourceCatalogSnapshot,
    pub settings: &'a GraphCompileSettings,
    pub basis: &'a PlanCompilationBasis,
}

pub fn analyze(input: GraphAnalysisInput<'_>) -> GraphAnalysis {
    let _ = (
        input.catalog.fingerprint(),
        input.settings.absolute_tolerance,
    );
    let nodes = input
        .document
        .nodes
        .values()
        .map(|node| GraphNodeFact {
            node_id: node.id,
            node_type: node.node_type.clone(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    GraphAnalysis {
        nodes,
        registry_fingerprint: input.basis.registry_fingerprint().as_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::{
        PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint,
    };
    use std::collections::BTreeMap;

    #[test]
    fn analysis_accepts_neutral_document_catalog_settings_and_basis() {
        let document = GraphDocument::default();
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            crate::graph::resource_catalog::ResourceCatalogFingerprint::from_bytes([3; 32]),
        );
        let settings = GraphCompileSettings {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-9,
        };
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("project".into()),
            PlanGraphRevision::from_existing(1),
            PlanRegistryFingerprint::from_bytes([4; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let analysis = analyze(GraphAnalysisInput {
            document: &document,
            catalog: &catalog,
            settings: &settings,
            basis: &basis,
        });
        assert!(analysis.nodes().is_empty());
        assert_eq!(analysis.registry_fingerprint(), &[4; 32]);
    }
}
