use crate::graph::analysis::GraphAnalysis;
use crate::graph_document::{GraphDocument, GraphResourcePath, GraphRevision};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorProjectionBasis {
    pub graph_path: GraphResourcePath,
    pub graph_revision: GraphRevision,
    pub registry_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorNodeModel {
    pub node_id: crate::graph_document::NodeId,
    pub node_type: crate::node_system::protocol::NodeTypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorConnectionModel {
    pub connection_id: crate::graph_document::ConnectionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorDiagnosticModel {
    pub code: Box<str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorCompilationOutcome {
    Complete,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorProjectionModel {
    pub basis: EditorProjectionBasis,
    pub graph_path: GraphResourcePath,
    pub source_revision: GraphRevision,
    pub nodes: Box<[EditorNodeModel]>,
    pub connections: Box<[EditorConnectionModel]>,
    pub diagnostics: Box<[EditorDiagnosticModel]>,
    pub outcome: EditorCompilationOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterEditorKind {
    Text,
    Number,
    Toggle,
    Select,
}

#[derive(Debug, thiserror::Error)]
pub enum EditorProjectionError {
    #[error("analysis and document revisions do not match")]
    RevisionMismatch,
    #[error("analysis and catalog registry fingerprints do not match")]
    RegistryMismatch,
    #[error("projection basis is stale")]
    StaleProjectionBasis,
    #[error("projection graphs are incompatible")]
    IncompatibleProjectionGraphs,
    #[error("projection delta is invalid")]
    InvalidDelta,
}

pub struct EditorProjectionInput<'a> {
    pub graph_path: &'a GraphResourcePath,
    pub document: &'a GraphDocument,
    pub analysis: &'a GraphAnalysis,
    pub registry_fingerprint: [u8; 32],
}

pub fn build_editor_projection(
    input: EditorProjectionInput<'_>,
) -> Result<EditorProjectionModel, EditorProjectionError> {
    if input.analysis.registry_fingerprint() != &input.registry_fingerprint {
        return Err(EditorProjectionError::RegistryMismatch);
    }
    let nodes = input
        .analysis
        .nodes()
        .iter()
        .map(|node| EditorNodeModel {
            node_id: node.node_id,
            node_type: node.node_type.clone(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let connections = input
        .document
        .connections
        .keys()
        .map(|connection_id| EditorConnectionModel {
            connection_id: *connection_id,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Ok(EditorProjectionModel {
        basis: EditorProjectionBasis {
            graph_path: input.graph_path.clone(),
            graph_revision: input.document.revision,
            registry_fingerprint: input.registry_fingerprint,
        },
        graph_path: input.graph_path.clone(),
        source_revision: input.document.revision,
        nodes,
        connections,
        diagnostics: Box::new([]),
        outcome: EditorCompilationOutcome::Complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::{
        PlanCompilationBasis, PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint,
    };
    use crate::graph::analysis::{GraphAnalysisInput, analyze};
    use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
    use crate::graph::settings::GraphCompileSettings;
    use std::collections::BTreeMap;

    #[test]
    fn editor_projection_preserves_document_identity_and_basis() {
        let document = GraphDocument::default();
        let path = GraphResourcePath::new("events/editor.yssbi-event").unwrap();
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([1; 32]),
        );
        let settings = GraphCompileSettings {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-9,
        };
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("project".into()),
            PlanGraphRevision::from_existing(0),
            PlanRegistryFingerprint::from_bytes([2; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let analysis = analyze(GraphAnalysisInput {
            document: &document,
            catalog: &catalog,
            settings: &settings,
            basis: &basis,
        });
        let model = build_editor_projection(EditorProjectionInput {
            graph_path: &path,
            document: &document,
            analysis: &analysis,
            registry_fingerprint: [2; 32],
        })
        .unwrap();
        assert_eq!(model.graph_path, path);
        assert_eq!(model.source_revision, GraphRevision::INITIAL);
    }
}
