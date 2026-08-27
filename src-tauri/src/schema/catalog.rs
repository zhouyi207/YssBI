use crate::application::catalog_query::CatalogQueryResult;
use crate::node_system::catalog::LocalizedCatalogDto;

impl From<CatalogQueryResult> for LocalizedCatalogDto {
    fn from(result: CatalogQueryResult) -> Self {
        let (project_instance_id, registry_fingerprint, resource_publication_revision, catalog) =
            result.into_transport_parts().into_fields();

        catalog.into_dto(
            project_instance_id.as_str(),
            registry_fingerprint.to_hex(),
            resource_publication_revision,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::catalog_query::LocalizedCatalogRequest;
    use crate::application::execution::{
        ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot,
    };
    use crate::database::runtime::DatabaseRuntimeRegistry;
    use crate::database_contract::{
        DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet,
        DatabaseId, DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use crate::execution::resource_preparation::ResourceProviderFactory;
    use crate::execution::state::ExecutionRuntimeState;
    use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
    use crate::graph::runtime_state::{
        GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState,
    };
    use crate::node_system::ProjectSessionId;
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::compiler::ProjectCompileCoordinator;
    use crate::project::{GraphDocumentKind, ProjectData};
    use std::collections::BTreeMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    fn application_with_function() -> (
        crate::project::fixtures::TempProject,
        crate::application::execution::ApplicationState,
    ) {
        let path = crate::graph_document::GraphResourcePath::new("functions/Opaque.yssbi-function")
            .unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            path.clone(),
            crate::project::GraphResourceDocument::new(
                "Opaque Function",
                GraphDocumentKind::Function,
            ),
        );
        let fixture = crate::project::fixtures::TempProject::activate(
            "catalog-schema-mapper",
            project.clone(),
        );
        let root = fixture.state().get_path().unwrap();
        crate::project::fixtures::write_graph(&project, &root, &path).unwrap();
        let project = Arc::new(fixture.state().clone());
        let project_instance_id = project.capture_project_session().unwrap().instance_id;
        let project_session_id = ProjectSessionId::new("catalog-schema-session");
        let execution_session_id = ExecutionSessionId::new(uuid::Uuid::new_v4());
        let builtin = build_builtin_node_system().unwrap();
        let graph = Arc::new(GraphRuntimeState::from_components(
            GraphRuntimeEpoch::from_existing(1),
            GraphRuntimeComponents {
                registry: builtin.registry,
                catalog: builtin.catalog,
                compiler: Arc::new(ProjectCompileCoordinator::new()),
                resource_catalog: Arc::new(ResourceCatalogSnapshot::new(
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                    ResourceCatalogFingerprint::from_bytes([0; 32]),
                )),
            },
        ));
        let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
            DatabaseId,
            DatabaseDeclarationObservation,
        )>())
        .unwrap();
        let database = Arc::new(
            DatabaseRuntimeRegistry::new()
                .open_session(DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(project_session_id.as_str().into()),
                    NonZeroU64::new(1).unwrap(),
                    None,
                    Vec::<DatabaseDecl>::new().into(),
                    observations,
                ))
                .unwrap(),
        );
        let execution = Arc::new(ExecutionRuntimeState::new(
            execution_session_id,
            RuntimeGeneration::from_existing(1),
        ));
        let session = Arc::new(ApplicationSession::new_for_test(
            ApplicationSessionEpoch::from_existing(1),
            project_instance_id,
            project_session_id.clone(),
            execution_session_id,
            RuntimeGeneration::from_existing(1),
            project,
            graph,
            execution,
            database,
            Arc::new(ResourceProviderFactory::new(
                project_session_id.as_str().into(),
            )),
        ));
        let slot = Arc::new(ApplicationSessionSlot::new());
        slot.publish_for_test(session);
        (
            fixture,
            crate::application::execution::ApplicationState::new(slot),
        )
    }

    #[test]
    fn catalog_mapper_preserves_metadata_and_resource_bound_wire_shape() {
        let (_fixture, application) = application_with_function();
        let project_instance_id = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let result = application
            .localized_node_catalog(LocalizedCatalogRequest::new(project_instance_id, "zh-CN"))
            .unwrap();
        let wire = serde_json::to_value(LocalizedCatalogDto::from(result)).unwrap();

        assert_eq!(
            wire["projectInstanceId"].as_str().unwrap().is_empty(),
            false
        );
        assert!(!wire["registryFingerprint"].as_str().unwrap().is_empty());
        assert_eq!(wire["resourcePublicationRevision"], 0);
        assert_eq!(wire["locale"], "zh-CN");
        let item = wire["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["resourcePath"] == "functions/Opaque.yssbi-function")
            .expect("function resource item is present");
        assert_eq!(item["resourceRevision"], 0);
        assert_eq!(item["creation"]["kind"], "resourceBound");
        assert_eq!(item["creation"]["createArgs"]["kind"], "function");
        assert!(wire.get("project_instance_id").is_none());
        assert!(wire.get("registry_fingerprint").is_none());
        assert!(wire.get("resource_publication_revision").is_none());
    }
}
