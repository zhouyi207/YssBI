use super::*;

use crate::application::execution::{ApplicationSessionEpoch, ApplicationSessionSlot};
use crate::database::runtime::DatabaseRuntimeRegistry;
use crate::database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::execution::resource_preparation::ResourceProviderFactory;
use crate::execution::state::ExecutionRuntimeState;
use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use crate::graph::runtime_state::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};
use crate::node_system::ProjectSessionId;
use crate::node_system::catalog::build_builtin_node_system;
use crate::node_system::compiler::ProjectCompileCoordinator;
use crate::project::ProjectData;
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::Arc;

fn application_with_empty_database(
    project: ProjectData,
) -> (crate::project::fixtures::TempProject, ApplicationState) {
    let fixture =
        crate::project::fixtures::TempProject::activate("catalog-schema-mismatch", project);
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
    let declarations: Arc<[DatabaseDecl]> = Vec::new().into();
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
                declarations,
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
    (fixture, ApplicationState::new(slot))
}

#[test]
fn localized_catalog_rejects_a_project_database_schema_mismatch() {
    let database = DatabaseDecl {
        id: DatabaseId::from_existing("sales".into()),
        engine: crate::database_contract::DatabaseEngine::InMemory {
            name: "sales".into(),
        },
        schema_version: 1,
        required: true,
        name: "Sales".into(),
    };
    let mut project = ProjectData::new();
    project.databases.insert("sales".into(), database);
    let (_fixture, application) = application_with_empty_database(project);
    let project_instance_id = application
        .capture_session()
        .unwrap()
        .project_instance_id()
        .clone();

    let error = application
        .localized_node_catalog(LocalizedCatalogRequest::new(project_instance_id, "en-US"))
        .unwrap_err();

    assert!(matches!(
        error,
        CatalogQueryApplicationError::Database(error)
            if error.code() == crate::database::error::DatabaseErrorCode::Conflict
    ));
}
