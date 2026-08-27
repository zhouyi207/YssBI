use crate::graph_document::GraphResourcePath;
use crate::node_system::catalog::LocalizedCatalogDto;
use crate::node_system::document::PortAddressDto;
use crate::project::ResourceRevision;
use crate::project::{ProjectFilesystemError, ProjectInstanceId, ProjectState};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogCompatibilityRequest {
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    pub graph_revision: ResourceRevision,
    pub source_port: PortAddressDto,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogCompatibilityError {
    #[error("graph revision does not match the analyzed compatibility source")]
    GraphRevisionConflict,
    #[error("catalog project authority does not match the analyzed graph")]
    CatalogProjectStale,
    #[error("compatibility source port is invalid")]
    CompatibleSourceInvalid,
    #[error("graph is not loaded")]
    GraphNotLoaded,
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
}

pub fn get_compatible_node_catalog(
    state: &ProjectState,
    request: CatalogCompatibilityRequest,
) -> Result<LocalizedCatalogDto, CatalogCompatibilityError> {
    let snapshot = state.catalog_snapshot(&request.project_instance_id)?;
    let projection = state.graph_projection_for_project(
        &request.project_instance_id,
        &request.graph_path,
        &request.locale,
    )?;
    if projection.basis.graph_revision != request.graph_revision.get() {
        return Err(CatalogCompatibilityError::GraphRevisionConflict);
    }
    if projection.basis.registry_fingerprint != *snapshot.registry.fingerprint() {
        return Err(CatalogCompatibilityError::CatalogProjectStale);
    }

    let mut source = crate::node_system::compatibility::source_from_projection(
        &projection,
        snapshot.registry.as_ref(),
        request.source_port,
    )
    .map_err(|_| CatalogCompatibilityError::CompatibleSourceInvalid)?;
    let document = state
        .loaded_graph_document_for_catalog(&snapshot, &request.graph_path)?
        .ok_or(CatalogCompatibilityError::GraphNotLoaded)?;
    crate::node_system::compatibility::refine_source_type(
        &mut source,
        &document,
        snapshot.registry.as_ref(),
        &snapshot.validation,
    );
    state.validate_catalog_snapshot_current(&snapshot)?;

    Ok(crate::node_system::compatibility::compatible_catalog(
        &snapshot,
        &request.graph_path,
        &source,
        &request.locale,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_document::{DocumentNode, NodeId, NodePosition, ParameterValues};
    use crate::node_system::document::PortAddressDto;
    use crate::node_system::protocol::NodeTypeId;
    use crate::project::{GraphDocumentKind, GraphResourceDocument, ProjectData};

    #[test]
    fn compatible_catalog_filters_against_current_analyzed_source() {
        let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        let source_node = NodeId::new();
        let fixture = fixture(&graph_path, source_node);
        let state = fixture.state();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;

        let catalog = get_compatible_node_catalog(
            state,
            CatalogCompatibilityRequest {
                project_instance_id,
                graph_path,
                graph_revision: ResourceRevision::new(1),
                source_port: PortAddressDto::Declared {
                    node_id: source_node.to_string().into(),
                    port_key: "value".into(),
                },
                locale: "en-US".into(),
            },
        )
        .unwrap();
        let ids = catalog
            .items
            .iter()
            .map(|item| item.node_type_id.as_ref())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(ids.contains("yssbi.numeric.add.int64"));
        assert!(!ids.contains("yssbi.logic.not"));
    }

    #[test]
    fn stale_graph_revision_returns_typed_compatibility_error() {
        let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        let source_node = NodeId::new();
        let fixture = fixture(&graph_path, source_node);
        let state = fixture.state();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;

        let error = get_compatible_node_catalog(
            state,
            CatalogCompatibilityRequest {
                project_instance_id,
                graph_path,
                graph_revision: ResourceRevision::INITIAL,
                source_port: PortAddressDto::Declared {
                    node_id: source_node.to_string().into(),
                    port_key: "value".into(),
                },
                locale: "en-US".into(),
            },
        )
        .unwrap_err();

        assert_eq!(error, CatalogCompatibilityError::GraphRevisionConflict);
    }

    fn fixture(
        graph_path: &GraphResourcePath,
        source_node: NodeId,
    ) -> crate::project::fixtures::TempProject {
        let mut graph = GraphResourceDocument::new("Main", GraphDocumentKind::Event);
        graph
            .document
            .create_node(DocumentNode {
                id: source_node,
                node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            })
            .unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(graph_path.clone(), graph);
        crate::project::fixtures::TempProject::activate("compatible-catalog-application", project)
    }
}
