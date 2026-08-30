use super::*;

use crate::application::execution::{
    ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot,
};
use crate::database::runtime::DatabaseRuntimeRegistry;
use crate::graph::error::GraphMutationError;
use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use crate::graph::runtime_state::{
    GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState, GraphRuntimeTestControl,
    GraphRuntimeTestEvent,
};
use crate::project::ProjectSessionId;
use crate::project::{GraphDocumentKind, GraphResourceDocument, ProjectData, ProjectState};
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use yss_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_execution::resource_preparation::ResourceProviderFactory;
use yss_execution::state::ExecutionRuntimeState;
use yss_graph_catalog::build_builtin_node_system;

struct TestProject {
    root: PathBuf,
    state: Arc<ProjectState>,
}

impl TestProject {
    fn active(label: &str, data: ProjectData) -> Self {
        let root = test_root(label);
        crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
        Self {
            root,
            state: Arc::new(state),
        }
    }

    fn unloaded(label: &str, data: ProjectData) -> Self {
        let root = test_root(label);
        crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        Self {
            root,
            state: Arc::new(state),
        }
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct StagedSession {
    _project: TestProject,
    session: Arc<ApplicationSession>,
    application: ApplicationState,
    slot: Arc<ApplicationSessionSlot>,
    control: GraphRuntimeTestControl,
}

fn test_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("yssbi-graph-open-{label}-{}", uuid::Uuid::new_v4()))
}

fn staged_session(project: TestProject, control: GraphRuntimeTestControl) -> StagedSession {
    let project_state = Arc::clone(&project.state);
    let project_instance_id = project_state.capture_project_session().unwrap().instance_id;
    let project_session_id =
        ProjectSessionId::new(format!("graph-open-project-{}", uuid::Uuid::new_v4()));
    let execution_session_id = ExecutionSessionId::new(uuid::Uuid::new_v4());
    let builtin = build_builtin_node_system().unwrap();
    let graph = Arc::new(GraphRuntimeState::new_for_test(
        GraphRuntimeEpoch::from_existing(1),
        GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
            resource_catalog: Arc::new(ResourceCatalogSnapshot::new(
                BTreeMap::new(),
                BTreeMap::new(),
                BTreeMap::new(),
                ResourceCatalogFingerprint::from_bytes([0; 32]),
            )),
        },
        control.clone(),
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
        project_state,
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
    StagedSession {
        _project: project,
        session,
        application,
        slot,
        control,
    }
}

fn graph_project(path: &GraphResourcePath) -> ProjectData {
    let mut data = ProjectData::new();
    let name = path
        .display_name()
        .split_once(".yssbi-")
        .map_or(path.display_name(), |(name, _)| name);
    data.graphs.insert(
        path.clone(),
        GraphResourceDocument::new(name, GraphDocumentKind::Event),
    );
    data
}

fn open_request(session: &StagedSession, path: &GraphResourcePath) -> OpenGraphRequest {
    OpenGraphRequest::new(
        session.session.project_instance_id().clone(),
        path.clone(),
        1,
        "en-US",
    )
}

#[test]
fn materialization_failure_preserves_loaded_residency_and_skips_projection() {
    let path = GraphResourcePath::new("events/MaterializationFailure.yssbi-event").unwrap();
    let control = GraphRuntimeTestControl::default();
    let session = staged_session(
        TestProject::unloaded("materialization-failure", graph_project(&path)),
        control.clone(),
    );
    assert!(
        session
            .session
            .project()
            .get_data()
            .unwrap()
            .graphs
            .is_empty()
    );
    control.fail_next_materialization();

    let error = session
        .application
        .open_graph(open_request(&session, &path))
        .unwrap_err();

    assert!(matches!(
        error,
        OpenGraphApplicationError::Materialization(GraphMutationError::Internal(_))
    ));
    let data = session.session.project().get_data().unwrap();
    assert!(data.graphs.contains_key(&path));
    assert_eq!(data.graphs[&path].document.revision, GraphRevision::INITIAL);
    assert_eq!(
        control.events(),
        [
            GraphRuntimeTestEvent::Bound,
            GraphRuntimeTestEvent::Materialized
        ]
    );
}

#[test]
fn graph_open_replacement_respects_final_materialization_commit_boundary() {
    let path = GraphResourcePath::new("events/ReplacementBoundary.yssbi-event").unwrap();
    let project = graph_project(&path);

    let control = GraphRuntimeTestControl::default();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    control.pause_after_materialization(Arc::clone(&entered), Arc::clone(&release));
    let session = staged_session(
        TestProject::unloaded("replacement-before-commit", project.clone()),
        control.clone(),
    );
    let replacement = staged_session(
        TestProject::active("replacement-before-commit-target", project.clone()),
        GraphRuntimeTestControl::default(),
    );
    let worker_application = session.application.clone();
    let request = open_request(&session, &path);
    let worker = thread::spawn(move || worker_application.open_graph(request));

    entered.wait();
    session
        .slot
        .publish_for_test(Arc::clone(&replacement.session));
    release.wait();
    let error = worker.join().unwrap().unwrap_err();

    assert!(matches!(error, OpenGraphApplicationError::SessionChanged));
    assert!(
        session
            .session
            .project()
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&path)
    );
    assert_eq!(
        control.events(),
        [
            GraphRuntimeTestEvent::Bound,
            GraphRuntimeTestEvent::Materialized
        ]
    );

    let control = GraphRuntimeTestControl::default();
    let session = staged_session(
        TestProject::unloaded("replacement-after-commit", project.clone()),
        control.clone(),
    );
    let replacement = staged_session(
        TestProject::active("replacement-after-commit-target", project),
        GraphRuntimeTestControl::default(),
    );
    let receipt = session
        .application
        .open_graph(open_request(&session, &path))
        .expect("the final materialization commit owns the successful result");
    session
        .slot
        .publish_for_test(Arc::clone(&replacement.session));

    assert_eq!(receipt.graph_path(), &path);
    assert_eq!(receipt.graph_revision(), GraphRevision::INITIAL);
    assert_eq!(
        control.events(),
        [
            GraphRuntimeTestEvent::Bound,
            GraphRuntimeTestEvent::Materialized
        ]
    );
}

#[test]
fn open_graph_rejects_a_replaced_captured_session_before_project_load() {
    let path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let session = staged_session(
        TestProject::active("stale-before-load", ProjectData::new()),
        GraphRuntimeTestControl::default(),
    );
    let replacement = staged_session(
        TestProject::active("stale-before-load-target", ProjectData::new()),
        GraphRuntimeTestControl::default(),
    );
    let captured = Arc::clone(&session.session);
    session
        .slot
        .publish_for_test(Arc::clone(&replacement.session));

    let request = OpenGraphRequest::new(captured.project_instance_id().clone(), path, 1, "en-US");
    let error = open_graph_in_session(&session.application, &captured, request).unwrap_err();

    assert!(matches!(error, OpenGraphApplicationError::SessionChanged));
    assert!(session.control.events().is_empty());
}
