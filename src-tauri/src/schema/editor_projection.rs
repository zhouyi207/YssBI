use crate::application::editor_projection::{EditorCompilationOutcome, EditorProjectionModel};
use crate::node_system::analysis::EditorGraphProjectionDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransportMappingError {
    #[error("editor projection transport basis is incomplete")]
    MissingResourceVersions,
    #[error("editor node transport facts are incomplete")]
    MissingNodeFacts,
    #[error("editor connection transport facts are incomplete")]
    MissingConnectionFacts,
    #[error("editor diagnostic transport facts are incomplete")]
    MissingDiagnosticFacts,
}

impl TryFrom<&EditorProjectionModel> for EditorGraphProjectionDto {
    type Error = TransportMappingError;

    fn try_from(model: &EditorProjectionModel) -> Result<Self, Self::Error> {
        if !model.nodes.is_empty() {
            return Err(TransportMappingError::MissingNodeFacts);
        }
        if !model.connections.is_empty() {
            return Err(TransportMappingError::MissingConnectionFacts);
        }
        if !model.diagnostics.is_empty() {
            return Err(TransportMappingError::MissingDiagnosticFacts);
        }

        match model.outcome {
            EditorCompilationOutcome::Complete | EditorCompilationOutcome::Incomplete => {}
        }

        Err(TransportMappingError::MissingResourceVersions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::editor_projection::{EditorProjectionBasis, EditorProjectionModel};
    use crate::graph_document::{GraphResourcePath, GraphRevision};

    #[test]
    fn editor_mapper_fails_closed_when_application_model_lacks_wire_facts() {
        let graph_path = GraphResourcePath::new("events/staged.yssbi-event").unwrap();
        let model = EditorProjectionModel {
            basis: EditorProjectionBasis {
                graph_path: graph_path.clone(),
                graph_revision: GraphRevision::new(4),
                registry_fingerprint: [5; 32],
            },
            graph_path,
            source_revision: GraphRevision::new(4),
            nodes: Box::new([]),
            connections: Box::new([]),
            diagnostics: Box::new([]),
            outcome: EditorCompilationOutcome::Complete,
        };

        assert_eq!(
            EditorGraphProjectionDto::try_from(&model),
            Err(TransportMappingError::MissingResourceVersions)
        );
    }
}
