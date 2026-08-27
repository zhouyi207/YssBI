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

fn staged_session(
    project: &ProjectData,
    label: &str,
) -> (
    crate::project::fixtures::TempProject,
    Arc<ApplicationSession>,
    ApplicationState,
    Arc<ApplicationSessionSlot>,
) {
    let fixture = crate::project::fixtures::TempProject::activate(label, project.clone());
    let project = Arc::new(fixture.state().clone());
    let project_instance_id = project.capture_project_session().unwrap().instance_id;
    let project_session_id = ProjectSessionId::new(format!("{label}-session"));
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
    slot.publish_for_test(Arc::clone(&session));
    let application = ApplicationState::new(Arc::clone(&slot));
    (fixture, session, application, slot)
}

#[test]
fn open_graph_rejects_a_replaced_captured_session_before_project_load() {
    let project = ProjectData::new();
    let (_fixture, captured, application, slot) = staged_session(&project, "graph-open-stale");
    let (_replacement_fixture, replacement, _replacement_application, _replacement_slot) =
        staged_session(&project, "graph-open-replacement");
    slot.publish_for_test(replacement);

    let path = crate::graph_document::GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let request = OpenGraphRequest::new(captured.project_instance_id().clone(), path, 1, "en-US");
    let error = open_graph_in_session(&application, &captured, request).unwrap_err();

    assert!(matches!(error, OpenGraphApplicationError::SessionChanged));
}
